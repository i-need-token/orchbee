import { describe, expect, it } from 'vitest';

import {
	parseClaudeConfiguration,
	parseGrokConfiguration,
	parseGitHubCopilotConfiguration,
} from '../agent-integration-state-parsers';

const resourceOptions = {
	expectedVersion: '0.4.10',
	legacyResourceRoots: ['/Applications/Orchbee.app/Contents/Resources/agent-observation-adapters'],
	resourceRoot: '/user-data/agent-observation-adapters/0.4.10',
} as const;

describe('Agent integration state parsers', () => {
	it('requires the exact Claude plugin, version, and owned marketplace source', () => {
		const plugin = (version = '0.4.10', enabled = true) =>
			JSON.stringify([
				{ enabled: true, id: 'other@orchbee-local', version },
				{ enabled, id: 'orchbee-agent-observation@orchbee-local', version },
			]);
		const marketplace = (sourcePath: string) =>
			JSON.stringify([{ name: 'orchbee-local', path: sourcePath, source: 'directory' }]);

		expect(
			parseClaudeConfiguration(
				plugin('0.4.10', false),
				marketplace(resourceOptions.resourceRoot),
				resourceOptions,
			),
		).toMatchObject({ marketplace: 'current', pluginPresent: true, state: 'disabled' });
		expect(
			parseClaudeConfiguration(
				plugin('0.3.0'),
				marketplace(resourceOptions.resourceRoot),
				resourceOptions,
			),
		).toMatchObject({ state: 'update-required' });
		expect(
			parseClaudeConfiguration(
				plugin(),
				marketplace('/user-data/agent-observation-adapters/0.3.0'),
				resourceOptions,
			),
		).toMatchObject({ marketplace: 'legacy', state: 'update-required' });
		expect(
			parseClaudeConfiguration(plugin(), marketplace('/third-party'), resourceOptions),
		).toMatchObject({ state: 'conflict' });
		expect(parseClaudeConfiguration('[]', '[]', resourceOptions)).toMatchObject({
			marketplace: 'absent',
			pluginPresent: false,
			state: 'not-installed',
		});
		expect(parseClaudeConfiguration('{"not":"an array"}', '[]', resourceOptions)).toBeNull();
	});

	it('classifies only source- and version-proven Grok copies as managed', () => {
		const entry = (source: string, version = '0.4.10', status = 'installed') =>
			JSON.stringify([{ name: 'orchbee-agent-observation', source, status, version }]);
		const currentBundle = `${resourceOptions.resourceRoot}/bundle`;

		expect(parseGrokConfiguration(entry(currentBundle), resourceOptions)).toEqual({
			pluginPresent: true,
			state: 'enabled',
		});
		expect(
			parseGrokConfiguration(entry(currentBundle, '0.4.10', 'disabled'), resourceOptions),
		).toEqual({
			pluginPresent: true,
			state: 'disabled',
		});
		expect(
			parseGrokConfiguration(
				entry(`${resourceOptions.legacyResourceRoots[0]}/bundle`, '0.3.0'),
				resourceOptions,
			),
		).toMatchObject({ state: 'update-required' });
		expect(parseGrokConfiguration(entry('/third-party/bundle'), resourceOptions)).toMatchObject({
			state: 'conflict',
		});
		expect(parseGrokConfiguration('[]', resourceOptions)).toEqual({
			pluginPresent: false,
			state: 'not-installed',
		});
	});

	it('requires matching owned Copilot plugin and marketplace sources', () => {
		const plugin = (installedFrom: string, enabled = true) =>
			JSON.stringify({
				plugins: [
					{
						enabled,
						installedFrom,
						name: 'orchbee-agent-observation',
						source: 'live-marketplace:orchbee-local',
					},
				],
			});
		const marketplace = (sourcePath: string) =>
			JSON.stringify([{ name: 'orchbee-local', source: `Local: ${sourcePath}` }]);

		expect(
			parseGitHubCopilotConfiguration(
				plugin(resourceOptions.resourceRoot),
				marketplace(resourceOptions.resourceRoot),
				resourceOptions,
			),
		).toMatchObject({ marketplace: 'current', pluginPresent: true, state: 'enabled' });
		expect(
			parseGitHubCopilotConfiguration(
				plugin(resourceOptions.legacyResourceRoots[0]),
				marketplace(resourceOptions.legacyResourceRoots[0]),
				resourceOptions,
			),
		).toMatchObject({ marketplace: 'legacy', state: 'update-required' });
		expect(
			parseGitHubCopilotConfiguration(
				plugin('/third-party'),
				marketplace(resourceOptions.resourceRoot),
				resourceOptions,
			),
		).toMatchObject({ state: 'conflict' });
		expect(
			parseGitHubCopilotConfiguration(JSON.stringify({ plugins: [] }), '[]', resourceOptions),
		).toMatchObject({ marketplace: 'absent', state: 'not-installed' });
		expect(parseGitHubCopilotConfiguration('[]', '[]', resourceOptions)).toBeNull();
	});
});
