// @vitest-environment jsdom

import { OrchbeeI18nProvider } from '@orchbee/i18n';
import {
	DESKTOP_AGENT_INTEGRATION_IDS,
	type DesktopAgentIntegrationManagementSnapshot,
} from '@orchbee/desktop-client-protocol';
import { cleanup, renderHook } from '@testing-library/react';
import { createElement, type ReactNode } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import type { AgentIntegrationManagementControl } from '../../../../../agent-integration-management';
import { useAgentIntegrationSettings } from '../useAgentIntegrationSettings';

const protocolSnapshot: DesktopAgentIntegrationManagementSnapshot = {
	integrations: DESKTOP_AGENT_INTEGRATION_IDS.map((integrationId) => ({
		availability: integrationId === 'codex' ? 'available' : 'not-found',
		integrationId,
		issue: null,
		managementMode: 'managed-local',
		state: integrationId === 'codex' ? 'not-installed' : 'unknown',
	})),
	launchedIntegrationIds: [],
	revision: 1,
};

function I18nWrapper({ children }: { readonly children: ReactNode }) {
	return createElement(OrchbeeI18nProvider, { locale: 'en-US' }, children);
}

afterEach(() => cleanup());

describe('useAgentIntegrationSettings', () => {
	it('projects all canonical Agents without triggering installation during render', () => {
		const enable = vi.fn<AgentIntegrationManagementControl['enable']>();
		const clientSnapshot = {
			...protocolSnapshot,
			failureCode: null,
			failureIntegrationId: null,
			pending: false,
			pendingIntegrationId: null,
		} as const;
		const control: AgentIntegrationManagementControl = {
			disable: vi.fn<AgentIntegrationManagementControl['disable']>(),
			enable,
			getSnapshot: () => clientSnapshot,
			refresh: vi.fn<AgentIntegrationManagementControl['refresh']>(),
			subscribe: () => () => undefined,
		};

		const rendered = renderHook(() => useAgentIntegrationSettings(control), {
			wrapper: I18nWrapper,
		});

		expect(rendered.result.current.rows).toHaveLength(DESKTOP_AGENT_INTEGRATION_IDS.length);
		expect(rendered.result.current.rows[1]).toMatchObject({
			agentName: 'Codex',
			checked: false,
			disabled: false,
			pending: false,
			status: 'Not enabled',
			toggleLabel: 'Codex integration',
		});
		expect(rendered.result.current.rows[0]).toMatchObject({
			disabled: true,
			toggleLabel: 'Claude Code integration',
		});
		expect(enable).not.toHaveBeenCalled();
	});
});
