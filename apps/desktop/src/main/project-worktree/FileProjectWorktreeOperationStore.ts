import { randomBytes } from 'node:crypto';
import { chmod, mkdir, open, readFile, rename, stat, unlink } from 'node:fs/promises';
import path from 'node:path';

import { z } from 'zod';

const operationSchema = z.strictObject({
	baseCommit: z.string().min(1),
	baseRef: z.string().min(1),
	branchName: z.string().min(1),
	createWorkspace: z.boolean(),
	directoryId: z.string().min(1),
	operationId: z.string().min(1),
	phase: z.enum(['git-created', 'prepared', 'recovery-required']),
	projectId: z.string().min(1),
	repositoryId: z.string().min(1),
	sourceDirectoryId: z.string().min(1),
	sourcePath: z.string().min(1),
	targetPath: z.string().min(1),
});

const operationFileSchema = z.strictObject({
	operations: z.array(operationSchema).max(1_024),
	version: z.literal(1),
});

export type ProjectWorktreeOperationRecord = z.infer<typeof operationSchema>;

export type ProjectWorktreeOperationStoreLoadResult =
	| { readonly kind: 'ready'; readonly operations: readonly ProjectWorktreeOperationRecord[] }
	| { readonly kind: 'unavailable'; readonly reason: 'invalid' | 'read-failed' };

export interface ProjectWorktreeOperationStore {
	load(): Promise<ProjectWorktreeOperationStoreLoadResult>;
	replace(operations: readonly ProjectWorktreeOperationRecord[]): Promise<void>;
}

export class FileProjectWorktreeOperationStore implements ProjectWorktreeOperationStore {
	readonly #filePath: string;
	#operationTail = Promise.resolve();
	readonly #randomToken: () => string;

	constructor(filePath: string, randomToken: () => string = () => randomBytes(16).toString('hex')) {
		this.#filePath = path.resolve(filePath);
		this.#randomToken = randomToken;
	}

	async load(): Promise<ProjectWorktreeOperationStoreLoadResult> {
		await this.#operationTail;
		try {
			const metadata = await stat(this.#filePath);
			if (!metadata.isFile() || metadata.size > 4 * 1024 * 1024) {
				return { kind: 'unavailable', reason: 'invalid' };
			}
			const encoded = await readFile(this.#filePath);
			let value: unknown;
			try {
				value = JSON.parse(new TextDecoder('utf-8', { fatal: true }).decode(encoded));
			} catch {
				return { kind: 'unavailable', reason: 'invalid' };
			}
			const parsed = operationFileSchema.safeParse(value);
			if (!parsed.success) return { kind: 'unavailable', reason: 'invalid' };
			const ids = new Set(parsed.data.operations.map((operation) => operation.operationId));
			return ids.size === parsed.data.operations.length
				? { kind: 'ready', operations: parsed.data.operations }
				: { kind: 'unavailable', reason: 'invalid' };
		} catch (error: unknown) {
			return isNodeError(error) && error.code === 'ENOENT'
				? { kind: 'ready', operations: [] }
				: { kind: 'unavailable', reason: 'read-failed' };
		}
	}

	replace(operations: readonly ProjectWorktreeOperationRecord[]): Promise<void> {
		const canonical = operationFileSchema.parse({ operations, version: 1 });
		const operation = this.#operationTail.then(async () => this.#write(canonical));
		this.#operationTail = operation.catch(() => undefined);
		return operation;
	}

	async #write(document: z.infer<typeof operationFileSchema>): Promise<void> {
		const directory = path.dirname(this.#filePath);
		await mkdir(directory, { mode: 0o700, recursive: true });
		if (process.platform !== 'win32') await chmod(directory, 0o700);
		const temporaryPath = `${this.#filePath}.${this.#randomToken()}.tmp`;
		let temporaryCreated = false;
		try {
			const temporary = await open(temporaryPath, 'wx', 0o600);
			temporaryCreated = true;
			try {
				await temporary.writeFile(`${JSON.stringify(document)}\n`, 'utf8');
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

function isNodeError(error: unknown): error is NodeJS.ErrnoException {
	return error instanceof Error && 'code' in error;
}
