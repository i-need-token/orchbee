import { randomBytes } from 'node:crypto';
import {
	chmodSync,
	closeSync,
	fsyncSync,
	mkdirSync,
	openSync,
	renameSync,
	unlinkSync,
	writeSync,
} from 'node:fs';
import { readFile } from 'node:fs/promises';
import path from 'node:path';

import { TerminalExitedSchema, type TerminalExited } from '@orchbee/terminal-protocol';
import { z } from 'zod';

const MAX_TERMINAL_EXIT_PUBLICATION_ENTRIES = 4096;

const TerminalExitPublicationFileSchema = z.strictObject({
	events: z.array(TerminalExitedSchema).max(MAX_TERMINAL_EXIT_PUBLICATION_ENTRIES),
	version: z.literal(1),
});

export type TerminalExitPublicationLedgerLoadResult =
	| { readonly kind: 'ready' }
	| { readonly kind: 'unavailable'; readonly reason: 'invalid' | 'read-failed' };

export interface TerminalExitPublicationEntry {
	readonly event: TerminalExited;
}

export class TerminalExitPublicationLedger {
	readonly #entries = new Map<string, TerminalExitPublicationEntry>();
	readonly #filePath: string;
	readonly #randomToken: () => string;
	#loaded = false;

	constructor(filePath: string, randomToken: () => string = () => randomBytes(16).toString('hex')) {
		this.#filePath = path.resolve(filePath);
		this.#randomToken = randomToken;
	}

	async load(): Promise<TerminalExitPublicationLedgerLoadResult> {
		this.#loaded = false;
		try {
			const encoded = await readFile(this.#filePath);
			let value: unknown;
			try {
				value = JSON.parse(new TextDecoder('utf-8', { fatal: true }).decode(encoded));
			} catch {
				return { kind: 'unavailable', reason: 'invalid' };
			}
			const parsed = TerminalExitPublicationFileSchema.safeParse(value);
			if (!parsed.success) return { kind: 'unavailable', reason: 'invalid' };
			const entries = new Map<string, TerminalExitPublicationEntry>();
			for (const event of parsed.data.events) {
				const key = terminalExitIdentityKey(event);
				const existing = entries.get(key);
				if (
					existing === undefined ||
					BigInt(event.finalRevision) > BigInt(existing.event.finalRevision)
				) {
					entries.set(key, { event });
				}
			}
			this.#replaceEntries(entries);
			this.#loaded = true;
			return { kind: 'ready' };
		} catch (error: unknown) {
			if (isNodeError(error) && error.code === 'ENOENT') {
				this.#replaceEntries(new Map());
				this.#loaded = true;
				return { kind: 'ready' };
			}
			return { kind: 'unavailable', reason: 'read-failed' };
		}
	}

	list(): readonly TerminalExitPublicationEntry[] {
		if (!this.#loaded) throw new Error('Terminal exit publication ledger is not loaded.');
		return [...this.#entries.values()];
	}

	has(event: TerminalExited): boolean {
		if (!this.#loaded) throw new Error('Terminal exit publication ledger is not loaded.');
		const existing = this.#entries.get(terminalExitIdentityKey(event));
		// Runtime replay/deduplication accepts an older event once a newer
		// publication for the same identity is durable. Migration evidence must
		// use hasExact instead; it cannot infer publication of another revision.
		return (
			existing !== undefined && BigInt(existing.event.finalRevision) >= BigInt(event.finalRevision)
		);
	}

	hasExact(event: TerminalExited): boolean {
		if (!this.#loaded) throw new Error('Terminal exit publication ledger is not loaded.');
		const existing = this.#entries.get(terminalExitIdentityKey(event));
		return (
			existing !== undefined && BigInt(existing.event.finalRevision) === BigInt(event.finalRevision)
		);
	}

	markPublished(event: TerminalExited): boolean {
		if (!this.#loaded) throw new Error('Terminal exit publication ledger is not loaded.');
		const canonical = TerminalExitedSchema.parse(event);
		const key = terminalExitIdentityKey(canonical);
		const existing = this.#entries.get(key);
		if (existing !== undefined) {
			const existingRevision = BigInt(existing.event.finalRevision);
			const eventRevision = BigInt(canonical.finalRevision);
			if (existingRevision >= eventRevision) return true;
		}
		if (existing === undefined && this.#entries.size >= MAX_TERMINAL_EXIT_PUBLICATION_ENTRIES) {
			return false;
		}
		const nextEntries = new Map(this.#entries);
		nextEntries.set(key, { event: canonical });
		this.#persist(nextEntries);
		return true;
	}

	#persist(entries: ReadonlyMap<string, TerminalExitPublicationEntry>): void {
		const document = TerminalExitPublicationFileSchema.parse({
			events: [...entries.values()].map((entry) => entry.event),
			version: 1,
		});
		const encoded = Buffer.from(`${JSON.stringify(document)}\n`, 'utf8');
		const directory = path.dirname(this.#filePath);
		mkdirSync(directory, { mode: 0o700, recursive: true });
		if (process.platform !== 'win32') chmodSync(directory, 0o700);
		const temporaryPath = `${this.#filePath}.${this.#randomToken()}.tmp`;
		let temporaryCreated = false;
		let fileDescriptor: number | undefined;
		try {
			fileDescriptor = openSync(temporaryPath, 'wx', 0o600);
			temporaryCreated = true;
			writeAll(fileDescriptor, encoded);
			fsyncSync(fileDescriptor);
			closeSync(fileDescriptor);
			fileDescriptor = undefined;
			renameSync(temporaryPath, this.#filePath);
			if (process.platform !== 'win32') {
				chmodSync(this.#filePath, 0o600);
				const directoryDescriptor = openSync(directory, 'r');
				try {
					fsyncSync(directoryDescriptor);
				} finally {
					closeSync(directoryDescriptor);
				}
			}
		} catch (error: unknown) {
			if (fileDescriptor !== undefined) closeSync(fileDescriptor);
			if (temporaryCreated) {
				try {
					unlinkSync(temporaryPath);
				} catch {
					// Preserve the original durable-write failure.
				}
			}
			throw error;
		}
		this.#replaceEntries(entries);
	}

	#replaceEntries(entries: ReadonlyMap<string, TerminalExitPublicationEntry>): void {
		this.#entries.clear();
		for (const [key, entry] of entries) this.#entries.set(key, entry);
	}
}

function terminalExitIdentityKey(event: TerminalExited): string {
	return `${event.identity.hostEpoch}:${event.identity.creationId}:${event.identity.terminalId}`;
}

function isNodeError(error: unknown): error is NodeJS.ErrnoException {
	return error instanceof Error && 'code' in error;
}

function writeAll(fileDescriptor: number, bytes: Uint8Array): void {
	let offset = 0;
	while (offset < bytes.byteLength) offset += writeSync(fileDescriptor, bytes, offset);
}
