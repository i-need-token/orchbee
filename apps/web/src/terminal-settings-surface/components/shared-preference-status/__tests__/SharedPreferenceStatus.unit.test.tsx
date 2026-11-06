// @vitest-environment jsdom
import { OrchbeeI18nProvider } from '@orchbee/i18n';
import { DEFAULT_USER_PREFERENCES, USER_PREFERENCE_KEYS } from '@orchbee/workbench-client-protocol';
import { act } from 'react';
import { createRoot } from 'react-dom/client';
import { expect, it, vi, onTestFinished } from 'vitest';
import type { UserPreferencesControl } from '../../../../user-preferences';
import { SharedPreferenceStatus } from '../SharedPreferenceStatus';

it.each([
	['en-US', "Settings file can't be read · using defaults · Reset all to replace it", 'Reset all'],
	['zh-CN', '无法读取设置文件 · 正在使用默认值 · 可用“全部重置”替换', '全部重置'],
] as const)(
	'keeps explicit Reset all available after an unreadable read in %s',
	async (locale, notice, label) => {
		Object.defineProperty(globalThis, 'IS_REACT_ACT_ENVIRONMENT', {
			configurable: true,
			value: true,
		});
		const refused = { kind: 'refused', reason: 'invalid-source' } as const;
		const reset = vi.fn<UserPreferencesControl['reset']>(async () => refused);
		const state = {
			available: false,
			pending: false,
			failure: 'invalid-source' as const,
			applicationIssue: null,
			draftResetRevision: 0,
			snapshot: { source: 'unreadable', values: DEFAULT_USER_PREFERENCES, claimed: [] },
		};
		const control: UserPreferencesControl = {
			getSnapshot: () => state,
			subscribe: () => () => {},
			update: async () => refused,
			refresh: async () => refused,
			reset,
		};
		const container = document.createElement('div');
		document.body.append(container);
		const root = createRoot(container);
		onTestFinished(async () => {
			await act(async () => root.unmount());
			container.remove();
			Object.defineProperty(globalThis, 'IS_REACT_ACT_ENVIRONMENT', {
				configurable: true,
				value: false,
			});
		});
		await act(async () =>
			root.render(
				<OrchbeeI18nProvider locale={locale}>
					<SharedPreferenceStatus control={control} />
				</OrchbeeI18nProvider>,
			),
		);
		expect(container.querySelector('[role="status"]')?.textContent).toBe(notice);
		const resetButton = () =>
			[...container.querySelectorAll('button')].find((button) => button.textContent === label);
		const first = resetButton();
		expect(first?.disabled).toBe(false);
		await act(async () => first?.click());
		expect(reset).not.toHaveBeenCalled();
		const confirm = resetButton();
		expect(confirm?.disabled).toBe(false);
		await act(async () => confirm?.click());
		expect(reset).toHaveBeenCalledExactlyOnceWith(USER_PREFERENCE_KEYS);
	},
);
