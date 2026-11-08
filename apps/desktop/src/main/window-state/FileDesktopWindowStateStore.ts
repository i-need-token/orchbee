import { randomBytes } from 'node:crypto';
import { chmod, mkdir, open, readFile, rename, stat, unlink } from 'node:fs/promises';
import path from 'node:path';

import {
	DESKTOP_WINDOW_STATE_VERSION,
	parseDesktopWindowState,
	type DesktopWindowState,
	type DesktopWindowStateLoadResult,
	type DesktopWindowStateStore,
} from './desktop-window-state';

const utf8Decoder = new TextDecoder('utf-8', { fatal: true });

export interface FileDesktopWindowStateStoreOptions {
	readonly randomToken?: () => string;
}

type JsonObject = Readonly<Record<string, unknown>>;

/**
 * The private window state file. Its `extensions` object holds fields a later release added; it is
 * kept unread and written back unchanged. While the file's version is one this build cannot use,
 * the store never rewrites it and keeps saved state in memory until the process ends.
 */
export class FileDesktopWindowStateStore implements DesktopWindowStateStore {
	#extensions: JsonObject | undefined;
	readonly #filePath: string;
	#memory: { state: DesktopWindowState | undefined } | undefined;
	#operationTail = Promise.resolve();
	readonly #randomToken: () => string;

	constructor(filePath: string, options: FileDesktopWindowStateStoreOptions = {}) {
		this.#filePath = path.resolve(filePath);
		this.#randomToken = options.randomToken ?? (() => randomBytes(16).toString('hex'));
	}

	async flush(): Promise<void> {
		await this.#operationTail;
	}

	async load(): Promise<DesktopWindowStateLoadResult> {
		await this.#operationTail;
		if (this.#memory !== undefined) {
			return this.#memory.state === undefined
				? { kind: 'missing' }
				: { kind: 'available', state: this.#memory.state };
		}
		const loaded = await readStateFile(this.#filePath);
		if (loaded.result.kind === 'unusable') this.#memory = { state: undefined };
		this.#extensions = loaded.extensions;
		return loaded.result;
	}

	save(state: DesktopWindowState): Promise<boolean> {
		const canonical = parseDesktopWindowState(state);
		if (canonical === null) return Promise.resolve(false);
		if (this.#memory !== undefined) {
			this.#memory.state = canonical;
			return Promise.resolve(true);
		}
		const result = this.#operationTail.then(async () => {
			try {
				await this.#writeState(canonical);
				return true;
			} catch {
				return false;
			}
		});
		this.#operationTail = result.then(() => undefined);
		return result;
	}

	async #writeState(state: DesktopWindowState): Promise<void> {
		const record =
			this.#extensions === undefined ? state : { ...state, extensions: this.#extensions };
		const encoded = Buffer.from(`${JSON.stringify(record)}\n`, 'utf8');
		const directory = path.dirname(this.#filePath);
		await mkdir(directory, { mode: 0o700, recursive: true });
		if (process.platform !== 'win32') await chmod(directory, 0o700);
		const temporaryPath = `${this.#filePath}.${this.#randomToken()}.tmp`;
		let temporaryCreated = false;
		try {
			const temporary = await open(temporaryPath, 'wx', 0o600);
			temporaryCreated = true;
			try {
				await temporary.writeFile(encoded);
				await temporary.sync();
			} finally {
				await temporary.close();
			}
			await rename(temporaryPath, this.#filePath);
		} catch (error: unknown) {
			if (temporaryCreated) await unlink(temporaryPath).catch(() => undefined);
			throw error;
		}
		if (process.platform !== 'win32') {
			await chmod(this.#filePath, 0o600);
			const directoryHandle = await open(directory, 'r');
			try {
				await directoryHandle.sync();
			} finally {
				await directoryHandle.close();
			}
		}
	}
}

interface LoadedStateFile {
	readonly extensions: JsonObject | undefined;
	readonly result: DesktopWindowStateLoadResult;
}

async function readStateFile(filePath: string): Promise<LoadedStateFile> {
	const invalid: LoadedStateFile = {
		extensions: undefined,
		result: { kind: 'unavailable', reason: 'invalid' },
	};
	try {
		const metadata = await stat(filePath);
		if (!metadata.isFile()) return invalid;
		const encoded = await readFile(filePath);
		let value: unknown;
		try {
			value = JSON.parse(utf8Decoder.decode(encoded));
		} catch {
			return invalid;
		}
		if (!isJsonObject(value)) return invalid;
		const { extensions, ...record } = value;
		const version = record['version'];
		if (
			typeof version === 'number' &&
			Number.isSafeInteger(version) &&
			version >= 0 &&
			version !== DESKTOP_WINDOW_STATE_VERSION
		) {
			const problem = version > DESKTOP_WINDOW_STATE_VERSION ? 'newer-format' : 'not-updated';
			return { extensions: undefined, result: { kind: 'unusable', problem, version } };
		}
		if (extensions !== undefined && !isJsonObject(extensions)) return invalid;
		const state = parseDesktopWindowState(record);
		return state === null ? invalid : { extensions, result: { kind: 'available', state } };
	} catch (error: unknown) {
		if (isNodeError(error) && error.code === 'ENOENT') {
			return { extensions: undefined, result: { kind: 'missing' } };
		}
		return { extensions: undefined, result: { kind: 'unavailable', reason: 'read-failed' } };
	}
}

function isJsonObject(value: unknown): value is JsonObject {
	return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function isNodeError(error: unknown): error is NodeJS.ErrnoException {
	return error instanceof Error && 'code' in error;
}
