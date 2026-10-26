import { mkdir, mkdtemp, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';

import { afterEach, describe, expect, it } from 'vitest';

import { terminalHostInputDigest } from '../terminalHostInputDigest';

const temporaryDirectories: string[] = [];

afterEach(async () => {
	await Promise.all(
		temporaryDirectories
			.splice(0)
			.map((directory) => rm(directory, { force: true, recursive: true })),
	);
});

describe('terminalHostInputDigest', () => {
	it('includes both shared PowerShell startup assets in the Host input generation', async () => {
		const repository = await mkdtemp(path.join(tmpdir(), 'orchbee-host-inputs-'));
		temporaryDirectories.push(repository);
		const host = path.join(repository, 'apps/terminal-host');
		const shared = path.join(repository, 'crates/local-runtime/src/directory/windows');
		await mkdir(path.join(host, 'src'), { recursive: true });
		await mkdir(shared, { recursive: true });
		await Promise.all([
			writeFile(path.join(host, 'build.zig'), 'build'),
			writeFile(path.join(host, 'build.zig.zon'), 'dependencies'),
			writeFile(path.join(host, 'src/main.zig'), 'main'),
			writeFile(path.join(shared, 'initialization.ps1'), 'common'),
			writeFile(path.join(shared, 'reader.ps1'), 'reader'),
		]);
		const original = await terminalHostInputDigest(host);
		for (const { name, contents } of [
			{ name: 'initialization', contents: 'common' },
			{ name: 'reader', contents: 'reader' },
		]) {
			const asset = path.join(shared, `${name}.ps1`);
			await writeFile(asset, `${contents}-changed`);
			expect(await terminalHostInputDigest(host)).not.toBe(original);
			await writeFile(asset, contents);
			expect(await terminalHostInputDigest(host)).toBe(original);
		}
		await writeFile(path.join(shared, 'unrelated.rs'), 'other runtime owner');
		expect(await terminalHostInputDigest(host)).toBe(original);
		await rm(path.join(shared, 'reader.ps1'));
		await expect(terminalHostInputDigest(host)).rejects.toMatchObject({ code: 'ENOENT' });
	});
});
