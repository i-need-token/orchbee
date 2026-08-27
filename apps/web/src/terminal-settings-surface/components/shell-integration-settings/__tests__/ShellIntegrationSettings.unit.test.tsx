// @vitest-environment jsdom

import { OrchbeeI18nProvider, type OrchbeeLocale } from '@orchbee/i18n';
import { act, createElement } from 'react';
import { createRoot } from 'react-dom/client';
import { afterAll, beforeAll, describe, expect, it, vi } from 'vitest';

import { TerminalFeedbackSettingsController } from '../../../../terminal-feedback-settings';
import { ShellIntegrationSettings } from '../ShellIntegrationSettings';

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

describe('ShellIntegrationSettings', () => {
	it.each([
		{
			automatic: 'automatically loads integration',
			locale: 'en-US' as const,
			manual: 'never edits shell startup files',
			reminder: 'Command completion reminders',
		},
		{
			automatic: '自动加载集成',
			locale: 'zh-CN' as const,
			manual: '永远不会编辑 Shell 启动文件',
			reminder: '命令完成提醒',
		},
	])(
		'renders automatic scope, manual fallback, and device-local reminder controls in $locale',
		async ({ automatic, locale, manual, reminder }) => {
			const container = document.createElement('div');
			document.body.append(container);
			const requestPermission = vi.fn<() => Promise<'granted'>>(() => Promise.resolve('granted'));
			const controller = new TerminalFeedbackSettingsController({
				notificationCapability: {
					permission: () => 'prompt',
					requestPermission,
					show: () => true,
				},
				storage: memoryStorage(),
			});
			const root = createRoot(container);

			await act(async () => {
				root.render(localized(locale, controller));
			});

			expect(container.textContent).toContain(reminder);
			expect(container.textContent).toContain(automatic);
			expect(container.textContent).toContain(manual);
			expect(container.textContent).toContain('Bash 3.2');
			expect(container.textContent).toContain(
				'test -r "$ORCHBEE_SHELL_INTEGRATION_DIR/orchbee.zsh"',
			);
			expect(controller.getSnapshot().preferences.completionReminderMode).toBe('never');

			const mode = requiredElement<HTMLSelectElement>(
				container.querySelector('[data-slot="completion-reminder-mode"]'),
			);
			await act(async () => {
				mode.value = 'always';
				mode.dispatchEvent(new Event('change', { bubbles: true }));
			});
			expect(controller.getSnapshot().preferences.completionReminderMode).toBe('always');
			const minimum = requiredElement<HTMLInputElement>(
				container.querySelector('[data-slot="completion-reminder-minimum"]'),
			);
			await act(async () => {
				setInputValue(minimum, '12');
				minimum.dispatchEvent(new Event('input', { bubbles: true }));
			});
			await act(async () => {
				minimum.dispatchEvent(new KeyboardEvent('keydown', { bubbles: true, key: 'Enter' }));
			});
			expect(controller.getSnapshot().preferences.completionReminderMinimumSeconds).toBe(12);

			const permission = requiredElement<HTMLButtonElement>(
				container.querySelector('[data-slot="request-terminal-notification-permission"]'),
			);
			await act(async () => permission.click());
			expect(requestPermission).toHaveBeenCalledOnce();
			expect(controller.getSnapshot().notificationPermission).toBe('granted');

			const reset = requiredElement<HTMLButtonElement>(
				container.querySelector('[data-slot="reset-terminal-feedback-settings"]'),
			);
			await act(async () => reset.click());
			expect(controller.getSnapshot().preferences).toEqual({
				completionReminderMinimumSeconds: 5,
				completionReminderMode: 'never',
			});

			await act(async () => root.unmount());
			controller.dispose();
			container.remove();
		},
	);
});

function localized(
	locale: OrchbeeLocale,
	controller: TerminalFeedbackSettingsController,
): ReturnType<typeof createElement> {
	return createElement(
		OrchbeeI18nProvider,
		{ locale },
		createElement(ShellIntegrationSettings, { controller }),
	);
}

function memoryStorage(): Pick<Storage, 'getItem' | 'setItem'> {
	const values = new Map<string, string>();
	return {
		getItem: (key) => values.get(key) ?? null,
		setItem: (key, value) => values.set(key, value),
	};
}

function requiredElement<T extends Element>(element: T | null): T {
	if (element === null) throw new Error('Expected test element.');
	return element;
}

function setInputValue(input: HTMLInputElement, value: string): void {
	const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')?.set;
	if (setter === undefined) throw new Error('The input value setter is unavailable.');
	setter.call(input, value);
}
