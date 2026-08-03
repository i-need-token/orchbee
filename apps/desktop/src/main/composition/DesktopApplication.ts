import { randomBytes } from 'node:crypto';
import { constants } from 'node:os';

import { createOrchbeeIntl, resolveOrchbeeLocale, workbenchTabMessages } from '@orchbee/i18n';
import {
	ClientIdSchema,
	decodeControlJson,
	MessageKind,
	TerminalExitedSchema,
	type DecodedProtocolEnvelope,
	type TerminalExited,
} from '@orchbee/terminal-protocol';
import {
	type TerminalPresentationIdSchema,
	terminalPresentationId,
	terminalPresentationOperationId,
	terminalPresentationRemovalId,
	type TerminalPresentationEvent,
} from '@orchbee/workbench/terminal-presentation';
import type {
	WorkbenchSharedDocument,
	WorkbenchSharedPanelNode,
	WorkbenchSharedDocumentRevision,
} from '@orchbee/workbench/document';
import {
	BrowserWindow,
	app,
	MessageChannelMain,
	type BrowserWindowConstructorOptions,
	type MessagePortMain,
} from 'electron';

import {
	DESKTOP_APPLICATION_LIFETIME_IPC_CHANNEL,
	DESKTOP_CLIENT_SESSION_IPC_CHANNEL,
	TERMINAL_PROJECTION_CONTROL_IPC_CHANNEL,
	DesktopApplicationExitRequestIdSchema,
	DesktopApplicationLifetimeBootstrapSchema,
	TerminalProjectionControlBootstrapSchema,
	type DesktopClientSessionBootstrap,
	type ProjectedAttachmentBootstrap,
	type DesktopApplicationExitReason,
	type DesktopApplicationExitRequestId,
	type DesktopApplicationExitFailure,
} from '../../shared';
import {
	DesktopApplicationLifetimeBroker,
	type DesktopApplicationLifetimeBrokerDependencies,
	type DesktopApplicationLifetimeDrainResult,
	type DesktopApplicationLifetimeExitResult,
} from '../application-lifetime';
import { ProjectedTerminalBroker, type ProjectedTerminalBrokerMetrics } from '../broker';
import {
	DesktopClientSessionBroker,
	DesktopClientSessionCoordinator,
	DesktopTerminalIntentReconciler,
	type DesktopClientSessionCoordinatorDependencies,
	type DesktopTerminalIntentReconcilerDependencies,
} from '../client-session';
import { DevelopmentHostRuntime, type HostConnection } from '../host';
import {
	TerminalProjectionControlBroker,
	type DesktopTerminalProjectionLease,
} from '../terminal-projection-control';
import { FileWorkbenchDocumentStore, type WorkbenchDocumentStore } from '../workbench-document';
import {
	composeDesktopTerminalRuntime,
	type DesktopTerminalRuntime,
} from './composeDesktopTerminalRuntime';
import { createDesktopWorkbenchInitializationDocument } from './createDesktopWorkbenchInitializationDocument';
import type { DesktopRuntimeConfiguration } from './resolveDesktopRuntimeConfiguration';

interface DesktopBrokerHandle {
	readonly metrics: ProjectedTerminalBrokerMetrics;
	dispose(): Promise<void>;
	start(): void;
}

interface DesktopClientSessionBrokerHandle {
	readonly bootstrap: DesktopClientSessionBootstrap;
	dispose(): Promise<void>;
	start(): void;
}

interface DesktopProjectionControlBrokerHandle {
	dispose(): Promise<void>;
	start(): void;
}

interface DesktopApplicationLifetimeBrokerHandle {
	dispose(): void;
	requestExit(
		requestId: DesktopApplicationExitRequestId,
		reason: DesktopApplicationExitReason,
	): Promise<DesktopApplicationLifetimeExitResult>;
	start(): void;
}

export interface DesktopApplicationOptions {
	readonly platform: NodeJS.Platform;
	readonly preloadPath: string;
	readonly rendererFilePath: string;
	readonly rendererUrl?: string;
	readonly runtime: DesktopRuntimeConfiguration;
	readonly singleInstanceLockOwned: boolean;
	readonly traceCopyPath: boolean;
}

export interface DesktopApplicationDependencies {
	readonly connectHost: (configuration: DesktopRuntimeConfiguration) => Promise<HostConnection>;
	readonly createBroker: (
		host: HostConnection,
		port: MessagePortMain,
		bootstrap: ProjectedAttachmentBootstrap,
	) => DesktopBrokerHandle;
	readonly createApplicationLifetimeBroker: (
		port: MessagePortMain,
		dependencies: DesktopApplicationLifetimeBrokerDependencies,
	) => DesktopApplicationLifetimeBrokerHandle;
	readonly createClientSessionBroker: (
		port: MessagePortMain,
		coordinator: DesktopClientSessionCoordinator,
		disposeAttachments: () => Promise<void>,
	) => DesktopClientSessionBrokerHandle;
	readonly createClientSessionCoordinator: (
		dependencies: DesktopClientSessionCoordinatorDependencies,
	) => DesktopClientSessionCoordinator;
	readonly createMessageChannel: () => {
		readonly port1: MessagePortMain;
		readonly port2: MessagePortMain;
	};
	readonly createProjectionControlBroker: (
		port: MessagePortMain,
		service: {
			open(
				presentationId: Parameters<DesktopTerminalRuntime['open']>[0],
			): Promise<DesktopTerminalProjectionLease>;
		},
	) => DesktopProjectionControlBrokerHandle;
	readonly createTerminalRuntime: (
		host: HostConnection,
		configuration: DesktopRuntimeConfiguration,
		randomIdentifier: () => string,
		createMessageChannel: DesktopApplicationDependencies['createMessageChannel'],
		createBroker: DesktopApplicationDependencies['createBroker'],
	) => DesktopTerminalRuntime;
	readonly createTerminalIntentReconciler: (
		dependencies: DesktopTerminalIntentReconcilerDependencies,
	) => DesktopTerminalIntentReconciler;
	readonly createWorkbenchDocumentStore: (filePath: string) => WorkbenchDocumentStore;
	readonly createWindow: (options: BrowserWindowConstructorOptions) => BrowserWindow;
	readonly getPreferredLocales: () => readonly string[];
	readonly randomIdentifier: () => string;
	readonly quitApplication: () => void;
	readonly writeTrace: (trace: unknown) => void;
}

const defaultDependencies: DesktopApplicationDependencies = {
	connectHost: (configuration) =>
		new DevelopmentHostRuntime({
			binaryPath: configuration.binaryPath,
			endpointId: configuration.endpointId,
			runtimeDirectory: configuration.runtimeDirectory,
		}).connect(),
	createBroker: (host, port, bootstrap) => new ProjectedTerminalBroker(host, port, bootstrap),
	createApplicationLifetimeBroker: (port, dependencies) =>
		new DesktopApplicationLifetimeBroker(port, dependencies),
	createClientSessionBroker: (port, coordinator, disposeAttachments) =>
		new DesktopClientSessionBroker(port, coordinator, { disposeAttachments }),
	createClientSessionCoordinator: (dependencies) =>
		new DesktopClientSessionCoordinator(dependencies),
	createMessageChannel: () => new MessageChannelMain(),
	createProjectionControlBroker: (port, service) =>
		new TerminalProjectionControlBroker(port, service),
	createTerminalRuntime: (
		host,
		configuration,
		randomIdentifier,
		createMessageChannel,
		createBroker,
	) =>
		composeDesktopTerminalRuntime({
			configuration,
			createBroker,
			createMessageChannel,
			host,
			randomIdentifier,
		}),
	createTerminalIntentReconciler: (dependencies) =>
		new DesktopTerminalIntentReconciler(dependencies),
	createWorkbenchDocumentStore: (filePath) => new FileWorkbenchDocumentStore(filePath),
	createWindow: (options) => new BrowserWindow(options),
	getPreferredLocales: () => app.getPreferredSystemLanguages(),
	randomIdentifier: () => randomBytes(16).toString('hex'),
	quitApplication: () => app.quit(),
	writeTrace: (trace) => console.info(JSON.stringify(trace)),
};

export class DesktopApplication {
	#applicationLifetimeBroker: DesktopApplicationLifetimeBrokerHandle | undefined;
	#clientSessionBroker: DesktopClientSessionBrokerHandle | undefined;
	#clientSessionCoordinator: DesktopClientSessionCoordinator | undefined;
	readonly #dependencies: DesktopApplicationDependencies;
	readonly #options: DesktopApplicationOptions;
	#connection: HostConnection | undefined;
	#documentFailure: Error | undefined;
	#documentGeneration = 0;
	#documentTransition = Promise.resolve();
	#exitRequest: Promise<DesktopApplicationLifetimeExitResult> | undefined;
	#projectionControlBroker: DesktopProjectionControlBrokerHandle | undefined;
	#quitPrepared = false;
	#removeHostListener: (() => void) | undefined;
	#shuttingDown = false;
	#terminalRuntime: DesktopTerminalRuntime | undefined;
	#terminalIntentReconciler: DesktopTerminalIntentReconciler | undefined;
	#window: BrowserWindow | undefined;
	readonly #workbenchDocumentStore: WorkbenchDocumentStore;

	constructor(
		options: DesktopApplicationOptions,
		dependencies: DesktopApplicationDependencies = defaultDependencies,
	) {
		if (!options.singleInstanceLockOwned) {
			throw new Error('Desktop recovery requires the Electron single-instance lock.');
		}
		this.#options = options;
		this.#dependencies = dependencies;
		this.#workbenchDocumentStore = dependencies.createWorkbenchDocumentStore(
			options.runtime.workbenchDocumentStorePath,
		);
	}

	get hasWindow(): boolean {
		return this.#window !== undefined && !this.#window.isDestroyed();
	}

	get quitPrepared(): boolean {
		return this.#quitPrepared;
	}

	focusWindow(): void {
		if (this.hasWindow) this.#window?.focus();
	}

	requestExit(reason: DesktopApplicationExitReason): Promise<DesktopApplicationLifetimeExitResult> {
		if (this.#exitRequest !== undefined) return this.#exitRequest;
		const broker = this.#applicationLifetimeBroker;
		if (broker === undefined) return Promise.resolve({ kind: 'unavailable' });
		const requestId = DesktopApplicationExitRequestIdSchema.parse(
			`desktop:exit:${this.#dependencies.randomIdentifier()}`,
		);
		const request = broker.requestExit(requestId, reason);
		this.#exitRequest = request;
		void request.then((result) => {
			if (this.#exitRequest === request && result.kind !== 'ready') {
				this.#exitRequest = undefined;
			}
		});
		return request;
	}

	async requestExitAndQuit(reason: DesktopApplicationExitReason): Promise<void> {
		if (this.#applicationLifetimeBroker === undefined && !this.#shuttingDown) {
			if (!this.hasWindow) await this.openWindow();
			await this.#documentTransition;
		}
		const result = await this.requestExit(reason);
		if (result.kind !== 'ready' || this.#quitPrepared) return;
		this.#quitPrepared = true;
		await this.shutdown();
		this.#dependencies.quitApplication();
	}

	async start(): Promise<void> {
		if (this.#connection !== undefined) throw new Error('Desktop application is already started.');
		const connection = await this.#dependencies.connectHost(this.#options.runtime);
		this.#connection = connection;
		try {
			await connection.negotiate(ClientIdSchema.parse(this.#dependencies.randomIdentifier()));
			const terminalRuntime = this.#dependencies.createTerminalRuntime(
				connection,
				this.#options.runtime,
				this.#dependencies.randomIdentifier,
				this.#dependencies.createMessageChannel,
				this.#dependencies.createBroker,
			);
			this.#terminalRuntime = terminalRuntime;
			const initialization = await terminalRuntime.initialize();
			if (initialization.kind !== 'ready') {
				throw new Error(
					`Desktop terminal runtime initialization stopped with ${initialization.kind}.`,
				);
			}
			const defaultCwd = this.#options.runtime.terminalLaunch.currentDirectory;
			if (defaultCwd === null) {
				throw new Error('Desktop terminal runtime requires a default CWD.');
			}
			const initialPresentationId = terminalPresentationId(
				`desktop:presentation:${this.#dependencies.randomIdentifier()}`,
			);
			const intl = createOrchbeeIntl(
				resolveOrchbeeLocale(this.#dependencies.getPreferredLocales()),
			);
			const createOperationId = () =>
				terminalPresentationOperationId(
					`desktop:operation:${this.#dependencies.randomIdentifier()}`,
				);
			const initializationDocument = createDesktopWorkbenchInitializationDocument({
				createOperationId,
				createPresentationId: () =>
					terminalPresentationId(`desktop:presentation:${this.#dependencies.randomIdentifier()}`),
				createRemovalId: () =>
					terminalPresentationRemovalId(`desktop:removal:${this.#dependencies.randomIdentifier()}`),
				defaultCwd,
				initialPresentationId,
				recoveredPresentations: terminalRuntime.getRecoveredPresentations(),
				terminalTitle: (sequence) =>
					intl.formatMessage(workbenchTabMessages.terminalTitle, { number: sequence }),
			});
			const terminalIntentReconciler = this.#dependencies.createTerminalIntentReconciler({
				createOperationId,
				execute: async (command) => terminalRuntime.execute(command),
				getTerminalExit: (presentationId) => {
					const terminalExit = terminalRuntime.getProjectionTarget(presentationId)?.terminalExit;
					return terminalExit === null || terminalExit === undefined
						? null
						: toDocumentExit(presentationId, terminalExit);
				},
			});
			const clientSessionCoordinator = this.#dependencies.createClientSessionCoordinator({
				createClientSessionId: () => `desktop:client:${this.#dependencies.randomIdentifier()}`,
				defaultCwd,
				getTerminalEvents: () => terminalIntentReconciler.getTerminalEvents(),
				initialPresentationId,
				initializationDocument,
				reconcileDocument: (document, publish) =>
					terminalIntentReconciler.reconcileDocument(document, publish),
				store: this.#workbenchDocumentStore,
				validateDocument: (document) => terminalIntentReconciler.validateDocument(document),
			});
			const coordinatorStart = await clientSessionCoordinator.start();
			if (coordinatorStart.kind !== 'ready') {
				throw new Error(
					`Desktop client session coordinator stopped with ${coordinatorStart.reason}.`,
				);
			}
			this.#terminalIntentReconciler = terminalIntentReconciler;
			this.#clientSessionCoordinator = clientSessionCoordinator;
			this.#removeHostListener = connection.onEnvelope(this.#handleHostEnvelope);
			await this.openWindow();
		} catch (error) {
			this.#removeHostListener?.();
			this.#removeHostListener = undefined;
			await this.#clientSessionCoordinator?.dispose();
			this.#clientSessionCoordinator = undefined;
			this.#terminalIntentReconciler = undefined;
			await this.#terminalRuntime?.dispose();
			this.#terminalRuntime = undefined;
			connection.close();
			this.#connection = undefined;
			throw error;
		}
	}

	async openWindow(): Promise<void> {
		if (this.hasWindow) {
			this.#window?.focus();
			return;
		}
		if (this.#connection === undefined || this.#terminalRuntime === undefined) {
			throw new Error('Desktop application must start before opening a window.');
		}
		this.#documentFailure = undefined;
		const window = this.#dependencies.createWindow({
			backgroundColor: '#1e1e1e',
			height: 720,
			show: false,
			webPreferences: {
				allowRunningInsecureContent: false,
				contextIsolation: true,
				nodeIntegration: false,
				preload: this.#options.preloadPath,
				sandbox: true,
				spellcheck: false,
				webSecurity: true,
			},
			width: 1024,
		});
		this.#window = window;
		let loadingGeneration = 0;
		window.webContents.setWindowOpenHandler(() => ({ action: 'deny' }));
		window.webContents.on('will-navigate', (event) => event.preventDefault());
		window.webContents.on('did-start-loading', () => {
			loadingGeneration = this.#beginRendererDocument();
		});
		window.webContents.on('did-finish-load', () => {
			const generation =
				loadingGeneration === 0 ? this.#beginRendererDocument() : loadingGeneration;
			loadingGeneration = 0;
			this.#queueRendererDocument(window, generation);
		});
		window.webContents.on('render-process-gone', () => {
			this.#beginRendererDocument();
			void this.#documentTransition.then(() => {
				if (this.#window === window && !window.isDestroyed() && !this.#shuttingDown) {
					window.reload();
				}
			});
		});
		window.on('close', (event) => {
			if (this.#options.platform !== 'darwin' && !this.#quitPrepared && !this.#shuttingDown) {
				event.preventDefault();
				void this.requestExitAndQuit('user-quit');
			}
		});
		window.once('ready-to-show', () => window.show());
		window.once('closed', () => {
			this.#writeCopyPathTrace('window-closed');
			this.#beginRendererDocument();
			if (this.#window === window) this.#window = undefined;
		});

		if (this.#options.rendererUrl === undefined) {
			await window.loadFile(this.#options.rendererFilePath);
		} else {
			const rendererUrl = new URL(this.#options.rendererUrl);
			if (rendererUrl.protocol !== 'http:' || rendererUrl.hostname !== '127.0.0.1') {
				throw new Error('Development renderer URL must use loopback HTTP.');
			}
			await window.loadURL(rendererUrl.href);
		}
		await this.#documentTransition;
		if (this.#documentFailure !== undefined) throw this.#documentFailure;
	}

	async shutdown(): Promise<void> {
		if (this.#shuttingDown) return this.#documentTransition;
		this.#shuttingDown = true;
		this.#removeHostListener?.();
		this.#removeHostListener = undefined;
		this.#writeCopyPathTrace('main-shutdown');
		this.#beginRendererDocument();
		await this.#documentTransition;
		await this.#clientSessionCoordinator?.dispose();
		this.#clientSessionCoordinator = undefined;
		this.#terminalIntentReconciler = undefined;
		await this.#terminalRuntime?.dispose();
		this.#connection?.close();
		this.#connection = undefined;
		this.#terminalRuntime = undefined;
	}

	#beginRendererDocument(): number {
		const generation = ++this.#documentGeneration;
		this.#applicationLifetimeBroker?.dispose();
		this.#applicationLifetimeBroker = undefined;
		const clientSessionBroker = this.#clientSessionBroker;
		this.#clientSessionBroker = undefined;
		const projectionBroker = this.#projectionControlBroker;
		this.#projectionControlBroker = undefined;
		this.#documentTransition = this.#documentTransition.then(async () => {
			if (clientSessionBroker === undefined) await projectionBroker?.dispose();
			else await clientSessionBroker.dispose();
		});
		return generation;
	}

	#queueRendererDocument(window: BrowserWindow, generation: number): void {
		this.#documentTransition = this.#documentTransition.then(async () => {
			try {
				this.#attachRendererDocument(window, generation);
			} catch (error) {
				this.#documentFailure =
					error instanceof Error ? error : new Error('Renderer document control failed.');
			}
		});
	}

	#attachRendererDocument(window: BrowserWindow, generation: number): void {
		const terminalRuntime = this.#terminalRuntime;
		const clientSessionCoordinator = this.#clientSessionCoordinator;
		if (
			terminalRuntime === undefined ||
			clientSessionCoordinator === undefined ||
			this.#window !== window ||
			window.isDestroyed() ||
			generation !== this.#documentGeneration ||
			this.#shuttingDown
		) {
			return;
		}
		const lifetimeChannel = this.#dependencies.createMessageChannel();
		const clientSessionChannel = this.#dependencies.createMessageChannel();
		const projectionChannel = this.#dependencies.createMessageChannel();
		const projectionBroker = this.#dependencies.createProjectionControlBroker(
			projectionChannel.port2,
			terminalRuntime,
		);
		const clientSessionBroker = this.#dependencies.createClientSessionBroker(
			clientSessionChannel.port2,
			clientSessionCoordinator,
			async () => projectionBroker.dispose(),
		);
		const lifetimeBroker = this.#dependencies.createApplicationLifetimeBroker(
			lifetimeChannel.port2,
			{
				drainForStop: this.#drainForApplicationStop,
				prepareMode: (mode) =>
					clientSessionCoordinator.beginQuiescing(
						clientSessionBroker.bootstrap.clientSessionId,
						mode,
					),
			},
		);
		this.#applicationLifetimeBroker = lifetimeBroker;
		this.#clientSessionBroker = clientSessionBroker;
		this.#projectionControlBroker = projectionBroker;
		clientSessionBroker.start();
		lifetimeBroker.start();
		projectionBroker.start();
		window.webContents.postMessage(
			DESKTOP_CLIENT_SESSION_IPC_CHANNEL,
			clientSessionBroker.bootstrap,
			[clientSessionChannel.port1],
		);
		window.webContents.postMessage(
			DESKTOP_APPLICATION_LIFETIME_IPC_CHANNEL,
			DesktopApplicationLifetimeBootstrapSchema.parse({
				type: DESKTOP_APPLICATION_LIFETIME_IPC_CHANNEL,
			}),
			[lifetimeChannel.port1],
		);
		window.webContents.postMessage(
			TERMINAL_PROJECTION_CONTROL_IPC_CHANNEL,
			TerminalProjectionControlBootstrapSchema.parse({
				type: TERMINAL_PROJECTION_CONTROL_IPC_CHANNEL,
			}),
			[projectionChannel.port1],
		);
	}

	readonly #drainForApplicationStop = async (
		documentRevision: WorkbenchSharedDocumentRevision,
	): Promise<DesktopApplicationLifetimeDrainResult> => {
		const terminalRuntime = this.#terminalRuntime;
		if (terminalRuntime === undefined) {
			return {
				failures: [typedExitFailure('unreferenced-binding-close-failed', 1, true)],
				kind: 'failed',
			};
		}
		const loaded = await this.#workbenchDocumentStore.load().catch(() => ({
			kind: 'unavailable' as const,
			reason: 'read-failed' as const,
		}));
		if (loaded.kind !== 'available' || loaded.document.revision !== documentRevision) {
			return {
				failures: [typedExitFailure('shared-document-control-unavailable', 1, true)],
				kind: 'failed',
			};
		}
		const referencedPresentationIds = collectDocumentPresentationIds(loaded.document);
		const drained = await terminalRuntime.closeBindingsNotReferencedBy(referencedPresentationIds);
		if (drained.failures.length > 0) {
			return {
				failures: [
					typedExitFailure(
						'unreferenced-binding-close-failed',
						drained.failures.length,
						drained.failures.every((item) => item.failure.retryable),
					),
				],
				kind: 'failed',
			};
		}
		const remainingOwnedBindings = terminalRuntime.getOwnedBindingCount();
		if (remainingOwnedBindings > 0) {
			return {
				failures: [typedExitFailure('terminal-stop-failed', remainingOwnedBindings, true)],
				kind: 'failed',
			};
		}
		let unknownHostTerminalCount: number;
		try {
			unknownHostTerminalCount = await terminalRuntime.inspectUnknownHostTerminalCount();
		} catch {
			return {
				failures: [typedExitFailure('host-inventory-unavailable', 1, true)],
				kind: 'failed',
			};
		}
		if (unknownHostTerminalCount > 0) {
			return {
				failures: [typedExitFailure('host-orphan-detected', unknownHostTerminalCount, false)],
				kind: 'failed',
			};
		}
		return { closedUnreferencedCount: drained.closedCount, kind: 'completed' };
	};

	readonly #handleHostEnvelope = (envelope: DecodedProtocolEnvelope): void => {
		if (envelope.kind !== MessageKind.terminalExited) return;
		const exited = decodeControlJson(envelope.payload, TerminalExitedSchema);
		if (!exited.ok) return;
		void this.#terminalRuntime?.recordExit(exited.value).then((presentationId) => {
			if (presentationId === null) return;
			const event = toDocumentExit(presentationId, exited.value);
			if (this.#terminalIntentReconciler?.recordExternalEvent(event) !== true) return;
			this.#clientSessionCoordinator?.publishTerminalEvent(event);
			this.#writeCopyPathTrace('terminal-exited');
		});
	};

	#writeCopyPathTrace(reason: string): void {
		if (!this.#options.traceCopyPath || this.#connection === undefined) return;
		this.#dependencies.writeTrace({
			brokers: this.#terminalRuntime?.getProjectionMetrics() ?? [],
			hostTransport: this.#connection.metrics,
			reason,
			type: 'orchbee.desktop.copy-path',
		});
	}
}

function collectDocumentPresentationIds(
	document: WorkbenchSharedDocument,
): ReadonlySet<ReturnType<typeof TerminalPresentationIdSchema.parse>> {
	const presentationIds = new Set<ReturnType<typeof TerminalPresentationIdSchema.parse>>();
	const visit = (node: WorkbenchSharedPanelNode): void => {
		if (node.kind === 'panel') {
			presentationIds.add(node.primaryPresentation.presentationId);
			for (const presentation of node.childPresentations) {
				presentationIds.add(presentation.presentationId);
			}
			return;
		}
		for (const child of node.children) visit(child.node);
	};
	for (const context of document.terminalContexts) {
		for (const tab of context.panelWorkspace.tabs) visit(tab.root);
	}
	return presentationIds;
}

function typedExitFailure(
	code: DesktopApplicationExitFailure['code'],
	count: number,
	retryable: boolean,
): DesktopApplicationExitFailure {
	return { code, count, retryable };
}

function toDocumentExit(
	presentationId: Parameters<DesktopTerminalRuntime['getProjectionTarget']>[0],
	exited: TerminalExited,
): Extract<TerminalPresentationEvent, { readonly type: 'terminal-exited' }> {
	if (exited.exit.kind === 'exit-code') {
		return { exitCode: exited.exit.code, presentationId, signal: null, type: 'terminal-exited' };
	}
	if (exited.exit.kind === 'signal') {
		return {
			exitCode: null,
			presentationId,
			signal: signalName(exited.exit.signal),
			type: 'terminal-exited',
		};
	}
	return { exitCode: null, presentationId, signal: null, type: 'terminal-exited' };
}

function signalName(signal: number): string {
	for (const [name, value] of Object.entries(constants.signals)) {
		if (value === signal) return name;
	}
	return `SIGNAL_${signal}`;
}
