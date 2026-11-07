import { agentIntegrationMessages, type MessageDescriptor } from '@orchbee/i18n';

import { describe, expect, it } from 'vitest';

import type {
	AgentIntegrationExplanation,
	AgentIntegrationSnapshotItem,
} from '../../../../../agent-integration-management';
import {
	resolveAgentIntegrationDiagnosisView,
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

	it('names a definite finding as missing and an unfinished inspection as unknown', () => {
		const format = (descriptor: MessageDescriptor): string => descriptor.id ?? 'missing-id';
		const formatList = (items: readonly string[]): string => items.join(', ');
		const explained = (item: Partial<AgentIntegrationSnapshotItem>, timedOut = false): string =>
			resolveAgentIntegrationDiagnosisView(
				{
					explanation: {
						adapter: 'ready',
						detail: { kind: 'plain' },
						item: { ...integration, ...item },
						plan: [],
						timedOut,
					} satisfies AgentIntegrationExplanation,
					integrationId: 'codex',
					kind: 'explained',
				},
				format,
				formatList,
			).headline;
		const unknown = agentIntegrationMessages.findingUnknown.id;

		expect(explained({ availability: 'not-found', state: 'unknown' })).toBe(
			agentIntegrationMessages.findingAgentMissing.id,
		);
		expect(explained({ state: 'not-installed' })).toBe(
			agentIntegrationMessages.findingNotInstalled.id,
		);
		expect(explained({ availability: 'not-found', state: 'unknown' }, true)).toBe(unknown);
		expect(explained({ issue: 'inspection-failed', state: 'unknown' })).toBe(unknown);
		expect(explained({ state: 'unknown' })).toBe(unknown);
		expect(
			resolveAgentIntegrationDiagnosisView(
				{ code: 'control-unavailable', integrationId: 'codex', kind: 'failed' },
				format,
				formatList,
			).headline,
		).toBe(unknown);
	});
});
