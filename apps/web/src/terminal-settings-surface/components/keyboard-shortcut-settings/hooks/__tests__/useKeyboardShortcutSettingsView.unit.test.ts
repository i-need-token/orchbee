// @vitest-environment jsdom

import { OrchbeeI18nProvider, useOrchbeeIntl } from '@orchbee/i18n';
import { act, cleanup, renderHook } from '@testing-library/react';
import { createElement, useMemo, type ReactNode } from 'react';
import { afterEach, describe, expect, it } from 'vitest';

import {
	KEYBOARD_SHORTCUT_CATALOG,
	KeyboardShortcutSettingsController,
	keyboardShortcutClaims,
	useKeyboardShortcutSettings,
} from '../../../../../keyboard-shortcuts';
import {
	useWorkbenchConfiguredCommandKeys,
	type WorkbenchConfiguredCommands,
} from '../../../../../workbench';
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
		expect(allRows.result.current.rows).toHaveLength(KEYBOARD_SHORTCUT_CATALOG.length);
		expect(allRows.result.current.rows[0]).toMatchObject({
			actionName: 'Search actions',
			chord: 'Ctrl+Shift+P',
			scope: 'Workbench',
			source: 'Default',
		});
		expect(
			allRows.result.current.rows.find((row) => row.actionName === 'Focus next Workbench region'),
		).toMatchObject({
			chord: 'Ctrl+F6',
			scope: 'Workbench',
		});
		expect(
			allRows.result.current.rows.find((row) => row.actionName === 'Resize active panel'),
		).toMatchObject({
			availability: 'When an active terminal panel is available',
			chord: 'Ctrl+Shift+R',
			scope: 'Active panel',
		});
		expect(
			allRows.result.current.rows.find((row) => row.actionName === 'Equalize panels'),
		).toMatchObject({
			availability: 'When the active tab has multiple panels',
			chord: 'Ctrl+Alt+=',
			scope: 'Active panel',
		});
		expect(
			allRows.result.current.rows.find((row) => row.actionName === 'Visually focus panel'),
		).toMatchObject({
			availability: 'When the active tab has multiple panels',
			chord: 'Ctrl+Shift+F7',
			scope: 'Active panel',
		});
		expect(
			allRows.result.current.rows.find((row) => row.actionName === 'Visually swap panel'),
		).toMatchObject({
			availability: 'When the active tab has multiple panels',
			chord: 'Ctrl+Shift+F8',
			scope: 'Active panel',
		});
		expect(
			allRows.result.current.rows.find(
				(row) => row.actionName === 'New terminal in current directory',
			),
		).toMatchObject({
			availability: 'When a terminal tab is active',
			chord: 'Ctrl+Alt+Enter',
			scope: 'Active terminal',
		});
		expect(
			allRows.result.current.rows.filter((row) => row.scope === 'Focused terminal'),
		).toHaveLength(15);
		expect(
			allRows.result.current.rows.find((row) => row.actionName === 'Show last command output'),
		).toMatchObject({
			availability: 'When a terminal is focused',
			chord: 'Ctrl+Shift+G',
			scope: 'Focused terminal',
		});
		expect(
			allRows.result.current.rows.find((row) => row.actionName === 'Show terminal hints'),
		).toMatchObject({
			availability: 'When a terminal is focused',
			chord: 'Ctrl+Shift+O',
			scope: 'Focused terminal',
		});
		expect(
			allRows.result.current.rows.find((row) => row.actionName === 'Show scrollback'),
		).toMatchObject({
			availability: 'When a terminal is focused',
			chord: 'Ctrl+Shift+H',
			scope: 'Focused terminal',
		});
		expect(
			allRows.result.current.rows.find(
				(row) => row.actionName === 'Scroll to previous shell prompt',
			),
		).toMatchObject({
			availability: 'When a terminal is focused',
			chord: 'Ctrl+Shift+Z',
			scope: 'Focused terminal',
		});
		expect(
			allRows.result.current.rows.find((row) => row.actionName === 'Scroll up one page'),
		).toMatchObject({
			availability: 'When a terminal is focused',
			chord: 'Ctrl+Shift+Page Up',
			scope: 'Focused terminal',
		});
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
			allRows.result.current.rows.find((row) => row.actionName === 'Focus panel 1'),
		).toMatchObject({
			availability: 'When that panel position exists',
			chord: 'Ctrl+Shift+1',
			scope: 'Active panel',
		});
		expect(
			allRows.result.current.rows.find((row) => row.actionName === 'Focus panel 10'),
		).toMatchObject({
			availability: 'When that panel position exists',
			chord: 'Ctrl+Shift+0',
			scope: 'Active panel',
		});
		expect(
			allRows.result.current.rows.find((row) => row.actionName === 'Move active panel left'),
		).toMatchObject({
			availability: 'When an adjacent panel exists in that direction',
			chord: 'Ctrl+Alt+Shift+←',
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
		expect(
			allRows.result.current.rows.find((row) => row.actionName === 'Move active tab forward'),
		).toMatchObject({
			availability: 'When the active tab can move inside its pinned or unpinned section',
			chord: 'Ctrl+Shift+.',
			scope: 'Workbench',
		});
		controller.dispose();
	});

	it('notes a default a configured command took until the person overrides it', () => {
		const controller = new KeyboardShortcutSettingsController({
			platform: 'standard',
			storage: { getItem: () => null, setItem: () => undefined },
		});
		const catalog = [
			{ bindings: ['ctrl+shift+p'], description: 'deploy', display: null, id: 'deploy' },
		];
		const commands: WorkbenchConfiguredCommands = {
			getSnapshot: () => catalog,
			run: () => Promise.resolve({ kind: 'accepted' }),
			subscribe: () => () => undefined,
		};
		const view = renderHook(
			() => {
				const intl = useOrchbeeIntl();
				const snapshot = useKeyboardShortcutSettings(controller);
				const claims = useMemo(() => keyboardShortcutClaims(snapshot, intl), [intl, snapshot]);
				const keys = useWorkbenchConfiguredCommandKeys({ claims, commands, platform: 'standard' });
				return useKeyboardShortcutSettingsView({
					controller,
					platform: 'standard',
					takenByConfiguredCommands: keys.takenBy,
				});
			},
			{ wrapper: I18nWrapper },
		);
		const searchRow = () =>
			view.result.current.rows.find((row) => row.shortcutId === 'workbench.search.actions.open');

		expect(searchRow()?.configuredCommandNote).toContain('deploy');
		expect(
			view.result.current.rows.filter((row) => row.configuredCommandNote !== null),
		).toHaveLength(1);

		act(() => {
			controller.setShortcut('workbench.search.actions.open', {
				code: 'KeyK',
				modifiers: ['mod', 'alt'],
			});
		});
		expect(searchRow()?.configuredCommandNote).toBeNull();
		controller.dispose();
	});
});
