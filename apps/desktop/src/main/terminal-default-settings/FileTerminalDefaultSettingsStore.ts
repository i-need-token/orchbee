import { randomBytes } from 'node:crypto';
import { chmod, mkdir, open, rename, unlink } from 'node:fs/promises';
import path from 'node:path';

import {
	TerminalConfigurationSchema,
	type TerminalConfiguration,
} from '@orchbee/terminal-protocol';
import { z } from 'zod';

import type {
	TerminalDefaultSettingsStore,
	TerminalDefaultSettingsStoreLoadResult,
} from './terminal-default-settings-store';

const MAXIMUM_RECORD_BYTES = 4 * 1024;
const utf8Decoder = new TextDecoder('utf-8', { fatal: true });

const TerminalDefaultSettingsRecordSchema = z.strictObject({
	configuration: TerminalConfigurationSchema,
	version: z.literal(1),
});

export interface FileTerminalDefaultSettingsStoreOptions {
	readonly randomToken?: (() => string) | undefined;
}

export class FileTerminalDefaultSettingsStore implements TerminalDefaultSettingsStore {
	readonly #filePath: string;
	#operationTail = Promise.resolve();
	readonly #randomToken: () => string;

	constructor(filePath: string, options: FileTerminalDefaultSettingsStoreOptions = {}) {
		this.#filePath = path.resolve(filePath);
		this.#randomToken = options.randomToken ?? (() => randomBytes(16).toString('hex'));
	}

	async load(): Promise<TerminalDefaultSettingsStoreLoadResult> {
		await this.#operationTail;
		return readSettingsFile(this.#filePath);
	}

	replace(configuration: TerminalConfiguration): Promise<void> {
		const canonical = TerminalConfigurationSchema.parse(configuration);
		const operation = this.#operationTail.then(async () => this.#writeConfiguration(canonical));
		this.#operationTail = operation.catch(() => undefined);
		return operation;
	}

	async #writeConfiguration(configuration: TerminalConfiguration): Promise<void> {
		const record = TerminalDefaultSettingsRecordSchema.parse({ configuration, version: 1 });
		const encoded = Buffer.from(`${JSON.stringify(record)}\n`, 'utf8');
		if (encoded.byteLength > MAXIMUM_RECORD_BYTES) {
			throw new Error('Terminal-default settings exceed the storage boundary.');
		}
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
			const directoryHandle = await open(directory, 'r').catch(() => undefined);
			if (directoryHandle !== undefined) {
				try {
					await directoryHandle.sync().catch(() => undefined);
				} finally {
					await directoryHandle.close().catch(() => undefined);
				}
			}
		}
	}
}

async function readSettingsFile(filePath: string): Promise<TerminalDefaultSettingsStoreLoadResult> {
	let handle: Awaited<ReturnType<typeof open>>;
	try {
		handle = await open(filePath, 'r');
	} catch (error: unknown) {
		if (isNodeError(error) && error.code === 'ENOENT') return { kind: 'missing' };
		return { kind: 'unavailable', reason: 'read-failed' };
	}
	try {
		const metadata = await handle.stat();
		if (!metadata.isFile() || metadata.size > MAXIMUM_RECORD_BYTES) {
			return { kind: 'unavailable', reason: 'invalid-record' };
		}
		const encoded = await handle.readFile();
		if (encoded.byteLength > MAXIMUM_RECORD_BYTES) {
			return { kind: 'unavailable', reason: 'invalid-record' };
		}
		let value: unknown;
		try {
			value = JSON.parse(utf8Decoder.decode(encoded));
		} catch {
			return { kind: 'unavailable', reason: 'invalid-record' };
		}
		const parsed = TerminalDefaultSettingsRecordSchema.safeParse(value);
		return parsed.success
			? { configuration: parsed.data.configuration, kind: 'available' }
			: { kind: 'unavailable', reason: 'invalid-record' };
	} catch {
		return { kind: 'unavailable', reason: 'read-failed' };
	} finally {
		await handle.close().catch(() => undefined);
	}
}

function isNodeError(error: unknown): error is NodeJS.ErrnoException {
	return error instanceof Error && 'code' in error;
}
