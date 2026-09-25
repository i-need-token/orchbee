import { spawnSync } from 'node:child_process';
import {
	appendFile,
	lstat,
	mkdir,
	mkdtemp,
	readFile,
	rm,
	symlink,
	writeFile,
} from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

import { describe, expect, it } from 'vitest';

import { GuardedLocalAgentIntegrationAdapter } from '../GuardedLocalAgentIntegrationAdapter';
import type {
	AgentIntegrationCommandResult,
	AgentIntegrationCommandRunner,
} from '../NodeAgentIntegrationCommandRunner';

describe('GuardedLocalAgentIntegrationAdapter', () => {
	it('installs and removes an exact provider-local bootstrap instead of a resource symlink', async () => {
		const root = await mkdtemp(path.join(tmpdir(), 'orchbee-agent-integration-'));
		try {
			const sourcePath = path.join(root, 'capsule', 'adapter.mjs');
			const targetPath = path.join(root, 'config', 'adapter.mjs');
			await mkdir(path.dirname(sourcePath), { recursive: true });
			await writeFile(sourcePath, 'export const OrchbeeAgentObservation = async () => ({});');
			const adapter = createModuleAdapter(sourcePath, targetPath);

			await expect(adapter.inspect()).resolves.toMatchObject({ state: 'not-installed' });
			await expect(adapter.enable()).resolves.toEqual({ kind: 'attempted' });
			const metadata = await lstat(targetPath);
			expect(metadata.isFile()).toBe(true);
			expect(metadata.isSymbolicLink()).toBe(false);
			expect(await readFile(targetPath, 'utf8')).toContain(
				'orchbee-agent-observation-bootstrap-v3:opencode',
			);
			await expect(adapter.inspect()).resolves.toMatchObject({ issue: null, state: 'enabled' });
			await expect(adapter.disable()).resolves.toEqual({ kind: 'attempted' });
			await expect(adapter.inspect()).resolves.toMatchObject({ state: 'not-installed' });
		} finally {
			await rm(root, { force: true, recursive: true });
		}
	});

	it('leaves its persisted bootstrap silent and successful after the Capsule disappears', async () => {
		const root = await mkdtemp(path.join(tmpdir(), 'orchbee-agent-integration-'));
		try {
			const capsuleRoot = path.join(root, 'capsule');
			const sourcePath = path.join(capsuleRoot, 'adapter.mjs');
			const targetPath = path.join(root, 'config', 'adapter.mjs');
			await mkdir(capsuleRoot, { recursive: true });
			await writeFile(sourcePath, 'throw new Error("must remain private");');
			const adapter = createModuleAdapter(sourcePath, targetPath);
			await adapter.enable();
			await rm(capsuleRoot, { force: true, recursive: true });

			await expect(adapter.inspect()).resolves.toMatchObject({ state: 'update-required' });
			const result = spawnSync(
				process.execPath,
				[
					'--input-type=module',
					'--eval',
					'const loaded = await import(process.argv[1]); await loaded.OrchbeeAgentObservation({});',
					pathToFileURL(targetPath).href,
				],
				{ timeout: 1_000 },
			);
			expect(result.status).toBe(0);
			expect(result.stdout).toHaveLength(0);
			expect(result.stderr).toHaveLength(0);
			await expect(adapter.disable()).resolves.toEqual({ kind: 'attempted' });
		} finally {
			await rm(root, { force: true, recursive: true });
		}
	});

	it('preserves a conflicting or modified user-owned target', async () => {
		const root = await mkdtemp(path.join(tmpdir(), 'orchbee-agent-integration-'));
		try {
			const sourcePath = path.join(root, 'capsule', 'adapter.mjs');
			const targetPath = path.join(root, 'config', 'adapter.mjs');
			await mkdir(path.dirname(sourcePath), { recursive: true });
			await mkdir(path.dirname(targetPath), { recursive: true });
			await writeFile(sourcePath, 'export {};');
			await writeFile(targetPath, 'user configuration');
			const adapter = createModuleAdapter(sourcePath, targetPath);

			await expect(adapter.inspect()).resolves.toMatchObject({ state: 'conflict' });
			await expect(adapter.enable()).resolves.toEqual({ kind: 'conflict' });
			await expect(adapter.disable()).resolves.toEqual({ kind: 'conflict' });
			await rm(targetPath);
			await adapter.enable();
			await appendFile(targetPath, '\n// user edit');
			await expect(adapter.inspect()).resolves.toMatchObject({ state: 'conflict' });
			await expect(adapter.disable()).resolves.toEqual({ kind: 'conflict' });
		} finally {
			await rm(root, { force: true, recursive: true });
		}
	});

	it('copies a self-contained guarded extension and verifies ownership before removal', async () => {
		const root = await mkdtemp(path.join(tmpdir(), 'orchbee-agent-integration-'));
		try {
			const capsuleRoot = path.join(root, 'capsule');
			const sourcePath = path.join(capsuleRoot, 'extension');
			const runtimePath = path.join(capsuleRoot, 'scripts');
			const targetPath = path.join(root, 'config', 'orchbee-agent-observation');
			await mkdir(path.join(sourcePath, 'hooks'), { recursive: true });
			await mkdir(runtimePath, { recursive: true });
			await writeFile(path.join(sourcePath, 'extension.json'), '{"name":"orchbee"}');
			await writeFile(path.join(sourcePath, 'hooks', 'hooks.json'), '{"hooks":{}}');
			for (const fileName of [
				'forward-hook.mjs',
				'hook-sanitizers.mjs',
				'observation-transport.mjs',
			]) {
				await writeFile(path.join(runtimePath, fileName), '');
			}
			const adapter = createExtensionAdapter(sourcePath, runtimePath, targetPath);

			await expect(adapter.enable()).resolves.toEqual({ kind: 'attempted' });
			expect((await lstat(targetPath)).isDirectory()).toBe(true);
			expect((await lstat(path.join(targetPath, 'scripts', 'forward-hook.mjs'))).isFile()).toBe(
				true,
			);
			await expect(adapter.inspect()).resolves.toMatchObject({ state: 'enabled' });
			await rm(capsuleRoot, { force: true, recursive: true });
			await expect(adapter.inspect()).resolves.toMatchObject({ state: 'update-required' });
			await expect(adapter.disable()).resolves.toEqual({ kind: 'attempted' });
		} finally {
			await rm(root, { force: true, recursive: true });
		}
	});

	it('reports an exact application-bundle symlink as update-required and migrates it', async () => {
		const root = await mkdtemp(path.join(tmpdir(), 'orchbee-agent-integration-'));
		try {
			const resourceRoot = path.join(root, 'user-data', '0.4.6');
			const legacySourcePath = path.join(
				root,
				'Orchbee.app',
				'Contents',
				'Resources',
				'agent-observation-adapters',
				'bundle',
				'opencode',
				'orchbee-agent-observation.mjs',
			);
			const sourcePath = path.join(
				resourceRoot,
				'bundle',
				'opencode',
				'orchbee-agent-observation.mjs',
			);
			const targetPath = path.join(root, 'config', 'adapter.mjs');
			await mkdir(path.dirname(sourcePath), { recursive: true });
			await mkdir(path.dirname(targetPath), { recursive: true });
			await writeFile(sourcePath, 'export {};');
			await symlink(legacySourcePath, targetPath, 'file');
			const adapter = createModuleAdapter(sourcePath, targetPath, resourceRoot);

			await expect(adapter.inspect()).resolves.toMatchObject({ state: 'update-required' });
			await expect(adapter.enable()).resolves.toEqual({ kind: 'attempted' });
			const metadata = await lstat(targetPath);
			expect(metadata.isFile()).toBe(true);
			expect(metadata.isSymbolicLink()).toBe(false);
			await expect(adapter.inspect()).resolves.toMatchObject({ state: 'enabled' });
		} finally {
			await rm(root, { force: true, recursive: true });
		}
	});

	it('recognizes an exact older Capsule symlink but preserves a foreign lookalike', async () => {
		const root = await mkdtemp(path.join(tmpdir(), 'orchbee-agent-integration-'));
		try {
			const resourceRoot = path.join(root, 'capsules', '0.4.6');
			const sourceRelativePath = path.join('bundle', 'opencode', 'orchbee-agent-observation.mjs');
			const sourcePath = path.join(resourceRoot, sourceRelativePath);
			const targetPath = path.join(root, 'config', 'adapter.mjs');
			await mkdir(path.dirname(sourcePath), { recursive: true });
			await mkdir(path.dirname(targetPath), { recursive: true });
			await writeFile(sourcePath, 'export {};');
			await symlink(path.join(root, 'capsules', '0.3.0', sourceRelativePath), targetPath);
			const adapter = createModuleAdapter(sourcePath, targetPath, resourceRoot);

			await expect(adapter.inspect()).resolves.toMatchObject({ state: 'update-required' });
			await rm(targetPath);
			const foreignSourcePath = path.join(root, 'foreign', sourceRelativePath);
			await symlink(foreignSourcePath, targetPath);
			await expect(adapter.inspect()).resolves.toMatchObject({ state: 'conflict' });
			await expect(adapter.enable()).resolves.toEqual({ kind: 'conflict' });
			await expect(adapter.disable()).resolves.toEqual({ kind: 'conflict' });
			expect((await lstat(targetPath)).isSymbolicLink()).toBe(true);
		} finally {
			await rm(root, { force: true, recursive: true });
		}
	});

	it('keeps an absent target installable before the stable Capsule is materialized', async () => {
		const root = await mkdtemp(path.join(tmpdir(), 'orchbee-agent-integration-'));
		try {
			const adapter = createModuleAdapter(
				path.join(root, 'missing-capsule', 'adapter.mjs'),
				path.join(root, 'config', 'adapter.mjs'),
			);
			await expect(adapter.inspect()).resolves.toMatchObject({
				issue: null,
				state: 'not-installed',
			});
		} finally {
			await rm(root, { force: true, recursive: true });
		}
	});
});

function createModuleAdapter(
	sourcePath: string,
	targetPath: string,
	resourceRoot = path.dirname(sourcePath),
): GuardedLocalAgentIntegrationAdapter {
	return new GuardedLocalAgentIntegrationAdapter({
		commandRunner: new AvailableRunner(),
		entryKind: 'opencode-module',
		executableName: 'opencode',
		integrationId: 'opencode',
		legacyResourceRoots: [],
		managementMode: 'managed-local',
		resourceRoot,
		sourcePath,
		targetPath,
	});
}

function createExtensionAdapter(
	sourcePath: string,
	runtimeSourcePath: string,
	targetPath: string,
): GuardedLocalAgentIntegrationAdapter {
	return new GuardedLocalAgentIntegrationAdapter({
		commandRunner: new AvailableRunner(),
		entryKind: 'extension-directory',
		executableName: 'gemini',
		integrationId: 'gemini',
		legacyResourceRoots: [],
		managementMode: 'managed-local',
		resourceRoot: path.dirname(sourcePath),
		runtimeSourcePath,
		sourcePath,
		targetPath,
	});
}

class AvailableRunner implements AgentIntegrationCommandRunner {
	resolveExecutable(): Promise<string | null> {
		return Promise.resolve('/bin/agent');
	}

	run(): Promise<AgentIntegrationCommandResult> {
		return Promise.resolve({ kind: 'failed' });
	}
}
