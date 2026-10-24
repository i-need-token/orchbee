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

const MAX_PENDING_TERMINAL_EXITS = 4096;
const MAX_REJECTED_TERMINAL_EXITS = 4096;

const TerminalExitSpillReasonSchema = z.enum([
	'association-pending',
	'host-admission',
	'queue-full',
	'shutdown',
	'unknown-identity',
]);
const TerminalExitRejectionReasonSchema = z.enum(['pending-journal-full', 'write-failed']);
const TerminalExitSpillEntrySchema = z.strictObject({
	event: TerminalExitedSchema,
	reason: TerminalExitSpillReasonSchema,
});
const TerminalExitRejectionEntrySchema = z.strictObject({
	event: TerminalExitedSchema,
	reason: TerminalExitRejectionReasonSchema,
});
const TerminalExitSpillFileSchema = z.strictObject({
	pending: z.array(TerminalExitSpillEntrySchema).max(MAX_PENDING_TERMINAL_EXITS),
	rejected: z.array(TerminalExitRejectionEntrySchema).max(MAX_REJECTED_TERMINAL_EXITS),
	version: z.literal(1),
});
export type TerminalExitSpillReason = z.infer<typeof TerminalExitSpillReasonSchema>;

type TerminalExitSpillLoadResult =
	| { readonly kind: 'ready' }
	| { readonly kind: 'unavailable'; readonly reason: 'invalid' | 'read-failed' };

type TerminalExitSpillAdmission = { readonly kind: 'admitted' } | { readonly kind: 'full' };
export type TerminalExitRejectionReason = z.infer<typeof TerminalExitRejectionReasonSchema>;

interface TerminalExitSpillEntry {
	readonly event: TerminalExited;
	readonly reason: TerminalExitSpillReason;
}

export interface TerminalExitSpillRejectionEntry {
	readonly event: TerminalExited;
	readonly reason: TerminalExitRejectionReason;
}

export class TerminalExitSpillStore {
	readonly #filePath: string;
	readonly #randomToken: () => string;
	readonly #entries = new Map<string, TerminalExitSpillEntry>();
	readonly #rejections = new Map<string, TerminalExitSpillRejectionEntry>();
	#loaded = false;

	constructor(filePath: string, randomToken: () => string = () => randomBytes(16).toString('hex')) {
		this.#filePath = path.resolve(filePath);
		this.#randomToken = randomToken;
	}

	async load(): Promise<TerminalExitSpillLoadResult> {
		// A failed reload must never leave callers using a partially replaced
		// in-memory state from an earlier successful load.
		this.#loaded = false;
		try {
			const encoded = await readFile(this.#filePath);
			let value: unknown;
			try {
				value = JSON.parse(new TextDecoder('utf-8', { fatal: true }).decode(encoded));
			} catch {
				return { kind: 'unavailable', reason: 'invalid' };
			}
			const parsed = TerminalExitSpillFileSchema.safeParse(value);
			if (!parsed.success) return { kind: 'unavailable', reason: 'invalid' };
			const entries = new Map<string, TerminalExitSpillEntry>();
			const rejections = new Map<string, TerminalExitSpillRejectionEntry>();
			for (const entry of parsed.data.pending) this.#setNewestPending(entries, entry);
			for (const rejection of parsed.data.rejected) this.#setNewestRejection(rejections, rejection);
			this.#normalizeStates(entries, rejections);
			if (
				entries.size > MAX_PENDING_TERMINAL_EXITS ||
				rejections.size > MAX_REJECTED_TERMINAL_EXITS
			) {
				return { kind: 'unavailable', reason: 'invalid' };
			}
			this.#replaceEntries(entries);
			this.#replaceRejections(rejections);
			this.#loaded = true;
			return { kind: 'ready' };
		} catch (error: unknown) {
			if (isNodeError(error) && error.code === 'ENOENT') {
				this.#replaceEntries(new Map());
				this.#replaceRejections(new Map());
				this.#loaded = true;
				return { kind: 'ready' };
			}
			return { kind: 'unavailable', reason: 'read-failed' };
		}
	}

	list(): readonly TerminalExitSpillEntry[] {
		if (!this.#loaded) throw new Error('Terminal exit spill store is not loaded.');
		return [...this.#entries.values()].map((entry) => ({ ...entry, state: 'pending' as const }));
	}

	listRejected(): readonly TerminalExitSpillRejectionEntry[] {
		if (!this.#loaded) throw new Error('Terminal exit spill store is not loaded.');
		return [...this.#rejections.values()];
	}

	isRejected(event: TerminalExited): boolean {
		if (!this.#loaded) throw new Error('Terminal exit spill store is not loaded.');
		const existing = this.#rejections.get(terminalExitIdentityKey(event));
		return (
			existing !== undefined && BigInt(existing.event.finalRevision) >= BigInt(event.finalRevision)
		);
	}

	admit(event: TerminalExited, reason: TerminalExitSpillReason): TerminalExitSpillAdmission {
		if (!this.#loaded) throw new Error('Terminal exit spill store is not loaded.');
		const canonical = TerminalExitedSchema.parse(event);
		const key = terminalExitIdentityKey(canonical);
		const existing = this.#entries.get(key);
		const rejected = this.#rejections.get(key);
		if (
			(rejected !== undefined &&
				BigInt(rejected.event.finalRevision) > BigInt(canonical.finalRevision)) ||
			(existing !== undefined &&
				BigInt(existing.event.finalRevision) > BigInt(canonical.finalRevision))
		) {
			return { kind: 'admitted' };
		}
		if (existing === undefined && this.#entries.size >= MAX_PENDING_TERMINAL_EXITS) {
			return { kind: 'full' };
		}
		if (
			existing !== undefined &&
			BigInt(existing.event.finalRevision) === BigInt(canonical.finalRevision) &&
			existing.reason === reason
		) {
			return { kind: 'admitted' };
		}
		const nextEntries = new Map(this.#entries);
		nextEntries.set(key, { event: canonical, reason });
		const nextRejections = new Map(this.#rejections);
		nextRejections.delete(key);
		this.#persist(nextEntries, nextRejections);
		this.#replaceEntries(nextEntries);
		this.#replaceRejections(nextRejections);
		return { kind: 'admitted' };
	}

	reject(
		event: TerminalExited,
		reason: TerminalExitRejectionReason,
	): { readonly kind: 'recorded' } | { readonly kind: 'full' } {
		if (!this.#loaded) throw new Error('Terminal exit spill store is not loaded.');
		const canonical = TerminalExitedSchema.parse(event);
		const key = terminalExitIdentityKey(canonical);
		const pending = this.#entries.get(key);
		const existing = this.#rejections.get(key);
		if (
			pending !== undefined &&
			BigInt(pending.event.finalRevision) >= BigInt(canonical.finalRevision)
		) {
			return { kind: 'recorded' };
		}
		if (
			existing !== undefined &&
			BigInt(existing.event.finalRevision) >= BigInt(canonical.finalRevision)
		) {
			return { kind: 'recorded' };
		}
		if (existing === undefined && this.#rejections.size >= MAX_REJECTED_TERMINAL_EXITS) {
			return { kind: 'full' };
		}
		const nextEntries = new Map(this.#entries);
		nextEntries.delete(key);
		const nextRejections = new Map(this.#rejections);
		nextRejections.set(key, { event: canonical, reason });
		this.#persist(nextEntries, nextRejections);
		this.#replaceEntries(nextEntries);
		this.#replaceRejections(nextRejections);
		return { kind: 'recorded' };
	}

	remove(eventOrIdentity: TerminalExited | string): void {
		if (!this.#loaded) throw new Error('Terminal exit spill store is not loaded.');
		const key =
			typeof eventOrIdentity === 'string'
				? eventOrIdentity
				: terminalExitIdentityKey(eventOrIdentity);
		const existing = this.#entries.get(key);
		if (
			existing === undefined ||
			(typeof eventOrIdentity !== 'string' &&
				BigInt(existing.event.finalRevision) > BigInt(eventOrIdentity.finalRevision))
		) {
			return;
		}
		const nextEntries = new Map(this.#entries);
		nextEntries.delete(key);
		this.#persist(nextEntries, this.#rejections);
		this.#replaceEntries(nextEntries);
	}

	#persist(
		entries: ReadonlyMap<string, TerminalExitSpillEntry>,
		rejections: ReadonlyMap<string, TerminalExitSpillRejectionEntry>,
	): void {
		const document = TerminalExitSpillFileSchema.parse({
			pending: [...entries.values()],
			rejected: [...rejections.values()],
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
	}

	#replaceEntries(entries: ReadonlyMap<string, TerminalExitSpillEntry>): void {
		this.#entries.clear();
		for (const [key, entry] of entries) this.#entries.set(key, entry);
	}

	#replaceRejections(rejections: ReadonlyMap<string, TerminalExitSpillRejectionEntry>): void {
		this.#rejections.clear();
		for (const [key, entry] of rejections) this.#rejections.set(key, entry);
	}

	#setNewestPending(
		entries: Map<string, TerminalExitSpillEntry>,
		entry: TerminalExitSpillEntry,
	): void {
		const key = terminalExitIdentityKey(entry.event);
		const existing = entries.get(key);
		if (
			existing === undefined ||
			BigInt(entry.event.finalRevision) > BigInt(existing.event.finalRevision)
		) {
			entries.set(key, entry);
		}
	}

	#setNewestRejection(
		rejections: Map<string, TerminalExitSpillRejectionEntry>,
		entry: TerminalExitSpillRejectionEntry,
	): void {
		const key = terminalExitIdentityKey(entry.event);
		const existing = rejections.get(key);
		if (
			existing === undefined ||
			BigInt(entry.event.finalRevision) > BigInt(existing.event.finalRevision)
		) {
			rejections.set(key, entry);
		}
	}

	#normalizeStates(
		entries: Map<string, TerminalExitSpillEntry>,
		rejections: Map<string, TerminalExitSpillRejectionEntry>,
	): void {
		for (const key of new Set([...entries.keys(), ...rejections.keys()])) {
			const pending = entries.get(key);
			const rejected = rejections.get(key);
			if (pending === undefined || rejected === undefined) continue;
			// Equal revisions prefer pending: an admission is the only state that
			// can make a rejected identity retryable again.
			if (BigInt(pending.event.finalRevision) >= BigInt(rejected.event.finalRevision)) {
				rejections.delete(key);
			} else {
				entries.delete(key);
			}
		}
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
