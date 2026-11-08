import { randomBytes } from 'node:crypto';
import { chmod, mkdir, open, readFile, rename, stat, unlink } from 'node:fs/promises';
import path from 'node:path';

import type { UpgradeBackup } from '../upgrade-backup';
import {
	DESKTOP_WINDOW_STATE_STEPS,
	DESKTOP_WINDOW_STATE_VERSION,
	parseDesktopWindowState,
	type DesktopWindowState,
	type DesktopWindowStateLoadResult,
	type DesktopWindowStateMigrationStep,
	type DesktopWindowStateStore,
} from './desktop-window-state';

const utf8Decoder = new TextDecoder('utf-8', { fatal: true });

export interface FileDesktopWindowStateStoreOptions {
	readonly randomToken?: () => string;
	/** The migration steps: the shipped {@link DESKTOP_WINDOW_STATE_STEPS} unless a test injects its own. */
	readonly steps?: readonly DesktopWindowStateMigrationStep[];
	/** Backs the record up before a migration replaces it. */
	readonly upgradeBackup: UpgradeBackup;
}

type JsonObject = Readonly<Record<string, unknown>>;

/**
 * The private window state file, which only Desktop Main writes. Its `extensions` object holds
 * fields a later release added; it is kept unread and written back unchanged. Loading an earlier
 * version migrates it, backs the original up, and atomically replaces the record before the state is
 * used. While the file's version is one this build cannot use, or an earlier one it could not
 * migrate, back up, or replace, the store never rewrites it and keeps saved state in memory until
 * the process ends.
 */
export class FileDesktopWindowStateStore implements DesktopWindowStateStore {
	#extensions: JsonObject | undefined;
	readonly #filePath: string;
	#memory: { state: DesktopWindowState | undefined } | undefined;
	#operationTail = Promise.resolve();
	readonly #randomToken: () => string;
	readonly #steps: readonly DesktopWindowStateMigrationStep[];
	readonly #upgradeBackup: UpgradeBackup;

	constructor(filePath: string, options: FileDesktopWindowStateStoreOptions) {
		this.#filePath = path.resolve(filePath);
		this.#randomToken = options.randomToken ?? (() => randomBytes(16).toString('hex'));
		this.#steps = options.steps ?? DESKTOP_WINDOW_STATE_STEPS;
		this.#upgradeBackup = options.upgradeBackup;
	}

	async flush(): Promise<void> {
		await this.#operationTail;
	}

	/** Loads run in order with saves, so no save replaces an earlier record before it is backed up. */
	load(): Promise<DesktopWindowStateLoadResult> {
		const result = this.#operationTail.then(async () => this.#load());
		this.#operationTail = result.then(
			() => undefined,
			() => undefined,
		);
		return result;
	}

	async #load(): Promise<DesktopWindowStateLoadResult> {
		if (this.#memory !== undefined) {
			return this.#memory.state === undefined
				? { kind: 'missing' }
				: { kind: 'available', state: this.#memory.state };
		}
		const loaded = await readStateFile(this.#filePath, this.#steps);
		const migrated = loaded.migrated;
		if (migrated !== undefined && loaded.result.kind === 'available') {
			try {
				await this.#upgradeBackup(this.#filePath, migrated.original);
				await this.#writeRecord(encodeRecord(loaded.result.state, loaded.extensions));
			} catch {
				this.#memory = { state: undefined };
				return { kind: 'unusable', problem: 'not-updated', version: migrated.from };
			}
		}
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
		await this.#writeRecord(encodeRecord(state, this.#extensions));
	}

	/** Flushes `encoded` into a temporary file and atomically renames it over the record. */
	async #writeRecord(encoded: Buffer): Promise<void> {
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

function encodeRecord(state: DesktopWindowState, extensions: JsonObject | undefined): Buffer {
	const record = extensions === undefined ? state : { ...state, extensions };
	return Buffer.from(`${JSON.stringify(record)}\n`, 'utf8');
}

interface LoadedStateFile {
	readonly extensions: JsonObject | undefined;
	/** Set when `result` is an earlier record migrated in memory, which the store persists. */
	readonly migrated?: { readonly from: number; readonly original: Buffer };
	readonly result: DesktopWindowStateLoadResult;
}

async function readStateFile(
	filePath: string,
	steps: readonly DesktopWindowStateMigrationStep[],
): Promise<LoadedStateFile> {
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
		const version = value['version'];
		if (
			typeof version === 'number' &&
			Number.isSafeInteger(version) &&
			version >= 0 &&
			version !== DESKTOP_WINDOW_STATE_VERSION
		) {
			const opened = version < DESKTOP_WINDOW_STATE_VERSION ? migrate(value, version, steps) : null;
			if (opened === null) {
				const problem = version > DESKTOP_WINDOW_STATE_VERSION ? 'newer-format' : 'not-updated';
				return { extensions: undefined, result: { kind: 'unusable', problem, version } };
			}
			return { ...opened, migrated: { from: version, original: encoded } };
		}
		return parseRecord(value) ?? invalid;
	} catch (error: unknown) {
		if (isNodeError(error) && error.code === 'ENOENT') {
			return { extensions: undefined, result: { kind: 'missing' } };
		}
		return { extensions: undefined, result: { kind: 'unavailable', reason: 'read-failed' } };
	}
}

/** The current record, or `null` when `value` is not one. */
function parseRecord(value: JsonObject): LoadedStateFile | null {
	const { extensions, ...record } = value;
	if (extensions !== undefined && !isJsonObject(extensions)) return null;
	const state = parseDesktopWindowState(record);
	return state === null ? null : { extensions, result: { kind: 'available', state } };
}

/**
 * Applies every step from `version` on; the result must pass the current strict parser, or the
 * record is one this build could not update.
 */
function migrate(
	value: JsonObject,
	version: number,
	steps: readonly DesktopWindowStateMigrationStep[],
): LoadedStateFile | null {
	let current = value;
	for (let from = version; from < DESKTOP_WINDOW_STATE_VERSION; from += 1) {
		const step = steps.find((candidate) => candidate.from === from);
		const next = step?.apply(current) ?? null;
		if (next === null || !isJsonObject(next) || next['version'] !== from + 1) return null;
		current = next;
	}
	return parseRecord(current);
}

function isJsonObject(value: unknown): value is JsonObject {
	return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function isNodeError(error: unknown): error is NodeJS.ErrnoException {
	return error instanceof Error && 'code' in error;
}
