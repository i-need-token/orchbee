import { mkdir, mkdtemp, readlink, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';

import { describe, expect, it } from 'vitest';

import type {
	AgentIntegrationCommandResult,
	AgentIntegrationCommandRunner,
} from '../NodeAgentIntegrationCommandRunner';
import { SymlinkAgentIntegrationAdapter } from '../SymlinkAgentIntegrationAdapter';

describe('SymlinkAgentIntegrationAdapter', () => {
	it('enables and disables only its exact managed symlink', async () => {
		const root = await mkdtemp(path.join(tmpdir(), 'orchbee-agent-integration-'));
		try {
			const sourcePath = path.join(root, 'bundle', 'adapter.mjs');
			const targetPath = path.join(root, 'config', 'adapter.mjs');
			await mkdir(path.dirname(sourcePath), { recursive: true });
			await writeFile(sourcePath, 'export {};', 'utf8');
			const adapter = createAdapter(sourcePath, targetPath);

			await expect(adapter.inspect()).resolves.toMatchObject({ state: 'not-installed' });
			await expect(adapter.enable()).resolves.toEqual({ kind: 'attempted' });
			expect(path.resolve(path.dirname(targetPath), await readlink(targetPath))).toBe(sourcePath);
			await expect(adapter.inspect()).resolves.toMatchObject({
				issue: null,
				state: 'enabled',
			});
			await expect(adapter.disable()).resolves.toEqual({ kind: 'attempted' });
			await expect(adapter.inspect()).resolves.toMatchObject({ state: 'not-installed' });
		} finally {
			await rm(root, { force: true, recursive: true });
		}
	});

	it('preserves a conflicting user-owned target', async () => {
		const root = await mkdtemp(path.join(tmpdir(), 'orchbee-agent-integration-'));
		try {
			const sourcePath = path.join(root, 'bundle', 'adapter.mjs');
			const targetPath = path.join(root, 'config', 'adapter.mjs');
			await mkdir(path.dirname(sourcePath), { recursive: true });
			await mkdir(path.dirname(targetPath), { recursive: true });
			await writeFile(sourcePath, 'export {};', 'utf8');
			await writeFile(targetPath, 'user configuration', 'utf8');
			const adapter = createAdapter(sourcePath, targetPath);

			await expect(adapter.inspect()).resolves.toMatchObject({ state: 'conflict' });
			await expect(adapter.enable()).resolves.toEqual({ kind: 'conflict' });
			await expect(adapter.disable()).resolves.toEqual({ kind: 'conflict' });
		} finally {
			await rm(root, { force: true, recursive: true });
		}
	});

	it('manages an exact extension-directory symlink without copying packaged resources', async () => {
		const root = await mkdtemp(path.join(tmpdir(), 'orchbee-agent-integration-'));
		try {
			const sourcePath = path.join(root, 'bundle', 'gemini');
			const targetPath = path.join(root, '.gemini', 'extensions', 'orchbee-agent-observation');
			await mkdir(sourcePath, { recursive: true });
			const adapter = createAdapter(sourcePath, targetPath, 'directory');

			await expect(adapter.enable()).resolves.toEqual({ kind: 'attempted' });
			expect(path.resolve(path.dirname(targetPath), await readlink(targetPath))).toBe(sourcePath);
			await expect(adapter.inspect()).resolves.toMatchObject({ issue: null, state: 'enabled' });
			await expect(adapter.disable()).resolves.toEqual({ kind: 'attempted' });
		} finally {
			await rm(root, { force: true, recursive: true });
		}
	});
});

function createAdapter(
	sourcePath: string,
	targetPath: string,
	sourceKind: 'directory' | 'file' = 'file',
): SymlinkAgentIntegrationAdapter {
	return new SymlinkAgentIntegrationAdapter({
		commandRunner: new AvailableRunner(),
		executableName: 'opencode',
		integrationId: 'opencode',
		managementMode: 'managed-local',
		sourceKind,
		sourcePath,
		targetPath,
	});
}

class AvailableRunner implements AgentIntegrationCommandRunner {
	resolveExecutable(): Promise<string | null> {
		return Promise.resolve('/bin/opencode');
	}

	run(): Promise<AgentIntegrationCommandResult> {
		return Promise.resolve({ kind: 'failed' });
	}
}
