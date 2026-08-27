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
		expect(allRows.result.current.rows).toHaveLength(29);
		expect(allRows.result.current.rows[0]).toMatchObject({
			actionName: 'Search actions',
			chord: 'Ctrl+Shift+P',
			scope: 'Workbench',
			source: 'Default',
		});
		expect(
			allRows.result.current.rows.filter((row) => row.scope === 'Focused terminal'),
		).toHaveLength(4);
		expect(
			allRows.result.current.rows.find((row) => row.actionName === 'Clear terminal'),
		).toMatchObject({
			availability: 'When a terminal is focused',
			chord: 'Ctrl+Shift+L',
			scope: 'Focused terminal',
		});
		expect(
			allRows.result.current.rows.find((row) => row.actionName === 'Close active terminal'),
		).toMatchObject({
			availability: 'When a terminal tab is active',
			chord: 'Ctrl+Shift+W',
			scope: 'Active terminal',
		});
		expect(
			allRows.result.current.rows.find((row) => row.actionName === 'Focus panel left'),
		).toMatchObject({
			availability: 'When an adjacent panel exists in that direction',
			chord: 'Ctrl+Alt+←',
			scope: 'Active panel',
		});
		expect(
			allRows.result.current.rows.find((row) => row.actionName === 'Select tab 2'),
		).toMatchObject({
			availability: 'When that tab position exists',
			chord: 'Ctrl+2',
			scope: 'Workbench',
		});
		expect(
			allRows.result.current.rows.find((row) => row.actionName === 'Select next tab'),
		).toMatchObject({
			availability: 'When the active terminal context has multiple tabs',
			chord: 'Ctrl+Page Down',
			scope: 'Workbench',
		});
		controller.dispose();
	});
});
