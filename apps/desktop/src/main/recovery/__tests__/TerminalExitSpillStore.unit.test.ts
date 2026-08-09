import { mkdir, mkdtemp, readFile, readdir, rm, stat, writeFile } from 'node:fs/promises';
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

import { TerminalExitSpillStore } from '../TerminalExitSpillStore';
import { TerminalExitPublicationLedger } from '../TerminalExitPublicationLedger';

const temporaryDirectories: string[] = [];

afterEach(async () => {
	await Promise.all(
		temporaryDirectories.splice(0).map(async (directory) => rm(directory, { recursive: true })),
	);
});

describe('TerminalExitSpillStore', () => {
	it('persists the newest identity revision and reloads it as the durable owner', async () => {
		const directory = await mkdtemp(path.join(tmpdir(), 'orchbee-terminal-exit-spill-'));
		temporaryDirectories.push(directory);
		const filePath = path.join(directory, 'recovery', 'terminal-exits-v1.json');
		const first = terminalExit('1');
		const newest = terminalExit('2');
		const store = new TerminalExitSpillStore(filePath, () => 'temporary');

		expect(await store.load()).toEqual({ kind: 'ready' });
		expect(store.admit(first, 'queue-full')).toEqual({ kind: 'admitted' });
		expect(store.admit(newest, 'shutdown')).toEqual({ kind: 'admitted' });
		expect(store.admit(first, 'unknown-identity')).toEqual({ kind: 'admitted' });
		expect(store.list()).toEqual([{ event: newest, reason: 'shutdown', state: 'pending' }]);
		store.remove(first);
		expect(store.list()).toEqual([{ event: newest, reason: 'shutdown', state: 'pending' }]);

		const reloaded = new TerminalExitSpillStore(filePath);
		expect(await reloaded.load()).toEqual({ kind: 'ready' });
		expect(reloaded.list()).toEqual([{ event: newest, reason: 'shutdown', state: 'pending' }]);
		expect(await readFile(filePath, 'utf8')).toContain('"version":2');
		expect(await readdir(path.dirname(filePath))).toEqual(['terminal-exits-v1.json']);
		expect((await stat(filePath)).mode & 0o777).toBe(0o600);
		expect((await stat(path.dirname(filePath))).mode & 0o777).toBe(0o700);
	});

	it('preserves a durable rejection separately from pending ownership', async () => {
		const directory = await mkdtemp(path.join(tmpdir(), 'orchbee-terminal-exit-spill-'));
		temporaryDirectories.push(directory);
		const filePath = path.join(directory, 'recovery', 'terminal-exits-v1.json');
		await mkdir(path.dirname(filePath), { recursive: true });
		const events = Array.from({ length: 4096 }, (_, index) => ({
			event: terminalExitFor(index, '1'),
			reason: 'unknown-identity' as const,
			state: 'pending' as const,
		}));
		await writeFile(filePath, JSON.stringify({ events, version: 1 }));
		const store = new TerminalExitSpillStore(filePath);

		expect(await store.load()).toEqual({ kind: 'ready' });
		expect(store.admit(terminalExitFor(4096, '1'), 'unknown-identity')).toEqual({ kind: 'full' });
		expect(store.reject(terminalExitFor(4096, '1'), 'pending-journal-full')).toEqual({
			kind: 'recorded',
		});
		expect(store.listRejected()).toEqual([
			{ event: terminalExitFor(4096, '1'), reason: 'pending-journal-full' },
		]);
	});

	it('normalizes pending and rejected states by identity and newest revision', async () => {
		const directory = await mkdtemp(path.join(tmpdir(), 'orchbee-terminal-exit-spill-'));
		temporaryDirectories.push(directory);
		const filePath = path.join(directory, 'recovery', 'terminal-exits-v1.json');
		const event1 = terminalExit('1');
		const event2 = terminalExit('2');
		const event3 = terminalExit('3');
		const store = new TerminalExitSpillStore(filePath);

		expect(await store.load()).toEqual({ kind: 'ready' });
		expect(store.admit(event2, 'queue-full')).toEqual({ kind: 'admitted' });
		expect(store.reject(event1, 'write-failed')).toEqual({ kind: 'recorded' });
		expect(store.list()).toEqual([{ event: event2, reason: 'queue-full', state: 'pending' }]);
		expect(store.reject(event3, 'write-failed')).toEqual({ kind: 'recorded' });
		expect(store.list()).toEqual([]);
		expect(store.admit(event3, 'shutdown')).toEqual({ kind: 'admitted' });
		expect(store.listRejected()).toEqual([]);

		const reloaded = new TerminalExitSpillStore(filePath);
		expect(await reloaded.load()).toEqual({ kind: 'ready' });
		expect(reloaded.list()).toEqual([{ event: event3, reason: 'shutdown', state: 'pending' }]);
		expect(reloaded.listRejected()).toEqual([]);
	});

	it('drops legacy published records only when the loaded ledger confirms them', async () => {
		const directory = await mkdtemp(path.join(tmpdir(), 'orchbee-terminal-exit-spill-'));
		temporaryDirectories.push(directory);
		const filePath = path.join(directory, 'recovery', 'terminal-exits-v1.json');
		await mkdir(path.dirname(filePath), { recursive: true });
		const event = terminalExit('10');
		await writeFile(
			filePath,
			JSON.stringify({ events: [{ event, reason: 'queue-full', state: 'published' }], version: 1 }),
		);

		const ledger = new TerminalExitPublicationLedger(
			path.join(directory, 'recovery', 'published.json'),
		);
		expect(await ledger.load()).toEqual({ kind: 'ready' });
		expect(ledger.markPublished(event)).toBe(true);
		const store = new TerminalExitSpillStore(filePath);
		expect(await store.load(ledger)).toEqual({ kind: 'ready' });
		expect(store.list()).toEqual([]);
		expect(await readFile(filePath, 'utf8')).toContain('"version":2');

		const reloaded = new TerminalExitSpillStore(filePath);
		expect(await reloaded.load(ledger)).toEqual({ kind: 'ready' });
		expect(reloaded.list()).toEqual([]);
	});

	it('fails closed for an older legacy published revision and keeps the v1 file unchanged', async () => {
		const directory = await mkdtemp(path.join(tmpdir(), 'orchbee-terminal-exit-spill-'));
		temporaryDirectories.push(directory);
		const filePath = path.join(directory, 'recovery', 'terminal-exits-v1.json');
		await mkdir(path.dirname(filePath), { recursive: true });
		const recorded = terminalExit('10');
		const older = terminalExit('9');
		const legacy = JSON.stringify({
			events: [{ event: older, reason: 'queue-full', state: 'published' }],
			version: 1,
		});
		await writeFile(filePath, legacy);

		const ledger = new TerminalExitPublicationLedger(path.join(directory, 'published.json'));
		expect(await ledger.load()).toEqual({ kind: 'ready' });
		expect(ledger.markPublished(recorded)).toBe(true);
		const store = new TerminalExitSpillStore(filePath);
		expect(await store.load(ledger)).toEqual({ kind: 'unavailable', reason: 'invalid' });
		expect(await readFile(filePath, 'utf8')).toBe(legacy);
		expect(() => store.list()).toThrow('not loaded');

		const exact = JSON.stringify({
			events: [{ event: recorded, reason: 'queue-full', state: 'published' }],
			version: 1,
		});
		await writeFile(filePath, exact);
		expect(await store.load(ledger)).toEqual({ kind: 'ready' });
		expect(store.list()).toEqual([]);
		expect(await readFile(filePath, 'utf8')).toContain('"version":2');
	});

	it('fails closed for legacy published records without ledger evidence', async () => {
		const directory = await mkdtemp(path.join(tmpdir(), 'orchbee-terminal-exit-spill-'));
		temporaryDirectories.push(directory);
		const filePath = path.join(directory, 'recovery', 'terminal-exits-v1.json');
		await mkdir(path.dirname(filePath), { recursive: true });
		const event = terminalExit('7');
		await writeFile(
			filePath,
			JSON.stringify({ events: [{ event, reason: 'queue-full', state: 'published' }], version: 1 }),
		);

		const store = new TerminalExitSpillStore(filePath);
		expect(await store.load()).toEqual({ kind: 'unavailable', reason: 'invalid' });
		expect(await readFile(filePath, 'utf8')).toContain('"version":1');
	});

	it('rejects an oversized legacy event array before migration', async () => {
		const directory = await mkdtemp(path.join(tmpdir(), 'orchbee-terminal-exit-spill-'));
		temporaryDirectories.push(directory);
		const filePath = path.join(directory, 'recovery', 'terminal-exits-v1.json');
		await mkdir(path.dirname(filePath), { recursive: true });
		const events = Array.from({ length: 4097 }, (_, index) => ({
			event: terminalExitFor(index, '1'),
			reason: 'queue-full' as const,
			state: 'pending' as const,
		}));
		await writeFile(filePath, JSON.stringify({ events, version: 1 }));

		const store = new TerminalExitSpillStore(filePath);
		expect(await store.load()).toEqual({ kind: 'unavailable', reason: 'invalid' });
	});

	it('stays unavailable after a failed reload of an already loaded instance', async () => {
		const directory = await mkdtemp(path.join(tmpdir(), 'orchbee-terminal-exit-spill-'));
		temporaryDirectories.push(directory);
		const filePath = path.join(directory, 'recovery', 'terminal-exits-v1.json');
		const store = new TerminalExitSpillStore(filePath);

		expect(await store.load()).toEqual({ kind: 'ready' });
		expect(store.admit(terminalExit('1'), 'queue-full')).toEqual({ kind: 'admitted' });
		await writeFile(filePath, '{not-json');
		expect(await store.load()).toEqual({ kind: 'unavailable', reason: 'invalid' });
		expect(() => store.list()).toThrow('not loaded');
		expect(() => store.listRejected()).toThrow('not loaded');
	});

	it('stays unavailable when legacy migration persistence fails', async () => {
		const directory = await mkdtemp(path.join(tmpdir(), 'orchbee-terminal-exit-spill-'));
		temporaryDirectories.push(directory);
		const filePath = path.join(directory, 'recovery', 'terminal-exits-v1.json');
		await mkdir(path.dirname(filePath), { recursive: true });
		const event = terminalExit('4');
		const ledger = new TerminalExitPublicationLedger(path.join(directory, 'published.json'));
		expect(await ledger.load()).toEqual({ kind: 'ready' });
		expect(ledger.markPublished(event)).toBe(true);
		await writeFile(
			filePath,
			JSON.stringify({ events: [{ event, reason: 'queue-full', state: 'published' }], version: 1 }),
		);
		const randomToken = 'migration-collision';
		await writeFile(`${filePath}.${randomToken}.tmp`, 'occupied');
		const store = new TerminalExitSpillStore(filePath, () => randomToken);

		expect(await store.load(ledger)).toEqual({ kind: 'unavailable', reason: 'read-failed' });
		expect(() => store.list()).toThrow('not loaded');
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
