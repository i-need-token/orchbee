// @vitest-environment jsdom

import { OrchbeeI18nProvider } from '@orchbee/i18n';
import {
	DESKTOP_AGENT_INTEGRATION_IDS,
	type DesktopAgentIntegrationId,
	type DesktopAgentIntegrationSnapshotItem,
} from '@orchbee/desktop-client-protocol';
import { act, createElement } from 'react';
import { createRoot } from 'react-dom/client';
import { afterAll, beforeAll, describe, expect, it, vi } from 'vitest';

import {
	AgentIntegrationReminderController,
	type AgentIntegrationManagementClientOperationResult,
	type AgentIntegrationManagementClientSnapshot,
	type AgentIntegrationManagementControl,
} from '../../agent-integration-management';
import { AgentIntegrationReminder } from '../AgentIntegrationReminder';

beforeAll(() => {
	Object.defineProperty(globalThis, 'IS_REACT_ACT_ENVIRONMENT', {
		configurable: true,
		value: true,
	});
});

afterAll(() => {
	Object.defineProperty(globalThis, 'IS_REACT_ACT_ENVIRONMENT', {
		configurable: true,
		value: false,
	});
});

describe('AgentIntegrationReminder', () => {
	it('shows one eligible local Agent and only opens Settings after user review', async () => {
		const container = document.createElement('div');
		document.body.append(container);
		const control = createTestControl({
			codex: { availability: 'available', state: 'not-installed' },
			grok: { availability: 'available', state: 'not-installed' },
		});
		const controller = new AgentIntegrationReminderController({
			getItem: () => null,
			setItem: () => undefined,
		});
		const onReview = vi.fn<(returnFocus: HTMLElement) => void>();
		const root = createRoot(container);

		await act(async () => {
			root.render(
				createElement(
					OrchbeeI18nProvider,
					{ locale: 'en-US' },
					createElement(AgentIntegrationReminder, { control, controller, onReview }),
				),
			);
		});

		const reminder = await vi.waitFor(() =>
			requiredElement<HTMLElement>(
				container.querySelector('[data-slot="agent-integration-reminder"]'),
			),
		);
		expect(reminder.dataset['agentIntegration']).toBe('codex');
		expect(reminder.textContent).toContain('Codex is installed locally');
		expect(control.enableCalls).toEqual([]);
		expect(control.disableCalls).toEqual([]);

		const review = requiredElement<HTMLButtonElement>(
			[...reminder.querySelectorAll('button')].find(
				(button) => button.textContent === 'Review integration',
			),
		);
		await act(async () => review.click());
		expect(onReview).toHaveBeenCalledWith(review);
		expect(container.querySelector('[data-slot="agent-integration-reminder"]')).toBeNull();
		await act(async () => {
			control.updateIntegration('codex', { state: 'enabled' });
			control.updateIntegration('grok', { state: 'disabled' });
		});
		expect(container.querySelector('[data-slot="agent-integration-reminder"]')).toBeNull();
		expect(control.enableCalls).toEqual([]);

		await act(async () => root.unmount());
		controller.dispose();
		container.remove();
	});

	it('persists an explicit dismissal without changing Agent configuration', async () => {
		const container = document.createElement('div');
		document.body.append(container);
		let serialized: string | null = null;
		const control = createTestControl({
			pi: { availability: 'available', state: 'disabled' },
		});
		const controller = new AgentIntegrationReminderController({
			getItem: () => serialized,
			setItem: (_key, value) => {
				serialized = value;
			},
		});
		const root = createRoot(container);
		await act(async () => {
			root.render(
				createElement(
					OrchbeeI18nProvider,
					{ locale: 'en-US' },
					createElement(AgentIntegrationReminder, {
						control,
						controller,
						onReview: () => undefined,
					}),
				),
			);
		});

		const dismiss = await vi.waitFor(() =>
			requiredElement<HTMLButtonElement>(
				container.querySelector("button[aria-label='Dismiss Pi integration reminder']"),
			),
		);
		await act(async () => dismiss.click());
		expect(JSON.parse(serialized ?? '')).toEqual({ dismissedIntegrationIds: ['pi'] });
		expect(control.enableCalls).toEqual([]);
		expect(control.disableCalls).toEqual([]);

		await act(async () => root.unmount());
		controller.dispose();
		container.remove();
	});
});

function requiredElement<ElementType extends Element>(
	element: ElementType | null | undefined,
): ElementType {
	if (element === null || element === undefined) throw new Error('Expected element.');
	return element;
}

interface TestControl extends AgentIntegrationManagementControl {
	readonly disableCalls: DesktopAgentIntegrationId[];
	readonly enableCalls: DesktopAgentIntegrationId[];
	updateIntegration(
		integrationId: DesktopAgentIntegrationId,
		patch: Partial<DesktopAgentIntegrationSnapshotItem>,
	): void;
}

function createTestControl(
	initial: Partial<
		Readonly<Record<DesktopAgentIntegrationId, Partial<DesktopAgentIntegrationSnapshotItem>>>
	>,
): TestControl {
	let snapshot: AgentIntegrationManagementClientSnapshot = {
		failureCode: null,
		failureIntegrationId: null,
		integrations: DESKTOP_AGENT_INTEGRATION_IDS.map((integrationId) => ({
			availability: 'not-found',
			integrationId,
			issue: null,
			managementMode: 'managed-local',
			state: 'not-installed',
			...initial[integrationId],
		})),
		pending: false,
		pendingIntegrationId: null,
		revision: 0,
	};
	const listeners = new Set<() => void>();
	const control: TestControl = {
		disableCalls: [],
		enableCalls: [],
		disable: async (integrationId): Promise<AgentIntegrationManagementClientOperationResult> => {
			control.disableCalls.push(integrationId);
			control.updateIntegration(integrationId, { state: 'disabled' });
			return { kind: 'completed', snapshot };
		},
		enable: async (integrationId): Promise<AgentIntegrationManagementClientOperationResult> => {
			control.enableCalls.push(integrationId);
			control.updateIntegration(integrationId, { state: 'enabled' });
			return { kind: 'completed', snapshot };
		},
		getSnapshot: () => snapshot,
		refresh: async (): Promise<AgentIntegrationManagementClientOperationResult> => ({
			kind: 'completed',
			snapshot,
		}),
		subscribe: (listener) => {
			listeners.add(listener);
			return () => {
				listeners.delete(listener);
			};
		},
		updateIntegration: (integrationId, patch) => {
			snapshot = {
				...snapshot,
				integrations: snapshot.integrations.map((integration) =>
					integration.integrationId === integrationId
						? { ...integration, ...patch, integrationId }
						: integration,
				),
				revision: snapshot.revision + 1,
			};
			for (const listener of listeners) listener();
		},
	};
	return control;
}
