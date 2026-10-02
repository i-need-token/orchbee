import path from 'node:path';

import { DESKTOP_AGENT_INTEGRATION_IDS } from '@orchbee/desktop-client-protocol';
import { describe, expect, it } from 'vitest';

import type {
	AgentIntegrationCommandResult,
	AgentIntegrationCommandRunner,
} from '../NodeAgentIntegrationCommandRunner';
import { createDesktopAgentIntegrationAdapters } from '../createDesktopAgentIntegrationAdapters';

describe('Desktop Agent integration adapters', () => {
	it('constructs the complete closed registry in canonical order', () => {
		const adapters = createDesktopAgentIntegrationAdapters({
			capsuleRoot: '/user-data/agent-observation-adapters/0.4.17',
			commandRunner: new FakeRunner(),
			homeDirectory: '/home/person',
			platform: 'darwin',
			resourceRoot: '/application/agent-observation-adapters',
		});

		expect(adapters.map(({ integrationId }) => integrationId)).toEqual(
			DESKTOP_AGENT_INTEGRATION_IDS,
		);
	});

	it('keeps platform policy outside provider inspection output', async () => {
		const adapters = createDesktopAgentIntegrationAdapters({
			capsuleRoot: '/user-data/agent-observation-adapters/0.4.17',
			commandRunner: new FakeRunner(),
			homeDirectory: '/home/person',
			platform: 'win32',
			resourceRoot: '/application/agent-observation-adapters',
		});

		const snapshot = await adapters[0]?.inspect();

		expect(snapshot).toMatchObject({
			integrationId: 'claude-code',
			issue: 'unsupported-platform',
			managementMode: 'manual-only',
		});
		expect(await adapters[0]?.enable()).toEqual({ kind: 'manual-action-required' });
	});

	it('uses the reviewed Copilot marketplace flow only after explicit enable', async () => {
		const runner = new SequencedRunner([
			{ kind: 'completed', stdout: JSON.stringify({ plugins: [] }) },
			{ kind: 'completed', stdout: '[]' },
			{ kind: 'completed', stdout: '' },
			{ kind: 'completed', stdout: '' },
		]);
		const resourceRoot = '/application/agent-observation-adapters';
		const capsuleRoot = '/user-data/agent-observation-adapters/0.4.17';
		const adapters = createDesktopAgentIntegrationAdapters({
			capsule: {
				isReady: async () => true,
				prepare: async () => true,
				rootPath: capsuleRoot,
			},
			capsuleRoot,
			commandRunner: runner,
			homeDirectory: '/home/person',
			platform: 'darwin',
			resourceRoot,
		});
		const copilot = adapters.find(({ integrationId }) => integrationId === 'github-copilot');
		if (copilot === undefined) throw new Error('Missing GitHub Copilot adapter.');

		await expect(copilot.enable()).resolves.toEqual({ kind: 'attempted' });
		expect(runner.commands).toEqual([
			['plugins', 'list', '--kind', 'plugin', '--json'],
			['plugins', 'marketplace', 'list', '--json'],
			['plugins', 'marketplace', 'add', capsuleRoot],
			['plugins', 'install', 'orchbee-agent-observation@orchbee-local'],
		]);
	});

	it('migrates only an exact legacy Claude marketplace after explicit enable', async () => {
		const legacyRoot = '/application/agent-observation-adapters';
		const capsuleRoot = '/user-data/agent-observation-adapters/0.4.17';
		const runner = new SequencedRunner([
			{
				kind: 'completed',
				stdout: JSON.stringify([
					{
						enabled: true,
						id: 'orchbee-agent-observation@orchbee-local',
						version: '0.3.0',
					},
				]),
			},
			{
				kind: 'completed',
				stdout: JSON.stringify([{ name: 'orchbee-local', path: legacyRoot, source: 'directory' }]),
			},
			...completedResults(5),
		]);
		const claude = createAdapters(runner, capsuleRoot, legacyRoot).find(
			({ integrationId }) => integrationId === 'claude-code',
		);
		if (claude === undefined) throw new Error('Missing Claude adapter.');

		await expect(claude.enable()).resolves.toEqual({ kind: 'attempted' });
		expect(runner.commands).toEqual([
			['plugin', 'list', '--json'],
			['plugin', 'marketplace', 'list', '--json'],
			['plugin', 'uninstall', 'orchbee-agent-observation@orchbee-local', '--scope', 'user'],
			['plugin', 'marketplace', 'remove', 'orchbee-local', '--scope', 'user'],
			['plugin', 'marketplace', 'add', capsuleRoot, '--scope', 'user'],
			['plugin', 'install', 'orchbee-agent-observation@orchbee-local', '--scope', 'user'],
			['plugin', 'enable', 'orchbee-agent-observation@orchbee-local', '--scope', 'user'],
		]);
	});

	it('preserves a foreign Claude marketplace collision without mutation', async () => {
		const runner = new SequencedRunner([
			{
				kind: 'completed',
				stdout: JSON.stringify([
					{
						enabled: true,
						id: 'orchbee-agent-observation@orchbee-local',
						version: '0.4.17',
					},
				]),
			},
			{
				kind: 'completed',
				stdout: JSON.stringify([
					{ name: 'orchbee-local', path: '/third-party', source: 'directory' },
				]),
			},
		]);
		const claude = createAdapters(
			runner,
			'/user-data/agent-observation-adapters/0.4.17',
			'/application/agent-observation-adapters',
		).find(({ integrationId }) => integrationId === 'claude-code');
		if (claude === undefined) throw new Error('Missing Claude adapter.');

		await expect(claude.enable()).resolves.toEqual({ kind: 'conflict' });
		expect(runner.commands).toHaveLength(2);
	});

	it('replaces a source-proven legacy Grok copy after explicit enable', async () => {
		const legacyRoot = '/application/agent-observation-adapters';
		const capsuleRoot = '/user-data/agent-observation-adapters/0.4.17';
		const runner = new SequencedRunner([
			{
				kind: 'completed',
				stdout: JSON.stringify([
					{
						name: 'orchbee-agent-observation',
						source: `${legacyRoot}/bundle`,
						status: 'installed',
						version: '0.3.0',
					},
				]),
			},
			...completedResults(2),
		]);
		const grok = createAdapters(runner, capsuleRoot, legacyRoot).find(
			({ integrationId }) => integrationId === 'grok',
		);
		if (grok === undefined) throw new Error('Missing Grok adapter.');

		await expect(grok.enable()).resolves.toEqual({ kind: 'attempted' });
		expect(runner.commands).toEqual([
			['plugin', 'list', '--json'],
			['plugin', 'uninstall', 'orchbee-agent-observation', '--confirm'],
			['plugin', 'install', path.join(capsuleRoot, 'bundle'), '--trust'],
		]);
	});

	it('repoints an exact legacy Copilot marketplace after explicit enable', async () => {
		const legacyRoot = '/application/agent-observation-adapters';
		const capsuleRoot = '/user-data/agent-observation-adapters/0.4.17';
		const runner = new SequencedRunner([
			{
				kind: 'completed',
				stdout: JSON.stringify({
					plugins: [
						{
							enabled: true,
							installedFrom: legacyRoot,
							name: 'orchbee-agent-observation',
							source: 'live-marketplace:orchbee-local',
						},
					],
				}),
			},
			{
				kind: 'completed',
				stdout: JSON.stringify([{ name: 'orchbee-local', source: `Local: ${legacyRoot}` }]),
			},
			...completedResults(4),
		]);
		const copilot = createAdapters(runner, capsuleRoot, legacyRoot).find(
			({ integrationId }) => integrationId === 'github-copilot',
		);
		if (copilot === undefined) throw new Error('Missing GitHub Copilot adapter.');

		await expect(copilot.enable()).resolves.toEqual({ kind: 'attempted' });
		expect(runner.commands).toEqual([
			['plugins', 'list', '--kind', 'plugin', '--json'],
			['plugins', 'marketplace', 'list', '--json'],
			['plugins', 'remove', 'orchbee-agent-observation@orchbee-local', '--plugin'],
			['plugins', 'marketplace', 'remove', 'orchbee-local'],
			['plugins', 'marketplace', 'add', capsuleRoot],
			['plugins', 'install', 'orchbee-agent-observation@orchbee-local'],
		]);
	});
});

function createAdapters(
	runner: AgentIntegrationCommandRunner,
	capsuleRoot: string,
	resourceRoot: string,
) {
	return createDesktopAgentIntegrationAdapters({
		capsule: {
			isReady: async () => true,
			prepare: async () => true,
			rootPath: capsuleRoot,
		},
		capsuleRoot,
		commandRunner: runner,
		homeDirectory: '/home/person',
		platform: 'darwin',
		resourceRoot,
	});
}

function completedResults(count: number): AgentIntegrationCommandResult[] {
	return Array.from({ length: count }, () => ({ kind: 'completed', stdout: '' }));
}

class FakeRunner implements AgentIntegrationCommandRunner {
	resolveExecutable(): Promise<string | null> {
		return Promise.resolve('/bin/agent');
	}

	run(): Promise<AgentIntegrationCommandResult> {
		return Promise.resolve({ kind: 'completed', stdout: '[]' });
	}
}

class SequencedRunner implements AgentIntegrationCommandRunner {
	readonly commands: string[][] = [];
	readonly #results: AgentIntegrationCommandResult[];

	constructor(results: AgentIntegrationCommandResult[]) {
		this.#results = results;
	}

	resolveExecutable(): Promise<string | null> {
		return Promise.resolve('/bin/copilot');
	}

	run(
		_executablePath: string,
		commandArguments: readonly string[],
	): Promise<AgentIntegrationCommandResult> {
		this.commands.push([...commandArguments]);
		return Promise.resolve(this.#results.shift() ?? { kind: 'failed' });
	}
}
