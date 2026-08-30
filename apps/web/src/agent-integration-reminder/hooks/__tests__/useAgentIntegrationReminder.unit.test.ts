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
	type AgentIntegrationManagementClientOperationResult,
	type AgentIntegrationManagementClientSnapshot,
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
	launchedIntegrationIds: [],
	revision: 1,
};

function I18nWrapper({ children }: { readonly children: ReactNode }) {
	return createElement(OrchbeeI18nProvider, { locale: 'en-US' }, children);
}

afterEach(() => cleanup());

describe('useAgentIntegrationReminder', () => {
	it('offers only the first launched eligible Agent and delegates explicit enablement', async () => {
		const enable = vi.fn<AgentIntegrationManagementControl['enable']>(async () => ({
			kind: 'completed',
			snapshot: protocolSnapshot,
		}));
		const clientSnapshot: AgentIntegrationManagementClientSnapshot = {
			...protocolSnapshot,
			failureCode: null,
			failureIntegrationId: null,
			launchedIntegrationIds: ['grok', 'codex'],
			pending: false,
			pendingIntegrationId: null,
		};
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
		const rendered = renderHook(() => useAgentIntegrationReminder(control, controller), {
			wrapper: I18nWrapper,
		});

		await waitFor(() => expect(rendered.result.current?.integrationId).toBe('grok'));
		expect(rendered.result.current?.body).toContain('You started Grok');
		expect(enable).not.toHaveBeenCalled();
		act(() => rendered.result.current?.onEnable());
		expect(enable).toHaveBeenCalledWith('grok');
		expect(controller.getSnapshot().activeIntegrationId).toBe('grok');
		expect(controller.offer('codex')).toBe(false);

		controller.dispose();
	});

	it('clears a concurrent dismissal after the explicit enable is verified', async () => {
		let settleEnable: (result: AgentIntegrationManagementClientOperationResult) => void = () =>
			undefined;
		const enable = vi.fn<AgentIntegrationManagementControl['enable']>(
			() =>
				new Promise((resolve) => {
					settleEnable = resolve;
				}),
		);
		const clientSnapshot: AgentIntegrationManagementClientSnapshot = {
			...protocolSnapshot,
			failureCode: null,
			failureIntegrationId: null,
			launchedIntegrationIds: ['grok'],
			pending: false,
			pendingIntegrationId: null,
		};
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
		const rendered = renderHook(() => useAgentIntegrationReminder(control, controller), {
			wrapper: I18nWrapper,
		});
		await waitFor(() => expect(rendered.result.current?.integrationId).toBe('grok'));
		const view = rendered.result.current;
		if (view === null) throw new Error('Expected an Agent integration reminder.');

		act(() => {
			view.onEnable();
			view.onDismiss();
		});
		expect(controller.getSnapshot().dismissedIntegrationIds).toEqual(['grok']);
		await act(async () => {
			settleEnable({ kind: 'completed', snapshot: protocolSnapshot });
		});
		expect(controller.getSnapshot().dismissedIntegrationIds).toEqual([]);

		controller.dispose();
	});
});
