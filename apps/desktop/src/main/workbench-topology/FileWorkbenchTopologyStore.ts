import { randomBytes } from 'node:crypto';
import { chmod, mkdir, open, readFile, rename, stat, unlink } from 'node:fs/promises';
import path from 'node:path';

import {
	parseWorkbenchTopologyDocument,
	WorkbenchTopologyRevisionSchema,
	type WorkbenchTopologyDocument,
	type WorkbenchTopologyRevision,
} from '@orchbee/workbench/topology';

import {
	decideWorkbenchTopologyCommit,
	type WorkbenchTopologyStore,
	type WorkbenchTopologyStoreCommitResult,
	type WorkbenchTopologyStoreLoadResult,
} from './decideWorkbenchTopologyCommit';

const utf8Decoder = new TextDecoder('utf-8', { fatal: true });

export interface FileWorkbenchTopologyStoreOptions {
	readonly randomToken?: () => string;
}

export class FileWorkbenchTopologyStore implements WorkbenchTopologyStore {
	readonly #filePath: string;
	readonly #randomToken: () => string;
	#operationTail = Promise.resolve();

	constructor(filePath: string, options: FileWorkbenchTopologyStoreOptions = {}) {
		this.#filePath = path.resolve(filePath);
		this.#randomToken = options.randomToken ?? (() => randomBytes(16).toString('hex'));
	}

	async load(): Promise<WorkbenchTopologyStoreLoadResult> {
		await this.#operationTail;
		return this.#loadCurrent();
	}

	commit(
		expectedRevision: WorkbenchTopologyRevision | null,
		document: WorkbenchTopologyDocument,
	): Promise<WorkbenchTopologyStoreCommitResult> {
		const canonicalExpectedRevision =
			expectedRevision === null ? null : WorkbenchTopologyRevisionSchema.parse(expectedRevision);
		const canonicalDocument = parseWorkbenchTopologyDocument(document);
		return this.#enqueue(async () => {
			const loaded = await this.#loadCurrent();
			const decision = decideWorkbenchTopologyCommit(
				loaded,
				canonicalExpectedRevision,
				canonicalDocument,
			);
			if (decision.kind !== 'write') return decision;
			try {
				await this.#writeDocument(decision.document);
				return { kind: 'committed', revision: decision.document.revision };
			} catch {
				return { kind: 'unavailable', reason: 'write-failed' };
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

	async #loadCurrent(): Promise<WorkbenchTopologyStoreLoadResult> {
		return readTopologyFile(this.#filePath);
	}

	async #writeDocument(document: WorkbenchTopologyDocument): Promise<void> {
		const canonical = parseWorkbenchTopologyDocument(document);
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

async function readTopologyFile(filePath: string): Promise<WorkbenchTopologyStoreLoadResult> {
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
		const parsed = parseTopologyFile(value);
		return parsed === null
			? { kind: 'unavailable', reason: 'invalid' }
			: { document: parsed, kind: 'available' };
	} catch (error) {
		if (isNodeError(error) && error.code === 'ENOENT') return { kind: 'missing' };
		return { kind: 'unavailable', reason: 'read-failed' };
	}
}

function parseTopologyFile(value: unknown): WorkbenchTopologyDocument | null {
	try {
		return parseWorkbenchTopologyDocument(value);
	} catch {
		return null;
	}
}

function isNodeError(error: unknown): error is NodeJS.ErrnoException {
	return error instanceof Error && 'code' in error;
}
