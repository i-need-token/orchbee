import { mkdtemp, readFile, rm, stat, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';

import { afterEach, describe, expect, it } from 'vitest';

import type { DesktopWindowLayout } from '../desktop-window-layout';
import { FileDesktopWindowLayoutStore } from '../FileDesktopWindowLayoutStore';

const temporaryDirectories: string[] = [];

afterEach(async () => {
	await Promise.all(
		temporaryDirectories.splice(0).map(async (directory) => rm(directory, { recursive: true })),
	);
});

describe('FileDesktopWindowLayoutStore', () => {
	it('atomically writes and restores one strict layout record', async () => {
		const directory = await createTemporaryDirectory();
		const filePath = path.join(directory, 'state', 'window-layout-v1.json');
		const store = new FileDesktopWindowLayoutStore(filePath, { randomToken: () => 'one' });
		const layout = layoutFixture();

		expect(await store.load()).toEqual({ kind: 'missing' });
		expect(await store.save(layout)).toBe(true);
		expect(await store.load()).toEqual({ kind: 'available', layout });
		expect((await readFile(filePath, 'utf8')).endsWith('\n')).toBe(true);
		const fileMode = (await stat(filePath)).mode & 0o777;
		const directoryMode = (await stat(path.dirname(filePath))).mode & 0o777;
		expect(process.platform === 'win32' || fileMode === 0o600).toBe(true);
		expect(process.platform === 'win32' || directoryMode === 0o700).toBe(true);
	});

	it('serializes writes and exposes the latest layout after flush', async () => {
		const directory = await createTemporaryDirectory();
		let token = 0;
		const store = new FileDesktopWindowLayoutStore(path.join(directory, 'layout.json'), {
			randomToken: () => `token-${++token}`,
		});
		const first = layoutFixture();
		const second = { ...first, activationOrder: [...first.activationOrder].reverse() };

		expect(await Promise.all([store.save(first), store.save(second)])).toEqual([true, true]);
		await store.flush();
		expect(await store.load()).toEqual({ kind: 'available', layout: second });
	});

	it('treats malformed, unknown, and invalid UTF-8 files as unavailable', async () => {
		const directory = await createTemporaryDirectory();
		const filePath = path.join(directory, 'layout.json');
		const store = new FileDesktopWindowLayoutStore(filePath);

		await writeFile(filePath, '{not-json', 'utf8');
		expect(await store.load()).toEqual({ kind: 'unavailable', reason: 'invalid' });
		await writeFile(filePath, JSON.stringify({ ...layoutFixture(), fullScreen: true }), 'utf8');
		expect(await store.load()).toEqual({ kind: 'unavailable', reason: 'invalid' });
		await writeFile(filePath, Buffer.from([0xff]));
		expect(await store.load()).toEqual({ kind: 'unavailable', reason: 'invalid' });
	});

	it('reports an atomic-write failure without replacing the current file', async () => {
		const directory = await createTemporaryDirectory();
		const filePath = path.join(directory, 'layout.json');
		const initial = layoutFixture();
		await writeFile(filePath, `${JSON.stringify(initial)}\n`, 'utf8');
		await writeFile(`${filePath}.collision.tmp`, 'occupied', 'utf8');
		const store = new FileDesktopWindowLayoutStore(filePath, {
			randomToken: () => 'collision',
		});

		expect(
			await store.save({ ...initial, activationOrder: [...initial.activationOrder].reverse() }),
		).toBe(false);
		expect(await store.load()).toEqual({ kind: 'available', layout: initial });
	});
});

function layoutFixture(): DesktopWindowLayout {
	return {
		activationOrder: ['desktop-window-1', 'desktop-window-2'],
		nextWindowSequence: 3,
		primaryWindowId: 'desktop-window-1',
		version: 1,
		windows: [
			{
				assignments: [{ tabIds: ['tab-1'], terminalContextId: 'context-1' }],
				bounds: { height: 720, width: 1_024, x: 0, y: 0 },
				maximized: false,
				ordinal: 1,
				windowId: 'desktop-window-1',
			},
			{
				assignments: [{ tabIds: ['tab-2'], terminalContextId: 'context-1' }],
				bounds: { height: 720, width: 1_024, x: 1_100, y: 0 },
				maximized: true,
				ordinal: 2,
				windowId: 'desktop-window-2',
			},
		],
	};
}

async function createTemporaryDirectory(): Promise<string> {
	const directory = await mkdtemp(path.join(tmpdir(), 'orchbee-window-layout-'));
	temporaryDirectories.push(directory);
	return directory;
}
