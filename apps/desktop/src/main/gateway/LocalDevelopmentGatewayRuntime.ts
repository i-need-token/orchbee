import { randomBytes } from 'node:crypto';

import {
	encodeGatewayBrowserFragment,
	GatewayBrowserConnectionSchema,
	GatewayCapabilitySchema,
	GatewayDesktopCommandSchema,
	GatewayUtilityEventSchema,
	type GatewayClientConnectionId,
} from '@orchbee/gateway-protocol';
import type { WorkbenchClientSessionBootstrap } from '@orchbee/workbench-client-protocol';
import { utilityProcess, type MessagePortMain } from 'electron';

export interface DesktopGatewayClientHandle {
	readonly bootstrap: WorkbenchClientSessionBootstrap;
	readonly clientSessionPort: MessagePortMain;
	readonly projectionPort: MessagePortMain;
	dispose(): Promise<void>;
}

export interface DesktopGatewayUtilityProcess {
	kill(): void;
	onExit(listener: (code: number) => void): () => void;
	onMessage(listener: (message: unknown) => void): () => void;
	onSpawn(listener: () => void): () => void;
	postMessage(message: unknown, ports?: readonly MessagePortMain[]): void;
}

export interface LocalDevelopmentGatewayRuntimeOptions {
	readonly entryPath: string;
	readonly rendererUrl: string;
}

export interface LocalDevelopmentGatewayRuntimeDependencies {
	readonly createCapability: () => ReturnType<typeof GatewayCapabilitySchema.parse>;
	readonly createProcess: (entryPath: string) => DesktopGatewayUtilityProcess;
	readonly openClient: (connectionId: GatewayClientConnectionId) => DesktopGatewayClientHandle;
}

export interface LocalDevelopmentGatewayStartResult {
	readonly browserUrl: string;
}

const defaultDependencies: Omit<LocalDevelopmentGatewayRuntimeDependencies, 'openClient'> = {
	createCapability: () => GatewayCapabilitySchema.parse(randomBytes(32).toString('base64url')),
	createProcess: (entryPath) => {
		const child = utilityProcess.fork(entryPath, [], {
			serviceName: 'Orchbee Local Browser Gateway',
			stdio: 'inherit',
		});
		return {
			kill: () => {
				child.kill();
			},
			onExit: (listener) => {
				child.on('exit', listener);
				return () => child.off('exit', listener);
			},
			onMessage: (listener) => {
				child.on('message', listener);
				return () => child.off('message', listener);
			},
			onSpawn: (listener) => {
				child.on('spawn', listener);
				return () => child.off('spawn', listener);
			},
			postMessage: (message, ports) =>
				child.postMessage(message, ports === undefined ? [] : [...ports]),
		};
	},
};

export class LocalDevelopmentGatewayRuntime {
	readonly #browserBaseUrl: URL;
	readonly #capability: ReturnType<typeof GatewayCapabilitySchema.parse>;
	readonly #clients = new Map<GatewayClientConnectionId, DesktopGatewayClientHandle>();
	readonly #dependencies: LocalDevelopmentGatewayRuntimeDependencies;
	#disposePromise: Promise<void> | undefined;
	#disposed = false;
	readonly #options: LocalDevelopmentGatewayRuntimeOptions;
	#process: DesktopGatewayUtilityProcess | undefined;
	#removeProcessListeners: readonly (() => void)[] = [];
	#resolveStart: ((result: LocalDevelopmentGatewayStartResult) => void) | undefined;
	#rejectStart: ((error: Error) => void) | undefined;
	#startPromise: Promise<LocalDevelopmentGatewayStartResult> | undefined;

	constructor(
		options: LocalDevelopmentGatewayRuntimeOptions,
		dependencies: Pick<LocalDevelopmentGatewayRuntimeDependencies, 'openClient'> &
			Partial<Omit<LocalDevelopmentGatewayRuntimeDependencies, 'openClient'>>,
	) {
		this.#options = options;
		this.#dependencies = { ...defaultDependencies, ...dependencies };
		this.#capability = this.#dependencies.createCapability();
		this.#browserBaseUrl = validateRendererUrl(options.rendererUrl);
	}

	start(): Promise<LocalDevelopmentGatewayStartResult> {
		if (this.#startPromise !== undefined) return this.#startPromise;
		if (this.#disposed) return Promise.reject(new Error('Local Browser Gateway is disposed.'));
		this.#startPromise = new Promise((resolve, reject) => {
			this.#resolveStart = resolve;
			this.#rejectStart = reject;
		});
		const process = this.#dependencies.createProcess(this.#options.entryPath);
		this.#process = process;
		this.#removeProcessListeners = [
			process.onSpawn(this.#handleSpawn),
			process.onMessage(this.#handleMessage),
			process.onExit(this.#handleExit),
		];
		return this.#startPromise;
	}

	dispose(): Promise<void> {
		return (this.#disposePromise ??= this.#dispose());
	}

	readonly #handleSpawn = (): void => {
		const process = this.#process;
		if (this.#disposed || process === undefined) return;
		process.postMessage(
			GatewayDesktopCommandSchema.parse({
				allowedOrigin: this.#browserBaseUrl.origin,
				capability: this.#capability,
				type: 'gateway-start',
			}),
		);
	};

	readonly #handleMessage = (message: unknown): void => {
		if (this.#disposed) return;
		const parsed = GatewayUtilityEventSchema.safeParse(message);
		if (!parsed.success) {
			this.#fail(new Error('Local Browser Gateway sent an invalid parent message.'));
			return;
		}
		const event = parsed.data;
		switch (event.type) {
			case 'gateway-listening': {
				const connection = GatewayBrowserConnectionSchema.parse({
					capability: this.#capability,
					port: event.port,
					protocolVersion: '1.0',
				});
				const browserUrl = new URL(this.#browserBaseUrl.href);
				browserUrl.searchParams.set('orchbee-runtime', 'gateway');
				browserUrl.hash = encodeGatewayBrowserFragment(connection);
				this.#resolveStart?.({ browserUrl: browserUrl.href });
				this.#resolveStart = undefined;
				this.#rejectStart = undefined;
				return;
			}
			case 'gateway-client-open-requested':
				setImmediate(() => {
					try {
						this.#openClient(event.connectionId);
					} catch {
						this.#fail(new Error('Local Browser Gateway client opening failed.'));
					}
				});
				return;
			case 'gateway-client-close-requested':
				void this.#closeClient(event.connectionId);
				return;
			case 'gateway-failed':
				this.#fail(new Error(`Local Browser Gateway failed with ${event.code}.`));
				return;
			case 'gateway-stopped':
				this.#fail(new Error('Local Browser Gateway stopped before Desktop disposal.'));
				return;
		}
	};

	readonly #handleExit = (code: number): void => {
		if (!this.#disposed) {
			this.#fail(new Error(`Local Browser Gateway exited unexpectedly with code ${code}.`));
		}
	};

	#openClient(connectionId: GatewayClientConnectionId): void {
		if (this.#disposed) return;
		const process = this.#process;
		if (process === undefined) {
			this.#fail(new Error('Local Browser Gateway process is unavailable.'));
			return;
		}
		if (this.#clients.has(connectionId)) {
			this.#fail(new Error('Local Browser Gateway repeated a client identity.'));
			return;
		}
		let client: DesktopGatewayClientHandle;
		try {
			client = this.#dependencies.openClient(connectionId);
		} catch {
			process.postMessage(
				GatewayDesktopCommandSchema.parse({
					connectionId,
					reason: 'client-session-unavailable',
					type: 'gateway-client-open-failed',
				}),
			);
			return;
		}
		this.#clients.set(connectionId, client);
		try {
			process.postMessage(
				GatewayDesktopCommandSchema.parse({
					bootstrap: client.bootstrap,
					connectionId,
					type: 'gateway-client-opened',
				}),
				[client.clientSessionPort, client.projectionPort],
			);
		} catch {
			this.#clients.delete(connectionId);
			void client.dispose();
			this.#fail(new Error('Local Browser Gateway client transfer failed.'));
		}
	}

	async #closeClient(connectionId: GatewayClientConnectionId): Promise<void> {
		const client = this.#clients.get(connectionId);
		if (client === undefined) return;
		this.#clients.delete(connectionId);
		await client.dispose();
	}

	#fail(error: Error): void {
		this.#rejectStart?.(error);
		this.#resolveStart = undefined;
		this.#rejectStart = undefined;
		void this.dispose();
	}

	async #dispose(): Promise<void> {
		if (this.#disposed) return;
		this.#disposed = true;
		this.#rejectStart?.(new Error('Local Browser Gateway stopped before listening.'));
		this.#resolveStart = undefined;
		this.#rejectStart = undefined;
		for (const removeListener of this.#removeProcessListeners) removeListener();
		this.#removeProcessListeners = [];
		const clients = [...this.#clients.values()];
		this.#clients.clear();
		await Promise.all(clients.map(async (client) => client.dispose()));
		const process = this.#process;
		this.#process = undefined;
		if (process !== undefined) {
			try {
				process.postMessage(GatewayDesktopCommandSchema.parse({ type: 'gateway-stop' }));
			} catch {
				// The process may already be gone; killing remains the final capability revocation.
			}
			process.kill();
		}
	}
}

function validateRendererUrl(value: string): URL {
	const url = new URL(value);
	if (
		url.protocol !== 'http:' ||
		url.hostname !== '127.0.0.1' ||
		url.username !== '' ||
		url.password !== ''
	) {
		throw new Error('Local Browser Gateway renderer URL must use loopback HTTP.');
	}
	return url;
}
