// @vitest-environment jsdom

import { OrchbeeI18nProvider } from '@orchbee/i18n';
import {
	DESKTOP_AGENT_INTEGRATION_IDS,
	type DesktopAgentIntegrationManagementSnapshot,
} from '@orchbee/desktop-client-protocol';
import { act, cleanup, renderHook, waitFor } from '@testing-library/react';
import { createElement, type ReactNode } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import {
	AgentIntegrationReminderController,
	type AgentIntegrationManagementControl,
} from '../../../agent-integration-management';
import { useAgentIntegrationReminder } from '../useAgentIntegrationReminder';

const protocolSnapshot: DesktopAgentIntegrationManagementSnapshot = {
	integrations: DESKTOP_AGENT_INTEGRATION_IDS.map((integrationId) => ({
		availability: integrationId === 'codex' || integrationId === 'grok' ? 'available' : 'not-found',
		integrationId,
		issue: null,
		managementMode: 'managed-local',
		state: integrationId === 'codex' || integrationId === 'grok' ? 'not-installed' : 'unknown',
	})),
	revision: 1,
};

function I18nWrapper({ children }: { readonly children: ReactNode }) {
	return createElement(OrchbeeI18nProvider, { locale: 'en-US' }, children);
}

afterEach(() => cleanup());

describe('useAgentIntegrationReminder', () => {
	it('offers only the first eligible Agent and delegates review without enabling it', async () => {
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
		const controller = new AgentIntegrationReminderController({
			getItem: () => null,
			setItem: () => undefined,
		});
		const onReview = vi.fn<(returnFocus: HTMLElement) => void>();
		const rendered = renderHook(() => useAgentIntegrationReminder(control, controller, onReview), {
			wrapper: I18nWrapper,
		});

		await waitFor(() => expect(rendered.result.current?.integrationId).toBe('codex'));
		expect(rendered.result.current?.body).toContain('Codex is installed locally');
		expect(enable).not.toHaveBeenCalled();
		const returnFocus = document.createElement('button');
		act(() => rendered.result.current?.onReview(returnFocus));
		expect(onReview).toHaveBeenCalledWith(returnFocus);
		expect(controller.getSnapshot().activeIntegrationId).toBeNull();
		expect(controller.offer('grok')).toBe(false);

		controller.dispose();
	});
});
