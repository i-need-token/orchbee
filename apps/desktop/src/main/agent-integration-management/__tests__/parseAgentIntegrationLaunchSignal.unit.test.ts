import { describe, expect, it } from 'vitest';

import { parseAgentIntegrationLaunchSignal } from '../parseAgentIntegrationLaunchSignal';

describe('parseAgentIntegrationLaunchSignal', () => {
	it('accepts only the closed integration registry without retaining command content', () => {
		expect(
			parseAgentIntegrationLaunchSignal(
				'{"integrationId":"claude-code","kind":"launch","version":3}',
			),
		).toEqual({
			integrationId: 'claude-code',
			signatureId: 'shell-preexec.executable.claude',
			source: 'terminal-command',
		});
		expect(
			parseAgentIntegrationLaunchSignal(
				'{"command":"claude --prompt secret","integrationId":"claude-code","kind":"launch","version":3}',
			),
		).toBeNull();
		expect(
			parseAgentIntegrationLaunchSignal(
				'{"integrationId":"unknown-agent","kind":"launch","version":3}',
			),
		).toBeNull();
		expect(
			parseAgentIntegrationLaunchSignal(
				'{"integrationId":"claude-code","kind":"launch","version":2}',
			),
		).toBeNull();
		expect(parseAgentIntegrationLaunchSignal('not-json')).toBeNull();
	});
});
