import { randomBytes } from 'node:crypto';

import {
	AttachTerminalSchema,
	AttachmentCapabilitiesSchema,
	AttachmentIdSchema,
	ClientIdSchema,
	decodeControlJson,
	MessageKind,
	TerminalExitedSchema,
	TerminalPhase,
	type DecodedProtocolEnvelope,
	type TerminalExited,
	type TerminalIdentity,
	type TerminalSummary,
} from '@orchbee/terminal-protocol';
import {
	BrowserWindow,
	MessageChannelMain,
	type BrowserWindowConstructorOptions,
	type MessagePortMain,
} from 'electron';

import { PROJECTED_ATTACHMENT_IPC_CHANNEL, ProjectedAttachmentBootstrapSchema } from '../../shared';
import { ProjectedTerminalBroker, type ProjectedTerminalBrokerMetrics } from '../broker';
import { DevelopmentHostRuntime, type HostConnection } from '../host';
import { FileTerminalBindingStore, TerminalRecoveryCoordinator } from '../recovery';
import {
	TerminalBindingLifecycle,
	TerminalCloseOrchestrator,
	type TerminalBindingLifecycleOutcome,
	type TerminalCloseOutcome,
} from '../terminal-lifetime';
import type { DesktopRuntimeConfiguration } from './resolveDesktopRuntimeConfiguration';

interface DesktopBrokerHandle {
	readonly metrics: ProjectedTerminalBrokerMetrics;
	dispose(): Promise<void>;
	start(): void;
}

interface DesktopTerminalLifecycleHandle {
	reconcile(): Promise<TerminalBindingLifecycleOutcome>;
	recordExit(event: TerminalExited): Promise<void>;
	requestClose(retireProjection: () => Promise<void>): Promise<TerminalCloseOutcome>;
}

export interface DesktopApplicationOptions {
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
		bootstrap: ReturnType<typeof ProjectedAttachmentBootstrapSchema.parse>,
	) => DesktopBrokerHandle;
	readonly createMessageChannel: () => {
		readonly port1: MessagePortMain;
		readonly port2: MessagePortMain;
	};
	readonly createTerminalLifecycle: (
		host: HostConnection,
		configuration: DesktopRuntimeConfiguration,
		randomIdentifier: () => string,
	) => DesktopTerminalLifecycleHandle;
	readonly createWindow: (options: BrowserWindowConstructorOptions) => BrowserWindow;
	readonly randomIdentifier: () => string;
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
	createMessageChannel: () => new MessageChannelMain(),
	createTerminalLifecycle: (host, configuration, randomIdentifier) => {
		const store = new FileTerminalBindingStore(configuration.terminalBindingStorePath);
		return new TerminalBindingLifecycle(
			new TerminalRecoveryCoordinator(
				host,
				store,
				{
					bindingId: configuration.recoveryBindingId,
					launch: configuration.terminalLaunch,
				},
				{ randomCreationId: randomIdentifier },
			),
			new TerminalCloseOrchestrator(host, store, configuration.recoveryBindingId),
		);
	},
	createWindow: (options) => new BrowserWindow(options),
	randomIdentifier: () => randomBytes(16).toString('hex'),
	writeTrace: (trace) => console.info(JSON.stringify(trace)),
};

export class DesktopApplication {
	readonly #dependencies: DesktopApplicationDependencies;
	readonly #options: DesktopApplicationOptions;
	#attachmentFailure: Error | undefined;
	#attachmentTransition = Promise.resolve();
	#broker: DesktopBrokerHandle | undefined;
	#connection: HostConnection | undefined;
	#documentGeneration = 0;
	#identity: TerminalIdentity | undefined;
	#terminalLifecycle: DesktopTerminalLifecycleHandle | undefined;
	#removeHostListener: (() => void) | undefined;
	#shuttingDown = false;
	#summary: TerminalSummary | null | undefined;
	#terminalClosing = false;
	#terminalExit: TerminalExited | undefined;
	#window: BrowserWindow | undefined;

	constructor(
		options: DesktopApplicationOptions,
		dependencies: DesktopApplicationDependencies = defaultDependencies,
	) {
		if (!options.singleInstanceLockOwned) {
			throw new Error('Desktop recovery requires the Electron single-instance lock.');
		}
		this.#options = options;
		this.#dependencies = dependencies;
	}

	get hasWindow(): boolean {
		return this.#window !== undefined && !this.#window.isDestroyed();
	}

	focusWindow(): void {
		if (this.hasWindow) this.#window?.focus();
	}

	async start(): Promise<void> {
		if (this.#connection !== undefined) throw new Error('Desktop application is already started.');
		const connection = await this.#dependencies.connectHost(this.#options.runtime);
		this.#connection = connection;
		try {
			await connection.negotiate(ClientIdSchema.parse(this.#dependencies.randomIdentifier()));
			const terminalLifecycle = this.#dependencies.createTerminalLifecycle(
				connection,
				this.#options.runtime,
				this.#dependencies.randomIdentifier,
			);
			this.#terminalLifecycle = terminalLifecycle;
			const outcome = await terminalLifecycle.reconcile();
			if (outcome.kind !== 'provisioning' && outcome.kind !== 'recovered') {
				throw new Error(`Desktop terminal recovery stopped with ${outcome.kind}.`);
			}
			this.#identity = outcome.identity;
			this.#summary = outcome.summary;
			this.#terminalExit = terminalExitFromSummary(outcome.summary);
			this.#removeHostListener = connection.onEnvelope(this.#handleHostEnvelope);
			await this.openWindow();
		} catch (error) {
			this.#removeHostListener?.();
			this.#removeHostListener = undefined;
			this.#terminalLifecycle = undefined;
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
		if (this.#connection === undefined || this.#identity === undefined) {
			throw new Error('Desktop application must start before opening a window.');
		}
		this.#attachmentFailure = undefined;
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
			this.#queueRendererAttachment(window, generation);
		});
		window.webContents.on('render-process-gone', () => {
			this.#beginRendererDocument();
			void this.#attachmentTransition.then(() => {
				if (this.#window === window && !window.isDestroyed() && !this.#shuttingDown) {
					window.reload();
				}
			});
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
		await this.#attachmentTransition;
		if (this.#attachmentFailure !== undefined) throw this.#attachmentFailure;
	}

	async requestTerminalClose(): Promise<TerminalCloseOutcome> {
		const terminalLifecycle = this.#terminalLifecycle;
		if (terminalLifecycle === undefined) {
			throw new Error('Desktop application must start before closing its terminal.');
		}
		return terminalLifecycle.requestClose(async () => {
			this.#terminalClosing = true;
			this.#writeCopyPathTrace('terminal-closing');
			this.#beginRendererDocument();
			await this.#attachmentTransition;
		});
	}

	async shutdown(): Promise<void> {
		if (this.#shuttingDown) return this.#attachmentTransition;
		this.#shuttingDown = true;
		this.#removeHostListener?.();
		this.#removeHostListener = undefined;
		this.#writeCopyPathTrace('main-shutdown');
		this.#beginRendererDocument();
		await this.#attachmentTransition;
		this.#connection?.close();
		this.#connection = undefined;
		this.#identity = undefined;
		this.#summary = undefined;
		this.#terminalClosing = false;
		this.#terminalExit = undefined;
		this.#terminalLifecycle = undefined;
	}

	#beginRendererDocument(): number {
		const generation = ++this.#documentGeneration;
		const broker = this.#broker;
		this.#broker = undefined;
		const retirement = broker?.dispose() ?? Promise.resolve();
		this.#attachmentTransition = this.#attachmentTransition.then(async () => retirement);
		return generation;
	}

	#queueRendererAttachment(window: BrowserWindow, generation: number): void {
		this.#attachmentTransition = this.#attachmentTransition.then(async () => {
			try {
				await this.#attachRendererDocument(window, generation);
			} catch (error) {
				this.#attachmentFailure =
					error instanceof Error ? error : new Error('Renderer attachment failed.');
			}
		});
	}

	async #attachRendererDocument(window: BrowserWindow, generation: number): Promise<void> {
		const connection = this.#connection;
		const identity = this.#identity;
		if (
			connection === undefined ||
			identity === undefined ||
			this.#window !== window ||
			window.isDestroyed() ||
			generation !== this.#documentGeneration ||
			this.#terminalClosing ||
			this.#shuttingDown
		) {
			return;
		}
		const readOnly =
			this.#terminalExit !== undefined ||
			this.#summary?.phase === TerminalPhase.finishing ||
			this.#summary?.phase === TerminalPhase.exited;
		const attachmentId = AttachmentIdSchema.parse(this.#dependencies.randomIdentifier());
		const capabilities = await connection.requestControl(
			MessageKind.attachTerminal,
			AttachTerminalSchema.parse({
				attachmentId,
				capabilities: { input: !readOnly, read: true, resize: !readOnly },
				requestResizeLease: !readOnly,
				terminalId: identity.terminalId,
			}),
			MessageKind.attachmentCapabilities,
			AttachmentCapabilitiesSchema,
		);
		if (
			capabilities.attachmentId !== attachmentId ||
			capabilities.terminalId !== identity.terminalId ||
			!capabilities.capabilities.read ||
			(readOnly &&
				(capabilities.capabilities.input ||
					capabilities.capabilities.resize ||
					capabilities.resizeLease)) ||
			(!readOnly &&
				(!capabilities.capabilities.input ||
					!capabilities.capabilities.resize ||
					!capabilities.resizeLease))
		) {
			throw new Error('Terminal Host did not grant the required local attachment capabilities.');
		}
		const bootstrap = ProjectedAttachmentBootstrapSchema.parse({
			attachmentId,
			canInput: capabilities.capabilities.input,
			canResize: capabilities.resizeLease,
			creationId: identity.creationId,
			hostEpoch: identity.hostEpoch,
			terminalExit: this.#terminalExit ?? null,
			terminalId: identity.terminalId,
		});
		const channel = this.#dependencies.createMessageChannel();
		const broker = this.#dependencies.createBroker(connection, channel.port2, bootstrap);
		if (
			this.#window !== window ||
			window.isDestroyed() ||
			generation !== this.#documentGeneration ||
			this.#shuttingDown
		) {
			await broker.dispose();
			return;
		}
		this.#broker = broker;
		broker.start();
		window.webContents.postMessage(PROJECTED_ATTACHMENT_IPC_CHANNEL, bootstrap, [channel.port1]);
	}

	readonly #handleHostEnvelope = (envelope: DecodedProtocolEnvelope): void => {
		if (envelope.kind !== MessageKind.terminalExited) return;
		const exited = decodeControlJson(envelope.payload, TerminalExitedSchema);
		if (!exited.ok || exited.value.identity.terminalId !== this.#identity?.terminalId) return;
		this.#terminalExit = exited.value;
		void this.#terminalLifecycle?.recordExit(exited.value).catch(() => {
			console.error('Orchbee Desktop could not persist terminal exit recovery state.');
		});
		this.#writeCopyPathTrace('terminal-exited');
	};

	#writeCopyPathTrace(reason: string): void {
		if (!this.#options.traceCopyPath || this.#connection === undefined) return;
		this.#dependencies.writeTrace({
			broker: this.#broker?.metrics ?? null,
			hostTransport: this.#connection.metrics,
			reason,
			type: 'orchbee.desktop.copy-path',
		});
	}
}

function terminalExitFromSummary(summary: TerminalSummary | null): TerminalExited | undefined {
	return summary?.phase === TerminalPhase.exited && summary.exit !== null
		? {
				exit: summary.exit,
				finalRevision: summary.revision,
				identity: summary.identity,
			}
		: undefined;
}
