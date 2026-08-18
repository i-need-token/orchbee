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
	it('projects the complete localized shell setup without changing feedback preferences', () => {
		const controller = new TerminalFeedbackSettingsController({
			notificationCapability: {
				permission: () => 'unsupported',
				requestPermission: () => Promise.resolve('unsupported'),
				show: () => false,
			},
			storage: { getItem: () => null, setItem: () => undefined },
		});
		const rendered = renderHook(() => useShellIntegrationSettings(controller), {
			wrapper: I18nWrapper,
		});

		expect(rendered.result.current.setupCards).toHaveLength(3);
		expect(rendered.result.current.setupCards[1]).toMatchObject({
			description:
				'Bash 4.4 or newer is required. The Bash 3.2 included with macOS is not supported.',
			id: 'bash',
			title: 'Bash',
		});
		expect(rendered.result.current.manualNotice).toContain('never edits shell startup files');
		expect(rendered.result.current.reminder.modeValue).toBe('never');
		expect(rendered.result.current.notification.permission).toBe('unsupported');
		expect(controller.getSnapshot().preferences.completionReminderMode).toBe('never');
		controller.dispose();
	});
});
