import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';

import { afterEach, describe, expect, it } from 'vitest';

import { FileTerminalDefaultSettingsStore } from '../FileTerminalDefaultSettingsStore';

const temporaryDirectories: string[] = [];

afterEach(async () => {
	await Promise.all(
		temporaryDirectories
			.splice(0)
			.map(async (directory) => rm(directory, { force: true, recursive: true })),
	);
});

describe('FileTerminalDefaultSettingsStore', () => {
	it('distinguishes missing data from invalid or foreign records', async () => {
		const filePath = await temporaryFilePath();
		const store = new FileTerminalDefaultSettingsStore(filePath);

		await expect(store.load()).resolves.toEqual({ kind: 'missing' });
		await writeFile(filePath, '{"configuration":{"scrollbackBytes":5},"version":2}\n', 'utf8');
		await expect(store.load()).resolves.toEqual({ kind: 'unavailable', reason: 'invalid-record' });
		await writeFile(
			filePath,
			'{"configuration":{"scrollbackBytes":5},"unknown":true,"version":1}\n',
			'utf8',
		);
		await expect(store.load()).resolves.toEqual({ kind: 'unavailable', reason: 'invalid-record' });
	});

	it('atomically replaces and restores one canonical record', async () => {
		const filePath = await temporaryFilePath();
		const store = new FileTerminalDefaultSettingsStore(filePath, { randomToken: () => 'first' });
		const configuration = { scrollbackBytes: 25_000_000 };

		await store.replace(configuration);

		await expect(store.load()).resolves.toEqual({ configuration, kind: 'available' });
		await expect(readFile(filePath, 'utf8')).resolves.toBe(
			`${JSON.stringify({ configuration, version: 1 })}\n`,
		);
	});

	it('rejects oversized input before JSON parsing', async () => {
		const filePath = await temporaryFilePath();
		await writeFile(filePath, Buffer.alloc(4 * 1024 + 1, 0x20));

		await expect(new FileTerminalDefaultSettingsStore(filePath).load()).resolves.toEqual({
			kind: 'unavailable',
			reason: 'invalid-record',
		});
	});

	it('preserves the previous file when a temporary-path collision blocks replacement', async () => {
		const filePath = await temporaryFilePath();
		const first = new FileTerminalDefaultSettingsStore(filePath, { randomToken: () => 'first' });
		await first.replace({ scrollbackBytes: 5 });
		await writeFile(`${filePath}.collision.tmp`, 'occupied', 'utf8');
		const failing = new FileTerminalDefaultSettingsStore(filePath, {
			randomToken: () => 'collision',
		});

		await expect(failing.replace({ scrollbackBytes: 10 })).rejects.toMatchObject({
			code: 'EEXIST',
		});
		await expect(first.load()).resolves.toEqual({
			configuration: { scrollbackBytes: 5 },
			kind: 'available',
		});
	});
});

async function temporaryFilePath(): Promise<string> {
	const directory = await mkdtemp(path.join(tmpdir(), 'orchbee-terminal-defaults-'));
	temporaryDirectories.push(directory);
	return path.join(directory, 'terminal-default-settings-v1.json');
}
