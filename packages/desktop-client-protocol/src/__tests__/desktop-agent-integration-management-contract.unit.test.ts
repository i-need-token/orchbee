import { describe, expect, it } from 'vitest';

import {
	DESKTOP_AGENT_INTEGRATION_IDS,
	DesktopAgentIntegrationManagementMainEventSchema,
	DesktopAgentIntegrationManagementRendererCommandSchema,
	DesktopAgentIntegrationManagementSnapshotSchema,
} from '../desktop-agent-integration-management-contract';

describe('Desktop Agent integration management contract', () => {
	it('accepts canonical inventory and explicit mutation commands', () => {
		expect(
			DesktopAgentIntegrationManagementRendererCommandSchema.parse({
				requestId: 'integration:1',
				type: 'desktop-agent-integration-refresh-requested',
			}),
		).toEqual({
			requestId: 'integration:1',
			type: 'desktop-agent-integration-refresh-requested',
		});
		expect(
			DesktopAgentIntegrationManagementRendererCommandSchema.parse({
				integrationId: 'codex',
				requestId: 'integration:2',
				type: 'desktop-agent-integration-enable-requested',
			}),
		).toMatchObject({ integrationId: 'codex' });
	});

	it('requires every integration exactly once in canonical order', () => {
		const integrations = DESKTOP_AGENT_INTEGRATION_IDS.map((integrationId) => ({
			availability: 'available' as const,
			integrationId,
			issue: null,
			managementMode: 'managed-local' as const,
			state: 'not-installed' as const,
		}));
		expect(
			DesktopAgentIntegrationManagementSnapshotSchema.parse({
				integrations,
				launchedIntegrationIds: ['grok', 'claude-code'],
				revision: 0,
			}),
		).toEqual({ integrations, launchedIntegrationIds: ['grok', 'claude-code'], revision: 0 });
		expect(
			DesktopAgentIntegrationManagementSnapshotSchema.safeParse({
				integrations: [...integrations].reverse(),
				launchedIntegrationIds: [],
				revision: 0,
			}).success,
		).toBe(false);
		expect(
			DesktopAgentIntegrationManagementSnapshotSchema.safeParse({
				integrations,
				launchedIntegrationIds: ['codex', 'codex'],
				revision: 0,
			}).success,
		).toBe(false);
	});

	it('publishes only a bounded Agent launch identity without command content', () => {
		const integration = {
			availability: 'available' as const,
			integrationId: 'claude-code' as const,
			issue: null,
			managementMode: 'managed-local' as const,
			state: 'not-installed' as const,
		};
		expect(
			DesktopAgentIntegrationManagementMainEventSchema.parse({
				integration,
				integrationId: 'claude-code',
				revision: 3,
				type: 'desktop-agent-integration-launch-observed',
			}),
		).toEqual({
			integration,
			integrationId: 'claude-code',
			revision: 3,
			type: 'desktop-agent-integration-launch-observed',
		});
		expect(
			DesktopAgentIntegrationManagementMainEventSchema.safeParse({
				command: 'claude --prompt secret',
				integration,
				integrationId: 'claude-code',
				revision: 3,
				type: 'desktop-agent-integration-launch-observed',
			}).success,
		).toBe(false);
		expect(
			DesktopAgentIntegrationManagementMainEventSchema.safeParse({
				integration,
				integrationId: 'claude-code',
				revision: -1,
				type: 'desktop-agent-integration-launch-observed',
			}).success,
		).toBe(false);
	});

	it('rejects raw diagnostic fields at the renderer boundary', () => {
		expect(
			DesktopAgentIntegrationManagementMainEventSchema.safeParse({
				requestId: 'integration:3',
				result: {
					code: 'operation-failed',
					integrationId: 'grok',
					kind: 'failed',
					rawStderr: 'private path',
					snapshot: {
						integrations: DESKTOP_AGENT_INTEGRATION_IDS.map((integrationId) => ({
							availability: 'unknown',
							integrationId,
							issue: 'inspection-failed',
							managementMode: 'managed-local',
							state: 'unknown',
						})),
						launchedIntegrationIds: [],
						revision: 1,
					},
				},
				type: 'desktop-agent-integration-responded',
			}).success,
		).toBe(false);
	});
});
