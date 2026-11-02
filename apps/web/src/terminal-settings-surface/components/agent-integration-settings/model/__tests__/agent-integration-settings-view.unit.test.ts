import { agentIntegrationMessages, type MessageDescriptor } from '@orchbee/i18n';

import { describe, expect, it, vi } from 'vitest';

import {
	AgentIntegrationReminderController,
	type AgentIntegrationManagementControl,
	type AgentIntegrationSnapshotItem,
} from '../../../../../agent-integration-management';
import {
	enableAgentIntegration,
	resolveAgentIntegrationFailureMessage,
	resolveAgentIntegrationStatusView,
} from '../agent-integration-settings-view';

const integration: AgentIntegrationSnapshotItem = {
	availability: 'available',
	integrationId: 'codex',
	issue: null,
	managementMode: 'managed-local',
	state: 'enabled',
};

describe('Agent integration Settings view model', () => {
	it('maps state, inspection, availability, and manual policy to distinct status tones', () => {
		const format = (descriptor: MessageDescriptor): string => descriptor.id ?? 'missing-id';

		expect(resolveAgentIntegrationStatusView(integration, format)).toEqual({
			status: agentIntegrationMessages.statusEnabled.id,
			statusTone: 'positive',
		});
		expect(
			resolveAgentIntegrationStatusView(
				{ ...integration, issue: 'inspection-failed', state: 'unknown' },
				format,
			),
		).toMatchObject({ statusTone: 'warning' });
		expect(
			resolveAgentIntegrationStatusView(
				{ ...integration, availability: 'not-found', state: 'unknown' },
				format,
			),
		).toMatchObject({ statusTone: 'neutral' });
		expect(
			resolveAgentIntegrationStatusView({ ...integration, managementMode: 'manual-only' }, format),
		).toMatchObject({ statusTone: 'neutral' });
		expect(resolveAgentIntegrationFailureMessage('verification-failed')).toBe(
			agentIntegrationMessages.failureVerificationFailed,
		);
	});

	it('restores a dismissed reminder only after Desktop verifies enablement', async () => {
		const controller = new AgentIntegrationReminderController({
			getItem: () => null,
			setItem: () => undefined,
		});
		controller.dismiss('codex');
		const enable = vi.fn<AgentIntegrationManagementControl['enable']>(async () => ({
			kind: 'completed',
			snapshot: { integrations: [], launchedIntegrationIds: [], revision: 1 },
		}));
		const control = {
			disable: vi.fn<AgentIntegrationManagementControl['disable']>(),
			enable,
			getSnapshot: vi.fn<AgentIntegrationManagementControl['getSnapshot']>(),
			refresh: vi.fn<AgentIntegrationManagementControl['refresh']>(),
			subscribe: vi.fn<AgentIntegrationManagementControl['subscribe']>(),
		} satisfies AgentIntegrationManagementControl;

		await enableAgentIntegration(control, controller, 'codex');

		expect(enable).toHaveBeenCalledWith('codex');
		expect(controller.getSnapshot().dismissedIntegrationIds).toEqual([]);
		controller.dispose();
	});
});
