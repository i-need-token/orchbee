import { spawn } from 'node:child_process';
import { hostname } from 'node:os';

import {
	DESKTOP_SERVER_CONNECTION_IPC_CHANNEL,
	DESKTOP_SERVER_CONNECTION_PORT_ROLES,
	DESKTOP_SERVER_CONNECTION_REQUEST_IPC_CHANNEL,
	DesktopServerConnectionDocumentSchema,
	type DesktopServerConnectionBootstrap,
	type DesktopServerConnectionPortRole,
	type DesktopServerConnectionReply,
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
	type DesktopWindowBackground,
} from '../background-tray';
import { isSameDesktopRendererOrigin } from '../desktop-renderer-protocol';
import type { DesktopDiagnosticRecorder } from '../diagnostics';
import {
	DesktopProjectFolderAcquisitionBroker,
	DesktopProjectFolderAcquisitionRuntime,
} from '../project-folder-acquisition';
import { ServerConnectionRelay, readServerRestoreState } from '../server-connection';
import type { DesktopWindowStateAttachment, DesktopWindowStateOwner } from '../window-state';

/** What the window's Client processes run under, and where its page comes from. */
export interface DesktopServerClientApplicationOptions {
	/**
	 * Keeps Desktop running in the notification area when the window closes while terminals run;
	 * absent where closing the window keeps its current behavior, and for a window started hidden.
	 */
	readonly background: DesktopWindowBackground | undefined;
	readonly checkoutName: DesktopServerConnectionBootstrap['checkoutName'];
	readonly clientEnvironment: NodeJS.ProcessEnv;
	readonly clientExecutable: string;
	readonly distribution: DesktopServerConnectionBootstrap['distribution'];
	/** Appends one record to the daily diagnostic log; text values must be tokens. */
	readonly recordEvent: DesktopDiagnosticRecorder;
	readonly platform: NodeJS.Platform;
	readonly preloadPath: string;
	readonly rendererUrl: URL;
	readonly sessionArguments: readonly string[];
	/** A window started hidden for automation is never shown and neither restores nor records its place. */
	readonly windowHidden: boolean;
	readonly windowStateOwner: DesktopWindowStateOwner;
}

const MACOS_TRAFFIC_LIGHT_POSITION = { x: 12, y: 12 } as const;
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
 * What Main holds for one document: its Client connection, the folder picker it may open, and its
 * background-status port.
 */
interface DocumentConnection {
	readonly backgroundStatus: DesktopBackgroundStatusPort;
	readonly folderAcquisition: DesktopProjectFolderAcquisitionBroker;
	readonly relay: ServerConnectionRelay;
}

/** Electron rejects a load that another navigation of the same window replaced with `ERR_ABORTED`. */
function isSupersededLoad(error: unknown): boolean {
	return error instanceof Error && 'code' in error && error.code === 'ERR_ABORTED';
}

/**
 * Desktop as a Server Client: one window, whose every document holds one Client connection through
 * an `orchbee server connect` relay. Main keeps what belongs to this computer: the window, its place
 * on screen, and the native folder picker a new Project starts from. Closing the window ends only
 * this Client, and the Session's terminals keep running; on Windows, a close while they run hides
 * the window instead, as the background lifecycle decides.
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
				: { titleBarOverlay: true }),
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
		window.webContents.on('render-process-gone', (_event, details) => {
			options.recordEvent('warning', 'renderer-gone', { reason: details.reason });
			endDocumentConnection();
			if (details.reason !== 'clean-exit' && !window.isDestroyed()) window.reload();
		});
		const background = options.background;
		if (background !== undefined) {
			window.on('show', () => background.windowShown());
			window.on('close', (event) => {
				// Whatever showed the window, the lifecycle decides its close from what is on screen.
				if (window.isVisible()) background.windowShown();
				if (!background.closeRequested()) return;
				event.preventDefault();
				window.hide();
			});
			// Logoff and shutdown are never delayed: the close that follows always proceeds.
			window.on('query-session-end', () => background.sessionEnding());
			window.on('session-end', () => background.sessionEnding());
		}
		window.once('closed', () => {
			background?.windowClosed();
			endDocumentConnection();
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
			if (this.#document === current) this.#document = undefined;
			// A folder made for a Project the document never confirmed is reported to the person here.
			current?.folderAcquisition.dispose();
			current = undefined;
		};
		window.webContents.ipc.on(DESKTOP_SERVER_CONNECTION_REQUEST_IPC_CHANNEL, (_event, value) => {
			const document = DesktopServerConnectionDocumentSchema.safeParse(value);
			if (!document.success) return;
			end();
			const request = requests;
			void readServerRestoreState(
				options.clientExecutable,
				options.sessionArguments,
				options.clientEnvironment,
			).then((restore) => {
				if (request !== requests || window.isDestroyed()) return;
				current = this.#connect(window, {
					bootstrap: {
						// A later document follows a person's own actions; an empty Session may be what they chose.
						automaticWorkspaceAllowed: !everConnected && restore.kind === 'intact',
						checkoutName: options.checkoutName,
						defaultDirectory: app.getPath('home'),
						distribution: options.distribution,
						loginShellAvailable: options.platform !== 'win32',
						sessionRefusal: restore.kind === 'refused' ? restore.refusal : null,
						terminalHostName: hostname(),
					},
					document: document.data,
				});
				this.#document = current;
				everConnected = true;
			});
		});
		return end;
	}

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
		const relay = new ServerConnectionRelay(relayProcess, connection.port1);
		const folderAcquisition = new DesktopProjectFolderAcquisitionBroker(
			folders.port1,
			new DesktopProjectFolderAcquisitionRuntime(window),
		);
		folderAcquisition.start();
		const backgroundStatusPort = serveDesktopBackgroundStatus(
			backgroundStatus.port1,
			options.background,
		);
		const pagePorts: Readonly<Record<DesktopServerConnectionPortRole, MessagePortMain>> = {
			'background-status': backgroundStatus.port2,
			'project-folder-acquisition': folders.port2,
			'server-connection': connection.port2,
		};
		window.webContents.postMessage(
			DESKTOP_SERVER_CONNECTION_IPC_CHANNEL,
			reply,
			DESKTOP_SERVER_CONNECTION_PORT_ROLES.map((role) => pagePorts[role]),
		);
		return { backgroundStatus: backgroundStatusPort, folderAcquisition, relay };
	}
}
