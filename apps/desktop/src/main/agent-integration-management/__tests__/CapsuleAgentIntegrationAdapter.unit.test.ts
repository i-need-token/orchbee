import { describe, expect, it, vi } from 'vitest';

import { CapsuleAgentIntegrationAdapter } from '../CapsuleAgentIntegrationAdapter';
import type { AgentObservationAdapterCapsule } from '../FileAgentObservationAdapterCapsule';
import type { AgentIntegrationAdapter } from '../agent-integration-adapter';

describe('CapsuleAgentIntegrationAdapter', () => {
	it('materializes only for explicit enable and blocks mutation when publication fails', async () => {
		const enable = vi.fn<AgentIntegrationAdapter['enable']>(async () => ({ kind: 'attempted' }));
		const delegate = createDelegate('not-installed', enable);
		const prepare = vi.fn<AgentObservationAdapterCapsule['prepare']>(async () => false);
		const isReady = vi.fn<AgentObservationAdapterCapsule['isReady']>(async () => false);
		const adapter = new CapsuleAgentIntegrationAdapter(delegate, {
			isReady,
			prepare,
			rootPath: '/stable/capsule',
		});

		await adapter.inspect();
		await adapter.disable();
		expect(prepare).not.toHaveBeenCalled();
		expect(isReady).not.toHaveBeenCalled();
		await expect(adapter.enable()).resolves.toEqual({ kind: 'failed' });
		expect(prepare).toHaveBeenCalledOnce();
		expect(enable).not.toHaveBeenCalled();
	});

	it('reports an enabled entry as update-required when its Capsule is gone', async () => {
		const adapter = new CapsuleAgentIntegrationAdapter(createDelegate('enabled'), {
			isReady: async () => false,
			prepare: async () => false,
			rootPath: '/stable/capsule',
		});

		await expect(adapter.inspect()).resolves.toMatchObject({ state: 'update-required' });
	});
});

function createDelegate(
	state: 'enabled' | 'not-installed',
	enable: AgentIntegrationAdapter['enable'] = vi.fn<AgentIntegrationAdapter['enable']>(
		async () => ({
			kind: 'attempted',
		}),
	),
): AgentIntegrationAdapter {
	return {
		disable: vi.fn<AgentIntegrationAdapter['disable']>(async () => ({ kind: 'attempted' })),
		enable,
		inspect: vi.fn<AgentIntegrationAdapter['inspect']>(async () => ({
			availability: 'available',
			integrationId: 'codex',
			issue: null,
			managementMode: 'managed-local',
			state,
		})),
		integrationId: 'codex',
		managementMode: 'managed-local',
	};
}
