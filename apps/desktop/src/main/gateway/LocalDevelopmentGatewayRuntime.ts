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
	readonly openClient: (
		connectionId: GatewayClientConnectionId,
	) => Promise<DesktopGatewayClientHandle>;
}

interface GatewayClientRecord {
	readonly handle: DesktopGatewayClientHandle;
	disposePromise: Promise<void> | undefined;
	retiring: boolean;
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
	readonly #clients = new Map<GatewayClientConnectionId, GatewayClientRecord>();
	readonly #clientOperationTails = new Map<GatewayClientConnectionId, Promise<void>>();
	readonly #clientRetryTimers = new Map<GatewayClientConnectionId, ReturnType<typeof setTimeout>>();
	readonly #dependencies: LocalDevelopmentGatewayRuntimeDependencies;
	#disposePromise: Promise<void> | undefined;
	#disposed = false;
	readonly #options: LocalDevelopmentGatewayRuntimeOptions;
	#process: DesktopGatewayUtilityProcess | undefined;
	#removeProcessListeners: readonly (() => void)[] = [];
	#resolveStart: ((result: LocalDevelopmentGatewayStartResult) => void) | undefined;
	#rejectStart: ((error: Error) => void) | undefined;
	#startPromise: Promise<LocalDevelopmentGatewayStartResult> | undefined;
	#backgroundFailure: Error | undefined;

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

	get backgroundFailure(): Error | undefined {
		return this.#backgroundFailure;
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

	readonly #handleSpawn = (): void => {
		const process = this.#process;
		if (this.#disposed || process === undefined) return;
		try {
			process.postMessage(
				GatewayDesktopCommandSchema.parse({
					allowedOrigin: this.#browserBaseUrl.origin,
					capability: this.#capability,
					type: 'gateway-start',
				}),
			);
		} catch (error: unknown) {
			this.#fail(toError(error, 'Local Browser Gateway start transfer failed.'));
		}
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
				this.#runBackground(
					this.#enqueueClientOperation(event.connectionId, () =>
						this.#openClient(event.connectionId),
					),
				);
				return;
			case 'gateway-client-close-requested':
				this.#runBackground(
					this.#enqueueClientOperation(event.connectionId, () =>
						this.#closeClient(event.connectionId),
					),
				);
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

	async #openClient(connectionId: GatewayClientConnectionId): Promise<void> {
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
			client = await this.#dependencies.openClient(connectionId);
		} catch (error: unknown) {
			this.#postClientOpenFailure(process, connectionId, 'client-session-unavailable');
			this.#fail(toError(error, 'Local Browser Gateway client opening failed.'));
			return;
		}
		const record: GatewayClientRecord = {
			disposePromise: undefined,
			handle: client,
			retiring: false,
		};
		this.#clients.set(connectionId, record);
		if (this.#disposed || this.#process !== process) {
			record.retiring = true;
			await this.#disposeClient(connectionId, record);
			return;
		}
		try {
			process.postMessage(
				GatewayDesktopCommandSchema.parse({
					bootstrap: client.bootstrap,
					connectionId,
					type: 'gateway-client-opened',
				}),
				[client.clientSessionPort, client.projectionPort],
			);
		} catch (error: unknown) {
			record.retiring = true;
			this.#postClientOpenFailure(process, connectionId, 'client-session-unavailable');
			let disposeError: unknown;
			try {
				await this.#disposeClient(connectionId, record);
			} catch (cleanupError: unknown) {
				disposeError = cleanupError;
				this.#scheduleClientRetry(connectionId, record);
			}
			const transferError = toError(error, 'Local Browser Gateway client transfer failed.');
			this.#fail(transferError);
			if (disposeError !== undefined) {
				throw new AggregateError(
					[transferError, disposeError],
					'Local Browser Gateway client transfer cleanup failed.',
				);
			}
			throw transferError;
		}
	}

	async #closeClient(connectionId: GatewayClientConnectionId): Promise<void> {
		const record = this.#clients.get(connectionId);
		if (record === undefined) return;
		record.retiring = true;
		try {
			await this.#disposeClient(connectionId, record);
		} catch (error: unknown) {
			this.#scheduleClientRetry(connectionId, record);
			throw error;
		}
	}

	#fail(error: Error): void {
		this.#backgroundFailure = error;
		this.#rejectStart?.(error);
		this.#resolveStart = undefined;
		this.#rejectStart = undefined;
		this.#runBackground(this.dispose());
	}

	dispose(): Promise<void> {
		if (this.#disposePromise !== undefined) return this.#disposePromise;
		const operation = this.#dispose();
		this.#disposePromise = operation.catch((error: unknown) => {
			this.#disposePromise = undefined;
			throw error;
		});
		return this.#disposePromise;
	}

	async #dispose(): Promise<void> {
		this.#disposed = true;
		this.#rejectStart?.(new Error('Local Browser Gateway stopped before listening.'));
		this.#resolveStart = undefined;
		this.#rejectStart = undefined;
		for (const removeListener of this.#removeProcessListeners) removeListener();
		this.#removeProcessListeners = [];
		await Promise.allSettled(this.#clientOperationTails.values());
		const cleanupResults = await Promise.allSettled(
			[...this.#clients.entries()].map(async ([connectionId, record]) => {
				record.retiring = true;
				await this.#disposeClient(connectionId, record);
			}),
		);
		const failures = cleanupResults.flatMap((result) =>
			result.status === 'rejected' ? [result.reason] : [],
		);
		const process = this.#process;
		if (process !== undefined) {
			try {
				process.postMessage(GatewayDesktopCommandSchema.parse({ type: 'gateway-stop' }));
			} catch (error: unknown) {
				// The process may already be gone; killing remains the final capability revocation.
				failures.push(error);
			}
			try {
				process.kill();
				this.#process = undefined;
			} catch (error: unknown) {
				failures.push(error);
			}
		}
		if (failures.length > 0) {
			throw new AggregateError(failures, 'Local Browser Gateway disposal did not converge.');
		}
	}

	#enqueueClientOperation(
		connectionId: GatewayClientConnectionId,
		operation: () => Promise<void>,
	): Promise<void> {
		const previous = this.#clientOperationTails.get(connectionId) ?? Promise.resolve();
		const result = previous.then(operation);
		const tail = result.then(
			() => undefined,
			() => undefined,
		);
		this.#clientOperationTails.set(connectionId, tail);
		void tail.then(() => {
			if (this.#clientOperationTails.get(connectionId) === tail) {
				this.#clientOperationTails.delete(connectionId);
			}
		});
		return result;
	}

	async #disposeClient(
		connectionId: GatewayClientConnectionId,
		record: GatewayClientRecord,
	): Promise<void> {
		if (record.disposePromise !== undefined) return record.disposePromise;
		const operation = Promise.resolve()
			.then(() => record.handle.dispose())
			.then(() => {
				if (this.#clients.get(connectionId) === record) this.#clients.delete(connectionId);
				const timer = this.#clientRetryTimers.get(connectionId);
				if (timer !== undefined) clearTimeout(timer);
				this.#clientRetryTimers.delete(connectionId);
			});
		record.disposePromise = operation.catch((error: unknown) => {
			record.disposePromise = undefined;
			throw error;
		});
		return record.disposePromise;
	}

	#scheduleClientRetry(connectionId: GatewayClientConnectionId, record: GatewayClientRecord): void {
		if (this.#disposed || this.#clientRetryTimers.has(connectionId)) return;
		const timer = setTimeout(() => {
			this.#clientRetryTimers.delete(connectionId);
			this.#runBackground(
				this.#disposeClient(connectionId, record).catch((error: unknown) => {
					this.#scheduleClientRetry(connectionId, record);
					throw error;
				}),
			);
		}, 25);
		timer.unref?.();
		this.#clientRetryTimers.set(connectionId, timer);
	}

	#runBackground(operation: Promise<unknown>): void {
		void operation.catch((error: unknown) => {
			this.#backgroundFailure = toError(
				error,
				'Local Browser Gateway background operation failed.',
			);
		});
	}

	#postClientOpenFailure(
		process: DesktopGatewayUtilityProcess,
		connectionId: GatewayClientConnectionId,
		reason: 'client-session-unavailable',
	): void {
		try {
			process.postMessage(
				GatewayDesktopCommandSchema.parse({
					connectionId,
					reason,
					type: 'gateway-client-open-failed',
				}),
			);
		} catch (error: unknown) {
			this.#backgroundFailure = toError(
				error,
				'Local Browser Gateway failed to report client opening failure.',
			);
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

function toError(error: unknown, fallback: string): Error {
	return error instanceof Error ? error : new Error(fallback, { cause: error });
}
