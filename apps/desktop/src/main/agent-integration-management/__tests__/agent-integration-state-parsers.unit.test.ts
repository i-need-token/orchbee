import { describe, expect, it } from 'vitest';

import {
	parseClaudeState,
	parseCodexState,
	parseGrokState,
	parseGitHubCopilotState,
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

	it('accepts only the exact enabled GitHub Copilot marketplace installation', () => {
		const resourceRoot = '/application/agent-observation-adapters';
		const exact = {
			plugins: [
				{
					enabled: true,
					installedFrom: resourceRoot,
					name: 'orchbee-agent-observation',
					source: 'live-marketplace:orchbee-local',
				},
			],
		};
		expect(parseGitHubCopilotState(JSON.stringify(exact), resourceRoot)).toBe('enabled');
		expect(
			parseGitHubCopilotState(
				JSON.stringify({
					plugins: [{ ...exact.plugins[0], installedFrom: '/different/resource' }],
				}),
				resourceRoot,
			),
		).toBe('conflict');
		expect(parseGitHubCopilotState(JSON.stringify({ plugins: [] }), resourceRoot)).toBe(
			'not-installed',
		);
		expect(parseGitHubCopilotState('[]', resourceRoot)).toBeNull();
	});
});
