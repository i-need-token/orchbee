import { randomBytes } from 'node:crypto';
import { constants } from 'node:os';

import {
	ClientIdSchema,
	decodeControlJson,
	MessageKind,
	TerminalExitedSchema,
	type DecodedProtocolEnvelope,
	type TerminalExited,
} from '@orchbee/terminal-protocol';
import { TerminalPresentationIdSchema } from '@orchbee/workbench/terminal-presentation';
import {
	BrowserWindow,
	MessageChannelMain,
	type BrowserWindowConstructorOptions,
	type MessagePortMain,
} from 'electron';

import {
	TERMINAL_DOCUMENT_CONTROL_IPC_CHANNEL,
	TERMINAL_PROJECTION_CONTROL_IPC_CHANNEL,
	WORKBENCH_TOPOLOGY_CONTROL_IPC_CHANNEL,
	TerminalDocumentControlBootstrapSchema,
	TerminalProjectionControlBootstrapSchema,
	WorkbenchTopologyControlBootstrapSchema,
	type ProjectedAttachmentBootstrap,
	type TerminalDocumentControlCommand,
	type TerminalDocumentControlEvent,
	type TerminalDocumentControlOperationResult,
} from '../../shared';
import { ProjectedTerminalBroker, type ProjectedTerminalBrokerMetrics } from '../broker';
import { DevelopmentHostRuntime, type HostConnection } from '../host';
import {
	TerminalDocumentControlBroker,
	type TerminalDocumentControlOperations,
	type TerminalDocumentControlResultPosted,
} from '../terminal-document-control';
import {
	TerminalProjectionControlBroker,
	type DesktopTerminalProjectionLease,
} from '../terminal-projection-control';
import {
	FileWorkbenchTopologyStore,
	WorkbenchTopologyControlBroker,
	type WorkbenchTopologyStore,
} from '../workbench-topology';
import {
	composeDesktopTerminalRuntime,
	type DesktopTerminalRuntime,
} from './composeDesktopTerminalRuntime';
import type { DesktopRuntimeConfiguration } from './resolveDesktopRuntimeConfiguration';

interface DesktopBrokerHandle {
	readonly metrics: ProjectedTerminalBrokerMetrics;
	dispose(): Promise<void>;
	start(): void;
}

interface DesktopDocumentBrokerHandle {
	dispose(): void;
	publishTerminalExit(
		event: Extract<TerminalDocumentControlEvent, { readonly type: 'terminal-exited' }>,
	): boolean;
	start(): void;
}

interface DesktopProjectionControlBrokerHandle {
	dispose(): Promise<void>;
	start(): void;
}

interface DesktopWorkbenchTopologyBrokerHandle {
	dispose(): void;
	start(): void;
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
		bootstrap: ProjectedAttachmentBootstrap,
	) => DesktopBrokerHandle;
	readonly createDocumentBroker: (
		port: MessagePortMain,
		operations: TerminalDocumentControlOperations,
		resultPosted: TerminalDocumentControlResultPosted,
	) => DesktopDocumentBrokerHandle;
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
	readonly createWorkbenchTopologyBroker: (
		port: MessagePortMain,
		store: WorkbenchTopologyStore,
	) => DesktopWorkbenchTopologyBrokerHandle;
	readonly createWorkbenchTopologyStore: (filePath: string) => WorkbenchTopologyStore;
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
	createDocumentBroker: (port, operations, resultPosted) =>
		new TerminalDocumentControlBroker(port, operations, resultPosted),
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
	createWorkbenchTopologyBroker: (port, store) => new WorkbenchTopologyControlBroker(port, store),
	createWorkbenchTopologyStore: (filePath) => new FileWorkbenchTopologyStore(filePath),
	createWindow: (options) => new BrowserWindow(options),
	randomIdentifier: () => randomBytes(16).toString('hex'),
	writeTrace: (trace) => console.info(JSON.stringify(trace)),
};

export class DesktopApplication {
	readonly #dependencies: DesktopApplicationDependencies;
	readonly #options: DesktopApplicationOptions;
	#connection: HostConnection | undefined;
	#documentBroker: DesktopDocumentBrokerHandle | undefined;
	#documentFailure: Error | undefined;
	#documentGeneration = 0;
	#documentTransition = Promise.resolve();
	#projectionControlBroker: DesktopProjectionControlBrokerHandle | undefined;
	#removeHostListener: (() => void) | undefined;
	#shuttingDown = false;
	#terminalRuntime: DesktopTerminalRuntime | undefined;
	#window: BrowserWindow | undefined;
	#workbenchTopologyBroker: DesktopWorkbenchTopologyBrokerHandle | undefined;
	readonly #workbenchTopologyStore: WorkbenchTopologyStore;

	constructor(
		options: DesktopApplicationOptions,
		dependencies: DesktopApplicationDependencies = defaultDependencies,
	) {
		if (!options.singleInstanceLockOwned) {
			throw new Error('Desktop recovery requires the Electron single-instance lock.');
		}
		this.#options = options;
		this.#dependencies = dependencies;
		this.#workbenchTopologyStore = dependencies.createWorkbenchTopologyStore(
			options.runtime.workbenchTopologyStorePath,
		);
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
			this.#removeHostListener = connection.onEnvelope(this.#handleHostEnvelope);
			await this.openWindow();
		} catch (error) {
			this.#removeHostListener?.();
			this.#removeHostListener = undefined;
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
		await this.#terminalRuntime?.dispose();
		this.#connection?.close();
		this.#connection = undefined;
		this.#terminalRuntime = undefined;
	}

	#beginRendererDocument(): number {
		const generation = ++this.#documentGeneration;
		this.#documentBroker?.dispose();
		this.#documentBroker = undefined;
		this.#workbenchTopologyBroker?.dispose();
		this.#workbenchTopologyBroker = undefined;
		const projectionBroker = this.#projectionControlBroker;
		this.#projectionControlBroker = undefined;
		this.#documentTransition = this.#documentTransition.then(async () =>
			projectionBroker?.dispose(),
		);
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
		if (
			terminalRuntime === undefined ||
			this.#window !== window ||
			window.isDestroyed() ||
			generation !== this.#documentGeneration ||
			this.#shuttingDown
		) {
			return;
		}
		const defaultCwd = this.#options.runtime.terminalLaunch.currentDirectory;
		if (defaultCwd === null) throw new Error('Desktop terminal runtime requires a default CWD.');
		const documentChannel = this.#dependencies.createMessageChannel();
		const projectionChannel = this.#dependencies.createMessageChannel();
		const topologyChannel = this.#dependencies.createMessageChannel();
		const documentBroker = this.#dependencies.createDocumentBroker(
			documentChannel.port2,
			terminalRuntime,
			this.#handleDocumentResultPosted,
		);
		const projectionBroker = this.#dependencies.createProjectionControlBroker(
			projectionChannel.port2,
			terminalRuntime,
		);
		const topologyBroker = this.#dependencies.createWorkbenchTopologyBroker(
			topologyChannel.port2,
			this.#workbenchTopologyStore,
		);
		this.#documentBroker = documentBroker;
		this.#projectionControlBroker = projectionBroker;
		this.#workbenchTopologyBroker = topologyBroker;
		documentBroker.start();
		projectionBroker.start();
		topologyBroker.start();
		const documentBootstrap = TerminalDocumentControlBootstrapSchema.parse({
			defaultCwd,
			initialPresentationId: TerminalPresentationIdSchema.parse(
				`desktop:presentation:${this.#dependencies.randomIdentifier()}`,
			),
			presentations: terminalRuntime.getRecoveredPresentations(),
			type: TERMINAL_DOCUMENT_CONTROL_IPC_CHANNEL,
		});
		window.webContents.postMessage(TERMINAL_DOCUMENT_CONTROL_IPC_CHANNEL, documentBootstrap, [
			documentChannel.port1,
		]);
		window.webContents.postMessage(
			TERMINAL_PROJECTION_CONTROL_IPC_CHANNEL,
			TerminalProjectionControlBootstrapSchema.parse({
				type: TERMINAL_PROJECTION_CONTROL_IPC_CHANNEL,
			}),
			[projectionChannel.port1],
		);
		window.webContents.postMessage(
			WORKBENCH_TOPOLOGY_CONTROL_IPC_CHANNEL,
			WorkbenchTopologyControlBootstrapSchema.parse({
				type: WORKBENCH_TOPOLOGY_CONTROL_IPC_CHANNEL,
			}),
			[topologyChannel.port1],
		);
	}

	readonly #handleDocumentResultPosted = (
		command: TerminalDocumentControlCommand,
		result: TerminalDocumentControlOperationResult,
	): void => {
		if (
			command.type !== 'terminal-provision-requested' ||
			result.type !== 'terminal-provision-completed'
		) {
			return;
		}
		const terminalExit = this.#terminalRuntime?.getProjectionTarget(
			command.presentationId,
		)?.terminalExit;
		if (terminalExit !== null && terminalExit !== undefined) {
			this.#documentBroker?.publishTerminalExit(
				toDocumentExit(command.presentationId, terminalExit),
			);
		}
	};

	readonly #handleHostEnvelope = (envelope: DecodedProtocolEnvelope): void => {
		if (envelope.kind !== MessageKind.terminalExited) return;
		const exited = decodeControlJson(envelope.payload, TerminalExitedSchema);
		if (!exited.ok) return;
		void this.#terminalRuntime?.recordExit(exited.value).then((presentationId) => {
			if (presentationId === null) return;
			this.#documentBroker?.publishTerminalExit(toDocumentExit(presentationId, exited.value));
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

function toDocumentExit(
	presentationId: Parameters<DesktopTerminalRuntime['getProjectionTarget']>[0],
	exited: TerminalExited,
): Extract<TerminalDocumentControlEvent, { readonly type: 'terminal-exited' }> {
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
