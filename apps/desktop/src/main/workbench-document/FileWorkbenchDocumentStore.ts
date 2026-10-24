import { randomBytes } from 'node:crypto';
import { chmod, mkdir, open, readFile, rename, stat, unlink } from 'node:fs/promises';
import path from 'node:path';

import {
	parseWorkbenchSharedDocument,
	WorkbenchSharedDocumentRevisionSchema,
	type WorkbenchSharedDocument,
	type WorkbenchSharedDocumentRevision,
} from '@orchbee/workbench/document';

import {
	decideWorkbenchDocumentCommit,
	type WorkbenchDocumentStore,
	type WorkbenchDocumentStoreCommitResult,
	type WorkbenchDocumentStoreLoadResult,
} from './decideWorkbenchDocumentCommit';

const utf8Decoder = new TextDecoder('utf-8', { fatal: true });

export interface FileWorkbenchDocumentStoreOptions {
	readonly randomToken?: () => string;
	readonly synchronizeParentDirectory?: (directoryPath: string) => Promise<void>;
}

export class FileWorkbenchDocumentStore implements WorkbenchDocumentStore {
	readonly #filePath: string;
	readonly #randomToken: () => string;
	readonly #synchronizeParentDirectory: (directoryPath: string) => Promise<void>;
	#operationTail = Promise.resolve();

	constructor(filePath: string, options: FileWorkbenchDocumentStoreOptions = {}) {
		this.#filePath = path.resolve(filePath);
		this.#randomToken = options.randomToken ?? (() => randomBytes(16).toString('hex'));
		this.#synchronizeParentDirectory =
			options.synchronizeParentDirectory ?? synchronizeParentDirectory;
	}

	load(): Promise<WorkbenchDocumentStoreLoadResult> {
		return this.#enqueue(async () => this.#loadCurrent());
	}

	commit(
		expectedRevision: WorkbenchSharedDocumentRevision | null,
		document: WorkbenchSharedDocument,
	): Promise<WorkbenchDocumentStoreCommitResult> {
		const canonicalExpectedRevision =
			expectedRevision === null
				? null
				: WorkbenchSharedDocumentRevisionSchema.parse(expectedRevision);
		const canonicalDocument = parseWorkbenchSharedDocument(document);
		return this.#enqueue(async () => {
			const loaded = await this.#loadCurrent();
			const decision = decideWorkbenchDocumentCommit(
				loaded,
				canonicalExpectedRevision,
				canonicalDocument,
			);
			if (decision.kind !== 'write') return decision;
			try {
				await this.#writeDocument(decision.document);
				return { kind: 'committed', revision: decision.document.revision };
			} catch (error) {
				return {
					kind: 'unavailable',
					reason:
						error instanceof WorkbenchDocumentWriteError && error.renameCompleted
							? 'write-indeterminate'
							: 'write-failed',
				};
			}
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

	async #loadCurrent(): Promise<WorkbenchDocumentStoreLoadResult> {
		return readDocumentFile(this.#filePath);
	}

	async #writeDocument(document: WorkbenchSharedDocument): Promise<void> {
		const canonical = parseWorkbenchSharedDocument(document);
		const encoded = Buffer.from(`${JSON.stringify(canonical)}\n`, 'utf8');
		const directory = path.dirname(this.#filePath);
		await mkdir(directory, { mode: 0o700, recursive: true });
		if (process.platform !== 'win32') await chmod(directory, 0o700);
		const temporaryPath = `${this.#filePath}.${this.#randomToken()}.tmp`;
		let temporaryCreated = false;
		let renameCompleted = false;
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
			renameCompleted = true;
			if (process.platform !== 'win32') await chmod(this.#filePath, 0o600);
			await this.#synchronizeParentDirectory(directory);
		} catch (error) {
			if (temporaryCreated && !renameCompleted) {
				await unlink(temporaryPath).catch(() => undefined);
			}
			throw new WorkbenchDocumentWriteError(renameCompleted, error);
		}
	}
}

async function readDocumentFile(filePath: string): Promise<WorkbenchDocumentStoreLoadResult> {
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
		const parsed = parseDocumentFile(value);
		return parsed === null
			? { kind: 'unavailable', reason: 'invalid' }
			: { document: parsed, kind: 'available' };
	} catch (error) {
		if (isNodeError(error) && error.code === 'ENOENT') return { kind: 'missing' };
		return { kind: 'unavailable', reason: 'read-failed' };
	}
}

function parseDocumentFile(value: unknown): WorkbenchSharedDocument | null {
	try {
		return parseWorkbenchSharedDocument(value);
	} catch {
		return null;
	}
}

async function synchronizeParentDirectory(directoryPath: string): Promise<void> {
	if (process.platform === 'win32') return;
	const directoryHandle = await open(directoryPath, 'r');
	try {
		await directoryHandle.sync();
	} finally {
		await directoryHandle.close();
	}
}

class WorkbenchDocumentWriteError extends Error {
	readonly renameCompleted: boolean;

	constructor(renameCompleted: boolean, cause: unknown) {
		super('Workbench document write failed.', { cause });
		this.name = 'WorkbenchDocumentWriteError';
		this.renameCompleted = renameCompleted;
	}
}

function isNodeError(error: unknown): error is NodeJS.ErrnoException {
	return error instanceof Error && 'code' in error;
}
