// @vitest-environment jsdom

import { OrchbeeI18nProvider } from '@orchbee/i18n';
import { cleanup, renderHook } from '@testing-library/react';
import { createElement, type ReactNode } from 'react';
import { afterEach, describe, expect, it } from 'vitest';

import { KeyboardShortcutSettingsController } from '../../../../../keyboard-shortcuts';
import { useKeyboardShortcutSettingsView } from '../useKeyboardShortcutSettingsView';

function I18nWrapper({ children }: { readonly children: ReactNode }) {
	return createElement(OrchbeeI18nProvider, { locale: 'en-US' }, children);
}

afterEach(() => cleanup());

describe('useKeyboardShortcutSettingsView', () => {
	it('projects the complete localized shortcut table', () => {
		const controller = new KeyboardShortcutSettingsController({
			platform: 'standard',
			storage: { getItem: () => null, setItem: () => undefined },
		});
		const allRows = renderHook(
			() =>
				useKeyboardShortcutSettingsView({
					controller,
					platform: 'standard',
				}),
			{ wrapper: I18nWrapper },
		);
		expect(allRows.result.current.rows).toHaveLength(13);
		expect(allRows.result.current.rows[0]).toMatchObject({
			actionName: 'Search actions',
			chord: 'Ctrl+Shift+P',
			scope: 'Workbench',
			source: 'Default',
		});
		expect(
			allRows.result.current.rows.filter((row) => row.scope === 'Focused terminal'),
		).toHaveLength(3);
		expect(
			allRows.result.current.rows.find((row) => row.actionName === 'Close active terminal'),
		).toMatchObject({
			availability: 'When a terminal tab is active',
			chord: 'Ctrl+Shift+W',
			scope: 'Active terminal',
		});
		controller.dispose();
	});
});
