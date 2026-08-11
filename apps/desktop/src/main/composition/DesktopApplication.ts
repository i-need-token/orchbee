import { randomBytes } from 'node:crypto';
import { constants } from 'node:os';

import { createOrchbeeIntl, resolveOrchbeeLocale, workbenchTabMessages } from '@orchbee/i18n';
import {
	DESKTOP_RENDERER_BOOTSTRAP_IPC_CHANNEL,
	DesktopApplicationExitRequestIdSchema,
	DesktopRendererBootstrapSchema,
	type DesktopApplicationExitFailure,
	type DesktopApplicationExitReason,
	type DesktopApplicationExitRequestId,
} from '@orchbee/desktop-client-protocol';
import {
	ClientIdSchema,
	decodeControlJson,
	MessageKind,
	TerminalDefaultsAcceptedSchema,
	TerminalExitedSchema,
	TerminalFailedSchema,
	type DecodedProtocolEnvelope,
	type TerminalExited,
	type TerminalFailed,
	type TerminalIdentity,
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
import type {
	ProjectedAttachmentBootstrap,
	WorkbenchClientSessionBootstrap,
} from '@orchbee/workbench-client-protocol';
import {
	BrowserWindow,
	app,
	MessageChannelMain,
	type BrowserWindowConstructorOptions,
	type MessagePortMain,
} from 'electron';

import {
	DesktopApplicationLifetimeBroker,
	type DesktopApplicationLifetimeBrokerDependencies,
	type DesktopApplicationLifetimeDrainResult,
	type DesktopApplicationLifetimeExitResult,
} from '../application-lifetime';
import {
	ProjectedTerminalBroker,
	type ProjectedTerminalBrokerEffectTarget,
	type ProjectedTerminalBrokerLifecycle,
	type ProjectedTerminalBrokerMetrics,
} from '../broker';
import {
	DesktopClientSessionBroker,
	DesktopClientSessionCoordinator,
	DesktopTerminalIntentReconciler,
	type DesktopClientSessionCoordinatorDependencies,
	type DesktopTerminalIntentReconcilerDependencies,
} from '../client-session';
import { type DesktopGatewayClientHandle } from '../gateway';
import {
	DesktopBrowserAccessBroker,
	DesktopBrowserAccessRuntime,
	type DesktopBrowserAccessBrokerRuntime,
	type DesktopBrowserAccessRuntimeOptions,
} from '../browser-access';
import { DevelopmentHostRuntime, type HostConnection } from '../host';
import {
	TerminalExitPublicationLedger,
	TerminalExitSpillStore,
	type TerminalExitSpillReason,
} from '../recovery';
import {
	TerminalProjectionControlBroker,
	type DesktopTerminalProjectionLease,
} from '../terminal-projection-control';
import type { DesktopTerminalExitRecordResult } from '../terminal-document-control';
import { FileWorkbenchDocumentStore, type WorkbenchDocumentStore } from '../workbench-document';
import {
	composeDesktopTerminalRuntime,
	type DesktopTerminalRuntime,
} from './composeDesktopTerminalRuntime';
import { createDesktopWorkbenchInitializationDocument } from './createDesktopWorkbenchInitializationDocument';
import type { DesktopRuntimeConfiguration } from './resolveDesktopRuntimeConfiguration';
import { startsDesktopRendererDocument } from './startsDesktopRendererDocument';

interface DesktopBrokerHandle {
	readonly metrics: ProjectedTerminalBrokerMetrics;
	dispose(): Promise<void>;
	start(): void;
}

interface DesktopClientSessionBrokerHandle {
	readonly bootstrap: WorkbenchClientSessionBootstrap;
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

interface DesktopBrowserAccessBrokerHandle {
	dispose(): void;
	start(): void;
}

interface DesktopBrowserAccessRuntimeHandle extends DesktopBrowserAccessBrokerRuntime {
	dispose(): Promise<void>;
}

interface GatewayClientResourceOwner {
	dispose(): Promise<void>;
}

type TerminalExitPublicationResult = 'already-committed' | 'committed' | 'pending';

const MAX_PENDING_TERMINAL_EXIT_RETRIES = 64;
const TERMINAL_EXIT_RETRY_DELAY_MILLISECONDS = 25;
const TERMINAL_EXIT_ASSOCIATION_RETRY_LIMIT = 3;

export interface DesktopApplicationOptions {
	readonly platform: NodeJS.Platform;
	readonly preloadPath: string;
	readonly rendererUrl: string;
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
		effectTarget: ProjectedTerminalBrokerEffectTarget,
		lifecycle?: ProjectedTerminalBrokerLifecycle,
	) => DesktopBrokerHandle;
	readonly createApplicationLifetimeBroker: (
		port: MessagePortMain,
		dependencies: DesktopApplicationLifetimeBrokerDependencies,
	) => DesktopApplicationLifetimeBrokerHandle;
	readonly createBrowserAccessBroker: (
		port: MessagePortMain,
		runtime: DesktopBrowserAccessBrokerRuntime,
	) => DesktopBrowserAccessBrokerHandle;
	readonly createBrowserAccessRuntime: (
		options: DesktopBrowserAccessRuntimeOptions,
		openClient: () => Promise<DesktopGatewayClientHandle>,
	) => DesktopBrowserAccessRuntimeHandle;
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
	readonly createTerminalExitSpillStore?: (filePath: string) => TerminalExitSpillStore;
	readonly createTerminalExitPublicationLedger?: (
		filePath: string,
	) => TerminalExitPublicationLedger;
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
			terminalDefaults: configuration.terminalConfiguration,
		}).connect(),
	createBroker: (host, port, bootstrap, effectTarget, lifecycle) =>
		new ProjectedTerminalBroker(host, port, bootstrap, undefined, effectTarget, lifecycle),
	createApplicationLifetimeBroker: (port, dependencies) =>
		new DesktopApplicationLifetimeBroker(port, dependencies),
	createBrowserAccessBroker: (port, runtime) => new DesktopBrowserAccessBroker(port, runtime),
	createBrowserAccessRuntime: (options, openClient) =>
		new DesktopBrowserAccessRuntime(options, { openClient }),
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
	createTerminalExitPublicationLedger: (filePath) => new TerminalExitPublicationLedger(filePath),
	createWorkbenchDocumentStore: (filePath) => new FileWorkbenchDocumentStore(filePath),
	createWindow: (options) => new BrowserWindow(options),
	getPreferredLocales: () => app.getPreferredSystemLanguages(),
	randomIdentifier: () => randomBytes(16).toString('hex'),
	quitApplication: () => app.quit(),
	writeTrace: (trace) => console.info(JSON.stringify(trace)),
};

const unavailableBrowserAccessRuntime: DesktopBrowserAccessBrokerRuntime = {
	copyAccessLink: async () => unavailableBrowserAccess(),
	getSnapshot: () => ({ phase: 'stopped' }),
	openLocalBrowser: async () => unavailableBrowserAccess(),
	start: async () => unavailableBrowserAccess(),
	stop: async () => unavailableBrowserAccess(),
};

function unavailableBrowserAccess(): never {
	throw Object.assign(new Error('Desktop Browser access resources are unavailable.'), {
		code: 'server-unavailable' as const,
	});
}

export class DesktopApplication {
	#applicationLifetimeBroker: DesktopApplicationLifetimeBrokerHandle | undefined;
	#browserAccessBroker: DesktopBrowserAccessBrokerHandle | undefined;
	#browserAccessRuntime: DesktopBrowserAccessRuntimeHandle | undefined;
	#clientSessionBroker: DesktopClientSessionBrokerHandle | undefined;
	#clientSessionCoordinator: DesktopClientSessionCoordinator | undefined;
	readonly #dependencies: DesktopApplicationDependencies;
	readonly #options: DesktopApplicationOptions;
	#connection: HostConnection | undefined;
	#documentFailure: Error | undefined;
	#documentGeneration = 0;
	#documentTransition = Promise.resolve();
	#exitRequest: Promise<DesktopApplicationLifetimeExitResult> | undefined;
	readonly #pendingGatewayClientCleanups = new Set<GatewayClientResourceOwner>();
	#pendingRendererBootstrapCleanup: (() => Promise<void>) | undefined;
	#projectionControlBroker: DesktopProjectionControlBrokerHandle | undefined;
	#quitPrepared = false;
	#removeHostListener: (() => void) | undefined;
	#shuttingDown = false;
	#shutdownPromise: Promise<void> | undefined;
	#terminalRuntime: DesktopTerminalRuntime | undefined;
	#terminalIntentReconciler: DesktopTerminalIntentReconciler | undefined;
	readonly #pendingTerminalExits = new Map<string, TerminalExited>();
	readonly #terminalExitAttempts = new Map<string, Promise<void>>();
	readonly #terminalExitPublicationReservations = new Map<
		string,
		{
			readonly event: TerminalExited;
			readonly presentationId: ReturnType<typeof terminalPresentationId>;
		}
	>();
	readonly #publishedTerminalExitRevisions = new Map<string, TerminalExited['finalRevision']>();
	readonly #associatedTerminalExitKeys = new Set<string>();
	readonly #unknownIdentityAttempts = new Set<string>();
	#terminalExitRetryOwnerDisposed = false;
	#terminalExitRetryOwnerClosing = false;
	#terminalExitRetryTimer: ReturnType<typeof setTimeout> | undefined;
	readonly #terminalExitAttemptCounts = new Map<string, number>();
	readonly #terminalExitSpillStore: TerminalExitSpillStore;
	readonly #terminalExitPublicationLedger: TerminalExitPublicationLedger;
	#terminalExitSpillReady = false;
	#pendingRendererDocumentRetirement: (() => Promise<void>) | undefined;
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
		this.#terminalExitSpillStore =
			dependencies.createTerminalExitSpillStore?.(options.runtime.terminalExitSpillPath) ??
			new TerminalExitSpillStore(options.runtime.terminalExitSpillPath);
		this.#terminalExitPublicationLedger =
			dependencies.createTerminalExitPublicationLedger?.(
				options.runtime.terminalExitPublicationLedgerPath,
			) ?? new TerminalExitPublicationLedger(options.runtime.terminalExitPublicationLedgerPath);
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
			const acceptedDefaults = await connection.requestControl(
				MessageKind.setTerminalDefaults,
				{ configuration: this.#options.runtime.terminalConfiguration },
				TerminalDefaultsAcceptedSchema,
			);
			if (
				acceptedDefaults.configuration.scrollbackBytes !==
				this.#options.runtime.terminalConfiguration.scrollbackBytes
			) {
				throw new Error('Terminal Host did not accept the requested terminal defaults.');
			}
			const publicationLedgerLoad = await this.#terminalExitPublicationLedger.load();
			if (publicationLedgerLoad.kind !== 'ready') {
				throw new Error(
					`Terminal exit publication ledger is unavailable: ${publicationLedgerLoad.reason}.`,
				);
			}
			const spillLoad = await this.#terminalExitSpillStore.load(
				this.#terminalExitPublicationLedger,
			);
			if (spillLoad.kind !== 'ready') {
				throw new Error(`Terminal exit spill store is unavailable: ${spillLoad.reason}.`);
			}
			this.#terminalExitSpillReady = true;
			const terminalRuntime = this.#dependencies.createTerminalRuntime(
				connection,
				this.#options.runtime,
				this.#dependencies.randomIdentifier,
				this.#dependencies.createMessageChannel,
				this.#dependencies.createBroker,
			);
			this.#terminalRuntime = terminalRuntime;
			terminalRuntime.setIdentityObservedListener?.(this.#handleTerminalIdentityObserved);
			this.#removeHostListener = connection.onEnvelope(this.#handleHostEnvelope);
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
				getTerminalOutcome: (presentationId) => {
					const terminalFailure = terminalRuntime.getTerminalFailure(presentationId);
					if (terminalFailure !== null) {
						return toDocumentFailure(presentationId, terminalFailure);
					}
					const terminalExit = terminalRuntime.getDurableTerminalExit(presentationId);
					return terminalExit === null ? null : toDocumentExit(presentationId, terminalExit);
				},
			});
			const clientSessionCoordinator = this.#dependencies.createClientSessionCoordinator({
				createClientSessionId: () => `desktop:client:${this.#dependencies.randomIdentifier()}`,
				defaultCwd,
				getTerminalEvents: () => terminalIntentReconciler.getTerminalEvents(),
				initialPresentationId,
				initializationDocument,
				reconcileDocument: (document, publish) =>
					terminalIntentReconciler.reconcileDocument(document, (event) => {
						if (event.type === 'terminal-exited') {
							return this.#commitReconciledTerminalExit(terminalRuntime, event, publish);
						}
						publish(event);
						return true;
					}),
				store: this.#workbenchDocumentStore,
				validateDocument: (document) => terminalIntentReconciler.validateDocument(document),
			});
			this.#terminalIntentReconciler = terminalIntentReconciler;
			this.#clientSessionCoordinator = clientSessionCoordinator;
			const coordinatorStart = await clientSessionCoordinator.start();
			if (coordinatorStart.kind !== 'ready') {
				throw new Error(
					`Desktop client session coordinator stopped with ${coordinatorStart.reason}.`,
				);
			}
			this.#promoteSpilledTerminalExits();
			this.#scheduleTerminalExitRetry();
			if (this.#options.runtime.browserAccess.kind === 'enabled') {
				this.#browserAccessRuntime = this.#dependencies.createBrowserAccessRuntime(
					{
						gatewayEntryPath: this.#options.runtime.browserAccess.gatewayEntryPath,
						rendererRoot: this.#options.runtime.browserAccess.rendererRoot,
						serverEntryPath: this.#options.runtime.browserAccess.serverEntryPath,
					},
					() => this.#openGatewayClient(),
				);
			}
			await this.openWindow();
		} catch (error) {
			this.#removeHostListener?.();
			this.#removeHostListener = undefined;
			let cleanupFailure: unknown;
			try {
				await this.#disposeTerminalExitRetryOwner();
			} catch (cleanupError: unknown) {
				cleanupFailure = cleanupError;
			}
			try {
				await this.#disposeBrowserAccessResources();
			} catch (cleanupError: unknown) {
				cleanupFailure ??= cleanupError;
			}
			try {
				const coordinator = this.#clientSessionCoordinator;
				await coordinator?.dispose();
				if (this.#clientSessionCoordinator === coordinator) {
					this.#clientSessionCoordinator = undefined;
					this.#terminalIntentReconciler = undefined;
				}
			} catch (cleanupError: unknown) {
				cleanupFailure ??= cleanupError;
			}
			try {
				const terminalRuntime = this.#terminalRuntime;
				await terminalRuntime?.dispose();
				if (this.#terminalRuntime === terminalRuntime) this.#terminalRuntime = undefined;
			} catch (cleanupError: unknown) {
				cleanupFailure ??= cleanupError;
			}
			connection.close();
			this.#connection = undefined;
			throw cleanupFailure ?? error;
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
		window.webContents.on('did-start-navigation', (navigation) => {
			if (!startsDesktopRendererDocument(navigation)) return;
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

		const rendererUrl = new URL(this.#options.rendererUrl);
		if (!isPermittedDesktopRendererUrl(rendererUrl)) {
			throw new Error('Desktop renderer URL must use loopback HTTP or the packaged origin.');
		}
		await window.loadURL(rendererUrl.href);
		await this.#documentTransition;
		if (this.#documentFailure !== undefined) throw this.#documentFailure;
	}

	async shutdown(): Promise<void> {
		if (this.#shutdownPromise !== undefined) return this.#shutdownPromise;
		this.#shuttingDown = true;
		const operation = this.#shutdownInternal();
		this.#shutdownPromise = operation.catch((error: unknown) => {
			this.#shutdownPromise = undefined;
			throw error;
		});
		return this.#shutdownPromise;
	}

	async #shutdownInternal(): Promise<void> {
		this.#removeHostListener?.();
		this.#removeHostListener = undefined;
		let firstFailure: unknown;
		try {
			await this.#disposeTerminalExitRetryOwner();
		} catch (error: unknown) {
			firstFailure = error;
			this.#writeCopyPathTrace('terminal-exit-retry-owner-failed-during-shutdown');
		}
		try {
			this.#writeCopyPathTrace('main-shutdown');
			this.#beginRendererDocument();
			await this.#documentTransition;
		} catch (error: unknown) {
			firstFailure ??= error;
		}
		try {
			await this.#disposeBrowserAccessResources();
		} catch (error: unknown) {
			firstFailure ??= error;
		}
		try {
			const coordinator = this.#clientSessionCoordinator;
			await coordinator?.dispose();
			if (this.#clientSessionCoordinator === coordinator) {
				this.#clientSessionCoordinator = undefined;
				this.#terminalIntentReconciler = undefined;
			}
		} catch (error: unknown) {
			firstFailure ??= error;
		}
		try {
			const terminalRuntime = this.#terminalRuntime;
			await terminalRuntime?.dispose();
			if (this.#terminalRuntime === terminalRuntime) this.#terminalRuntime = undefined;
		} catch (error: unknown) {
			firstFailure ??= error;
		}
		this.#connection?.close();
		this.#connection = undefined;
		if (firstFailure !== undefined) throw firstFailure;
	}

	async #disposePendingGatewayClientCleanups(): Promise<void> {
		if (this.#pendingGatewayClientCleanups.size === 0) return;
		const results = await Promise.allSettled(
			[...this.#pendingGatewayClientCleanups].map((owner) => owner.dispose()),
		);
		const failures = results.flatMap((result) =>
			result.status === 'rejected' ? [result.reason] : [],
		);
		if (failures.length > 0) {
			throw new AggregateError(failures, 'Desktop Gateway client cleanup did not converge.');
		}
	}

	async #disposeBrowserAccessResources(): Promise<void> {
		const browserAccessRuntime = this.#browserAccessRuntime;
		const failures: unknown[] = [];
		try {
			await browserAccessRuntime?.dispose();
		} catch (error: unknown) {
			failures.push(error);
		}
		try {
			await this.#disposePendingGatewayClientCleanups();
		} catch (error: unknown) {
			failures.push(error);
		}
		if (failures.length > 0) {
			throw new AggregateError(failures, 'Desktop Browser access disposal did not converge.');
		}
		if (this.#browserAccessRuntime === browserAccessRuntime) {
			this.#browserAccessRuntime = undefined;
		}
	}

	#beginRendererDocument(): number {
		const generation = ++this.#documentGeneration;
		this.#applicationLifetimeBroker?.dispose();
		this.#applicationLifetimeBroker = undefined;
		this.#browserAccessBroker?.dispose();
		this.#browserAccessBroker = undefined;
		const clientSessionBroker = this.#clientSessionBroker;
		this.#clientSessionBroker = undefined;
		const projectionBroker = this.#projectionControlBroker;
		this.#projectionControlBroker = undefined;
		let retirementPromise: Promise<void> | undefined;
		const retire = async (): Promise<void> => {
			if (retirementPromise !== undefined) return retirementPromise;
			const operation = (async () => {
				let firstFailure: unknown;
				let clientDisposeFailed = false;
				try {
					await clientSessionBroker?.dispose();
				} catch (error: unknown) {
					clientDisposeFailed = true;
					firstFailure = error;
				}
				if (clientSessionBroker === undefined || clientDisposeFailed) {
					try {
						await projectionBroker?.dispose();
					} catch (error: unknown) {
						firstFailure ??= error;
					}
				}
				if (firstFailure !== undefined) throw firstFailure;
			})();
			retirementPromise = operation.catch((error: unknown) => {
				retirementPromise = undefined;
				throw error;
			});
			return retirementPromise;
		};
		if (clientSessionBroker !== undefined || projectionBroker !== undefined) {
			this.#pendingRendererDocumentRetirement ??= retire;
		}
		const transition = this.#documentTransition.then(async () => {
			let firstFailure: unknown;
			if (this.#pendingRendererBootstrapCleanup !== undefined) {
				const cleanup = this.#pendingRendererBootstrapCleanup;
				try {
					await cleanup();
					if (this.#pendingRendererBootstrapCleanup === cleanup) {
						this.#pendingRendererBootstrapCleanup = undefined;
					}
				} catch (error: unknown) {
					firstFailure = error;
				}
			}
			const retirement = this.#pendingRendererDocumentRetirement;
			if (retirement !== undefined) {
				try {
					await retirement();
					if (this.#pendingRendererDocumentRetirement === retirement) {
						this.#pendingRendererDocumentRetirement = undefined;
					}
				} catch (error: unknown) {
					firstFailure ??= error;
				}
			}
			if (firstFailure !== undefined) throw firstFailure;
			this.#documentFailure = undefined;
		});
		this.#documentTransition = transition.catch((error: unknown) => {
			this.#documentFailure =
				error instanceof Error ? error : new Error('Renderer document cleanup failed.');
		});
		return generation;
	}

	#queueRendererDocument(window: BrowserWindow, generation: number): void {
		this.#documentTransition = this.#documentTransition.then(async () => {
			try {
				await this.#attachRendererDocument(window, generation);
			} catch (error) {
				this.#documentFailure =
					error instanceof Error ? error : new Error('Renderer document control failed.');
			}
		});
	}

	async #attachRendererDocument(window: BrowserWindow, generation: number): Promise<void> {
		const terminalRuntime = this.#terminalRuntime;
		const clientSessionCoordinator = this.#clientSessionCoordinator;
		if (
			terminalRuntime === undefined ||
			clientSessionCoordinator === undefined ||
			this.#pendingRendererDocumentRetirement !== undefined ||
			this.#pendingRendererBootstrapCleanup !== undefined ||
			this.#window !== window ||
			window.isDestroyed() ||
			generation !== this.#documentGeneration ||
			this.#shuttingDown
		) {
			return;
		}
		let lifetimeChannel:
			| { readonly port1: MessagePortMain; readonly port2: MessagePortMain }
			| undefined;
		let browserAccessChannel:
			| { readonly port1: MessagePortMain; readonly port2: MessagePortMain }
			| undefined;
		let clientSessionChannel:
			| { readonly port1: MessagePortMain; readonly port2: MessagePortMain }
			| undefined;
		let projectionChannel:
			| { readonly port1: MessagePortMain; readonly port2: MessagePortMain }
			| undefined;
		let projectionBroker: DesktopProjectionControlBrokerHandle | undefined;
		let clientSessionBroker: DesktopClientSessionBrokerHandle | undefined;
		let lifetimeBroker: DesktopApplicationLifetimeBrokerHandle | undefined;
		let browserAccessBroker: DesktopBrowserAccessBrokerHandle | undefined;
		const cleanup = async (): Promise<void> => {
			let firstFailure: unknown;
			try {
				lifetimeBroker?.dispose();
			} catch (error: unknown) {
				firstFailure ??= error;
			}
			try {
				browserAccessBroker?.dispose();
			} catch (error: unknown) {
				firstFailure ??= error;
			}
			let clientDisposeFailed = false;
			try {
				await clientSessionBroker?.dispose();
			} catch (error: unknown) {
				clientDisposeFailed = true;
				firstFailure ??= error;
			}
			if (clientSessionBroker === undefined || clientDisposeFailed) {
				try {
					await projectionBroker?.dispose();
				} catch (error: unknown) {
					firstFailure ??= error;
				}
			}
			closeRendererChannel(lifetimeChannel);
			closeRendererChannel(browserAccessChannel);
			closeRendererChannel(clientSessionChannel);
			closeRendererChannel(projectionChannel);
			if (firstFailure !== undefined) throw firstFailure;
		};
		try {
			lifetimeChannel = this.#dependencies.createMessageChannel();
			browserAccessChannel = this.#dependencies.createMessageChannel();
			clientSessionChannel = this.#dependencies.createMessageChannel();
			projectionChannel = this.#dependencies.createMessageChannel();
			projectionBroker = this.#dependencies.createProjectionControlBroker(
				projectionChannel.port2,
				terminalRuntime,
			);
			clientSessionBroker = this.#dependencies.createClientSessionBroker(
				clientSessionChannel.port2,
				clientSessionCoordinator,
				async () => {
					await projectionBroker?.dispose();
				},
			);
			lifetimeBroker = this.#dependencies.createApplicationLifetimeBroker(lifetimeChannel.port2, {
				drainForStop: this.#drainForApplicationStop,
				prepareMode: (mode) =>
					clientSessionCoordinator.beginQuiescing(
						clientSessionBroker?.bootstrap.clientSessionId ?? '',
						mode,
					),
			});
			browserAccessBroker = this.#dependencies.createBrowserAccessBroker(
				browserAccessChannel.port2,
				this.#browserAccessRuntime ?? unavailableBrowserAccessRuntime,
			);
			clientSessionBroker.start();
			browserAccessBroker.start();
			lifetimeBroker.start();
			projectionBroker.start();
			window.webContents.postMessage(
				DESKTOP_RENDERER_BOOTSTRAP_IPC_CHANNEL,
				DesktopRendererBootstrapSchema.parse({
					clientSession: clientSessionBroker.bootstrap,
					type: DESKTOP_RENDERER_BOOTSTRAP_IPC_CHANNEL,
				}),
				[
					clientSessionChannel.port1,
					projectionChannel.port1,
					lifetimeChannel.port1,
					browserAccessChannel.port1,
				],
			);
			this.#applicationLifetimeBroker = lifetimeBroker;
			this.#browserAccessBroker = browserAccessBroker;
			this.#clientSessionBroker = clientSessionBroker;
			this.#projectionControlBroker = projectionBroker;
		} catch (error: unknown) {
			this.#pendingRendererBootstrapCleanup = cleanup;
			await cleanup();
			if (this.#pendingRendererBootstrapCleanup === cleanup) {
				this.#pendingRendererBootstrapCleanup = undefined;
			}
			throw error;
		}
	}

	async #openGatewayClient(): Promise<DesktopGatewayClientHandle> {
		const terminalRuntime = this.#terminalRuntime;
		const clientSessionCoordinator = this.#clientSessionCoordinator;
		if (
			terminalRuntime === undefined ||
			clientSessionCoordinator === undefined ||
			this.#shuttingDown
		) {
			throw new Error('Desktop Gateway client control is unavailable.');
		}
		let clientSessionChannel:
			| { readonly port1: MessagePortMain; readonly port2: MessagePortMain }
			| undefined;
		let projectionChannel:
			| { readonly port1: MessagePortMain; readonly port2: MessagePortMain }
			| undefined;
		let projectionBroker: DesktopProjectionControlBrokerHandle | undefined;
		let clientSessionBroker: DesktopClientSessionBrokerHandle | undefined;
		let disposalPromise: Promise<void> | undefined;
		let disposed = false;
		const cleanup = async (): Promise<void> => {
			if (disposed) return;
			if (disposalPromise !== undefined) return disposalPromise;
			const operation = (async () => {
				let firstFailure: unknown;
				let clientDisposeFailed = false;
				try {
					await clientSessionBroker?.dispose();
				} catch (error: unknown) {
					clientDisposeFailed = true;
					firstFailure = error;
				}
				if (clientSessionBroker === undefined || clientDisposeFailed) {
					try {
						await projectionBroker?.dispose();
					} catch (error: unknown) {
						firstFailure ??= error;
					}
				}
				closeRendererChannel(clientSessionChannel);
				closeRendererChannel(projectionChannel);
				if (firstFailure !== undefined) throw firstFailure;
				disposed = true;
				this.#pendingGatewayClientCleanups.delete(owner);
			})();
			disposalPromise = operation.catch((error: unknown) => {
				disposalPromise = undefined;
				throw error;
			});
			return disposalPromise;
		};
		const owner: GatewayClientResourceOwner = { dispose: cleanup };
		this.#pendingGatewayClientCleanups.add(owner);
		try {
			clientSessionChannel = this.#dependencies.createMessageChannel();
			projectionChannel = this.#dependencies.createMessageChannel();
			projectionBroker = this.#dependencies.createProjectionControlBroker(projectionChannel.port2, {
				open: async (presentationId) =>
					terminalRuntime.open(presentationId, { brokerEffectTarget: 'client' }),
			});
			clientSessionBroker = this.#dependencies.createClientSessionBroker(
				clientSessionChannel.port2,
				clientSessionCoordinator,
				async () => projectionBroker?.dispose(),
			);
			clientSessionBroker.start();
			projectionBroker.start();
			return {
				bootstrap: clientSessionBroker.bootstrap,
				clientSessionPort: clientSessionChannel.port1,
				dispose: cleanup,
				projectionPort: projectionChannel.port1,
			};
		} catch (error: unknown) {
			try {
				await cleanup();
			} catch (cleanupError: unknown) {
				throw new AggregateError(
					[error, cleanupError],
					'Desktop Gateway client bootstrap compensation failed.',
				);
			}
			throw error;
		}
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
		if (envelope.kind === MessageKind.terminalExited) {
			const exited = decodeControlJson(envelope.payload, TerminalExitedSchema);
			if (exited.ok) this.#acceptTerminalExit(exited.value);
			return;
		}
		if (envelope.kind !== MessageKind.terminalFailed) return;
		const failed = decodeControlJson(envelope.payload, TerminalFailedSchema);
		if (!failed.ok) return;
		this.#acceptTerminalFailure(failed.value);
	};

	#acceptTerminalFailure(event: TerminalFailed): void {
		const runtime = this.#terminalRuntime;
		if (runtime === undefined) return;
		void runtime
			.recordFailure(event)
			.then((presentationId) => {
				if (presentationId === null) return;
				const reconciler = this.#terminalIntentReconciler;
				const coordinator = this.#clientSessionCoordinator;
				if (reconciler === undefined || coordinator === undefined) return;
				const documentEvent = toDocumentFailure(presentationId, event);
				if (!reconciler.recordExternalEvent(documentEvent)) return;
				coordinator.publishTerminalEvent(documentEvent);
				this.#writeCopyPathTrace('terminal-failed');
			})
			.catch(() => this.#writeCopyPathTrace('terminal-failure-association-failed'));
	}

	#acceptTerminalExit(event: TerminalExited): void {
		const key = terminalExitIdentityKey(event);
		if (
			this.#isTerminalExitPublished(event) &&
			!this.#terminalExitPublicationReservations.has(key)
		) {
			return;
		}
		if (this.#terminalExitSpillReady && this.#terminalExitSpillStore.isRejected(event)) return;
		if (!this.#admitPendingTerminalExit(event, 'host-admission')) return;
		if (this.#terminalExitRetryOwnerDisposed || this.#terminalExitRetryOwnerClosing) {
			if (!this.#spillTerminalExit(event, 'shutdown')) {
				throw new Error('Desktop terminal exit owner is closed and its spill is full.');
			}
			this.#writeCopyPathTrace('terminal-exit-spilled-during-shutdown');
			return;
		}
		const existing = this.#pendingTerminalExits.get(key);
		if (
			existing === undefined &&
			this.#pendingTerminalExits.size >= MAX_PENDING_TERMINAL_EXIT_RETRIES
		) {
			// The Host event has no ACK/redelivery. Persist before returning from
			// this listener so a full in-memory owner cannot lose the envelope.
			const spill = this.#spillTerminalExit(event, 'queue-full');
			if (!spill) {
				this.#writeCopyPathTrace('terminal-exit-spill-full');
				throw new Error('Desktop terminal exit spill admission is full.');
			}
			this.#writeCopyPathTrace('terminal-exit-spilled');
			this.#scheduleTerminalExitRetry();
			return;
		}
		if (existing === undefined) {
			this.#pendingTerminalExits.set(key, event);
			this.#terminalExitAttemptCounts.set(key, 0);
		} else if (BigInt(event.finalRevision) > BigInt(existing.finalRevision)) {
			this.#pendingTerminalExits.set(key, event);
			this.#terminalExitAttemptCounts.set(key, 0);
		}
		void this.#attemptTerminalExit(key);
	}

	#admitPendingTerminalExit(event: TerminalExited, reason: TerminalExitSpillReason): boolean {
		if (!this.#terminalExitSpillReady) {
			throw new Error('Desktop terminal exit pending journal is not ready.');
		}
		try {
			const admission = this.#terminalExitSpillStore.admit(event, reason);
			if (admission.kind === 'admitted') return true;
			const rejection = this.#terminalExitSpillStore.reject(event, 'pending-journal-full');
			if (rejection.kind === 'recorded') {
				this.#writeCopyPathTrace('terminal-exit-rejected-pending-journal-full');
				return false;
			}
			throw new Error('Desktop terminal exit rejection journal is full.');
		} catch (error: unknown) {
			try {
				const rejection = this.#terminalExitSpillStore.reject(event, 'write-failed');
				if (rejection.kind === 'recorded') {
					this.#writeCopyPathTrace('terminal-exit-rejected-write-failed');
					return false;
				}
			} catch (rejectionError: unknown) {
				throw new AggregateError(
					[error, rejectionError],
					'Desktop terminal exit pending journal failed closed.',
				);
			}
			throw error;
		}
	}

	#attemptTerminalExit(key: string): Promise<void> {
		const existingAttempt = this.#terminalExitAttempts.get(key);
		if (existingAttempt !== undefined) return existingAttempt;
		const operation = this.#recordPendingTerminalExit(key).catch(() => {
			// A failed attempt keeps the event in the Main-owned bounded queue and
			// schedules a later retry. This branch is deliberately contained so a
			// Host envelope can never create an unhandled rejection.
			this.#writeCopyPathTrace('terminal-exit-persistence-failed');
		});
		this.#terminalExitAttempts.set(key, operation);
		void operation.then(() => {
			if (this.#terminalExitAttempts.get(key) === operation) {
				this.#terminalExitAttempts.delete(key);
			}
			if (
				!this.#terminalExitRetryOwnerDisposed &&
				!this.#terminalExitRetryOwnerClosing &&
				this.#pendingTerminalExits.has(key)
			) {
				this.#scheduleTerminalExitRetry();
			}
		});
		return operation;
	}

	#publishTerminalExit(
		runtime: DesktopTerminalRuntime,
		event: TerminalExited,
		presentationId: ReturnType<typeof terminalPresentationId>,
		publish: (
			event: Extract<TerminalPresentationEvent, { readonly type: 'terminal-exited' }>,
		) => unknown,
	): TerminalExitPublicationResult {
		const key = terminalExitIdentityKey(event);
		const reconciler = this.#terminalIntentReconciler;
		if (reconciler === undefined) return 'pending';
		let currentReservation = this.#terminalExitPublicationReservations.get(key);
		if (
			currentReservation === undefined ||
			BigInt(currentReservation.event.finalRevision) !== BigInt(event.finalRevision)
		) {
			const documentEvent = toDocumentExit(presentationId, event);
			if (this.#isTerminalExitPublished(event) && reconciler.hasExternalEvent(documentEvent)) {
				return 'already-committed';
			}
			if (!reconciler.reserveExternalEvent(documentEvent)) return 'pending';
			currentReservation = {
				event,
				presentationId,
			};
			this.#terminalExitPublicationReservations.set(key, currentReservation);
		}
		if (currentReservation === undefined) return 'pending';
		const reservationEvent = currentReservation.event;
		const documentEvent = toDocumentExit(currentReservation.presentationId, reservationEvent);
		if (
			!reconciler.hasExternalEvent(documentEvent) &&
			!reconciler.reserveExternalEvent(documentEvent)
		) {
			return 'pending';
		}
		const ledgerCommitted = this.#isTerminalExitPublished(reservationEvent);
		if (!ledgerCommitted) {
			let committed: boolean;
			try {
				committed = runtime.commitExitPublication?.(reservationEvent) ?? true;
			} catch {
				this.#writeCopyPathTrace('terminal-exit-adapter-commit-failed');
				return 'pending';
			}
			if (!committed) return 'pending';
			const marked = this.#markTerminalExitPublished(reservationEvent);
			if (!marked) return 'pending';
			this.#publishedTerminalExitRevisions.set(key, reservationEvent.finalRevision);
		}
		const externalEventCommitted = reconciler.commitExternalEvent(documentEvent);
		if (!externalEventCommitted) return 'pending';
		try {
			if (publish(documentEvent) === false) return 'pending';
		} catch {
			this.#writeCopyPathTrace('terminal-exit-publication-failed');
			return 'pending';
		}
		this.#terminalExitPublicationReservations.delete(key);
		this.#removePublishedTerminalExitFromPending(reservationEvent);
		return ledgerCommitted ? 'already-committed' : 'committed';
	}

	#commitReconciledTerminalExit(
		runtime: DesktopTerminalRuntime,
		event: TerminalPresentationEvent,
		publish: (event: TerminalPresentationEvent) => unknown,
	): boolean {
		if (event.type !== 'terminal-exited') return true;
		const terminalExit = runtime.getDurableTerminalExit(event.presentationId);
		if (terminalExit === null) return false;
		const publication = this.#publishTerminalExit(
			runtime,
			terminalExit,
			event.presentationId,
			publish,
		);
		if (publication === 'pending') {
			this.#acceptTerminalExit(terminalExit);
			return false;
		}
		return true;
	}

	#isTerminalExitPublished(event: TerminalExited): boolean {
		const publishedRevision = this.#publishedTerminalExitRevisions.get(
			terminalExitIdentityKey(event),
		);
		if (
			publishedRevision !== undefined &&
			BigInt(publishedRevision) >= BigInt(event.finalRevision)
		) {
			return true;
		}
		if (!this.#terminalExitSpillReady) return false;
		try {
			return this.#terminalExitPublicationLedger.has(event);
		} catch {
			return false;
		}
	}

	#markTerminalExitPublished(event: TerminalExited): boolean {
		try {
			return this.#terminalExitPublicationLedger.markPublished(event);
		} catch {
			this.#writeCopyPathTrace('terminal-exit-publication-ledger-mark-failed');
			return false;
		}
	}

	async #recordPendingTerminalExit(key: string): Promise<void> {
		const event = this.#pendingTerminalExits.get(key);
		const runtime = this.#terminalRuntime;
		if (event === undefined || runtime === undefined) return;
		this.#terminalExitAttemptCounts.set(key, (this.#terminalExitAttemptCounts.get(key) ?? 0) + 1);
		const outcome = await this.#recordTerminalExit(runtime, event);
		if (outcome.kind === 'retryable') {
			this.#writeCopyPathTrace(`terminal-exit-${outcome.reason}`);
			if (
				(outcome.reason === 'association' || outcome.reason === 'buffer-full') &&
				(this.#terminalExitAttemptCounts.get(key) ?? 0) >= TERMINAL_EXIT_ASSOCIATION_RETRY_LIMIT
			) {
				if (this.#spillTerminalExit(event, 'association-pending')) {
					runtime.releasePendingExit?.(event);
					this.#deleteTerminalExitIfCurrent(key, event, false);
					this.#writeCopyPathTrace('terminal-exit-association-spilled');
				} else {
					this.#writeCopyPathTrace('terminal-exit-spill-full');
				}
			}
			return;
		}
		if (outcome.kind === 'unattributed') {
			this.#unknownIdentityAttempts.add(key);
			if (this.#spillTerminalExit(event, 'unknown-identity')) {
				this.#deleteTerminalExitIfCurrent(key, event, false);
				this.#writeCopyPathTrace('terminal-exit-unknown-identity');
			} else {
				this.#writeCopyPathTrace('terminal-exit-spill-full');
			}
			return;
		}
		if (outcome.kind === 'committed') {
			const publication = this.#publishTerminalExit(
				runtime,
				event,
				outcome.presentationId,
				(documentEvent) => {
					const coordinator = this.#clientSessionCoordinator;
					if (coordinator === undefined)
						throw new Error('Desktop client session coordinator is unavailable.');
					return coordinator.publishTerminalEvent(documentEvent);
				},
			);
			if (publication === 'pending') return;
			if (publication === 'committed') this.#writeCopyPathTrace('terminal-exited');
			this.#deleteTerminalExitIfCurrent(key, event);
			return;
		}
		if (outcome.kind === 'ignored') {
			// A close lifecycle can consume the exit while retiring its binding;
			// there is no publication to commit or spill in that case.
			if (outcome.reason === 'closed' || this.#isTerminalExitPublished(event)) {
				this.#deleteTerminalExitIfCurrent(key, event);
			}
			return;
		}
		if (outcome.kind === 'durable' || outcome.kind === 'published') {
			const publication = this.#publishTerminalExit(
				runtime,
				event,
				outcome.presentationId,
				(documentEvent) => {
					const coordinator = this.#clientSessionCoordinator;
					if (coordinator === undefined)
						throw new Error('Desktop client session coordinator is unavailable.');
					return coordinator.publishTerminalEvent(documentEvent);
				},
			);
			if (publication === 'pending') return;
			if (publication === 'committed') this.#writeCopyPathTrace('terminal-exited');
		}
		this.#deleteTerminalExitIfCurrent(key, event);
	}

	async #recordTerminalExit(
		runtime: DesktopTerminalRuntime,
		event: TerminalExited,
	): Promise<DesktopTerminalExitRecordResult> {
		if (typeof runtime.recordExitOutcome === 'function') {
			return runtime.recordExitOutcome(event);
		}
		const presentationId = await runtime.recordExit(event);
		return presentationId === null
			? { kind: 'ignored' }
			: { finalRevision: event.finalRevision, kind: 'durable', presentationId };
	}

	#removePublishedTerminalExitFromPending(event: TerminalExited): void {
		if (!this.#terminalExitSpillReady) return;
		try {
			this.#terminalExitSpillStore.remove(event);
		} catch {
			// The publication ledger and reconciler are already authoritative. Keep
			// the pending journal entry as a durable compaction retry if this cleanup
			// write fails.
			this.#writeCopyPathTrace('terminal-exit-pending-journal-compaction-failed');
		}
	}

	#deleteTerminalExitIfCurrent(key: string, event: TerminalExited, removeSpill = true): void {
		const current = this.#pendingTerminalExits.get(key);
		if (current !== undefined && BigInt(current.finalRevision) <= BigInt(event.finalRevision)) {
			this.#pendingTerminalExits.delete(key);
			this.#terminalExitAttemptCounts.delete(key);
			if (removeSpill) this.#removePublishedTerminalExitFromPending(event);
		}
	}

	#scheduleTerminalExitRetry(): void {
		if (
			this.#terminalExitRetryOwnerDisposed ||
			this.#terminalExitRetryOwnerClosing ||
			(this.#pendingTerminalExits.size === 0 && !this.#hasRetryableSpilledTerminalExits()) ||
			this.#terminalExitRetryTimer !== undefined
		) {
			return;
		}
		const timer = setTimeout(() => {
			this.#terminalExitRetryTimer = undefined;
			this.#promoteSpilledTerminalExits();
			for (const key of this.#pendingTerminalExits.keys()) void this.#attemptTerminalExit(key);
			this.#scheduleTerminalExitRetry();
		}, TERMINAL_EXIT_RETRY_DELAY_MILLISECONDS);
		timer.unref?.();
		this.#terminalExitRetryTimer = timer;
	}

	async #disposeTerminalExitRetryOwner(): Promise<void> {
		if (this.#terminalExitRetryOwnerDisposed && this.#pendingTerminalExits.size === 0) return;
		this.#terminalExitRetryOwnerClosing = true;
		if (this.#terminalExitRetryTimer !== undefined) {
			clearTimeout(this.#terminalExitRetryTimer);
			this.#terminalExitRetryTimer = undefined;
		}
		await Promise.all(this.#terminalExitAttempts.values());
		for (const key of this.#pendingTerminalExits.keys()) {
			await this.#attemptTerminalExit(key);
		}
		await Promise.all(this.#terminalExitAttempts.values());
		if (this.#pendingTerminalExits.size > 0) {
			let spillFailure: Error | undefined;
			for (const [key, event] of this.#pendingTerminalExits) {
				try {
					if (!this.#spillTerminalExit(event, 'shutdown')) {
						spillFailure = new Error('Desktop terminal exit spill admission is full.');
						break;
					}
					this.#deleteTerminalExitIfCurrent(key, event, false);
				} catch (error: unknown) {
					spillFailure = error instanceof Error ? error : new Error('Terminal exit spill failed.');
					break;
				}
			}
			if (spillFailure !== undefined) {
				this.#terminalExitRetryOwnerClosing = false;
				this.#writeCopyPathTrace('terminal-exit-retry-pending-on-dispose');
				throw spillFailure;
			}
		}
		this.#terminalExitRetryOwnerDisposed = true;
		this.#terminalExitRetryOwnerClosing = false;
	}

	#spillTerminalExit(event: TerminalExited, reason: TerminalExitSpillReason): boolean {
		if (!this.#terminalExitSpillReady) return false;
		return this.#terminalExitSpillStore.admit(event, reason).kind === 'admitted';
	}

	#hasRetryableSpilledTerminalExits(): boolean {
		if (!this.#terminalExitSpillReady) return false;
		return this.#terminalExitSpillStore.list().some((entry) => {
			if (this.#isTerminalExitPublished(entry.event)) return false;
			if (
				entry.reason === 'host-admission' ||
				entry.reason === 'queue-full' ||
				entry.reason === 'shutdown'
			) {
				return true;
			}
			const key = terminalExitIdentityKey(entry.event);
			return this.#associatedTerminalExitKeys.has(key) && !this.#unknownIdentityAttempts.has(key);
		});
	}

	#promoteSpilledTerminalExits(): void {
		if (!this.#terminalExitSpillReady) return;
		const available = MAX_PENDING_TERMINAL_EXIT_RETRIES - this.#pendingTerminalExits.size;
		if (available <= 0) return;
		const candidates = this.#terminalExitSpillStore
			.list()
			.filter((entry) => {
				if (this.#isTerminalExitPublished(entry.event)) {
					this.#removePublishedTerminalExitFromPending(entry.event);
					return false;
				}
				if (
					entry.reason === 'host-admission' ||
					entry.reason === 'queue-full' ||
					entry.reason === 'shutdown'
				) {
					return true;
				}
				const key = terminalExitIdentityKey(entry.event);
				return this.#associatedTerminalExitKeys.has(key) && !this.#unknownIdentityAttempts.has(key);
			})
			.slice(0, available);
		for (const entry of candidates) {
			const key = terminalExitIdentityKey(entry.event);
			if (this.#pendingTerminalExits.has(key)) continue;
			this.#pendingTerminalExits.set(key, entry.event);
			this.#terminalExitAttemptCounts.set(key, 0);
			void this.#attemptTerminalExit(key);
		}
	}

	readonly #handleTerminalIdentityObserved = (identity: TerminalIdentity): void => {
		const key = terminalExitIdentityKey(identity);
		this.#associatedTerminalExitKeys.add(key);
		this.#unknownIdentityAttempts.delete(key);
		if (this.#pendingTerminalExits.has(key)) {
			void this.#attemptTerminalExit(key);
		} else {
			this.#promoteSpilledTerminalExits();
		}
		this.#scheduleTerminalExitRetry();
	};

	#writeCopyPathTrace(reason: string): void {
		if (!this.#options.traceCopyPath || this.#connection === undefined) return;
		try {
			this.#dependencies.writeTrace({
				brokers: this.#terminalRuntime?.getProjectionMetrics() ?? [],
				hostTransport: this.#connection.metrics,
				reason,
				type: 'orchbee.desktop.copy-path',
			});
		} catch {
			// Diagnostics are subordinate to the durable owner and cannot change its
			// retry or rejection state.
		}
	}
}

function isPermittedDesktopRendererUrl(url: URL): boolean {
	if (url.username !== '' || url.password !== '') return false;
	return (
		(url.protocol === 'http:' && url.hostname === '127.0.0.1') ||
		(url.protocol === 'orchbee:' && url.hostname === 'app' && url.port === '')
	);
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

function closeRendererChannel(
	channel: { readonly port1: MessagePortMain; readonly port2: MessagePortMain } | undefined,
): void {
	if (channel === undefined) return;
	try {
		channel.port1.close();
	} catch {
		// Renderer bootstrap rollback is best effort after ownership is retained.
	}
	try {
		channel.port2.close();
	} catch {
		// Renderer bootstrap rollback is best effort after ownership is retained.
	}
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

function toDocumentFailure(
	presentationId: Parameters<DesktopTerminalRuntime['getProjectionTarget']>[0],
	failed: TerminalFailed,
): Extract<TerminalPresentationEvent, { readonly type: 'terminal-failed' }> {
	return {
		failure: {
			code: `terminal-runtime-${failed.failure.kind}-${failed.failure.stage}`,
			retryable: false,
		},
		presentationId,
		type: 'terminal-failed',
	};
}

function terminalExitIdentityKey(source: TerminalExited | TerminalIdentity): string {
	const identity = 'identity' in source ? source.identity : source;
	return `${identity.hostEpoch}:${identity.creationId}:${identity.terminalId}`;
}

function signalName(signal: number): string {
	for (const [name, value] of Object.entries(constants.signals)) {
		if (value === signal) return name;
	}
	return `SIGNAL_${signal}`;
}
