import { chmod, mkdtemp, open, readFile, rm, stat, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { afterEach, describe, expect, it } from 'vitest';

import { PersonalAccessTokenVerifierSchema } from '@orchbee/remote-client-protocol';

import { FileDesktopPersonalAccessTokenStore } from '../FileDesktopPersonalAccessTokenStore';

let directory = '';
type SaveFailureStage = 'chmod' | 'close' | 'rename' | 'sync' | 'write';
const saveFailureStages: readonly SaveFailureStage[] =
	process.platform === 'win32'
		? ['write', 'sync', 'close', 'rename']
		: ['chmod', 'write', 'sync', 'close', 'rename'];

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

	it.skipIf(process.platform === 'win32')(
		'replaces a permissive prior file with the private temporary-file mode',
		async () => {
			directory = await mkdtemp(path.join(tmpdir(), 'orchbee-personal-access-token-'));
			const filePath = path.join(directory, 'personal-access-token-v1.json');
			const verifier = PersonalAccessTokenVerifierSchema.parse('v'.repeat(43));
			await writeFile(filePath, 'prior verifier\n', { encoding: 'utf8', mode: 0o666 });
			await chmod(filePath, 0o666);

			await new FileDesktopPersonalAccessTokenStore(filePath, {
				randomToken: () => 'replace-mode',
			}).save(verifier);

			expect((await stat(filePath)).mode & 0o777).toBe(0o600);
			expect(await readFile(filePath, 'utf8')).toBe(
				`${JSON.stringify({ verifier, version: 1 })}\n`,
			);
		},
	);

	it.each(saveFailureStages)(
		'preserves the committed verifier and removes the temporary file after a %s failure',
		async (failure) => {
			directory = await mkdtemp(path.join(tmpdir(), 'orchbee-personal-access-token-'));
			const filePath = path.join(directory, 'personal-access-token-v1.json');
			const temporaryPath = `${filePath}.${failure}.tmp`;
			const committedVerifier = PersonalAccessTokenVerifierSchema.parse('o'.repeat(43));
			const replacementVerifier = PersonalAccessTokenVerifierSchema.parse('n'.repeat(43));
			await writeFile(
				filePath,
				`${JSON.stringify({ verifier: committedVerifier, version: 1 })}\n`,
				'utf8',
			);
			const injectedFailure = new Error(`injected ${failure} failure`);
			const store = new FileDesktopPersonalAccessTokenStore(filePath, {
				openTemporaryFile: async (candidatePath) => {
					const handle = await open(candidatePath, 'wx', 0o600);
					let closeCalls = 0;
					return {
						chmod: async (mode) => {
							if (failure === 'chmod') throw injectedFailure;
							await handle.chmod(mode);
						},
						close: async () => {
							if (failure === 'close' && closeCalls++ === 0) throw injectedFailure;
							await handle.close();
						},
						sync: async () => {
							if (failure === 'sync') throw injectedFailure;
							await handle.sync();
						},
						writeFile: async (data, encoding) => {
							if (failure === 'write') throw injectedFailure;
							await handle.writeFile(data, encoding);
						},
					};
				},
				randomToken: () => failure,
				replaceFile:
					failure === 'rename'
						? async () => {
								throw injectedFailure;
							}
						: undefined,
			});

			await expect(store.save(replacementVerifier)).rejects.toBe(injectedFailure);
			expect(await store.load()).toBe(committedVerifier);
			await expect(stat(temporaryPath)).rejects.toMatchObject({ code: 'ENOENT' });
		},
	);
});
