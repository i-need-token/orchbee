import { agentIntegrationMessages, type MessageDescriptor } from '@orchbee/i18n';

import { describe, expect, it } from 'vitest';

import type { AgentIntegrationSnapshotItem } from '../../../../../agent-integration-management';
import {
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
});
