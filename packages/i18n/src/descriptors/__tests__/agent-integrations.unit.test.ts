import { describe, expect, it } from 'vitest';

import { enUSAgentIntegrationMessages } from '../../locales';
import { agentIntegrationMessages } from '../agent-integrations';

describe('agentIntegrationMessages', () => {
	it('keeps the reminder settings-only and exposes the update-required state', () => {
		expect(agentIntegrationMessages.reminderOpenSettings).toMatchObject({
			defaultMessage: enUSAgentIntegrationMessages['agent.integrations.reminder.openSettings'],
			id: 'agent.integrations.reminder.openSettings',
		});
		expect(agentIntegrationMessages.statusUpdateRequired).toMatchObject({
			defaultMessage: enUSAgentIntegrationMessages['agent.integrations.status.updateRequired'],
			id: 'agent.integrations.status.updateRequired',
		});
	});
});
