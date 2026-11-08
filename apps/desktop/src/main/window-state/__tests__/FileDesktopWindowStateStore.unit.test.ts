import { mkdir, mkdtemp, readdir, readFile, rm, stat, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';

import { afterEach, describe, expect, it } from 'vitest';

import { createUpgradeBackup } from '../../upgrade-backup';
import {
	createDesktopWindowState,
	type DesktopWindowStateMigrationStep,
} from '../desktop-window-state';
import {
	FileDesktopWindowStateStore,
	type FileDesktopWindowStateStoreOptions,
} from '../FileDesktopWindowStateStore';

const PRODUCT_VERSION = '1.2.3';

const temporaryDirectories: string[] = [];

afterEach(async () => {
	await Promise.all(
		temporaryDirectories.splice(0).map(async (directory) => rm(directory, { recursive: true })),
	);
});

describe('FileDesktopWindowStateStore', () => {
	it('atomically writes and restores one strict window-state record', async () => {
		const directory = await createTemporaryDirectory();
		const filePath = path.join(directory, 'state', 'desktop-window.json');
		const store = createStore(directory, filePath, { randomToken: () => 'one' });
		const state = requiredState({ height: 700, width: 1_100, x: 20, y: 30 }, true);

		expect(await store.load()).toEqual({ kind: 'missing' });
		expect(await store.save(state)).toBe(true);
		expect(await store.load()).toEqual({ kind: 'available', state });
		expect((await readFile(filePath, 'utf8')).endsWith('\n')).toBe(true);
		const fileMode = (await stat(filePath)).mode & 0o777;
		const directoryMode = (await stat(path.dirname(filePath))).mode & 0o777;
		expect(process.platform === 'win32' || fileMode === 0o600).toBe(true);
		expect(process.platform === 'win32' || directoryMode === 0o700).toBe(true);
	});

	it('serializes captures and exposes the latest state after flush', async () => {
		const directory = await createTemporaryDirectory();
		let token = 0;
		const store = createStore(directory, path.join(directory, 'window.json'), {
			randomToken: () => `token-${++token}`,
		});
		const first = requiredState({ height: 600, width: 900, x: 0, y: 0 }, false);
		const second = requiredState({ height: 800, width: 1_200, x: 50, y: 70 }, true);

		expect(await Promise.all([store.save(first), store.save(second)])).toEqual([true, true]);
		await store.flush();
		expect(await store.load()).toEqual({ kind: 'available', state: second });
	});

	it('treats malformed, unknown, and invalid UTF-8 files as unavailable', async () => {
		const directory = await createTemporaryDirectory();
		const filePath = path.join(directory, 'window.json');
		const store = createStore(directory, filePath);

		await writeFile(filePath, '{not-json', 'utf8');
		expect(await store.load()).toEqual({ kind: 'unavailable', reason: 'invalid' });
		await writeFile(
			filePath,
			JSON.stringify({
				bounds: { height: 720, width: 1_024, x: 0, y: 0 },
				fullScreen: true,
				maximized: false,
				version: 1,
			}),
			'utf8',
		);
		expect(await store.load()).toEqual({ kind: 'unavailable', reason: 'invalid' });
		await writeFile(filePath, Buffer.from([0xff]));
		expect(await store.load()).toEqual({ kind: 'unavailable', reason: 'invalid' });
	});

	it('keeps extensions, and never rewrites a record whose version it cannot use', async () => {
		const directory = await createTemporaryDirectory();
		const filePath = path.join(directory, 'window.json');
		const bounds = { height: 720, width: 1_024, x: 0, y: 0 };
		await writeFile(
			filePath,
			JSON.stringify({ bounds, extensions: { later: [1] }, maximized: false, version: 1 }),
		);
		const store = createStore(directory, filePath);
		expect((await store.load()).kind).toBe('available');
		expect(await store.save(requiredState(bounds, true))).toBe(true);
		expect(JSON.parse(await readFile(filePath, 'utf8'))).toMatchObject({
			extensions: { later: [1] },
			maximized: true,
		});

		const newer = JSON.stringify({ placement: 'later', version: 2 });
		await writeFile(filePath, newer);
		const kept = createStore(directory, filePath);
		expect(await kept.load()).toEqual({ kind: 'unusable', problem: 'newer-format', version: 2 });
		const moved = requiredState({ ...bounds, x: 40 }, false);
		expect(await kept.save(moved)).toBe(true);
		await kept.flush();
		expect(await kept.load()).toEqual({ kind: 'available', state: moved });
		expect(await readFile(filePath, 'utf8')).toBe(newer);
	});

	it('reports an atomic-write failure without replacing the current file', async () => {
		const directory = await createTemporaryDirectory();
		const filePath = path.join(directory, 'window.json');
		const initial = requiredState({ height: 600, width: 900, x: 0, y: 0 }, false);
		await writeFile(filePath, `${JSON.stringify(initial)}\n`, 'utf8');
		await writeFile(`${filePath}.collision.tmp`, 'occupied', 'utf8');
		const store = createStore(directory, filePath, { randomToken: () => 'collision' });

		expect(await store.save(requiredState({ height: 800, width: 1_200, x: 50, y: 70 }, true))).toBe(
			false,
		);
		expect(await store.load()).toEqual({ kind: 'available', state: initial });
	});
});

describe('FileDesktopWindowStateStore migration', () => {
	const bounds = { height: 720, width: 1_024, x: 0, y: 0 };
	// Version 0 of a synthetic earlier format kept the maximized state as `zoomed`.
	const steps: readonly DesktopWindowStateMigrationStep[] = [
		{
			apply: ({ zoomed, ...record }) =>
				typeof zoomed === 'boolean' ? { ...record, maximized: zoomed, version: 1 } : null,
			from: 0,
		},
	];
	const earlier = `${JSON.stringify({ bounds, extensions: { later: [1] }, version: 0, zoomed: true })}\n`;

	it('backs an earlier record up at its relative path, then replaces it once', async () => {
		const directory = await createTemporaryDirectory();
		const filePath = path.join(directory, 'internal', 'state', 'desktop-window.json');
		await mkdir(path.dirname(filePath), { recursive: true });
		await writeFile(filePath, earlier);

		const store = createStore(directory, filePath, { steps });
		const state = requiredState(bounds, true);
		expect(await store.load()).toEqual({ kind: 'available', state });
		const backups = path.join(directory, 'internal', 'backups');
		const folders = await readdir(backups);
		expect(folders).toHaveLength(1);
		expect(folders[0]).toMatch(/^\d{4}-\d{2}-\d{2}-upgrade-1\.2\.3$/u);
		const copy = path.join(backups, folders[0] ?? '', 'internal', 'state', 'desktop-window.json');
		expect(await readFile(copy, 'utf8')).toBe(earlier);
		expect(JSON.parse(await readFile(filePath, 'utf8'))).toEqual({
			...state,
			extensions: { later: [1] },
		});

		// The record is current from then on: a later opening neither migrates nor backs up again.
		const replaced = await readFile(filePath, 'utf8');
		expect(await createStore(directory, filePath, { steps }).load()).toEqual({
			kind: 'available',
			state,
		});
		expect(await readFile(filePath, 'utf8')).toBe(replaced);
		expect(await readdir(path.dirname(copy))).toEqual(['desktop-window.json']);
	});

	const failing: readonly DesktopWindowStateMigrationStep[] = [{ apply: () => null, from: 0 }];
	const failures: readonly {
		readonly name: string;
		readonly prepare: (directory: string, filePath: string) => Promise<void>;
		readonly steps: readonly DesktopWindowStateMigrationStep[];
	}[] = [
		{ name: 'a step', prepare: async () => undefined, steps: failing },
		{ name: 'no shipped step', prepare: async () => undefined, steps: [] },
		{
			name: 'the backup',
			prepare: async (directory) => {
				await writeFile(path.join(directory, 'internal', 'backups'), 'not a folder');
			},
			steps,
		},
		{
			name: 'the replacement',
			prepare: async (_, filePath) => {
				await writeFile(`${filePath}.collision.tmp`, 'occupied');
			},
			steps,
		},
	];

	it.each(failures)(
		'leaves the record unchanged when $name fails',
		async ({ prepare, steps: injected }) => {
			const directory = await createTemporaryDirectory();
			const filePath = path.join(directory, 'internal', 'state', 'desktop-window.json');
			await mkdir(path.dirname(filePath), { recursive: true });
			await writeFile(filePath, earlier);
			await prepare(directory, filePath);

			const store = createStore(directory, filePath, {
				randomToken: () => 'collision',
				steps: injected,
			});
			expect(await store.load()).toEqual({ kind: 'unusable', problem: 'not-updated', version: 0 });
			expect(await store.save(requiredState({ ...bounds, x: 40 }, false))).toBe(true);
			await store.flush();
			expect(await readFile(filePath, 'utf8')).toBe(earlier);
		},
	);
});

function createStore(
	dataRoot: string,
	filePath: string,
	options: Omit<FileDesktopWindowStateStoreOptions, 'upgradeBackup'> = {},
): FileDesktopWindowStateStore {
	return new FileDesktopWindowStateStore(filePath, {
		...options,
		upgradeBackup: createUpgradeBackup(dataRoot, PRODUCT_VERSION),
	});
}

function requiredState(
	bounds: {
		readonly height: number;
		readonly width: number;
		readonly x: number;
		readonly y: number;
	},
	maximized: boolean,
) {
	const state = createDesktopWindowState(bounds, maximized);
	if (state === null) throw new Error('Test window state must be valid.');
	return state;
}

async function createTemporaryDirectory(): Promise<string> {
	const directory = await mkdtemp(path.join(tmpdir(), 'orchbee-window-state-'));
	temporaryDirectories.push(directory);
	return directory;
}
