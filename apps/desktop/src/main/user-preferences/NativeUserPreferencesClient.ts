import { spawn, type ChildProcessWithoutNullStreams } from 'node:child_process';
import { createHash } from 'node:crypto';
import { lstat, open } from 'node:fs/promises';
import path from 'node:path';

import {
	MAXIMUM_USER_PREFERENCES_BYTES,
	UserPreferencesRequestSchema,
	UserPreferencesResponseSchema,
	type UserPreferencesRequest,
	type UserPreferencesResponse,
} from '@orchbee/workbench-client-protocol';
import { z } from 'zod';

const manifestSchema = z.strictObject({
	version: z.literal(1),
	executable: z.enum(['orchbee-settings', 'orchbee-settings.exe']),
	sha256: z.string().regex(/^[\da-f]{64}$/u),
});
const MAXIMUM_EXECUTABLE_BYTES = 256 * 1024 * 1024;
const EXCHANGE_DEADLINE_MS = 2000;

type HelperExchange<T> =
	| { readonly kind: 'response'; readonly response: T }
	| { readonly kind: 'unavailable' }
	| { readonly kind: 'outcome-unknown' };
export type UserPreferencesExchange = HelperExchange<UserPreferencesResponse>;

export interface NativeUserPreferencesClientOptions {
	readonly artifactDirectory: string;
	readonly environment: NodeJS.ProcessEnv;
}

/** One exact helper per exchange; retirement always terminates and joins the owned process. */
export class NativeUserPreferencesClient {
	readonly #artifactDirectory: string;
	readonly #environment: NodeJS.ProcessEnv;
	#active: Promise<void> | undefined;
	#discoveringShell = false;
	#child: ChildProcessWithoutNullStreams | undefined;
	#disposed = false;
	#cancel: (() => void) | undefined;

	constructor(options: NativeUserPreferencesClientOptions) {
		if (!path.isAbsolute(options.artifactDirectory))
			throw new Error('Settings artifact directory must be absolute.');
		this.#artifactDirectory = options.artifactDirectory;
		this.#environment = { ...options.environment };
	}

	execute(request: UserPreferencesRequest): Promise<UserPreferencesExchange> {
		if (this.#disposed) return Promise.resolve({ kind: 'unavailable' });
		if (this.#active !== undefined) {
			// Shell discovery is read-only. Wait before the first request handoff; never replay it.
			if (this.#discoveringShell) return this.#active.then(() => this.execute(request));
			return Promise.resolve({ kind: 'response', response: { kind: 'refused', reason: 'busy' } });
		}
		const parsed = UserPreferencesRequestSchema.safeParse(request);
		if (!parsed.success) {
			return Promise.resolve({
				kind: 'response',
				response: { kind: 'refused', reason: 'invalid-request' },
			});
		}
		return this.#run(parsed.data, '--stdio-v1', parsed.data.operation !== 'read', (value, code) => {
			const response = UserPreferencesResponseSchema.parse(value);
			if (code !== (response.kind === 'snapshot' ? 0 : 1))
				throw new Error('Invalid helper exit status.');
			return response;
		});
	}

	/** The same native shell selection policy used by Worker, without launching a shell. */
	async resolveSystemShell(): Promise<string | null> {
		// Read-only discovery waits for the active exchange; writes are never replayed.
		while (this.#active !== undefined && !this.#disposed) await this.#active;
		if (this.#disposed) return null;
		this.#discoveringShell = true;
		const result = await this.#run({}, '--system-shell-v1', false, (value, code) => {
			if (code !== 0) throw new Error('System shell is unavailable.');
			const executable = z
				.string()
				.min(1)
				.max(4096)
				.refine((text) => !text.includes('\0') && path.isAbsolute(text))
				.parse(value);
			return executable;
		});
		this.#discoveringShell = false;
		return result.kind === 'response' ? result.response : null;
	}

	#run<T>(
		request: object,
		argument: string,
		mutating: boolean,
		decode: (value: unknown, code: number | null) => T,
	): Promise<HelperExchange<T>> {
		const result = this.#execute(request, argument, mutating, decode).finally(() => {
			this.#active = undefined;
		});
		this.#active = result.then(() => undefined);
		return result;
	}

	async dispose(): Promise<void> {
		this.#disposed = true;
		this.#cancel?.();
		await this.#active;
	}

	async #execute<T>(
		request: object,
		argument: string,
		mutating: boolean,
		decode: (value: unknown, code: number | null) => T,
	): Promise<HelperExchange<T>> {
		let executable: string;
		try {
			const manifestPath = path.join(this.#artifactDirectory, 'manifest.json');
			const manifestStat = await lstat(manifestPath);
			if (!manifestStat.isFile() || manifestStat.size > 4096) return { kind: 'unavailable' };
			const manifest = manifestSchema.parse(JSON.parse(await readManifest(manifestPath)));
			const expectedName =
				process.platform === 'win32' ? 'orchbee-settings.exe' : 'orchbee-settings';
			if (manifest.executable !== expectedName) return { kind: 'unavailable' };
			executable = path.join(this.#artifactDirectory, manifest.executable);
			const stat = await lstat(executable);
			if (!stat.isFile() || stat.size > MAXIMUM_EXECUTABLE_BYTES) return { kind: 'unavailable' };
			if (!(await matchesArtifactDigest(executable, manifest.sha256))) {
				return { kind: 'unavailable' };
			}
		} catch {
			return { kind: 'unavailable' };
		}
		if (this.#disposed) return { kind: 'unavailable' };
		const payload = Buffer.from(JSON.stringify(request), 'utf8');
		const uncertain = (): HelperExchange<T> => ({
			kind: mutating ? 'outcome-unknown' : 'unavailable',
		});
		return new Promise<HelperExchange<T>>((resolve) => {
			let child: ChildProcessWithoutNullStreams;
			try {
				child = spawn(executable, [argument], {
					cwd: this.#artifactDirectory,
					env: this.#environment,
					shell: false,
					stdio: ['pipe', 'pipe', 'pipe'],
					windowsHide: true,
				});
			} catch {
				resolve({ kind: 'unavailable' });
				return;
			}
			this.#child = child;
			let aborted = false;
			let spawnFailed = false;
			let outputBytes = 0;
			let diagnosticBytes = 0;
			const output: Buffer[] = [];
			const cancel = (): void => {
				aborted = true;
				// The helper never spawns descendants. Close, rather than kill(), is the join boundary.
				if (child.exitCode === null && child.signalCode === null) child.kill();
			};
			this.#cancel = cancel;
			const timer = setTimeout(cancel, EXCHANGE_DEADLINE_MS);
			child.stdout.on('data', (chunk: Buffer) => {
				outputBytes += chunk.byteLength;
				if (outputBytes > MAXIMUM_USER_PREFERENCES_BYTES + 1) cancel();
				else output.push(chunk);
			});
			child.stderr.on('data', (chunk: Buffer) => {
				diagnosticBytes += chunk.byteLength;
				if (diagnosticBytes > 4096) cancel();
			});
			child.on('error', () => {
				spawnFailed = child.pid === undefined;
				aborted = true;
			});
			child.stdin.on('error', cancel);
			child.on('close', (code) => {
				clearTimeout(timer);
				if (this.#child === child) this.#child = undefined;
				if (this.#cancel === cancel) this.#cancel = undefined;
				if (spawnFailed) {
					resolve({ kind: 'unavailable' });
					return;
				}
				if (aborted || diagnosticBytes !== 0) {
					resolve(uncertain());
					return;
				}
				try {
					const response = decode(
						JSON.parse(new TextDecoder('utf-8', { fatal: true }).decode(Buffer.concat(output))),
						code,
					);
					resolve({ kind: 'response', response });
				} catch {
					resolve(uncertain());
				}
			});
			child.stdin.end(payload);
		});
	}
}

async function readManifest(filePath: string): Promise<string> {
	const file = await open(filePath, 'r');
	try {
		const buffer = Buffer.alloc(4097);
		let total = 0;
		while (total < buffer.length) {
			const { bytesRead } = await file.read(buffer, total, buffer.length - total, null);
			if (bytesRead === 0) break;
			total += bytesRead;
		}
		if (total > 4096) throw new Error('Settings manifest exceeds its byte boundary.');
		return new TextDecoder('utf-8', { fatal: true }).decode(buffer.subarray(0, total));
	} finally {
		await file.close();
	}
}

async function matchesArtifactDigest(filePath: string, expected: string): Promise<boolean> {
	const file = await open(filePath, 'r');
	try {
		const digest = createHash('sha256');
		const buffer = Buffer.alloc(64 * 1024);
		let total = 0;
		while (total <= MAXIMUM_EXECUTABLE_BYTES) {
			const { bytesRead } = await file.read(
				buffer,
				0,
				Math.min(buffer.length, MAXIMUM_EXECUTABLE_BYTES + 1 - total),
				null,
			);
			if (bytesRead === 0) return digest.digest('hex') === expected;
			total += bytesRead;
			digest.update(buffer.subarray(0, bytesRead));
		}
		return false;
	} finally {
		await file.close();
	}
}
