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
} from '../../../../agent-integration-management';
import { AgentIntegrationSettings } from '../AgentIntegrationSettings';

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

describe('AgentIntegrationSettings', () => {
	it('changes only after an explicit action and keeps reminder preference in sync', async () => {
		const container = document.createElement('div');
		document.body.append(container);
		let serialized = JSON.stringify({ dismissedIntegrationIds: [] });
		const reminderController = new AgentIntegrationReminderController({
			getItem: () => serialized,
			setItem: (_key, value) => {
				serialized = value;
			},
		});
		const control = createTestControl({
			codex: { availability: 'available', state: 'enabled' },
		});
		const root = createRoot(container);
		await act(async () => {
			root.render(
				createElement(
					OrchbeeI18nProvider,
					{ locale: 'en-US' },
					createElement(AgentIntegrationSettings, {
						control,
						reminderController,
					}),
				),
			);
		});

		expect(control.enableCalls).toEqual([]);
		expect(control.disableCalls).toEqual([]);
		expect(container.textContent).toContain('never installs adapters automatically');
		const disable = findButton(container, 'Disable');
		await act(async () => disable.click());
		expect(control.disableCalls).toEqual(['codex']);
		expect(JSON.parse(serialized)).toEqual({ dismissedIntegrationIds: ['codex'] });

		const enable = findButton(container, 'Enable', false);
		await act(async () => enable.click());
		await vi.waitFor(() => {
			expect(control.enableCalls).toEqual(['codex']);
			expect(JSON.parse(serialized)).toEqual({ dismissedIntegrationIds: [] });
		});

		await act(async () => root.unmount());
		reminderController.dispose();
		container.remove();
	});
});

function findButton(
	container: HTMLElement,
	label: string,
	includeDisabled = true,
): HTMLButtonElement {
	const button = [...container.querySelectorAll('button')].find(
		(candidate) => candidate.textContent === label && (includeDisabled || !candidate.disabled),
	);
	if (button === undefined) throw new Error(`Expected ${label} button.`);
	return button;
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
