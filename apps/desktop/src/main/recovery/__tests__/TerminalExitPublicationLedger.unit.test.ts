import { mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';

import {
	CanonicalUint64TextSchema,
	CreationIdSchema,
	HostEpochSchema,
	TerminalIdSchema,
	type TerminalExited,
} from '@orchbee/terminal-protocol';
import { afterEach, describe, expect, it } from 'vitest';

import { TerminalExitPublicationLedger } from '../TerminalExitPublicationLedger';

const temporaryDirectories: string[] = [];

afterEach(async () => {
	await Promise.all(
		temporaryDirectories.splice(0).map(async (directory) => rm(directory, { recursive: true })),
	);
});

describe('TerminalExitPublicationLedger', () => {
	it('persists one idempotent publication fact independently of pending spill state', async () => {
		const directory = await mkdtemp(path.join(tmpdir(), 'orchbee-terminal-exit-publication-'));
		temporaryDirectories.push(directory);
		const filePath = path.join(directory, 'recovery', 'published-v1.json');
		const event = terminalExit('7');
		const ledger = new TerminalExitPublicationLedger(filePath);

		expect(await ledger.load()).toEqual({ kind: 'ready' });
		expect(ledger.markPublished(event)).toBe(true);
		expect(ledger.markPublished(event)).toBe(true);
		expect(ledger.has(event)).toBe(true);
		expect(ledger.hasExact(event)).toBe(true);
		expect(ledger.list()).toEqual([{ event }]);
		expect(await readFile(filePath, 'utf8')).toContain('"version":1');

		const reloaded = new TerminalExitPublicationLedger(filePath);
		expect(await reloaded.load()).toEqual({ kind: 'ready' });
		expect(reloaded.has(event)).toBe(true);
	});

	it('keeps runtime newer-revision matching separate from exact migration evidence', async () => {
		const directory = await mkdtemp(path.join(tmpdir(), 'orchbee-terminal-exit-publication-'));
		temporaryDirectories.push(directory);
		const ledger = new TerminalExitPublicationLedger(path.join(directory, 'published.json'));
		const latest = terminalExit('10');
		const older = terminalExit('9');

		expect(await ledger.load()).toEqual({ kind: 'ready' });
		expect(ledger.markPublished(latest)).toBe(true);
		expect(ledger.has(older)).toBe(true);
		expect(ledger.hasExact(older)).toBe(false);
		expect(ledger.hasExact(latest)).toBe(true);
		expect(ledger.hasExact(terminalExitFor(1, '10'))).toBe(false);
		expect(ledger.list()).toEqual([{ event: latest }]);
	});

	it('becomes unloaded when a reload contains invalid data', async () => {
		const directory = await mkdtemp(path.join(tmpdir(), 'orchbee-terminal-exit-publication-'));
		temporaryDirectories.push(directory);
		const filePath = path.join(directory, 'published.json');
		const event = terminalExit('10');
		await writeFile(filePath, JSON.stringify({ events: [event], version: 1 }));
		const ledger = new TerminalExitPublicationLedger(filePath);

		expect(await ledger.load()).toEqual({ kind: 'ready' });
		await writeFile(filePath, '{ invalid');

		expect(await ledger.load()).toEqual({ kind: 'unavailable', reason: 'invalid' });
		expect(() => ledger.list()).toThrow('not loaded');
		expect(() => ledger.has(event)).toThrow('not loaded');
		expect(() => ledger.hasExact(event)).toThrow('not loaded');
	});

	it('becomes unloaded when a reload cannot read the file', async () => {
		const directory = await mkdtemp(path.join(tmpdir(), 'orchbee-terminal-exit-publication-'));
		temporaryDirectories.push(directory);
		const filePath = path.join(directory, 'published.json');
		const event = terminalExit('10');
		await writeFile(filePath, JSON.stringify({ events: [event], version: 1 }));
		const ledger = new TerminalExitPublicationLedger(filePath);

		expect(await ledger.load()).toEqual({ kind: 'ready' });
		await rm(filePath);
		await mkdir(filePath);

		expect(await ledger.load()).toEqual({ kind: 'unavailable', reason: 'read-failed' });
		expect(() => ledger.list()).toThrow('not loaded');
		expect(() => ledger.has(event)).toThrow('not loaded');
		expect(() => ledger.hasExact(event)).toThrow('not loaded');
	});

	it('retains the durable pending owner when the publication ledger is full', async () => {
		const directory = await mkdtemp(path.join(tmpdir(), 'orchbee-terminal-exit-publication-'));
		temporaryDirectories.push(directory);
		const filePath = path.join(directory, 'recovery', 'published-v1.json');
		const events = Array.from({ length: 4096 }, (_, index) => terminalExitFor(index, '1'));
		await mkdir(path.dirname(filePath), { recursive: true });
		await writeFile(filePath, JSON.stringify({ events, version: 1 }));
		const ledger = new TerminalExitPublicationLedger(filePath);

		expect(await ledger.load()).toEqual({ kind: 'ready' });
		expect(ledger.markPublished(terminalExitFor(4096, '1'))).toBe(false);
		expect(ledger.has(terminalExitFor(4096, '1'))).toBe(false);
	});
});

function terminalExit(finalRevision: string): TerminalExited {
	return terminalExitFor(0, finalRevision);
}

function terminalExitFor(index: number, finalRevision: string): TerminalExited {
	const creationSuffix = (index + 1).toString(16).padStart(32, '0');
	return {
		exit: { code: 1, kind: 'exit-code' },
		finalRevision: CanonicalUint64TextSchema.parse(finalRevision),
		identity: {
			creationId: CreationIdSchema.parse(creationSuffix),
			hostEpoch: HostEpochSchema.parse('22'.repeat(16)),
			terminalId: TerminalIdSchema.parse((index + 1).toString(16).padStart(32, '0')),
		},
	};
}
