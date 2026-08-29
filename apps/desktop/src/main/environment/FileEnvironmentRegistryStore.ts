import { randomBytes } from 'node:crypto';
import { chmod, mkdir, open, rename, unlink } from 'node:fs/promises';
import path from 'node:path';

import {
	DESKTOP_ENVIRONMENT_REGISTRY_LIMITS,
	EnvironmentRegistryDocumentSchema,
	createEmptyEnvironmentRegistryDocument,
	type EnvironmentRegistryDocument,
} from './environment-registry-contract';

const utf8Decoder = new TextDecoder('utf-8', { fatal: true });

export type EnvironmentRegistryStoreUnavailableReason = 'invalid-record' | 'read-failed';

export type EnvironmentRegistryStoreLoadResult =
	| { readonly document: EnvironmentRegistryDocument; readonly kind: 'ready' }
	| { readonly kind: 'unavailable'; readonly reason: EnvironmentRegistryStoreUnavailableReason };

export type EnvironmentRegistryStoreCommitResult =
	| { readonly kind: 'committed' }
	| { readonly document: EnvironmentRegistryDocument; readonly kind: 'conflict' }
	| { readonly kind: 'indeterminate'; readonly reason: 'durability-unconfirmed' }
	| {
			readonly kind: 'unavailable';
			readonly reason:
				| EnvironmentRegistryStoreUnavailableReason
				| 'current-record-missing'
				| 'write-failed';
	  };

export interface EnvironmentRegistryStore {
	commit(
		expectedRevision: number,
		document: EnvironmentRegistryDocument,
	): Promise<EnvironmentRegistryStoreCommitResult>;
	load(): Promise<EnvironmentRegistryStoreLoadResult>;
}

interface FileEnvironmentRegistryStoreOptions {
	readonly platform?: NodeJS.Platform;
	readonly randomToken?: () => string;
	readonly synchronizeDirectory?: (directory: string) => Promise<void>;
}

type EnvironmentRegistryFileReadResult =
	| { readonly document: EnvironmentRegistryDocument; readonly kind: 'ready' }
	| { readonly kind: 'missing' }
	| { readonly kind: 'unavailable'; readonly reason: EnvironmentRegistryStoreUnavailableReason };

export class FileEnvironmentRegistryStore implements EnvironmentRegistryStore {
	readonly #filePath: string;
	#operationTail = Promise.resolve();
	readonly #platform: NodeJS.Platform;
	readonly #randomToken: () => string;
	readonly #synchronizeDirectory: (directory: string) => Promise<void>;

	constructor(filePath: string, options: FileEnvironmentRegistryStoreOptions = {}) {
		this.#filePath = path.resolve(filePath);
		this.#platform = options.platform ?? process.platform;
		this.#randomToken = options.randomToken ?? (() => randomBytes(16).toString('hex'));
		this.#synchronizeDirectory = options.synchronizeDirectory ?? synchronizeDirectory;
	}

	commit(
		expectedRevision: number,
		document: EnvironmentRegistryDocument,
	): Promise<EnvironmentRegistryStoreCommitResult> {
		const canonical = EnvironmentRegistryDocumentSchema.parse(document);
		const operation = this.#operationTail.then(async () =>
			this.#commit(expectedRevision, canonical),
		);
		this.#operationTail = operation.then(
			() => undefined,
			() => undefined,
		);
		return operation;
	}

	async load(): Promise<EnvironmentRegistryStoreLoadResult> {
		await this.#operationTail;
		const result = await readEnvironmentRegistryFile(this.#filePath);
		return result.kind === 'missing'
			? { document: createEmptyEnvironmentRegistryDocument(), kind: 'ready' }
			: result;
	}

	async #commit(
		expectedRevision: number,
		document: EnvironmentRegistryDocument,
	): Promise<EnvironmentRegistryStoreCommitResult> {
		const current = await readEnvironmentRegistryFile(this.#filePath);
		if (current.kind === 'unavailable') return current;
		if (current.kind === 'missing' && expectedRevision !== 0) {
			return { kind: 'unavailable', reason: 'current-record-missing' };
		}
		const currentDocument =
			current.kind === 'missing' ? createEmptyEnvironmentRegistryDocument() : current.document;
		if (currentDocument.revision !== expectedRevision) {
			return { document: currentDocument, kind: 'conflict' };
		}

		const encoded = Buffer.from(`${JSON.stringify(document)}\n`, 'utf8');
		if (encoded.byteLength > DESKTOP_ENVIRONMENT_REGISTRY_LIMITS.maximumDocumentBytes + 1) {
			return { kind: 'unavailable', reason: 'write-failed' };
		}
		const directory = path.dirname(this.#filePath);
		const temporaryPath = `${this.#filePath}.${this.#randomToken()}.tmp`;
		let temporaryCreated = false;
		try {
			await mkdir(directory, { mode: 0o700, recursive: true });
			if (this.#platform !== 'win32') await chmod(directory, 0o700);
			const temporary = await open(temporaryPath, 'wx', 0o600);
			temporaryCreated = true;
			try {
				await temporary.writeFile(encoded);
				await temporary.sync();
			} finally {
				await temporary.close();
			}
			await rename(temporaryPath, this.#filePath);
		} catch {
			if (temporaryCreated) await unlink(temporaryPath).catch(() => undefined);
			return { kind: 'unavailable', reason: 'write-failed' };
		}
		if (this.#platform !== 'win32') {
			try {
				await this.#synchronizeDirectory(directory);
			} catch {
				return { kind: 'indeterminate', reason: 'durability-unconfirmed' };
			}
		}
		return { kind: 'committed' };
	}
}

async function readEnvironmentRegistryFile(
	filePath: string,
): Promise<EnvironmentRegistryFileReadResult> {
	let handle: Awaited<ReturnType<typeof open>>;
	try {
		handle = await open(filePath, 'r');
	} catch (error: unknown) {
		return isNodeError(error) && error.code === 'ENOENT'
			? { kind: 'missing' }
			: { kind: 'unavailable', reason: 'read-failed' };
	}
	try {
		const metadata = await handle.stat();
		if (
			!metadata.isFile() ||
			metadata.size > DESKTOP_ENVIRONMENT_REGISTRY_LIMITS.maximumDocumentBytes + 1
		) {
			return { kind: 'unavailable', reason: 'invalid-record' };
		}
		const encoded = await handle.readFile();
		if (encoded.byteLength > DESKTOP_ENVIRONMENT_REGISTRY_LIMITS.maximumDocumentBytes + 1) {
			return { kind: 'unavailable', reason: 'invalid-record' };
		}
		let value: unknown;
		try {
			value = JSON.parse(utf8Decoder.decode(encoded));
		} catch {
			return { kind: 'unavailable', reason: 'invalid-record' };
		}
		const parsed = EnvironmentRegistryDocumentSchema.safeParse(value);
		return parsed.success
			? { document: parsed.data, kind: 'ready' }
			: { kind: 'unavailable', reason: 'invalid-record' };
	} catch {
		return { kind: 'unavailable', reason: 'read-failed' };
	} finally {
		await handle.close().catch(() => undefined);
	}
}

async function synchronizeDirectory(directory: string): Promise<void> {
	const directoryHandle = await open(directory, 'r');
	try {
		await directoryHandle.sync();
	} finally {
		await directoryHandle.close().catch(() => undefined);
	}
}

function isNodeError(error: unknown): error is NodeJS.ErrnoException {
	return error instanceof Error && 'code' in error;
}
