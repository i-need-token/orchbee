// @vitest-environment jsdom

import { OrchbeeI18nProvider } from '@orchbee/i18n';

import { act, createElement } from 'react';
import { createRoot } from 'react-dom/client';
import { afterAll, beforeAll, describe, expect, it, vi } from 'vitest';

import {
	AGENT_INTEGRATION_IDS,
	type AgentIntegrationId,
	type AgentIntegrationManagementClientOperationResult,
	type AgentIntegrationManagementClientSnapshot,
	type AgentIntegrationManagementControl,
	type AgentIntegrationSnapshotItem,
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
	it('uses authoritative switches', async () => {
		const container = document.createElement('div');
		document.body.append(container);
		const control = createTestControl({
			codex: { availability: 'available', state: 'enabled' },
		});
		const root = createRoot(container);
		await act(async () => {
			root.render(
				createElement(
					OrchbeeI18nProvider,
					{ locale: 'en-US' },
					createElement(AgentIntegrationSettings, { control }),
				),
			);
		});

		expect(control.enableCalls).toEqual([]);
		expect(control.disableCalls).toEqual([]);
		expect(container.textContent).toContain('never installs adapters automatically');
		const integrationSwitch = findSwitch(container, 'Codex integration');
		expect(integrationSwitch.getAttribute('aria-checked')).toBe('true');
		expect(findButtonWithExactText(container, 'Disable')).toBeUndefined();
		expect(findButtonWithExactText(container, 'Enable')).toBeUndefined();

		await act(async () => integrationSwitch.click());
		expect(control.disableCalls).toEqual(['codex']);
		expect(integrationSwitch.getAttribute('aria-checked')).toBe('false');

		await act(async () => integrationSwitch.click());
		await vi.waitFor(() => {
			expect(control.enableCalls).toEqual(['codex']);
			expect(integrationSwitch.getAttribute('aria-checked')).toBe('true');
		});

		await act(async () => root.unmount());
		container.remove();
	});

	it('does not move a switch before Main publishes a re-inspected state', async () => {
		const container = document.createElement('div');
		document.body.append(container);
		const control = createTestControl(
			{ codex: { availability: 'available', state: 'enabled' } },
			false,
		);
		const root = createRoot(container);
		await act(async () => {
			root.render(
				createElement(
					OrchbeeI18nProvider,
					{ locale: 'en-US' },
					createElement(AgentIntegrationSettings, { control }),
				),
			);
		});

		const integrationSwitch = findSwitch(container, 'Codex integration');
		await act(async () => integrationSwitch.click());
		expect(control.disableCalls).toEqual(['codex']);
		expect(integrationSwitch.getAttribute('aria-checked')).toBe('true');

		await act(async () => {
			control.updateIntegration('codex', { state: 'disabled' });
		});
		expect(integrationSwitch.getAttribute('aria-checked')).toBe('false');

		await act(async () => root.unmount());
		container.remove();
	});
});

function findSwitch(container: HTMLElement, label: string): HTMLElement {
	const element = [...container.querySelectorAll("[data-slot='switch']")].find(
		(candidate) => candidate.getAttribute('aria-label') === label,
	);
	if (!(element instanceof HTMLElement)) throw new Error(`Expected ${label} switch.`);
	return element;
}

function findButtonWithExactText(
	container: HTMLElement,
	text: string,
): HTMLButtonElement | undefined {
	return [...container.querySelectorAll('button')].find(
		(candidate) => candidate.textContent === text,
	);
}

interface TestControl extends AgentIntegrationManagementControl {
	readonly disableCalls: AgentIntegrationId[];
	readonly enableCalls: AgentIntegrationId[];
	updateIntegration(
		integrationId: AgentIntegrationId,
		patch: Partial<AgentIntegrationSnapshotItem>,
	): void;
}

function createTestControl(
	initial: Partial<Readonly<Record<AgentIntegrationId, Partial<AgentIntegrationSnapshotItem>>>>,
	autoPublish = true,
): TestControl {
	let snapshot: AgentIntegrationManagementClientSnapshot = {
		failureCode: null,
		failureIntegrationId: null,
		integrations: AGENT_INTEGRATION_IDS.map((integrationId) => ({
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
			if (!autoPublish) return { kind: 'unavailable' };
			control.updateIntegration(integrationId, { state: 'disabled' });
			return { kind: 'completed', snapshot };
		},
		enable: async (integrationId): Promise<AgentIntegrationManagementClientOperationResult> => {
			control.enableCalls.push(integrationId);
			if (!autoPublish) return { kind: 'unavailable' };
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
