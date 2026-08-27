import { chmod, mkdtemp, readFile, rm, stat, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { afterEach, describe, expect, it } from 'vitest';

import { PersonalAccessTokenVerifierSchema } from '@orchbee/remote-client-protocol';

import { FileDesktopPersonalAccessTokenStore } from '../FileDesktopPersonalAccessTokenStore';

let directory = '';

afterEach(async () => {
	if (directory !== '') await rm(directory, { force: true, recursive: true });
	directory = '';
});

describe('FileDesktopPersonalAccessTokenStore', () => {
	it('treats a missing store as unconfigured and atomically persists only the verifier', async () => {
		directory = await mkdtemp(path.join(tmpdir(), 'orchbee-personal-access-token-'));
		const filePath = path.join(directory, 'nested', 'personal-access-token-v1.json');
		const verifier = PersonalAccessTokenVerifierSchema.parse('v'.repeat(43));
		const store = new FileDesktopPersonalAccessTokenStore(filePath, { randomToken: () => 'write' });

		await expect(store.load()).resolves.toBeNull();
		await store.save(verifier);

		expect(await store.load()).toBe(verifier);
		expect(await readFile(filePath, 'utf8')).toBe(`${JSON.stringify({ verifier, version: 1 })}\n`);
		const persistedMode = (await stat(filePath)).mode & 0o777;
		expect(process.platform === 'win32' || persistedMode === 0o600).toBe(true);
	});

	it('rejects malformed, oversized, and non-regular verifier stores', async () => {
		directory = await mkdtemp(path.join(tmpdir(), 'orchbee-personal-access-token-'));
		const filePath = path.join(directory, 'personal-access-token-v1.json');
		const store = new FileDesktopPersonalAccessTokenStore(filePath);

		await writeFile(filePath, '{invalid', 'utf8');
		await expect(store.load()).rejects.toThrow(/invalid JSON/u);
		await writeFile(filePath, 'x'.repeat(4 * 1024 + 1), 'utf8');
		await expect(store.load()).rejects.toThrow(/store is invalid/u);
		await rm(filePath);
		await chmod(directory, 0o700);
		const directoryStore = new FileDesktopPersonalAccessTokenStore(directory);
		await expect(directoryStore.load()).rejects.toThrow(/store is invalid/u);
	});
});
