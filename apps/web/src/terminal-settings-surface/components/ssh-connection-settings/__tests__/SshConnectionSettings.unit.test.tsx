// @vitest-environment jsdom

import { DesktopSshEnvironmentCatalogSchema } from '@orchbee/desktop-client-protocol';
import { OrchbeeI18nProvider } from '@orchbee/i18n';
import { act, createElement } from 'react';
import { createRoot } from 'react-dom/client';
import { afterAll, beforeAll, describe, expect, it, vi } from 'vitest';

import type {
	DesktopSshConnectionManagementControl,
	DesktopSshConnectionManagementSnapshot,
} from '../../../../ssh-connection-management';
import { SshConnectionSettings } from '../SshConnectionSettings';

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

describe('SshConnectionSettings', () => {
	it('discovers without connecting and keeps add, connect, and disconnect explicit', async () => {
		const container = document.createElement('div');
		document.body.append(container);
		const control = createControl();
		const root = createRoot(container);
		await act(async () => {
			root.render(
				createElement(
					OrchbeeI18nProvider,
					{ locale: 'en-US' },
					createElement(SshConnectionSettings, { control }),
				),
			);
		});

		expect(container.textContent).toContain('Your credentials stay private');
		expect(container.textContent).toContain('staging-api');
		expect(container.textContent).toContain('Used by 3 projects');
		expect(container.textContent).toContain('gpu-lab');
		expect(control.refreshConnections).toHaveBeenCalledOnce();
		expect(control.addConnection).not.toHaveBeenCalled();
		expect(control.connectEnvironment).not.toHaveBeenCalled();
		expect(control.disconnectEnvironment).not.toHaveBeenCalled();

		await act(async () => findButton(container, 'Add').click());
		expect(control.addConnection).toHaveBeenCalledWith('gpu-lab');
		expect(control.connectEnvironment).not.toHaveBeenCalled();

		await act(async () => findButton(container, 'Connect').click());
		expect(control.connectEnvironment).toHaveBeenCalledWith(
			'environment:22222222222222222222222222222222',
			'dev@192.0.2.42',
		);

		await act(async () => findButton(container, 'Disconnect').click());
		expect(control.disconnectEnvironment).toHaveBeenCalledWith(
			'environment:11111111111111111111111111111111',
		);

		const input = requiredElement<HTMLInputElement>(
			container.querySelector('#ssh-connection-destination'),
		);
		await act(async () => {
			setInputValue(input, 'ops@example.com');
			input.dispatchEvent(new Event('input', { bubbles: true }));
		});
		await act(async () => findButton(container, 'Add connection').click());
		expect(control.addConnection).toHaveBeenLastCalledWith('ops@example.com');

		await act(async () => root.unmount());
		container.remove();
	});
});

interface TestControl extends DesktopSshConnectionManagementControl {
	readonly addConnection: ReturnType<
		typeof vi.fn<DesktopSshConnectionManagementControl['addConnection']>
	>;
	readonly connectEnvironment: ReturnType<
		typeof vi.fn<DesktopSshConnectionManagementControl['connectEnvironment']>
	>;
	readonly disconnectEnvironment: ReturnType<
		typeof vi.fn<DesktopSshConnectionManagementControl['disconnectEnvironment']>
	>;
	readonly refreshConnections: ReturnType<
		typeof vi.fn<DesktopSshConnectionManagementControl['refreshConnections']>
	>;
}

function createControl(): TestControl {
	const snapshot: DesktopSshConnectionManagementSnapshot = {
		catalog: DesktopSshEnvironmentCatalogSchema.parse({
			discoveryIssue: null,
			managed: [
				{
					blockedReason: null,
					destination: 'staging-api',
					environmentId: 'environment:11111111111111111111111111111111',
					name: 'staging-api',
					projectCount: 3,
					state: 'connected',
				},
				{
					blockedReason: null,
					destination: 'dev@192.0.2.42',
					environmentId: 'environment:22222222222222222222222222222222',
					name: 'dev@192.0.2.42',
					projectCount: 0,
					state: 'disconnected',
				},
			],
			registryIssue: null,
			suggestions: [{ destination: 'gpu-lab' }],
		}),
		failure: null,
		pending: null,
	};
	return {
		addConnection: vi.fn<DesktopSshConnectionManagementControl['addConnection']>(async () => true),
		clearFailure: vi.fn<DesktopSshConnectionManagementControl['clearFailure']>(),
		connectEnvironment: vi.fn<DesktopSshConnectionManagementControl['connectEnvironment']>(),
		disconnectEnvironment: vi.fn<DesktopSshConnectionManagementControl['disconnectEnvironment']>(),
		getManagementSnapshot: () => snapshot,
		refreshConnections: vi.fn<DesktopSshConnectionManagementControl['refreshConnections']>(),
		subscribeManagement: () => () => undefined,
	};
}

function findButton(container: HTMLElement, label: string): HTMLButtonElement {
	const button = [...container.querySelectorAll('button')].find(
		(candidate) => candidate.textContent?.trim() === label,
	);
	if (button === undefined) throw new Error(`Expected ${label} button.`);
	return button;
}

function requiredElement<ElementType extends Element>(element: ElementType | null): ElementType {
	if (element === null) throw new Error('Expected element.');
	return element;
}

function setInputValue(input: HTMLInputElement, value: string): void {
	const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')?.set;
	setter?.call(input, value);
}
