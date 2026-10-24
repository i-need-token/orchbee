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

const temporaryDirectories: string[] = [];

afterEach(async () => {
	await Promise.all(
		temporaryDirectories.splice(0).map(async (directory) => rm(directory, { recursive: true })),
	);
});

describe('TerminalExitSpillStore', () => {
	it.runIf(process.platform !== 'win32')(
		'protects POSIX journal files and directories with owner-only modes',
		async () => {
			const directory = await mkdtemp(path.join(tmpdir(), 'orchbee-terminal-exit-spill-'));
			temporaryDirectories.push(directory);
			const filePath = path.join(directory, 'recovery', 'terminal-exits-v1.json');
			const store = new TerminalExitSpillStore(filePath);
			await store.load();
			expect(store.admit(terminalExit('1'), 'queue-full')).toEqual({ kind: 'admitted' });
			expect((await stat(filePath)).mode & 0o777).toBe(0o600);
			expect((await stat(path.dirname(filePath))).mode & 0o777).toBe(0o700);
		},
	);
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
		expect(await readFile(filePath, 'utf8')).toContain('"version":1');
		expect(await readdir(path.dirname(filePath))).toEqual(['terminal-exits-v1.json']);
	});

	it('preserves a durable rejection separately from pending ownership', async () => {
		const directory = await mkdtemp(path.join(tmpdir(), 'orchbee-terminal-exit-spill-'));
		temporaryDirectories.push(directory);
		const filePath = path.join(directory, 'recovery', 'terminal-exits-v1.json');
		await mkdir(path.dirname(filePath), { recursive: true });
		const pending = Array.from({ length: 4096 }, (_, index) => ({
			event: terminalExitFor(index, '1'),
			reason: 'unknown-identity' as const,
		}));
		await writeFile(filePath, JSON.stringify({ pending, rejected: [], version: 1 }));
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

	it.each([
		'{not-json',
		JSON.stringify({
			events: [{ event: terminalExit('1'), reason: 'queue-full', state: 'published' }],
			version: 1,
		}),
		JSON.stringify({ pending: [], rejected: [], version: 2 }),
	])(
		'preserves invalid records and stays unavailable after a failed reload: %s',
		async (invalid) => {
			const directory = await mkdtemp(path.join(tmpdir(), 'orchbee-terminal-exit-spill-'));
			temporaryDirectories.push(directory);
			const filePath = path.join(directory, 'recovery', 'terminal-exits-v1.json');
			const store = new TerminalExitSpillStore(filePath);

			expect(await store.load()).toEqual({ kind: 'ready' });
			expect(store.admit(terminalExit('1'), 'queue-full')).toEqual({ kind: 'admitted' });
			await writeFile(filePath, invalid);
			expect(await store.load()).toEqual({ kind: 'unavailable', reason: 'invalid' });
			expect(() => store.list()).toThrow('not loaded');
			expect(() => store.listRejected()).toThrow('not loaded');
			expect(await readFile(filePath, 'utf8')).toBe(invalid);
		},
	);
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
