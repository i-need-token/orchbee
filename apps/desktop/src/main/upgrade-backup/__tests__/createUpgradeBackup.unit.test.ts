import { mkdtemp, readdir, readFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';

import { afterEach, describe, expect, it } from 'vitest';

import { createUpgradeBackup } from '../createUpgradeBackup';

const temporaryDirectories: string[] = [];

afterEach(async () => {
	await Promise.all(
		temporaryDirectories.splice(0).map(async (directory) => rm(directory, { recursive: true })),
	);
});

// The native writers follow the same rules under the `upgrade-backup-folders` contract.
describe('upgrade backups', () => {
	it('keeps the newest three folders, and a version reuses its folder whatever the day', async () => {
		const root = await createTemporaryDirectory();
		const record = path.join(root, 'internal', 'state', 'desktop-window.json');
		const backUp = async (date: string, version: string): Promise<string> =>
			createUpgradeBackup(root, version, { today: () => date })(record, Buffer.from(version));
		for (const [date, version] of [
			['2026-01-01', '0.9.0'],
			['2026-02-01', '0.10.0'],
			['2026-02-01', '0.10.1'],
			['2026-03-01', '1.0.0'],
		] as const) {
			await backUp(date, version);
		}
		const reused = await backUp('2026-04-01', '0.10.1');
		expect(path.relative(root, reused)).toBe(
			path.join('internal', 'backups', '2026-02-01-upgrade-0.10.1', path.relative(root, record)),
		);
		expect((await readdir(path.join(root, 'internal', 'backups'))).sort()).toEqual([
			'2026-02-01-upgrade-0.10.0',
			'2026-02-01-upgrade-0.10.1',
			'2026-03-01-upgrade-1.0.0',
		]);
	});

	it('copies a record at its relative path and never replaces the first original', async () => {
		const root = await createTemporaryDirectory();
		const backUp = createUpgradeBackup(root, '1.0.0+build/7', { today: () => '2026-05-01' });
		const record = path.join(root, 'internal', 'state', 'desktop-window.json');
		const folder = path.join(root, 'internal', 'backups', '2026-05-01-upgrade-1.0.0-build-7');

		const first = await backUp(record, Buffer.from('first'));
		expect(first).toBe(path.join(folder, 'internal', 'state', 'desktop-window.json'));
		expect(await backUp(record, Buffer.from('first'))).toBe(first);
		const second = await backUp(record, Buffer.from('second'));
		expect(path.basename(second)).toBe('desktop-window-2.json');
		expect(await readFile(first, 'utf8')).toBe('first');
		expect(await readFile(second, 'utf8')).toBe('second');
		await expect(backUp(path.join(tmpdir(), 'outside.json'), Buffer.from('x'))).rejects.toThrow(
			'The record is outside the data root.',
		);
	});
});

async function createTemporaryDirectory(): Promise<string> {
	const directory = await mkdtemp(path.join(tmpdir(), 'orchbee-upgrade-backup-'));
	temporaryDirectories.push(directory);
	return directory;
}
