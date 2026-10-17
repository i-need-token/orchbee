import { createHash } from 'node:crypto';
import { access, mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';

import { afterEach, describe, expect, it } from 'vitest';

import {
	createNodeDesktopDevelopmentRuntimeDependencies,
	terminalHostInputDigest,
} from '../createNodeDesktopDevelopmentRuntimeDependencies';

const temporaryDirectories: string[] = [];

afterEach(async () => {
	await Promise.all(
		temporaryDirectories
			.splice(0)
			.map((directory) => rm(directory, { force: true, recursive: true })),
	);
});

describe('createNodeDesktopDevelopmentRuntimeDependencies', () => {
	it('includes both shared PowerShell startup assets in the Host input generation', async () => {
		const repository = await mkdtemp(path.join(tmpdir(), 'orchbee-host-inputs-'));
		temporaryDirectories.push(repository);
		const host = path.join(repository, 'apps/terminal-host');
		const shared = path.join(repository, 'crates/local-site-runtime/src');
		await mkdir(path.join(host, 'src'), { recursive: true });
		await mkdir(shared, { recursive: true });
		await Promise.all([
			writeFile(path.join(host, 'build.zig'), 'build'),
			writeFile(path.join(host, 'build.zig.zon'), 'dependencies'),
			writeFile(path.join(host, 'src/main.zig'), 'main'),
			writeFile(path.join(shared, 'windows_powershell_initialization.ps1'), 'common'),
			writeFile(path.join(shared, 'windows_powershell_reader.ps1'), 'reader'),
		]);
		const original = await terminalHostInputDigest(host);
		for (const { name, contents } of [
			{ name: 'initialization', contents: 'common' },
			{ name: 'reader', contents: 'reader' },
		]) {
			const asset = path.join(shared, `windows_powershell_${name}.ps1`);
			await writeFile(asset, `${contents}-changed`);
			expect(await terminalHostInputDigest(host)).not.toBe(original);
			await writeFile(asset, contents);
			expect(await terminalHostInputDigest(host)).toBe(original);
		}
		await writeFile(path.join(shared, 'unrelated.rs'), 'other runtime owner');
		expect(await terminalHostInputDigest(host)).toBe(original);
		await rm(path.join(shared, 'windows_powershell_reader.ps1'));
		await expect(terminalHostInputDigest(host)).rejects.toMatchObject({ code: 'ENOENT' });
	});

	it('creates an isolated workspace and captures an immutable Host generation', async () => {
		const dependencies = createNodeDesktopDevelopmentRuntimeDependencies({
			error: () => undefined,
			info: () => undefined,
			warn: () => undefined,
		});
		const sourceDirectory = await mkdtemp(path.join(tmpdir(), 'orchbee-host-source-'));
		temporaryDirectories.push(sourceDirectory);
		const sourcePath = path.join(sourceDirectory, 'orchbee-terminal-host');
		const sourceContents = 'host-generation-one';
		await writeFile(sourcePath, sourceContents, 'utf8');
		const workspace = await dependencies.createWorkspace({
			ORCHBEE_DESKTOP_ENDPOINT_ID: 'ab'.repeat(16),
			ORCHBEE_TERMINAL_SCROLLBACK_BYTES: '25000000',
		});
		const workspaceRoot = path.dirname(workspace.restartPlanPath);

		const candidate = await dependencies.captureHostCandidate(
			{
				binaryPath: sourcePath,
				digest: createHash('sha256').update(sourceContents).digest('hex'),
				inputDigest: 'input-generation-one',
			},
			workspace,
			3,
		);
		expect(candidate).toMatchObject({ generation: 3 });
		expect(candidate.binaryPath).not.toBe(sourcePath);
		expect(await readFile(candidate.binaryPath, 'utf8')).toBe('host-generation-one');
		expect(workspace).toMatchObject({
			endpointId: 'ab'.repeat(16),
			terminalScrollbackBytes: '25000000',
		});
		await dependencies.releaseHostCandidate(candidate);
		await expect(access(candidate.binaryPath)).rejects.toMatchObject({ code: 'ENOENT' });
		await expect(
			dependencies.captureHostCandidate(
				{
					binaryPath: sourcePath,
					digest: 'stale-digest',
					inputDigest: 'input-generation-one',
				},
				workspace,
				4,
			),
		).rejects.toThrow('changed while its generation was being captured');
		await expect(access(path.join(workspaceRoot, 'host-generations', '4'))).rejects.toMatchObject({
			code: 'ENOENT',
		});

		await dependencies.cleanupWorkspace(workspace);
		await expect(access(workspaceRoot)).rejects.toMatchObject({ code: 'ENOENT' });
	});

	it.runIf(process.platform !== 'win32')(
		'keeps the supervised endpoint beneath the Darwin socket pathname bound',
		async () => {
			const dependencies = createNodeDesktopDevelopmentRuntimeDependencies({
				error: () => undefined,
				info: () => undefined,
				warn: () => undefined,
			});
			const workspace = await dependencies.createWorkspace({});
			temporaryDirectories.push(path.dirname(workspace.restartPlanPath));

			expect(
				Buffer.byteLength(path.join(workspace.runtimeDirectory, `${workspace.endpointId}.s`)),
			).toBeLessThan(104);
		},
	);

	it('rejects malformed endpoint and scrollback configuration before starting processes', async () => {
		const dependencies = createNodeDesktopDevelopmentRuntimeDependencies({
			error: () => undefined,
			info: () => undefined,
			warn: () => undefined,
		});

		await expect(
			dependencies.createWorkspace({ ORCHBEE_DESKTOP_ENDPOINT_ID: 'invalid' }),
		).rejects.toThrow('32 lowercase hex digits');
		await expect(
			dependencies.createWorkspace({ ORCHBEE_TERMINAL_SCROLLBACK_BYTES: 'too-many' }),
		).rejects.toThrow('decimal byte count');
	});
});
