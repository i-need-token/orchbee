import { randomBytes } from 'node:crypto';
import { chmod, mkdir, open, readFile, rename, stat, unlink } from 'node:fs/promises';
import path from 'node:path';

import { PROTOCOL_LIMITS } from '@orchbee/terminal-protocol';

import {
	TerminalRecoveryLedgerDocumentSchema,
	type TerminalRecoveryLedger,
	type TerminalRecoveryLedgerDocument,
	type TerminalRecoveryLedgerLoadResult,
} from './recovery-binding';

const MAX_LEDGER_BYTES = PROTOCOL_LIMITS.maxControlJsonBytes * 2;

export interface FileTerminalRecoveryLedgerDependencies {
	readonly randomToken: () => string;
}

const defaultDependencies: FileTerminalRecoveryLedgerDependencies = {
	randomToken: () => randomBytes(16).toString('hex'),
};

export class FileTerminalRecoveryLedger implements TerminalRecoveryLedger {
	readonly #dependencies: FileTerminalRecoveryLedgerDependencies;
	readonly #filePath: string;

	constructor(
		filePath: string,
		dependencies: FileTerminalRecoveryLedgerDependencies = defaultDependencies,
	) {
		this.#filePath = path.resolve(filePath);
		this.#dependencies = dependencies;
	}

	async load(): Promise<TerminalRecoveryLedgerLoadResult> {
		try {
			const metadata = await stat(this.#filePath);
			if (!metadata.isFile() || metadata.size > MAX_LEDGER_BYTES) {
				return { kind: 'unavailable', reason: 'invalid' };
			}
			const encoded = await readFile(this.#filePath);
			if (encoded.byteLength > MAX_LEDGER_BYTES) {
				return { kind: 'unavailable', reason: 'invalid' };
			}
			let value: unknown;
			try {
				value = JSON.parse(encoded.toString('utf8'));
			} catch {
				return { kind: 'unavailable', reason: 'invalid' };
			}
			const parsed = TerminalRecoveryLedgerDocumentSchema.safeParse(value);
			return parsed.success
				? { document: parsed.data, kind: 'available' }
				: { kind: 'unavailable', reason: 'invalid' };
		} catch (error) {
			if (isNodeError(error) && error.code === 'ENOENT') return { kind: 'missing' };
			return { kind: 'unavailable', reason: 'read-failed' };
		}
	}

	async save(document: TerminalRecoveryLedgerDocument): Promise<void> {
		const canonical = TerminalRecoveryLedgerDocumentSchema.parse(document);
		const encoded = Buffer.from(`${JSON.stringify(canonical)}\n`, 'utf8');
		if (encoded.byteLength > MAX_LEDGER_BYTES) {
			throw new Error('Terminal recovery ledger exceeds its bounded size.');
		}
		const directory = path.dirname(this.#filePath);
		await mkdir(directory, { mode: 0o700, recursive: true });
		if (process.platform !== 'win32') await chmod(directory, 0o700);
		const temporaryPath = `${this.#filePath}.${this.#dependencies.randomToken()}.tmp`;
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

function isNodeError(error: unknown): error is NodeJS.ErrnoException {
	return error instanceof Error && 'code' in error;
}
