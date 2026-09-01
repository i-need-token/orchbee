import type { DesktopAgentIntegrationSnapshotItem } from '@orchbee/desktop-client-protocol';
import { describe, expect, it } from 'vitest';

import { isAgentIntegrationReminderEligible } from '../isAgentIntegrationReminderEligible';

const integration: DesktopAgentIntegrationSnapshotItem = {
	availability: 'available',
	integrationId: 'codex',
	issue: null,
	managementMode: 'managed-local',
	state: 'not-installed',
};

describe('isAgentIntegrationReminderEligible', () => {
	it('accepts only actionable local integrations with a trustworthy inspection', () => {
		expect(isAgentIntegrationReminderEligible(integration)).toBe(true);
		expect(isAgentIntegrationReminderEligible({ ...integration, state: 'disabled' })).toBe(true);
		expect(isAgentIntegrationReminderEligible({ ...integration, state: 'update-required' })).toBe(
			true,
		);
		expect(isAgentIntegrationReminderEligible({ ...integration, state: 'enabled' })).toBe(false);
		expect(
			isAgentIntegrationReminderEligible({ ...integration, managementMode: 'manual-only' }),
		).toBe(false);
		expect(isAgentIntegrationReminderEligible({ ...integration, issue: 'inspection-failed' })).toBe(
			false,
		);
		expect(isAgentIntegrationReminderEligible({ ...integration, availability: 'not-found' })).toBe(
			false,
		);
	});
});
