import { createHash } from 'node:crypto';
import { access, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';

import { afterEach, describe, expect, it } from 'vitest';

import { createNodeDesktopDevelopmentRuntimeDependencies } from '../createNodeDesktopDevelopmentRuntimeDependencies';

const temporaryDirectories: string[] = [];

afterEach(async () => {
	await Promise.all(
		temporaryDirectories
			.splice(0)
			.map((directory) => rm(directory, { force: true, recursive: true })),
	);
});

describe('createNodeDesktopDevelopmentRuntimeDependencies', () => {
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
