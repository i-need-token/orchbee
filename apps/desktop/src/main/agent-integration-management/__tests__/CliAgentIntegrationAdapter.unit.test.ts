import { describe, expect, it, vi } from 'vitest';

import type {
	AgentIntegrationCommandResult,
	AgentIntegrationCommandRunner,
} from '../NodeAgentIntegrationCommandRunner';
import { CliAgentIntegrationAdapter } from '../CliAgentIntegrationAdapter';

describe('CliAgentIntegrationAdapter', () => {
	it('uses read-only inspection to distinguish unavailable, enabled, and malformed states', async () => {
		const runner = new FakeRunner();
		const adapter = createAdapter(runner);

		await expect(adapter.inspect()).resolves.toMatchObject({
			availability: 'not-found',
			issue: null,
			state: 'unknown',
		});
		runner.executablePath = '/bin/codex';
		runner.result = { kind: 'completed', stdout: 'enabled' };
		await expect(adapter.inspect()).resolves.toMatchObject({
			availability: 'available',
			issue: null,
			state: 'enabled',
		});
		runner.result = { kind: 'completed', stdout: 'unexpected' };
		await expect(adapter.inspect()).resolves.toMatchObject({
			issue: 'inspection-failed',
			state: 'unknown',
		});
	});

	it('never invokes a mutation operation in manual-only mode', async () => {
		const runner = new FakeRunner();
		runner.executablePath = '/bin/codex';
		const enable = vi.fn<() => Promise<{ readonly kind: 'attempted' }>>(async () => ({
			kind: 'attempted',
		}));
		const adapter = new CliAgentIntegrationAdapter({
			commandRunner: runner,
			disable: enable,
			enable,
			executableName: 'codex',
			inspectArguments: ['plugin', 'list'],
			integrationId: 'codex',
			managementMode: 'manual-only',
			parseState: () => 'not-installed',
			resourceRoot: '/resource',
		});

		await expect(adapter.enable()).resolves.toEqual({ kind: 'manual-action-required' });
		expect(enable).not.toHaveBeenCalled();
	});
});

function createAdapter(runner: AgentIntegrationCommandRunner): CliAgentIntegrationAdapter {
	return new CliAgentIntegrationAdapter({
		commandRunner: runner,
		disable: async () => ({ kind: 'attempted' }),
		enable: async () => ({ kind: 'attempted' }),
		executableName: 'codex',
		inspectArguments: ['plugin', 'list'],
		integrationId: 'codex',
		managementMode: 'managed-local',
		parseState: (stdout) => (stdout === 'enabled' ? 'enabled' : null),
		resourceRoot: '/resource',
	});
}

class FakeRunner implements AgentIntegrationCommandRunner {
	executablePath: string | null = null;
	result: AgentIntegrationCommandResult = { kind: 'failed' };

	resolveExecutable(): Promise<string | null> {
		return Promise.resolve(this.executablePath);
	}

	run(): Promise<AgentIntegrationCommandResult> {
		return Promise.resolve(this.result);
	}
}
