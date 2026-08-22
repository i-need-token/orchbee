import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';

import { DESKTOP_TERMINAL_LAUNCH_PROFILE_LIMITS } from '@orchbee/desktop-client-protocol';
import { afterEach, describe, expect, it } from 'vitest';

import { FileTerminalLaunchProfileStore } from '../FileTerminalLaunchProfileStore';
import { createBuiltInTerminalLaunchProfileCatalog } from '../terminal-launch-profile-catalog';

const temporaryDirectories: string[] = [];

afterEach(async () => {
	await Promise.all(
		temporaryDirectories
			.splice(0)
			.map(async (directory) => rm(directory, { force: true, recursive: true })),
	);
});

describe('FileTerminalLaunchProfileStore', () => {
	it('distinguishes a missing catalog from invalid persisted data', async () => {
		const filePath = await temporaryFilePath();
		const store = new FileTerminalLaunchProfileStore(filePath);

		await expect(store.load()).resolves.toEqual({ kind: 'missing' });
		await writeFile(filePath, '{"version":2}\n', 'utf8');
		await expect(store.load()).resolves.toEqual({ kind: 'unavailable', reason: 'invalid-record' });
	});

	it('atomically replaces and restores one canonical catalog', async () => {
		const filePath = await temporaryFilePath();
		const store = new FileTerminalLaunchProfileStore(filePath, { randomToken: () => 'first' });
		const catalog = createBuiltInTerminalLaunchProfileCatalog('linux');

		await store.replace(catalog);

		await expect(store.load()).resolves.toEqual({ catalog, kind: 'available' });
		await expect(readFile(filePath, 'utf8')).resolves.toBe(`${JSON.stringify(catalog)}\n`);
	});

	it('rejects oversized input before JSON parsing', async () => {
		const filePath = await temporaryFilePath();
		await writeFile(
			filePath,
			Buffer.alloc(DESKTOP_TERMINAL_LAUNCH_PROFILE_LIMITS.maximumCatalogBytes + 2, 0x20),
		);
		const store = new FileTerminalLaunchProfileStore(filePath);

		await expect(store.load()).resolves.toEqual({ kind: 'unavailable', reason: 'invalid-record' });
	});

	it('preserves the previous file when atomic replacement cannot create its temporary file', async () => {
		const filePath = await temporaryFilePath();
		const catalog = createBuiltInTerminalLaunchProfileCatalog('linux');
		const first = new FileTerminalLaunchProfileStore(filePath, { randomToken: () => 'first' });
		await first.replace(catalog);
		await writeFile(`${filePath}.collision.tmp`, 'occupied', 'utf8');
		const failing = new FileTerminalLaunchProfileStore(filePath, {
			randomToken: () => 'collision',
		});

		await expect(failing.replace(catalog)).rejects.toMatchObject({ code: 'EEXIST' });
		await expect(first.load()).resolves.toEqual({ catalog, kind: 'available' });
	});
});

async function temporaryFilePath(): Promise<string> {
	const directory = await mkdtemp(path.join(tmpdir(), 'orchbee-terminal-profiles-'));
	temporaryDirectories.push(directory);
	return path.join(directory, 'terminal-launch-profiles-v1.json');
}
