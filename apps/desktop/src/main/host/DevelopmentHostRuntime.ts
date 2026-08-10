import { spawn } from 'node:child_process';
import { mkdir } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as wait } from 'node:timers/promises';

import {
	TerminalConfigurationSchema,
	type TerminalConfiguration,
} from '@orchbee/terminal-protocol';

import { HostConnection } from './HostConnection';
import {
	connectAuthenticatedPosixHost,
	preparePosixHostRuntimeDirectory,
} from './posix-host-endpoint';

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
	readonly terminalDefaults: TerminalConfiguration;
}

export interface DevelopmentHostRuntimeDependencies {
	readonly connect: (
		endpoint: string,
		binaryPath: string,
		runtimeDirectory: string,
		endpointId: string,
		platform: NodeJS.Platform,
	) => Promise<HostConnection>;
	readonly launch: (binaryPath: string, arguments_: readonly string[]) => Promise<void>;
	readonly prepareDirectory: (
		directory: string,
		platform: NodeJS.Platform,
		binaryPath: string,
	) => Promise<void>;
	readonly wait: (milliseconds: number) => Promise<void>;
}

const defaultDependencies: DevelopmentHostRuntimeDependencies = {
	connect: (endpoint, binaryPath, runtimeDirectory, endpointId, platform) =>
		platform === 'win32'
			? HostConnection.connectNamedPipe(endpoint)
			: connectAuthenticatedPosixHost(binaryPath, runtimeDirectory, endpointId),
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
	prepareDirectory: (directory, platform, binaryPath) =>
		platform === 'win32'
			? mkdir(directory, { mode: 0o700, recursive: true }).then(() => undefined)
			: preparePosixHostRuntimeDirectory(binaryPath, directory),
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
	readonly #terminalDefaults: TerminalConfiguration;

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
		this.#terminalDefaults = TerminalConfigurationSchema.parse(options.terminalDefaults);
		if (this.#attempts < 1) throw new Error('At least one Host connection attempt is required.');
		if (this.#retryDelayMilliseconds < 0) {
			throw new Error('Host connection retry delay must not be negative.');
		}
	}

	get endpoint(): string {
		return this.#platform === 'win32'
			? `\\\\.\\pipe\\orchbee-terminal-host-${this.#endpointId}`
			: path.join(this.#runtimeDirectory, `${this.#endpointId}.s`);
	}

	async connect(): Promise<HostConnection> {
		await this.#dependencies.prepareDirectory(
			this.#runtimeDirectory,
			this.#platform,
			this.#binaryPath,
		);
		try {
			return await this.#connectOnce();
		} catch {
			await this.#dependencies.launch(this.#binaryPath, [
				'serve',
				'--runtime-directory',
				this.#runtimeDirectory,
				'--endpoint-id',
				this.#endpointId,
				'--scrollback-bytes',
				String(this.#terminalDefaults.scrollbackBytes),
			]);
		}

		let lastError = new Error('Development Terminal Host did not publish its endpoint.');
		for (let attempt = 0; attempt < this.#attempts; attempt += 1) {
			try {
				return await this.#connectOnce();
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

	#connectOnce(): Promise<HostConnection> {
		return this.#dependencies.connect(
			this.endpoint,
			this.#binaryPath,
			this.#runtimeDirectory,
			this.#endpointId,
			this.#platform,
		);
	}
}
