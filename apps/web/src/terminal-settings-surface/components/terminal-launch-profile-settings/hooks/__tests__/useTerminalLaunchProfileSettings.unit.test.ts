// @vitest-environment jsdom

import { OrchbeeI18nProvider } from '@orchbee/i18n';
import { cleanup, renderHook } from '@testing-library/react';
import { createElement, type ReactNode } from 'react';
import { afterEach, describe, expect, it } from 'vitest';

import {
	TerminalLaunchProfileSnapshotSchema,
	type TerminalLaunchProfileControl,
} from '../../../../../terminal-launch-profile-control';
import { useTerminalLaunchProfileSettings } from '../useTerminalLaunchProfileSettings';

function I18nWrapper({ children }: { readonly children: ReactNode }) {
	return createElement(OrchbeeI18nProvider, { locale: 'en-US' }, children);
}

afterEach(() => cleanup());

describe('useTerminalLaunchProfileSettings', () => {
	it('projects the localized built-in profile and platform login capability', () => {
		const snapshot = {
			...TerminalLaunchProfileSnapshotSchema.parse({
				capabilities: { loginShell: false },
				catalog: {
					defaultProfileId: 'system-default',
					profiles: [
						{
							command: { kind: 'system-shell', login: false },
							id: 'system-default',
							name: null,
						},
					],
					version: 1,
				},
				source: 'built-in',
			}),
			failureCode: null,
			pending: false,
			readOnly: false,
			refusal: null,
		};
		const unavailable = async (): Promise<{ readonly kind: 'unavailable' }> => ({
			kind: 'unavailable',
		});
		const client: TerminalLaunchProfileControl = {
			createProfile: unavailable,
			deleteProfile: unavailable,
			getSnapshot: () => snapshot,
			reset: unavailable,
			selectDefault: unavailable,
			subscribe: () => () => undefined,
			updateProfile: unavailable,
		};

		const rendered = renderHook(() => useTerminalLaunchProfileSettings(client), {
			wrapper: I18nWrapper,
		});

		expect(rendered.result.current.profiles).toMatchObject([
			{ defaultLabel: 'Default', label: 'System shell', selected: true },
		]);
		expect(rendered.result.current.notice).toContain('new terminal starts');
		expect(rendered.result.current.commandEditor).toMatchObject({
			description: 'Login-shell mode is unavailable on this operating system.',
			kind: 'system-shell',
			loginDisabled: true,
		});
	});
});
