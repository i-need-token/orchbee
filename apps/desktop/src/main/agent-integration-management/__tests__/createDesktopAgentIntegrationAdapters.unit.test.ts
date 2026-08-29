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
		const adapters = createDesktopAgentIntegrationAdapters({
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
			['plugins', 'marketplace', 'add', resourceRoot],
			['plugins', 'install', 'orchbee-agent-observation@orchbee-local'],
		]);
	});
});

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
