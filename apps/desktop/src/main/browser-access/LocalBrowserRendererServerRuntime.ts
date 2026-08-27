import { createInterface } from 'node:readline';

import type { GatewayBindHost } from '@orchbee/gateway-protocol';
import type {
	BrowserSessionSigningKey,
	PersonalAccessTokenVerifier,
} from '@orchbee/remote-client-protocol';
import { utilityProcess } from 'electron';
import { z } from 'zod';

const SERVER_LISTENING_PREFIX = 'ORCHBEE_SERVER_LISTENING=';
const ServerListeningSchema = z.strictObject({
	host: z.enum(['127.0.0.1', '0.0.0.0']),
	port: z.number().int().min(1).max(65_535),
});

interface DesktopServerUtilityProcess {
	kill(): void;
	onExit(listener: (code: number) => void): () => void;
	onStdoutLine(listener: (line: string) => void): () => void;
}

interface LocalBrowserRendererServerRuntimeOptions {
	readonly authentication: {
		readonly personalAccessTokenVerifier: PersonalAccessTokenVerifier;
		readonly sessionSigningKey: BrowserSessionSigningKey;
	};
	readonly bindHost: GatewayBindHost;
	readonly entryPath: string;
	readonly rendererRoot: string;
}

interface LocalBrowserRendererServerRuntimeDependencies {
	readonly clearStartTimeout: (timeout: ReturnType<typeof setTimeout>) => void;
	readonly createProcess: (
		options: LocalBrowserRendererServerRuntimeOptions,
	) => DesktopServerUtilityProcess;
	readonly scheduleStartTimeout: (listener: () => void) => ReturnType<typeof setTimeout>;
}

interface LocalBrowserRendererServerStartResult {
	readonly port: number;
}

const defaultDependencies: LocalBrowserRendererServerRuntimeDependencies = {
	clearStartTimeout: (timeout) => clearTimeout(timeout),
	createProcess: (options) => {
		const child = utilityProcess.fork(options.entryPath, [], {
			env: {
				ORCHBEE_SERVER_HOST: options.bindHost,
				ORCHBEE_SERVER_PERSONAL_ACCESS_TOKEN_VERIFIER:
					options.authentication.personalAccessTokenVerifier,
				ORCHBEE_SERVER_PORT: '0',
				ORCHBEE_SERVER_RENDERER_ROOT: options.rendererRoot,
				ORCHBEE_SERVER_SESSION_SIGNING_KEY: options.authentication.sessionSigningKey,
			},
			serviceName: 'Orchbee Browser Renderer Server',
			stdio: 'pipe',
		});
		const stdout = child.stdout;
		const stderr = child.stderr;
		if (stdout === null) {
			child.kill();
			throw new Error('Orchbee Browser Renderer Server stdout is unavailable.');
		}
		stderr?.resume();
		const lines = createInterface({ input: stdout });
		return {
			kill: () => {
				lines.close();
				child.kill();
			},
			onExit: (listener) => {
				child.on('exit', listener);
				return () => child.off('exit', listener);
			},
			onStdoutLine: (listener) => {
				lines.on('line', listener);
				return () => lines.off('line', listener);
			},
		};
	},
	scheduleStartTimeout: (listener) => {
		const timeout = setTimeout(listener, 10_000);
		timeout.unref?.();
		return timeout;
	},
};

export class LocalBrowserRendererServerRuntime {
	readonly #dependencies: LocalBrowserRendererServerRuntimeDependencies;
	#disposed = false;
	#disposePromise: Promise<void> | undefined;
	readonly #options: LocalBrowserRendererServerRuntimeOptions;
	#process: DesktopServerUtilityProcess | undefined;
	#rejectStart: ((error: Error) => void) | undefined;
	readonly #removeListeners: (() => void)[] = [];
	#resolveStart: ((result: LocalBrowserRendererServerStartResult) => void) | undefined;
	#startPromise: Promise<LocalBrowserRendererServerStartResult> | undefined;
	#startTimeout: ReturnType<typeof setTimeout> | undefined;
	#backgroundFailure: Error | undefined;

	constructor(
		options: LocalBrowserRendererServerRuntimeOptions,
		dependencies: Partial<LocalBrowserRendererServerRuntimeDependencies> = {},
	) {
		this.#options = options;
		this.#dependencies = { ...defaultDependencies, ...dependencies };
	}

	get backgroundFailure(): Error | undefined {
		return this.#backgroundFailure;
	}

	start(): Promise<LocalBrowserRendererServerStartResult> {
		if (this.#startPromise !== undefined) return this.#startPromise;
		if (this.#disposed) {
			return Promise.reject(new Error('Orchbee Browser Renderer Server is disposed.'));
		}
		this.#startPromise = new Promise((resolve, reject) => {
			this.#resolveStart = resolve;
			this.#rejectStart = reject;
		});
		try {
			const process = this.#dependencies.createProcess(this.#options);
			this.#process = process;
			this.#removeListeners.push(
				process.onStdoutLine(this.#handleStdoutLine),
				process.onExit(this.#handleExit),
			);
			this.#startTimeout = this.#dependencies.scheduleStartTimeout(() => {
				this.#fail(new Error('Orchbee Browser Renderer Server start timed out.'));
			});
		} catch (error: unknown) {
			this.#fail(toError(error, 'Orchbee Browser Renderer Server process creation failed.'));
		}
		return this.#startPromise;
	}

	readonly #handleStdoutLine = (line: string): void => {
		if (this.#disposed || this.#resolveStart === undefined) return;
		if (!line.startsWith(SERVER_LISTENING_PREFIX)) return;
		let value: unknown;
		try {
			value = JSON.parse(line.slice(SERVER_LISTENING_PREFIX.length));
		} catch {
			this.#fail(new Error('Orchbee Browser Renderer Server emitted invalid readiness JSON.'));
			return;
		}
		const parsed = ServerListeningSchema.safeParse(value);
		if (!parsed.success || parsed.data.host !== this.#options.bindHost) {
			this.#fail(new Error('Orchbee Browser Renderer Server emitted invalid readiness data.'));
			return;
		}
		this.#clearTimeout();
		this.#resolveStart({ port: parsed.data.port });
		this.#resolveStart = undefined;
		this.#rejectStart = undefined;
	};

	readonly #handleExit = (code: number): void => {
		if (this.#disposed) return;
		this.#fail(new Error(`Orchbee Browser Renderer Server exited unexpectedly with code ${code}.`));
	};

	#fail(error: Error): void {
		this.#backgroundFailure = error;
		this.#clearTimeout();
		this.#rejectStart?.(error);
		this.#resolveStart = undefined;
		this.#rejectStart = undefined;
		void this.dispose().catch(() => undefined);
	}

	dispose(): Promise<void> {
		if (this.#disposePromise !== undefined) return this.#disposePromise;
		this.#disposePromise = Promise.resolve().then(() => {
			this.#disposed = true;
			this.#clearTimeout();
			this.#rejectStart?.(new Error('Orchbee Browser Renderer Server stopped before listening.'));
			this.#resolveStart = undefined;
			this.#rejectStart = undefined;
			for (const removeListener of this.#removeListeners.splice(0)) removeListener();
			const process = this.#process;
			this.#process = undefined;
			process?.kill();
		});
		return this.#disposePromise;
	}

	#clearTimeout(): void {
		const timeout = this.#startTimeout;
		if (timeout === undefined) return;
		this.#startTimeout = undefined;
		this.#dependencies.clearStartTimeout(timeout);
	}
}

function toError(error: unknown, fallback: string): Error {
	return error instanceof Error ? error : new Error(fallback, { cause: error });
}
