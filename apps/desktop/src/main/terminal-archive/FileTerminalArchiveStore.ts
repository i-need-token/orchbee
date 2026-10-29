import { createHash, randomBytes } from 'node:crypto';
import { chmod, mkdir, open, readdir, rename, stat, unlink, utimes } from 'node:fs/promises';
import path from 'node:path';

import type { TerminalPresentationId } from '@orchbee/workbench-client-protocol';

import {
	decodeTerminalArchive,
	encodeTerminalArchive,
	MAX_TERMINAL_ARCHIVE_BYTES,
	MAX_TERMINAL_ARCHIVE_STORE_BYTES,
	type TerminalArchive,
	type TerminalArchiveWrite,
} from './terminal-archive-document';

const ARCHIVE_FILE_SUFFIX = '.obta';

export interface FileTerminalArchiveStoreOptions {
	readonly maximumStoreBytes?: number;
	readonly now?: () => Date;
	readonly randomToken?: () => string;
}

interface ArchiveFile {
	readonly modifiedAt: number;
	readonly name: string;
	readonly path: string;
	readonly size: number;
}

export class FileTerminalArchiveStore {
	readonly #directory: string;
	readonly #maximumStoreBytes: number;
	readonly #now: () => Date;
	readonly #randomToken: () => string;
	#operationTail = Promise.resolve();

	constructor(directory: string, options: FileTerminalArchiveStoreOptions = {}) {
		this.#directory = path.resolve(directory);
		this.#maximumStoreBytes = options.maximumStoreBytes ?? MAX_TERMINAL_ARCHIVE_STORE_BYTES;
		if (!Number.isSafeInteger(this.#maximumStoreBytes) || this.#maximumStoreBytes < 1) {
			throw new Error('Terminal archive store capacity must be a positive safe integer.');
		}
		this.#now = options.now ?? (() => new Date());
		this.#randomToken = options.randomToken ?? (() => randomBytes(16).toString('hex'));
	}

	load(presentationId: TerminalPresentationId): Promise<TerminalArchive | null> {
		return this.#enqueue(async () => {
			const filePath = this.#archivePath(presentationId);
			try {
				const encoded = await readBoundedArchive(filePath);
				if (encoded === null) {
					await unlink(filePath).catch(() => undefined);
					return null;
				}
				const decoded = decodeTerminalArchive(encoded);
				if (!decoded.ok || decoded.value.presentationId !== presentationId) {
					await unlink(filePath).catch(() => undefined);
					return null;
				}
				const now = this.#now();
				await utimes(filePath, now, now).catch(() => undefined);
				return decoded.value;
			} catch (error) {
				if (isNodeError(error) && error.code === 'ENOENT') return null;
				return null;
			}
		});
	}

	put(archive: TerminalArchiveWrite): Promise<void> {
		const encoded = encodeTerminalArchive(archive);
		if (!encoded.ok) return Promise.reject(new Error(encoded.detail));
		return this.#enqueue(async () => {
			await this.#ensureDirectory();
			const destination = this.#archivePath(archive.presentationId);
			const temporary = path.join(this.#directory, `.terminal-archive-${this.#randomToken()}.tmp`);
			let created = false;
			try {
				const handle = await open(temporary, 'wx', 0o600);
				created = true;
				try {
					await handle.writeFile(encoded.value);
					await handle.sync();
				} finally {
					await handle.close();
				}
				await rename(temporary, destination);
				created = false;
				const now = this.#now();
				await utimes(destination, now, now);
				if (process.platform !== 'win32') await chmod(destination, 0o600);
				await this.#syncDirectory();
				await this.#enforceCapacity();
			} catch (error) {
				if (created) await unlink(temporary).catch(() => undefined);
				throw error;
			}
		});
	}

	remove(presentationId: TerminalPresentationId): Promise<void> {
		return this.#enqueue(async () => {
			await unlink(this.#archivePath(presentationId)).catch((error: unknown) => {
				if (!isNodeError(error) || error.code !== 'ENOENT') throw error;
			});
		});
	}

	reconcile(retainedPresentationIds: ReadonlySet<TerminalPresentationId>): Promise<void> {
		return this.#enqueue(async () => {
			let entries;
			try {
				entries = await readdir(this.#directory, { withFileTypes: true });
			} catch (error) {
				if (isNodeError(error) && error.code === 'ENOENT') return;
				throw error;
			}
			for (const entry of entries) {
				const filePath = path.join(this.#directory, entry.name);
				if (!entry.isFile()) continue;
				if (entry.name.endsWith('.tmp')) {
					await unlink(filePath).catch(() => undefined);
					continue;
				}
				if (!entry.name.endsWith(ARCHIVE_FILE_SUFFIX)) continue;
				let keep = false;
				try {
					const encoded = await readBoundedArchive(filePath);
					if (encoded !== null) {
						const decoded = decodeTerminalArchive(encoded);
						keep =
							decoded.ok &&
							retainedPresentationIds.has(decoded.value.presentationId) &&
							filePath === this.#archivePath(decoded.value.presentationId);
					}
				} catch {
					keep = false;
				}
				if (!keep) await unlink(filePath).catch(() => undefined);
			}
			await this.#enforceCapacity();
		});
	}

	#enqueue<Result>(operation: () => Promise<Result>): Promise<Result> {
		const result = this.#operationTail.then(operation);
		this.#operationTail = result.then(
			() => undefined,
			() => undefined,
		);
		return result;
	}

	#archivePath(presentationId: TerminalPresentationId): string {
		const digest = createHash('sha256').update(presentationId).digest('hex');
		return path.join(this.#directory, `${digest}${ARCHIVE_FILE_SUFFIX}`);
	}

	async #ensureDirectory(): Promise<void> {
		await mkdir(this.#directory, { mode: 0o700, recursive: true });
		if (process.platform !== 'win32') await chmod(this.#directory, 0o700);
	}

	async #syncDirectory(): Promise<void> {
		if (process.platform === 'win32') return;
		const handle = await open(this.#directory, 'r');
		try {
			await handle.sync();
		} finally {
			await handle.close();
		}
	}

	async #enforceCapacity(): Promise<void> {
		const files = await this.#archiveFiles();
		let total = files.reduce((sum, file) => sum + file.size, 0);
		files.sort(
			(left, right) => left.modifiedAt - right.modifiedAt || left.name.localeCompare(right.name),
		);
		for (const file of files) {
			if (total <= this.#maximumStoreBytes) break;
			await unlink(file.path);
			total -= file.size;
		}
	}

	async #archiveFiles(): Promise<ArchiveFile[]> {
		let entries;
		try {
			entries = await readdir(this.#directory, { withFileTypes: true });
		} catch (error) {
			if (isNodeError(error) && error.code === 'ENOENT') return [];
			throw error;
		}
		const files: ArchiveFile[] = [];
		for (const entry of entries) {
			if (!entry.isFile() || !entry.name.endsWith(ARCHIVE_FILE_SUFFIX)) continue;
			const filePath = path.join(this.#directory, entry.name);
			try {
				const metadata = await stat(filePath);
				if (!metadata.isFile()) continue;
				files.push({
					modifiedAt: metadata.mtimeMs,
					name: entry.name,
					path: filePath,
					size: metadata.size,
				});
			} catch (error) {
				if (!isNodeError(error) || error.code !== 'ENOENT') throw error;
			}
		}
		return files;
	}
}

async function readBoundedArchive(filePath: string): Promise<Uint8Array | null> {
	const handle = await open(filePath, 'r');
	try {
		const metadata = await handle.stat();
		if (metadata.size > MAX_TERMINAL_ARCHIVE_BYTES) return null;

		const encoded = Buffer.allocUnsafe(MAX_TERMINAL_ARCHIVE_BYTES + 1);
		let byteLength = 0;
		while (byteLength < encoded.byteLength) {
			const result = await handle.read(encoded, byteLength, encoded.byteLength - byteLength);
			if (result.bytesRead === 0) break;
			byteLength += result.bytesRead;
		}
		if (byteLength > MAX_TERMINAL_ARCHIVE_BYTES) return null;
		return encoded.subarray(0, byteLength);
	} finally {
		await handle.close();
	}
}

function isNodeError(error: unknown): error is NodeJS.ErrnoException {
	return error instanceof Error && 'code' in error;
}
