// @vitest-environment jsdom

import { OrchbeeI18nProvider } from '@orchbee/i18n';
import { cleanup, renderHook } from '@testing-library/react';
import { createElement, type ReactNode } from 'react';
import { afterEach, describe, expect, it } from 'vitest';

import { TerminalFeedbackSettingsController } from '../../../../../terminal-feedback-settings';
import { useShellIntegrationSettings } from '../useShellIntegrationSettings';

function I18nWrapper({ children }: { readonly children: ReactNode }) {
	return createElement(OrchbeeI18nProvider, { locale: 'en-US' }, children);
}

afterEach(() => cleanup());

describe('useShellIntegrationSettings', () => {
	it('projects localized shell setup and filters without changing feedback preferences', () => {
		const controller = new TerminalFeedbackSettingsController({
			notificationCapability: {
				permission: () => 'unsupported',
				requestPermission: () => Promise.resolve('unsupported'),
				show: () => false,
			},
			storage: { getItem: () => null, setItem: () => undefined },
		});
		const rendered = renderHook(({ query }) => useShellIntegrationSettings(controller, query), {
			initialProps: { query: 'Bash 4.4' },
			wrapper: I18nWrapper,
		});

		expect(rendered.result.current).toMatchObject({
			manualNotice: null,
			noResults: null,
			notification: null,
			reminder: null,
			reset: null,
			setupCards: [
				{
					description:
						'Bash 4.4 or newer is required. The Bash 3.2 included with macOS is not supported.',
					id: 'bash',
					title: 'Bash',
				},
			],
		});
		expect(controller.getSnapshot().preferences.completionReminderMode).toBe('never');

		rendered.rerender({ query: 'not a settings term' });
		expect(rendered.result.current.setupCards).toHaveLength(0);
		expect(rendered.result.current.noResults).toBe('No settings found.');
		controller.dispose();
	});
});
