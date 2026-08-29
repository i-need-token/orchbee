import { describe, expect, it } from 'vitest';

import { enUSAgentIntegrationMessages } from '../../locales';
import { agentIntegrationMessages } from '../agent-integrations';

describe('agentIntegrationMessages', () => {
	it('uses canonical English defaults for every Agent-integration descriptor', () => {
		for (const descriptor of Object.values(agentIntegrationMessages)) {
			expect(descriptor.id).toBeTypeOf('string');
			expect(descriptor.defaultMessage).toBe(
				enUSAgentIntegrationMessages[descriptor.id as keyof typeof enUSAgentIntegrationMessages],
			);
		}
	});
});
