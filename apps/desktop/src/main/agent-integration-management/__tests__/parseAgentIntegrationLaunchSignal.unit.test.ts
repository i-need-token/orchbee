import { describe, expect, it } from 'vitest';

import { parseAgentIntegrationLaunchSignal } from '../parseAgentIntegrationLaunchSignal';

describe('parseAgentIntegrationLaunchSignal', () => {
	it('accepts only the closed integration registry without retaining command content', () => {
		expect(parseAgentIntegrationLaunchSignal('{"integrationId":"claude-code","version":3}')).toBe(
			'claude-code',
		);
		expect(
			parseAgentIntegrationLaunchSignal(
				'{"command":"claude --prompt secret","integrationId":"claude-code","version":3}',
			),
		).toBeNull();
		expect(
			parseAgentIntegrationLaunchSignal('{"integrationId":"unknown-agent","version":3}'),
		).toBeNull();
		expect(
			parseAgentIntegrationLaunchSignal('{"integrationId":"claude-code","version":2}'),
		).toBeNull();
		expect(parseAgentIntegrationLaunchSignal('not-json')).toBeNull();
	});
});
