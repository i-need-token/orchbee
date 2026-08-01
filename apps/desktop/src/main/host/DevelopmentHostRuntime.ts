import { spawn } from 'node:child_process';
import { chmod, mkdir } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as wait } from 'node:timers/promises';

import { HostConnection } from './HostConnection';

const DEFAULT_CONNECTION_ATTEMPTS = 250;
const DEFAULT_RETRY_DELAY_MILLISECONDS = 20;
const ENDPOINT_ID_PATTERN = /^[0-9a-f]{32}$/;

export interface DevelopmentHostRuntimeOptions {
	readonly binaryPath: string;
	readonly connectionAttempts?: number;
	readonly endpointId: string;
	readonly platform?: NodeJS.Platform;
	readonly retryDelayMilliseconds?: number;
	readonly runtimeDirectory: string;
}

export interface DevelopmentHostRuntimeDependencies {
	readonly connect: (endpoint: string) => Promise<HostConnection>;
	readonly launch: (binaryPath: string, arguments_: readonly string[]) => Promise<void>;
	readonly prepareDirectory: (directory: string, platform: NodeJS.Platform) => Promise<void>;
	readonly wait: (milliseconds: number) => Promise<void>;
}

const defaultDependencies: DevelopmentHostRuntimeDependencies = {
	connect: (endpoint) => HostConnection.connect(endpoint),
	launch: (binaryPath, arguments_) =>
		new Promise((resolve, reject) => {
			const child = spawn(binaryPath, arguments_, {
				detached: true,
				stdio: 'ignore',
			});
			const handleError = (error: Error): void => reject(error);
			child.once('error', handleError);
			child.once('spawn', () => {
				child.off('error', handleError);
				child.unref();
				resolve();
			});
		}),
	prepareDirectory: async (directory, platform) => {
		await mkdir(directory, { mode: 0o700, recursive: true });
		if (platform !== 'win32') await chmod(directory, 0o700);
	},
	wait: (milliseconds) => wait(milliseconds),
};

export class DevelopmentHostRuntime {
	readonly #attempts: number;
	readonly #binaryPath: string;
	readonly #dependencies: DevelopmentHostRuntimeDependencies;
	readonly #endpointId: string;
	readonly #platform: NodeJS.Platform;
	readonly #retryDelayMilliseconds: number;
	readonly #runtimeDirectory: string;

	constructor(
		options: DevelopmentHostRuntimeOptions,
		dependencies: DevelopmentHostRuntimeDependencies = defaultDependencies,
	) {
		if (!path.isAbsolute(options.binaryPath)) {
			throw new Error('Development Terminal Host binary path must be absolute.');
		}
		if (!path.isAbsolute(options.runtimeDirectory)) {
			throw new Error('Development Terminal Host runtime directory must be absolute.');
		}
		if (!ENDPOINT_ID_PATTERN.test(options.endpointId)) {
			throw new Error('Development Terminal Host endpoint ID must be 32 lowercase hex digits.');
		}
		this.#attempts = options.connectionAttempts ?? DEFAULT_CONNECTION_ATTEMPTS;
		this.#binaryPath = options.binaryPath;
		this.#dependencies = dependencies;
		this.#endpointId = options.endpointId;
		this.#platform = options.platform ?? process.platform;
		this.#retryDelayMilliseconds =
			options.retryDelayMilliseconds ?? DEFAULT_RETRY_DELAY_MILLISECONDS;
		this.#runtimeDirectory = options.runtimeDirectory;
		if (this.#attempts < 1) throw new Error('At least one Host connection attempt is required.');
		if (this.#retryDelayMilliseconds < 0) {
			throw new Error('Host connection retry delay must not be negative.');
		}
	}

	get endpoint(): string {
		return this.#platform === 'win32'
			? `\\\\.\\pipe\\orchbee-terminal-host-${this.#endpointId}`
			: path.join(this.#runtimeDirectory, `terminal-host-${this.#endpointId}.sock`);
	}

	async connect(): Promise<HostConnection> {
		await this.#dependencies.prepareDirectory(this.#runtimeDirectory, this.#platform);
		try {
			return await this.#dependencies.connect(this.endpoint);
		} catch {
			await this.#dependencies.launch(this.#binaryPath, [
				'serve',
				'--runtime-directory',
				this.#runtimeDirectory,
				'--endpoint-id',
				this.#endpointId,
			]);
		}

		let lastError = new Error('Development Terminal Host did not publish its endpoint.');
		for (let attempt = 0; attempt < this.#attempts; attempt += 1) {
			try {
				return await this.#dependencies.connect(this.endpoint);
			} catch (error) {
				lastError =
					error instanceof Error
						? error
						: new Error('Development Terminal Host connection failed.');
				if (attempt + 1 < this.#attempts) {
					await this.#dependencies.wait(this.#retryDelayMilliseconds);
				}
			}
		}
		throw lastError;
	}
}
