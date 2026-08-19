import { randomBytes } from 'node:crypto';
import { chmod, mkdir, open, rename, unlink } from 'node:fs/promises';
import path from 'node:path';

import {
	DESKTOP_TERMINAL_LAUNCH_PROFILE_LIMITS,
	DesktopTerminalLaunchProfileCatalogSchema,
	type DesktopTerminalLaunchProfileCatalog,
} from '@orchbee/desktop-client-protocol';

import type {
	TerminalLaunchProfileStore,
	TerminalLaunchProfileStoreLoadResult,
} from './terminal-launch-profile-store';

const utf8Decoder = new TextDecoder('utf-8', { fatal: true });

export interface FileTerminalLaunchProfileStoreOptions {
	readonly randomToken?: () => string;
}

export class FileTerminalLaunchProfileStore implements TerminalLaunchProfileStore {
	readonly #filePath: string;
	#operationTail = Promise.resolve();
	readonly #randomToken: () => string;

	constructor(filePath: string, options: FileTerminalLaunchProfileStoreOptions = {}) {
		this.#filePath = path.resolve(filePath);
		this.#randomToken = options.randomToken ?? (() => randomBytes(16).toString('hex'));
	}

	async load(): Promise<TerminalLaunchProfileStoreLoadResult> {
		await this.#operationTail;
		return readCatalogFile(this.#filePath);
	}

	replace(catalog: DesktopTerminalLaunchProfileCatalog): Promise<void> {
		const canonical = DesktopTerminalLaunchProfileCatalogSchema.parse(catalog);
		const operation = this.#operationTail.then(async () => this.#writeCatalog(canonical));
		this.#operationTail = operation.catch(() => undefined);
		return operation;
	}

	async #writeCatalog(catalog: DesktopTerminalLaunchProfileCatalog): Promise<void> {
		const encoded = Buffer.from(`${JSON.stringify(catalog)}\n`, 'utf8');
		if (encoded.byteLength > DESKTOP_TERMINAL_LAUNCH_PROFILE_LIMITS.maximumCatalogBytes + 1) {
			throw new Error('Terminal launch-profile catalog exceeds the storage boundary.');
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
			// The temporary file already owns mode 0600. Rename is the catalog commit point;
			// a best-effort directory sync after it must not report that the old catalog survived.
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

async function readCatalogFile(filePath: string): Promise<TerminalLaunchProfileStoreLoadResult> {
	let handle: Awaited<ReturnType<typeof open>>;
	try {
		handle = await open(filePath, 'r');
	} catch (error: unknown) {
		if (isNodeError(error) && error.code === 'ENOENT') return { kind: 'missing' };
		return { kind: 'unavailable', reason: 'read-failed' };
	}
	try {
		const metadata = await handle.stat();
		if (
			!metadata.isFile() ||
			metadata.size > DESKTOP_TERMINAL_LAUNCH_PROFILE_LIMITS.maximumCatalogBytes + 1
		) {
			return { kind: 'unavailable', reason: 'invalid-record' };
		}
		const encoded = await handle.readFile();
		if (encoded.byteLength > DESKTOP_TERMINAL_LAUNCH_PROFILE_LIMITS.maximumCatalogBytes + 1) {
			return { kind: 'unavailable', reason: 'invalid-record' };
		}
		let value: unknown;
		try {
			value = JSON.parse(utf8Decoder.decode(encoded));
		} catch {
			return { kind: 'unavailable', reason: 'invalid-record' };
		}
		const parsed = DesktopTerminalLaunchProfileCatalogSchema.safeParse(value);
		return parsed.success
			? { catalog: parsed.data, kind: 'available' }
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
