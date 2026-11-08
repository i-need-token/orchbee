import { mkdtemp, readFile, rm, stat, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';

import { afterEach, describe, expect, it } from 'vitest';

import { createDesktopWindowState } from '../desktop-window-state';
import { FileDesktopWindowStateStore } from '../FileDesktopWindowStateStore';

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
		const store = new FileDesktopWindowStateStore(filePath, { randomToken: () => 'one' });
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
		const store = new FileDesktopWindowStateStore(path.join(directory, 'window.json'), {
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
		const store = new FileDesktopWindowStateStore(filePath);

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

	it('reports an atomic-write failure without replacing the current file', async () => {
		const directory = await createTemporaryDirectory();
		const filePath = path.join(directory, 'window.json');
		const initial = requiredState({ height: 600, width: 900, x: 0, y: 0 }, false);
		await writeFile(filePath, `${JSON.stringify(initial)}\n`, 'utf8');
		await writeFile(`${filePath}.collision.tmp`, 'occupied', 'utf8');
		const store = new FileDesktopWindowStateStore(filePath, {
			randomToken: () => 'collision',
		});

		expect(await store.save(requiredState({ height: 800, width: 1_200, x: 50, y: 70 }, true))).toBe(
			false,
		);
		expect(await store.load()).toEqual({ kind: 'available', state: initial });
	});
});

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
