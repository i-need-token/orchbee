import { describe, expect, it } from 'vitest';

import type {
	AgentIntegrationCommandResult,
	AgentIntegrationCommandRunner,
} from '../NodeAgentIntegrationCommandRunner';
import { createDesktopAgentIntegrationAdapters } from '../createDesktopAgentIntegrationAdapters';

describe('Desktop Agent integration adapters', () => {
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
});

class FakeRunner implements AgentIntegrationCommandRunner {
	resolveExecutable(): Promise<string | null> {
		return Promise.resolve('/bin/agent');
	}

	run(): Promise<AgentIntegrationCommandResult> {
		return Promise.resolve({ kind: 'completed', stdout: '[]' });
	}
}
