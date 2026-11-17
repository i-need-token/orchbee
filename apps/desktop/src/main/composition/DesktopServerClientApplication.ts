import { spawn } from 'node:child_process';
import { hostname } from 'node:os';

import {
	DESKTOP_SERVER_CONNECTION_IPC_CHANNEL,
	DESKTOP_SERVER_CONNECTION_PORT_ROLES,
	DESKTOP_SERVER_CONNECTION_REQUEST_IPC_CHANNEL,
	DESKTOP_WINDOW_CAPTION_IPC_CHANNEL,
	DESKTOP_WINDOW_FAULT_IPC_CHANNEL,
	DesktopServerConnectionDocumentSchema,
	DesktopWindowCaptionColorSchema,
	DesktopWindowFaultSiteSchema,
	type DesktopRelaunchPlace,
	type DesktopServerConnectionBootstrap,
	type DesktopServerConnectionPortRole,
	type DesktopServerConnectionReply,
	type DesktopSessionRefusal,
} from '@orchbee/desktop-client-protocol';
import {
	BrowserWindow,
	MessageChannelMain,
	app,
	screen,
	shell,
	type MessagePortMain,
} from 'electron';

import {
	serveDesktopBackgroundStatus,
	type DesktopBackgroundStatusPort,
	type DesktopBackgroundStatusSink,
	type DesktopWindowBackground,
} from '../background-tray';
import { isSameDesktopRendererOrigin } from '../desktop-renderer-protocol';
import {
	containListenerFault,
	DesktopWindowFaultLog,
	type DesktopDiagnosticRecorder,
} from '../diagnostics';
import {
	DesktopProjectFolderAcquisitionBroker,
	DesktopProjectFolderAcquisitionRuntime,
} from '../project-folder-acquisition';
import { ServerConnectionRelay, readServerStartAnswer } from '../server-connection';
import type { DesktopWindowStateAttachment, DesktopWindowStateOwner } from '../window-state';
import type { DesktopWindowUpdatePort } from '../window-update';
import { followRendererCrashes } from './followRendererCrashes';

/** What the window's Client processes run under, and where its page comes from. */
export interface DesktopServerClientApplicationOptions {
	/**
	 * Keeps Desktop running, in the notification area or the Dock, when the window closes while
	 * terminals run; absent where closing the window keeps its current behavior, and for a window
	 * started hidden.
	 */
	readonly background: DesktopWindowBackground | undefined;
	/** The version this window runs, which About shows. */
	readonly applicationVersion: string;
	readonly checkoutName: DesktopServerConnectionBootstrap['checkoutName'];
	readonly clientEnvironment: NodeJS.ProcessEnv;
	readonly clientExecutable: string;
	readonly distribution: DesktopServerConnectionBootstrap['distribution'];
	/**
	 * Given the first document's `server start` refusal, or null when it was not refused or why is
	 * unknown; true when Desktop exits instead, and that document is never connected.
	 */
	readonly exitsOnFirstStartAnswer: (refusal: DesktopSessionRefusal | null) => boolean;
	/** Appends one record to the daily diagnostic log; text values must be tokens. */
	readonly recordEvent: DesktopDiagnosticRecorder;
	readonly platform: NodeJS.Platform;
	readonly preloadPath: string;
	/** Where the first document opens when Update relaunched this window; null otherwise. */
	readonly relaunchPlace: DesktopRelaunchPlace | null;
	readonly rendererUrl: URL;
	readonly sessionArguments: readonly string[];
	/** A window started hidden for automation is never shown and neither restores nor records its place. */
	readonly windowHidden: boolean;
	readonly windowStateOwner: DesktopWindowStateOwner;
	/**
	 * Serves a document's window-update port, what it returns ending that service, and says whether
	 * it can ever offer a newer version.
	 */
	readonly windowUpdate: {
		readonly offersUpdates: boolean;
		serve(port: DesktopWindowUpdatePort): { close(): void };
	};
}

const MACOS_TRAFFIC_LIGHT_POSITION = { x: 12, y: 12 } as const;
/** The Windows caption buttons show the page's chrome beneath them, in whatever theme it has. */
const WINDOWS_CAPTION_BACKGROUND = '#00000000';
const HIDDEN_WINDOW_BOUNDS = { height: 720, width: 1100 } as const;
/** What a terminal link may open, as the page's own link policy permits. */
const EXTERNAL_LINK_PROTOCOLS = new Set(['http:', 'https:', 'mailto:']);

/** A link the page opens asks for a window; the person's browser takes it instead. */
function openExternalLink(address: string): void {
	let url: URL;
	try {
		url = new URL(address);
	} catch {
		return;
	}
	if (!EXTERNAL_LINK_PROTOCOLS.has(url.protocol)) return;
	// The page has already counted the link as opened, as the TUI counts an accepted request.
	shell.openExternal(url.href).catch(() => undefined);
}

/**
 * The Windows caption buttons' symbols take the color the page's theme gives them, each time it
 * changes; until the first document says, they keep the system's.
 */
function followPageCaptionColor(
	window: BrowserWindow,
	recordEvent: DesktopDiagnosticRecorder,
): void {
	window.webContents.ipc.on(
		DESKTOP_WINDOW_CAPTION_IPC_CHANNEL,
		containListenerFault(
			recordEvent,
			'ipc-window-caption',
			(_event: unknown, value: unknown) => {
				const symbolColor = DesktopWindowCaptionColorSchema.safeParse(value);
				if (!symbolColor.success || window.isDestroyed()) return;
				window.setTitleBarOverlay({ symbolColor: symbolColor.data });
			},
			undefined,
		),
	);
}

/**
 * Records, by site, the faults the window's pages report, under a bound that a page which keeps
 * failing cannot exceed; returns what writes the count it dropped once the window is gone.
 */
function recordPageFaults(
	window: BrowserWindow,
	recordEvent: DesktopDiagnosticRecorder,
): () => void {
	const log = new DesktopWindowFaultLog(recordEvent);
	window.webContents.ipc.on(
		DESKTOP_WINDOW_FAULT_IPC_CHANNEL,
		containListenerFault(
			recordEvent,
			'ipc-window-fault',
			(_event: unknown, value: unknown) => {
				const site = DesktopWindowFaultSiteSchema.safeParse(value);
				if (site.success) log.record(site.data);
			},
			undefined,
		),
	);
	return () => log.dispose();
}

/**
 * Hides the window whose close became a hide. A macOS window hidden in full screen would leave its
 * empty Space behind, so it leaves full screen first and hides once it has.
 */
function hideWindow(window: BrowserWindow, platform: NodeJS.Platform): void {
	if (platform !== 'darwin' || !window.isFullScreen()) {
		window.hide();
		return;
	}
	window.once('leave-full-screen', () => {
		if (!window.isDestroyed()) window.hide();
	});
	window.setFullScreen(false);
}

/**
 * What Main holds for one document: its Client connection, the folder picker it may open, its
 * background-status port, and its window-update port.
 */
interface DocumentConnection {
	readonly backgroundStatus: DesktopBackgroundStatusPort;
	readonly folderAcquisition: DesktopProjectFolderAcquisitionBroker;
	readonly relay: ServerConnectionRelay;
	readonly windowUpdate: { close(): void };
}

/** The page's background reports, each of which costs only its own call when it throws. */
function containBackgroundSink(
	sink: DesktopBackgroundStatusSink | undefined,
	recordEvent: DesktopDiagnosticRecorder,
): DesktopBackgroundStatusSink | undefined {
	if (sink === undefined) return undefined;
	const site = 'port-background-status';
	return {
		quitAllRequested: containListenerFault(
			recordEvent,
			site,
			sink.quitAllRequested.bind(sink),
			undefined,
		),
		sessionStopAnswered: containListenerFault(
			recordEvent,
			site,
			sink.sessionStopAnswered.bind(sink),
			undefined,
		),
		summaryChanged: containListenerFault(
			recordEvent,
			site,
			sink.summaryChanged.bind(sink),
			undefined,
		),
		windowRequested: containListenerFault(
			recordEvent,
			site,
			sink.windowRequested.bind(sink),
			undefined,
		),
	};
}

/** Electron rejects a load that another navigation of the same window replaced with `ERR_ABORTED`. */
function isSupersededLoad(error: unknown): boolean {
	return error instanceof Error && 'code' in error && error.code === 'ERR_ABORTED';
}

/**
 * Desktop as a Server Client: one window, whose every document holds one Client connection through
 * an `orchbee server connect` relay. Main keeps what belongs to this computer: the window, its place
 * on screen, and the native folder picker a new Project starts from. Closing the window ends only
 * this Client, and the Session's terminals keep running; on Windows and macOS, a close while they
 * run hides the window instead, as the background lifecycle decides.
 */
export class DesktopServerClientApplication {
	/** The current document's connection, which Quit All's Session stop goes through. */
	#document: DocumentConnection | undefined;
	readonly #options: DesktopServerClientApplicationOptions;
	#opening: Promise<void> | undefined;
	#window: BrowserWindow | undefined;
	#windowStateAttachment: DesktopWindowStateAttachment | undefined;

	constructor(options: DesktopServerClientApplicationOptions) {
		this.#options = options;
	}

	get hasWindow(): boolean {
		return this.#window !== undefined && !this.#window.isDestroyed();
	}

	/** The open window, which a dialog of Main's belongs to. */
	get window(): BrowserWindow | undefined {
		return this.hasWindow ? this.#window : undefined;
	}

	focusWindow(): void {
		const window = this.#window;
		if (window === undefined || window.isDestroyed() || this.#options.windowHidden) return;
		if (window.isMinimized()) window.restore();
		window.show();
		window.focus();
	}

	getWindowState(): 'full-screen' | 'maximized' | 'normal' | 'unavailable' {
		const window = this.#window;
		if (window === undefined || window.isDestroyed()) return 'unavailable';
		if (window.isFullScreen()) return 'full-screen';
		return window.isMaximized() ? 'maximized' : 'normal';
	}

	/** Asks the current document's page to stop the Session under `stopId`; false without one. */
	requestSessionStop(stopId: string): boolean {
		return this.#document?.backgroundStatus.requestSessionStop(stopId) ?? false;
	}

	/** Opens the window, or brings the open one forward. */
	openWindow(): Promise<void> {
		if (this.hasWindow) {
			this.focusWindow();
			return Promise.resolve();
		}
		this.#opening ??= this.#open().finally(() => {
			this.#opening = undefined;
		});
		return this.#opening;
	}

	/** Records where the window is and waits for that record, before the process exits. */
	async settleWindowState(): Promise<void> {
		this.#windowStateAttachment?.capture();
		this.#windowStateAttachment?.dispose();
		this.#windowStateAttachment = undefined;
		await this.#options.windowStateOwner.flush().catch(() => undefined);
	}

	toggleWindowFullScreen(): boolean {
		const window = this.#window;
		if (window === undefined || window.isDestroyed()) return false;
		window.setFullScreen(!window.isFullScreen());
		return true;
	}

	toggleWindowMaximized(): boolean {
		const window = this.#window;
		if (window === undefined || window.isDestroyed() || window.isFullScreen()) return false;
		if (window.isMaximized()) window.unmaximize();
		else window.maximize();
		return true;
	}

	async #open(): Promise<void> {
		const options = this.#options;
		const launchState = options.windowHidden
			? { bounds: HIDDEN_WINDOW_BOUNDS, maximized: false }
			: await options.windowStateOwner.loadLaunchState({
					primaryWorkArea: screen.getPrimaryDisplay().workArea,
					workAreas: screen.getAllDisplays().map((display) => display.workArea),
				});
		const window = new BrowserWindow({
			...launchState.bounds,
			show: false,
			...(options.platform === 'darwin'
				? { trafficLightPosition: MACOS_TRAFFIC_LIGHT_POSITION }
				: {
						titleBarOverlay:
							options.platform === 'win32' ? { color: WINDOWS_CAPTION_BACKGROUND } : true,
					}),
			titleBarStyle: 'hidden',
			webPreferences: {
				allowRunningInsecureContent: false,
				// A document behind other windows still has to keep its terminal current.
				backgroundThrottling: false,
				contextIsolation: true,
				nodeIntegration: false,
				preload: options.preloadPath,
				sandbox: true,
				spellcheck: false,
				webSecurity: true,
			},
		});
		this.#window = window;
		if (options.platform === 'win32') followPageCaptionColor(window, options.recordEvent);
		const endPageFaults = recordPageFaults(window, options.recordEvent);
		let attachment: DesktopWindowStateAttachment | undefined;
		if (!options.windowHidden) {
			attachment = options.windowStateOwner.attach(window);
			this.#windowStateAttachment = attachment;
			if (launchState.maximized) window.maximize();
			window.once('ready-to-show', () => window.show());
		}
		window.webContents.setWindowOpenHandler(({ url }) => {
			openExternalLink(url);
			return { action: 'deny' };
		});
		window.webContents.on('will-navigate', (event, navigationUrl) => {
			if (!isSameDesktopRendererOrigin(navigationUrl, options.rendererUrl)) event.preventDefault();
		});
		const endDocumentConnection = this.#serveDocuments(window);
		followRendererCrashes(window, options.recordEvent, endDocumentConnection);
		const background = options.background;
		if (background !== undefined) {
			window.on('show', () => background.windowShown());
			window.on('close', (event) => {
				// Whatever showed the window, the lifecycle decides its close from what is on screen.
				if (window.isVisible()) background.windowShown();
				if (!background.closeRequested()) return;
				event.preventDefault();
				hideWindow(window, options.platform);
			});
			// Logoff and shutdown are never delayed: the close that follows always proceeds.
			window.on('query-session-end', () => background.sessionEnding());
			window.on('session-end', () => background.sessionEnding());
		}
		window.once('closed', () => {
			background?.windowClosed();
			endDocumentConnection();
			endPageFaults();
			attachment?.dispose();
			if (this.#windowStateAttachment === attachment) this.#windowStateAttachment = undefined;
			if (this.#window === window) this.#window = undefined;
		});
		try {
			await window.loadURL(options.rendererUrl.href);
		} catch (error) {
			// A window closed before its first document finished, or a newer document of the same window
			// took that load over; neither is a window that failed to open.
			if (window.isDestroyed() || isSupersededLoad(error)) return;
			throw error;
		}
	}

	/**
	 * A document asks once it is listening, and its asking ends what the one it replaced held.
	 * Returns what ends the current document's connection and folder picker.
	 */
	#serveDocuments(window: BrowserWindow): () => void {
		const options = this.#options;
		let current: DocumentConnection | undefined;
		let requests = 0;
		let everConnected = false;
		const end = (): void => {
			requests += 1;
			current?.relay.close();
			current?.backgroundStatus.close();
			current?.windowUpdate.close();
			if (this.#document === current) this.#document = undefined;
			// A folder made for a Project the document never confirmed is reported to the person here.
			current?.folderAcquisition.dispose();
			current = undefined;
		};
		const onRequest = (_event: unknown, value: unknown): void => {
			const document = DesktopServerConnectionDocumentSchema.safeParse(value);
			if (!document.success) return;
			end();
			const request = requests;
			void readServerStartAnswer(
				options.clientExecutable,
				options.sessionArguments,
				options.clientEnvironment,
			)
				.then(({ negotiatedCapabilities, restore }) => {
					if (request !== requests || window.isDestroyed()) return;
					const refusal = restore.kind === 'refused' ? restore.refusal : null;
					if (!everConnected && options.exitsOnFirstStartAnswer(refusal)) return;
					current = this.#connect(window, {
						bootstrap: {
							applicationVersion: options.applicationVersion,
							// A later document follows a person's own actions; an empty Session may be what they chose.
							automaticWorkspaceAllowed: !everConnected && restore.kind === 'intact',
							checkoutName: options.checkoutName,
							defaultDirectory: app.getPath('home'),
							distribution: options.distribution,
							loginShellAvailable: options.platform !== 'win32',
							// The relay's own handshake joins the same Server with the same Client, so it
							// negotiates the names this start reported.
							negotiatedCapabilities:
								negotiatedCapabilities.length === 0 ? null : [...negotiatedCapabilities],
							quitAllAvailable: options.background !== undefined,
							relaunchPlace: everConnected ? null : options.relaunchPlace,
							sessionRefusal: refusal,
							terminalHostName: hostname(),
							windowUpdatesOffered: options.windowUpdate.offersUpdates,
						},
						document: document.data,
					});
					this.#document = current;
					everConnected = true;
				})
				.catch(() => {
					// This document gets no connection and a reload asks again; the window and Main go on.
					options.recordEvent('error', 'document-connection-failed');
				});
		};
		window.webContents.ipc.on(
			DESKTOP_SERVER_CONNECTION_REQUEST_IPC_CHANNEL,
			containListenerFault(
				options.recordEvent,
				'ipc-server-connection-request',
				onRequest,
				undefined,
			),
		);
		return end;
	}

	/** Connects one document; what a failure had already started is ended before it rethrows. */
	#connect(window: BrowserWindow, reply: DesktopServerConnectionReply): DocumentConnection {
		const options = this.#options;
		const relayProcess = spawn(
			options.clientExecutable,
			[...options.sessionArguments, 'server', 'connect'],
			{ env: options.clientEnvironment, stdio: ['pipe', 'pipe', 'inherit'], windowsHide: true },
		);
		const connection = new MessageChannelMain();
		const folders = new MessageChannelMain();
		const backgroundStatus = new MessageChannelMain();
		const windowUpdate = new MessageChannelMain();
		const relay = new ServerConnectionRelay(relayProcess, connection.port1, options.recordEvent);
		const folderAcquisition = new DesktopProjectFolderAcquisitionBroker(
			folders.port1,
			new DesktopProjectFolderAcquisitionRuntime(window),
		);
		let backgroundStatusPort: DesktopBackgroundStatusPort | undefined;
		let windowUpdatePort: { close(): void } | undefined;
		try {
			folderAcquisition.start();
			backgroundStatusPort = serveDesktopBackgroundStatus(
				backgroundStatus.port1,
				containBackgroundSink(options.background, options.recordEvent),
			);
			windowUpdatePort = options.windowUpdate.serve(windowUpdate.port1);
			const pagePorts: Readonly<Record<DesktopServerConnectionPortRole, MessagePortMain>> = {
				'background-status': backgroundStatus.port2,
				'project-folder-acquisition': folders.port2,
				'server-connection': connection.port2,
				'window-update': windowUpdate.port2,
			};
			window.webContents.postMessage(
				DESKTOP_SERVER_CONNECTION_IPC_CHANNEL,
				reply,
				DESKTOP_SERVER_CONNECTION_PORT_ROLES.map((role) => pagePorts[role]),
			);
		} catch (error) {
			relay.close();
			backgroundStatusPort?.close();
			windowUpdatePort?.close();
			folderAcquisition.dispose();
			throw error;
		}
		return {
			backgroundStatus: backgroundStatusPort,
			folderAcquisition,
			relay,
			windowUpdate: windowUpdatePort,
		};
	}
}
