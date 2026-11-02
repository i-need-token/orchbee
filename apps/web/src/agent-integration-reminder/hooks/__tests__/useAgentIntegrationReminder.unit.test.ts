// @vitest-environment jsdom

import { OrchbeeI18nProvider } from '@orchbee/i18n';

import { act, cleanup, renderHook, waitFor } from '@testing-library/react';
import { createElement, type ReactNode } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import {
	AGENT_INTEGRATION_IDS,
	AgentIntegrationReminderController,
	type AgentIntegrationManagementClientSnapshot,
	type AgentIntegrationManagementControl,
	type AgentIntegrationManagementSnapshot,
} from '../../../agent-integration-management';
import { useAgentIntegrationReminder } from '../useAgentIntegrationReminder';

const protocolSnapshot: AgentIntegrationManagementSnapshot = {
	integrations: AGENT_INTEGRATION_IDS.map((integrationId) => ({
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
	it('offers only the first launched eligible Agent and delegates settings navigation', async () => {
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
		const onOpenSettings = vi.fn<Parameters<typeof useAgentIntegrationReminder>[2]>();
		const rendered = renderHook(
			() => useAgentIntegrationReminder(control, controller, onOpenSettings),
			{
				wrapper: I18nWrapper,
			},
		);

		await waitFor(() => expect(rendered.result.current?.integrationId).toBe('grok'));
		expect(rendered.result.current?.body).toContain('Orchbee detected Grok');
		expect(enable).not.toHaveBeenCalled();
		act(() => rendered.result.current?.onAction());
		expect(onOpenSettings).toHaveBeenCalledWith('grok');
		expect(enable).not.toHaveBeenCalled();
		expect(controller.getSnapshot().activeIntegrationId).toBeNull();
		expect(controller.offer('codex')).toBe(false);

		controller.dispose();
	});

	it('keeps an explicit dismissal persisted without opening settings or changing configuration', async () => {
		const enable = vi.fn<AgentIntegrationManagementControl['enable']>();
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
			setItem: vi.fn<(key: string, value: string) => void>(),
		});
		const onOpenSettings = vi.fn<Parameters<typeof useAgentIntegrationReminder>[2]>();
		const rendered = renderHook(
			() => useAgentIntegrationReminder(control, controller, onOpenSettings),
			{ wrapper: I18nWrapper },
		);
		await waitFor(() => expect(rendered.result.current?.integrationId).toBe('grok'));
		const view = rendered.result.current;
		if (view === null) throw new Error('Expected an Agent integration reminder.');

		act(() => view.onDismiss());
		expect(controller.getSnapshot().dismissedIntegrationIds).toEqual(['grok']);
		expect(onOpenSettings).not.toHaveBeenCalled();
		expect(enable).not.toHaveBeenCalled();

		controller.dispose();
	});
});
