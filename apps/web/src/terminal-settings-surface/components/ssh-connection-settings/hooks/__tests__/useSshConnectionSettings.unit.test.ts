// @vitest-environment jsdom

import { DesktopSshEnvironmentCatalogSchema } from '@orchbee/desktop-client-protocol';
import { OrchbeeI18nProvider } from '@orchbee/i18n';
import { act, cleanup, renderHook } from '@testing-library/react';
import { createElement, type ReactNode } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import type {
	DesktopSshConnectionManagementControl,
	DesktopSshConnectionManagementSnapshot,
} from '../../../../../ssh-connection-management';
import { useSshConnectionSettings } from '../useSshConnectionSettings';

afterEach(() => cleanup());

describe('useSshConnectionSettings', () => {
	it('maps reusable connections and keeps Add, Connect, and Disconnect explicit', () => {
		const control = createControl({
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
						blockedReason: 'authentication-required',
						destination: 'gpu-lab',
						environmentId: 'environment:22222222222222222222222222222222',
						name: 'GPU lab',
						projectCount: 0,
						state: 'blocked',
					},
				],
				registryIssue: null,
				suggestions: [{ destination: 'production-readonly' }],
			}),
			failure: null,
			pending: null,
		});
		const rendered = renderHook(() => useSshConnectionSettings(control), {
			wrapper: I18nWrapper,
		});

		expect(control.refreshConnections).toHaveBeenCalledOnce();
		expect(rendered.result.current.managed[0]).toMatchObject({
			actionLabel: 'Disconnect',
			projectUsage: 'Used by 3 projects',
			status: 'Connected',
			statusTone: 'positive',
		});
		expect(rendered.result.current.managed[1]).toMatchObject({
			actionLabel: 'Try again',
			projectUsage: 'Not used by a project yet',
			status: 'Needs attention',
			statusTone: 'warning',
		});

		act(() => rendered.result.current.managed[0]?.onAction());
		expect(control.disconnectEnvironment).toHaveBeenCalledWith(
			'environment:11111111111111111111111111111111',
		);
		act(() => rendered.result.current.managed[1]?.onAction());
		expect(control.connectEnvironment).toHaveBeenCalledWith(
			'environment:22222222222222222222222222222222',
			'GPU lab',
		);
		act(() => rendered.result.current.suggestions[0]?.onAdd());
		expect(control.addConnection).toHaveBeenCalledWith('production-readonly');
	});

	it('makes partial discovery and unavailable storage visible without enabling mutations', () => {
		const control = createControl({
			catalog: DesktopSshEnvironmentCatalogSchema.parse({
				discoveryIssue: 'truncated',
				managed: [],
				registryIssue: 'unavailable',
				suggestions: [{ destination: 'build-host' }],
			}),
			failure: 'storage',
			pending: null,
		});
		const rendered = renderHook(() => useSshConnectionSettings(control), {
			wrapper: I18nWrapper,
		});

		expect(rendered.result.current.registryWarning).toContain(
			'Saved SSH connections are temporarily unavailable',
		);
		expect(rendered.result.current.discoveryWarning).toContain(
			'only the first available Host entries are shown',
		);
		expect(rendered.result.current.failure).toContain('could not save this connection safely');
		expect(rendered.result.current.suggestions[0]?.actionDisabled).toBe(true);
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

function I18nWrapper({ children }: { readonly children: ReactNode }): ReactNode {
	return createElement(OrchbeeI18nProvider, { locale: 'en-US' }, children);
}

function createControl(snapshot: DesktopSshConnectionManagementSnapshot): TestControl {
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
