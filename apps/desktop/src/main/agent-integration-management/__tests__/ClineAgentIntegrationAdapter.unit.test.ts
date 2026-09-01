import { mkdir, mkdtemp, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';

import { describe, expect, it } from 'vitest';

import type {
	AgentIntegrationCommandResult,
	AgentIntegrationCommandRunner,
} from '../NodeAgentIntegrationCommandRunner';
import { ClineAgentIntegrationAdapter } from '../ClineAgentIntegrationAdapter';

describe('ClineAgentIntegrationAdapter', () => {
	it('uses Cline commands only after explicit enable or disable actions', async () => {
		const root = await mkdtemp(path.join(tmpdir(), 'orchbee-cline-integration-'));
		try {
			const homeDirectory = path.join(root, 'home');
			const resourceRoot = path.join(root, 'resources');
			await writeManagedPackage(path.join(resourceRoot, 'bundle'));
			const runner = new RecordingRunner();
			const adapter = new ClineAgentIntegrationAdapter({
				commandRunner: runner,
				homeDirectory,
				managementMode: 'managed-local',
				resourceRoot,
			});

			await expect(adapter.inspect()).resolves.toMatchObject({
				availability: 'available',
				issue: null,
				state: 'not-installed',
			});
			expect(runner.commands).toEqual([]);
			await expect(adapter.enable()).resolves.toEqual({ kind: 'attempted' });
			expect(runner.commands).toEqual([
				['plugin', 'install', path.join(resourceRoot, 'bundle'), '--json'],
			]);

			const installedPackage = path.join(
				homeDirectory,
				'.cline',
				'plugins',
				'_installed',
				'local',
				'bundle-reviewed',
				'package',
			);
			await writeManagedPackage(installedPackage);
			await mkdir(path.join(installedPackage, 'cline'), { recursive: true });
			await writeFile(
				path.join(installedPackage, 'cline', 'orchbee-agent-observation.js'),
				'export default {};',
				'utf8',
			);
			await expect(adapter.inspect()).resolves.toMatchObject({ state: 'enabled' });
			await expect(adapter.disable()).resolves.toEqual({ kind: 'attempted' });
			expect(runner.commands.at(-1)).toEqual([
				'plugin',
				'uninstall',
				'orchbee-agent-observation',
				'--json',
			]);
		} finally {
			await rm(root, { force: true, recursive: true });
		}
	});

	it('preserves user-owned or unmarked Cline plugin entries as conflicts', async () => {
		const root = await mkdtemp(path.join(tmpdir(), 'orchbee-cline-integration-'));
		try {
			const homeDirectory = path.join(root, 'home');
			const resourceRoot = path.join(root, 'resources');
			await writeManagedPackage(path.join(resourceRoot, 'bundle'));
			const pluginRoot = path.join(homeDirectory, '.cline', 'plugins');
			await mkdir(pluginRoot, { recursive: true });
			await writeFile(
				path.join(pluginRoot, 'orchbee-agent-observation.js'),
				'// user-owned',
				'utf8',
			);
			const adapter = new ClineAgentIntegrationAdapter({
				commandRunner: new RecordingRunner(),
				homeDirectory,
				managementMode: 'managed-local',
				resourceRoot,
			});
			await expect(adapter.inspect()).resolves.toMatchObject({ state: 'conflict' });
			await expect(adapter.enable()).resolves.toEqual({ kind: 'conflict' });
		} finally {
			await rm(root, { force: true, recursive: true });
		}
	});

	it('classifies an older owned contract as update-required and replaces it only on enable', async () => {
		const root = await mkdtemp(path.join(tmpdir(), 'orchbee-cline-integration-'));
		try {
			const homeDirectory = path.join(root, 'home');
			const resourceRoot = path.join(root, 'resources');
			await writeManagedPackage(path.join(resourceRoot, 'bundle'));
			const installedPackage = path.join(
				homeDirectory,
				'.cline',
				'plugins',
				'_installed',
				'local',
				'bundle-old',
				'package',
			);
			await writeManagedPackage(installedPackage, 1);
			await mkdir(path.join(installedPackage, 'cline'), { recursive: true });
			await writeFile(
				path.join(installedPackage, 'cline', 'orchbee-agent-observation.js'),
				'export default {};',
				'utf8',
			);
			const runner = new RecordingRunner();
			const adapter = new ClineAgentIntegrationAdapter({
				commandRunner: runner,
				homeDirectory,
				managementMode: 'managed-local',
				resourceRoot,
			});

			await expect(adapter.inspect()).resolves.toMatchObject({ state: 'update-required' });
			expect(runner.commands).toEqual([]);
			await expect(adapter.enable()).resolves.toEqual({ kind: 'attempted' });
			expect(runner.commands).toEqual([
				['plugin', 'uninstall', 'orchbee-agent-observation', '--json'],
				['plugin', 'install', path.join(resourceRoot, 'bundle'), '--json'],
			]);
		} finally {
			await rm(root, { force: true, recursive: true });
		}
	});
});

async function writeManagedPackage(packageRoot: string, contractVersion = 3): Promise<void> {
	await mkdir(packageRoot, { recursive: true });
	await writeFile(
		path.join(packageRoot, 'package.json'),
		JSON.stringify({
			name: 'orchbee-agent-observation',
			orchbeeAgentObservation: { contractVersion },
		}),
		'utf8',
	);
}

class RecordingRunner implements AgentIntegrationCommandRunner {
	readonly commands: string[][] = [];

	resolveExecutable(): Promise<string | null> {
		return Promise.resolve('/bin/cline');
	}

	run(
		_executablePath: string,
		commandArguments: readonly string[],
	): Promise<AgentIntegrationCommandResult> {
		this.commands.push([...commandArguments]);
		return Promise.resolve({ kind: 'completed', stdout: '{}' });
	}
}
