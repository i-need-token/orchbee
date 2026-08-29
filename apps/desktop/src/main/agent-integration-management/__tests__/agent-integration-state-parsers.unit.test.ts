import { describe, expect, it } from 'vitest';

import {
	parseClaudeState,
	parseCodexState,
	parseGrokState,
} from '../agent-integration-state-parsers';

describe('Agent integration state parsers', () => {
	it('parses only the exact managed plugin identity from bounded JSON shapes', () => {
		expect(
			parseClaudeState(
				JSON.stringify([
					{ enabled: true, id: 'other@orchbee-local' },
					{ enabled: false, id: 'orchbee-agent-observation@orchbee-local' },
				]),
			),
		).toBe('disabled');
		expect(
			parseCodexState(
				JSON.stringify({
					installed: [
						{
							enabled: true,
							pluginId: 'orchbee-agent-observation@orchbee-local',
						},
					],
				}),
			),
		).toBe('enabled');
		expect(
			parseGrokState(JSON.stringify([{ name: 'orchbee-agent-observation', status: 'disabled' }])),
		).toBe('disabled');
		expect(parseClaudeState('{"not":"an array"}')).toBeNull();
	});
});
