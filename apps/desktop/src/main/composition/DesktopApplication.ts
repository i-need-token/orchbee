import { randomBytes } from 'node:crypto';

import {
	AttachTerminalSchema,
	AttachmentCapabilitiesSchema,
	AttachmentIdSchema,
	ClientIdSchema,
	CreateTerminalV1_1Schema,
	decodeControlJson,
	MessageKind,
	TerminalExitedSchema,
	TerminalIdentitySchema,
	type DecodedProtocolEnvelope,
	type TerminalIdentity,
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
import type { DesktopRuntimeConfiguration } from './resolveDesktopRuntimeConfiguration';

interface DesktopBrokerHandle {
	readonly metrics: ProjectedTerminalBrokerMetrics;
	dispose(): void;
	start(): void;
}

export interface DesktopApplicationOptions {
	readonly preloadPath: string;
	readonly rendererFilePath: string;
	readonly rendererUrl?: string;
	readonly runtime: DesktopRuntimeConfiguration;
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
	createWindow: (options) => new BrowserWindow(options),
	randomIdentifier: () => randomBytes(16).toString('hex'),
	writeTrace: (trace) => console.info(JSON.stringify(trace)),
};

export class DesktopApplication {
	readonly #dependencies: DesktopApplicationDependencies;
	readonly #options: DesktopApplicationOptions;
	#broker: DesktopBrokerHandle | undefined;
	#connection: HostConnection | undefined;
	#identity: TerminalIdentity | undefined;
	#removeTraceListener: (() => void) | undefined;
	#window: BrowserWindow | undefined;

	constructor(
		options: DesktopApplicationOptions,
		dependencies: DesktopApplicationDependencies = defaultDependencies,
	) {
		this.#options = options;
		this.#dependencies = dependencies;
	}

	get hasWindow(): boolean {
		return this.#window !== undefined && !this.#window.isDestroyed();
	}

	async start(): Promise<void> {
		if (this.#connection !== undefined) throw new Error('Desktop application is already started.');
		const connection = await this.#dependencies.connectHost(this.#options.runtime);
		this.#connection = connection;
		await connection.negotiate(ClientIdSchema.parse(this.#dependencies.randomIdentifier()));
		this.#identity = await connection.requestControl(
			MessageKind.createTerminal,
			CreateTerminalV1_1Schema.parse(this.#options.runtime.createTerminal),
			MessageKind.createTerminal,
			TerminalIdentitySchema,
		);
		if (this.#options.traceCopyPath) {
			this.#removeTraceListener = connection.onEnvelope(this.#traceTerminalExit);
		}
		await this.openWindow();
	}

	async openWindow(): Promise<void> {
		if (this.hasWindow) {
			this.#window?.focus();
			return;
		}
		const connection = this.#connection;
		const identity = this.#identity;
		if (connection === undefined || identity === undefined) {
			throw new Error('Desktop application must start before opening a window.');
		}
		const attachmentId = AttachmentIdSchema.parse(this.#dependencies.randomIdentifier());
		const capabilities = await connection.requestControl(
			MessageKind.attachTerminal,
			AttachTerminalSchema.parse({
				attachmentId,
				capabilities: { input: true, read: true, resize: true },
				requestResizeLease: true,
				terminalId: identity.terminalId,
			}),
			MessageKind.attachmentCapabilities,
			AttachmentCapabilitiesSchema,
		);
		if (
			!capabilities.capabilities.input ||
			!capabilities.capabilities.read ||
			!capabilities.capabilities.resize ||
			!capabilities.resizeLease
		) {
			throw new Error('Terminal Host did not grant the required local attachment capabilities.');
		}
		const bootstrap = ProjectedAttachmentBootstrapSchema.parse({
			attachmentId,
			canResize: capabilities.resizeLease,
			creationId: identity.creationId,
			hostEpoch: identity.hostEpoch,
			terminalId: identity.terminalId,
		});
		const channel = this.#dependencies.createMessageChannel();
		const broker = this.#dependencies.createBroker(connection, channel.port2, bootstrap);
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
		this.#broker = broker;
		broker.start();
		window.webContents.setWindowOpenHandler(() => ({ action: 'deny' }));
		window.webContents.on('will-navigate', (event) => event.preventDefault());
		window.webContents.once('did-finish-load', () => {
			window.webContents.postMessage(PROJECTED_ATTACHMENT_IPC_CHANNEL, bootstrap, [channel.port1]);
		});
		window.webContents.once('render-process-gone', () => broker.dispose());
		window.once('ready-to-show', () => window.show());
		window.once('closed', () => {
			this.#writeCopyPathTrace('window-closed');
			broker.dispose();
			if (this.#broker === broker) this.#broker = undefined;
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
	}

	shutdown(): void {
		this.#removeTraceListener?.();
		this.#removeTraceListener = undefined;
		this.#writeCopyPathTrace('main-shutdown');
		this.#broker?.dispose();
		this.#broker = undefined;
		this.#connection?.close();
		this.#connection = undefined;
		this.#identity = undefined;
	}

	readonly #traceTerminalExit = (envelope: DecodedProtocolEnvelope): void => {
		if (envelope.kind !== MessageKind.terminalExited) return;
		const exited = decodeControlJson(envelope.payload, TerminalExitedSchema);
		if (exited.ok && exited.value.identity.terminalId === this.#identity?.terminalId) {
			this.#writeCopyPathTrace('terminal-exited');
		}
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
