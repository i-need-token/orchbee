import { randomBytes } from 'node:crypto';
import { chmod, mkdir, open, readFile, rename, stat, unlink } from 'node:fs/promises';
import path from 'node:path';

import {
	createTerminalBindingStoreDocument,
	putTerminalBinding,
	removeTerminalBinding,
	TerminalBindingIdSchema,
	TerminalBindingSchema,
	TerminalBindingStoreFileSchema,
	type TerminalBinding,
	type TerminalBindingId,
	type TerminalBindingStore,
	type TerminalBindingStoreDocument,
	type TerminalBindingStoreLoadResult,
} from './terminal-binding-store';

const utf8Decoder = new TextDecoder('utf-8', { fatal: true });

export interface FileTerminalBindingStoreOptions {
	readonly randomToken?: () => string;
}

export class FileTerminalBindingStore implements TerminalBindingStore {
	readonly #filePath: string;
	readonly #randomToken: () => string;
	#writeTail = Promise.resolve();

	constructor(filePath: string, options: FileTerminalBindingStoreOptions = {}) {
		this.#filePath = path.resolve(filePath);
		this.#randomToken = options.randomToken ?? (() => randomBytes(16).toString('hex'));
	}

	async load(): Promise<TerminalBindingStoreLoadResult> {
		await this.#writeTail;
		return this.#loadCurrent();
	}

	async put(binding: TerminalBinding): Promise<void> {
		const canonical = TerminalBindingSchema.parse(binding);
		return this.#enqueueMutation(async () => {
			const loaded = await this.#loadCurrent();
			const document = requireWritableDocument(loaded);
			await this.#writeDocument(putTerminalBinding(document, canonical));
		});
	}

	async remove(bindingId: TerminalBindingId): Promise<void> {
		const canonical = TerminalBindingIdSchema.parse(bindingId);
		return this.#enqueueMutation(async () => {
			const loaded = await this.#loadCurrent();
			const document = requireWritableDocument(loaded);
			if (!document.bindings.some((binding) => binding.bindingId === canonical)) return;
			await this.#writeDocument(removeTerminalBinding(document, canonical));
		});
	}

	#enqueueMutation(operation: () => Promise<void>): Promise<void> {
		const result = this.#writeTail.then(operation);
		this.#writeTail = result.catch(() => undefined);
		return result;
	}

	async #loadCurrent(): Promise<TerminalBindingStoreLoadResult> {
		return readStoreFile(this.#filePath);
	}

	async #writeDocument(document: TerminalBindingStoreDocument): Promise<void> {
		const canonical = createTerminalBindingStoreDocument(document.bindings);
		const encoded = Buffer.from(`${JSON.stringify(canonical)}\n`, 'utf8');
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
		} catch (error) {
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

async function readStoreFile(filePath: string): Promise<TerminalBindingStoreLoadResult> {
	try {
		const metadata = await stat(filePath);
		if (!metadata.isFile()) return { kind: 'unavailable', reason: 'invalid' };
		const encoded = await readFile(filePath);
		let value: unknown;
		try {
			value = JSON.parse(utf8Decoder.decode(encoded));
		} catch {
			return { kind: 'unavailable', reason: 'invalid' };
		}
		const parsed = TerminalBindingStoreFileSchema.safeParse(value);
		return parsed.success
			? { document: parsed.data, kind: 'available' }
			: { kind: 'unavailable', reason: 'invalid' };
	} catch (error) {
		if (isNodeError(error) && error.code === 'ENOENT') return { kind: 'missing' };
		return { kind: 'unavailable', reason: 'read-failed' };
	}
}

function requireWritableDocument(
	loaded: TerminalBindingStoreLoadResult,
): TerminalBindingStoreDocument {
	if (loaded.kind === 'available') return loaded.document;
	if (loaded.kind === 'missing') return createTerminalBindingStoreDocument();
	throw new Error(`Terminal binding store is unavailable: ${loaded.reason}.`);
}

function isNodeError(error: unknown): error is NodeJS.ErrnoException {
	return error instanceof Error && 'code' in error;
}
