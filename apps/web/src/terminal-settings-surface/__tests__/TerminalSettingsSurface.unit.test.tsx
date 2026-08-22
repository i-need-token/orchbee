// @vitest-environment jsdom

import { OrchbeeI18nProvider } from '@orchbee/i18n';
import type { TerminalInputConfiguration } from '@orchbee/terminal';
import { act, createElement } from 'react';
import { createRoot } from 'react-dom/client';
import { afterAll, beforeAll, describe, expect, it, vi } from 'vitest';

import { TerminalSettingsSurface } from '../TerminalSettingsSurface';
import { KeyboardShortcutSettingsController } from '../../keyboard-shortcuts';
import { MacosTerminalInputSettingsController } from '../../macos-terminal-input-settings';
import {
	DEFAULT_TERMINAL_APPEARANCE_PREFERENCES,
	TerminalSettingsController,
} from '../../terminal-settings';
import { REMOTE_DEVELOPMENT_DIAGNOSTIC_COMMAND } from '../constants/remote-development-diagnostic';

type TerminalAppearanceStorage = ConstructorParameters<
	typeof TerminalSettingsController
>[0]['storage'];

beforeAll(() => {
	Object.defineProperty(globalThis, 'IS_REACT_ACT_ENVIRONMENT', {
		configurable: true,
		value: true,
	});
	Object.defineProperty(Element.prototype, 'scrollIntoView', {
		configurable: true,
		value: vi.fn<Element['scrollIntoView']>(),
	});
});

afterAll(() => {
	Object.defineProperty(globalThis, 'IS_REACT_ACT_ENVIRONMENT', {
		configurable: true,
		value: false,
	});
});

describe('TerminalSettingsSurface', () => {
	it('focuses global search, navigates categories, and routes theme choices outside presentation configuration', async () => {
		const container = document.createElement('div');
		document.body.append(container);
		const storage = memoryStorage();
		const controller = new TerminalSettingsController({
			storage,
			themeTarget: document.documentElement,
		});
		const keyboardShortcutController = new KeyboardShortcutSettingsController({
			platform: 'apple',
			storage,
		});
		const presentationListener =
			vi.fn<(configuration: ReturnType<typeof controller.current>) => void>();
		controller.subscribe(presentationListener);
		controller.open();
		const close = vi.fn<() => void>();
		const root = createRoot(container);

		await act(async () => {
			root.render(
				createElement(
					OrchbeeI18nProvider,
					{ locale: 'en-US' },
					createElement(TerminalSettingsSurface, {
						controller,
						keyboardShortcutController,
						keyboardShortcutPlatform: 'apple',
						onRequestClose: close,
					}),
				),
			);
		});

		const search = requiredElement<HTMLInputElement>(
			container.querySelector("input[aria-label='Search settings']"),
		);
		expect(document.activeElement).toBe(search);
		expect(
			requiredElement<HTMLElement>(container.querySelector('[data-settings-content]')).tabIndex,
		).toBe(0);
		const back = requiredElement<HTMLButtonElement>(
			container.querySelector('button[data-settings-back]'),
		);
		const title = requiredElement<HTMLElement>(back.querySelector('span:not([aria-hidden])'));
		expect(back.getAttribute('aria-label')).toBe('Back');
		expect(title.textContent).toBe('Settings');
		await act(async () => title.click());
		expect(close).toHaveBeenCalledOnce();

		expect(container.textContent).toContain('On startup');
		expect(container.textContent).toContain('Planned');
		expect(container.querySelectorAll('[data-settings-group]')).toHaveLength(5);
		const plannedControls = container.querySelectorAll('[data-planned-setting]');
		expect(plannedControls).toHaveLength(8);
		for (const control of plannedControls) {
			expect(control.querySelectorAll('button, input, select')).toHaveLength(1);
			expect(control.querySelector('[disabled]')).not.toBeNull();
		}

		await act(async () => {
			setInputValue(search, 'does not exist');
			search.dispatchEvent(new Event('input', { bubbles: true }));
		});
		expect(container.textContent).toContain('No settings found.');
		expect(container.textContent).toContain('On startup');
		expect(container.querySelectorAll('[data-settings-group]')).toHaveLength(0);
		expect(container.querySelector('[role="listbox"]')).not.toBeNull();
		const emptyResult = requiredElement<HTMLElement>(
			container.querySelector('[role="option"][aria-disabled="true"]'),
		);
		expect(emptyResult.getAttribute('aria-selected')).toBe('false');
		expect(emptyResult.textContent).toBe('No settings found.');

		await act(async () => {
			setInputValue(search, 'font');
			search.dispatchEvent(new Event('input', { bubbles: true }));
		});
		expect(container.textContent).toContain('On startup');
		expect(container.textContent).toContain('Text›Terminal font family');
		expect(container.querySelector('[role="listbox"]')).not.toBeNull();
		await act(async () =>
			requiredElement<HTMLButtonElement>(
				container.querySelector("button[data-settings-search-result='text.font-family']"),
			).click(),
		);
		expect(search.value).toBe('');
		expect(container.querySelectorAll('[data-settings-group]')).toHaveLength(5);
		expect(container.textContent).toContain('Terminal font family');
		await act(async () =>
			requiredElement<HTMLButtonElement>(
				container.querySelector("button[data-settings-category='appearance']"),
			).click(),
		);
		const applicationTheme = requiredElement<HTMLSelectElement>(
			container.querySelector("select[aria-label='Application theme']"),
		);
		await act(async () => {
			applicationTheme.value = 'dark';
			applicationTheme.dispatchEvent(new Event('change', { bubbles: true }));
		});
		expect(document.documentElement.dataset['theme']).toBe('dark');
		expect(presentationListener).not.toHaveBeenCalled();

		await act(async () =>
			requiredElement<HTMLButtonElement>(
				container.querySelector("button[data-settings-category='colors']"),
			).click(),
		);
		const terminalTheme = requiredElement<HTMLSelectElement>(
			container.querySelector("select[aria-label='Terminal display theme']"),
		);
		await act(async () => {
			terminalTheme.value = 'dark';
			terminalTheme.dispatchEvent(new Event('change', { bubbles: true }));
		});
		expect(presentationListener).not.toHaveBeenCalled();
		expect(controller.current().defaultColorOverride).toBeNull();
		expect(document.documentElement.dataset['theme']).toBe('dark');
		expect(storage.setItem).toHaveBeenCalledTimes(2);

		await act(async () =>
			requiredElement<HTMLButtonElement>(
				container.querySelector("button[data-settings-category='general']"),
			).click(),
		);
		const reset = requiredElement<HTMLButtonElement>(
			[...container.querySelectorAll('button')].find(
				(button) => button.textContent?.trim() === 'Reset',
			),
		);
		await act(async () => reset.click());
		expect(controller.getSnapshot().preferences).toEqual(DEFAULT_TERMINAL_APPEARANCE_PREFERENCES);
		await act(async () =>
			requiredElement<HTMLButtonElement>(
				container.querySelector("button[data-settings-category='appearance']"),
			).click(),
		);
		expect(
			requiredElement<HTMLSelectElement>(
				container.querySelector("select[aria-label='Application theme']"),
			).value,
		).toBe('light');
		await act(async () =>
			requiredElement<HTMLButtonElement>(
				container.querySelector("button[data-settings-category='colors']"),
			).click(),
		);
		expect(
			requiredElement<HTMLSelectElement>(
				container.querySelector("select[aria-label='Terminal display theme']"),
			).value,
		).toBe('host');

		await act(async () => root.unmount());
		container.remove();
		controller.dispose();
		keyboardShortcutController.dispose();
	});

	it('supports Cursor-style result selection, Enter activation, and layered Escape', async () => {
		const container = document.createElement('div');
		document.body.append(container);
		const storage = memoryStorage();
		const controller = new TerminalSettingsController({
			storage,
			themeTarget: document.documentElement,
		});
		const keyboardShortcutController = new KeyboardShortcutSettingsController({
			platform: 'apple',
			storage,
		});
		const close = vi.fn<() => void>();
		controller.open();
		const root = createRoot(container);
		await act(async () => {
			root.render(
				createElement(
					OrchbeeI18nProvider,
					{ locale: 'en-US' },
					createElement(TerminalSettingsSurface, {
						controller,
						keyboardShortcutController,
						keyboardShortcutPlatform: 'apple',
						onRequestClose: close,
					}),
				),
			);
		});

		const search = requiredElement<HTMLInputElement>(
			container.querySelector("input[aria-label='Search settings']"),
		);
		await act(async () => {
			setInputValue(search, 'theme');
			search.dispatchEvent(new Event('input', { bubbles: true }));
		});
		expect(container.textContent).toContain('On startup');
		expect(
			container
				.querySelector("[data-settings-search-result='appearance.application-theme']")
				?.getAttribute('data-active'),
		).toBe('true');

		await act(async () => {
			search.dispatchEvent(
				new KeyboardEvent('keydown', { bubbles: true, cancelable: true, key: 'ArrowDown' }),
			);
		});
		expect(
			container
				.querySelector("[data-settings-search-result='colors.display-theme']")
				?.getAttribute('data-active'),
		).toBe('true');
		await act(async () => {
			search.dispatchEvent(
				new KeyboardEvent('keydown', { bubbles: true, cancelable: true, key: 'Enter' }),
			);
		});
		expect(search.value).toBe('');
		expect(document.activeElement).toBe(search);
		expect(container.querySelector('[data-settings-content="colors"]')).not.toBeNull();
		expect(container.querySelector("[id='colors.display-theme']")).not.toBeNull();

		await act(async () => {
			setInputValue(search, 'font');
			search.dispatchEvent(new Event('input', { bubbles: true }));
		});
		await act(async () => {
			search.dispatchEvent(
				new KeyboardEvent('keydown', { bubbles: true, cancelable: true, key: 'Escape' }),
			);
		});
		expect(search.value).toBe('');
		expect(close).not.toHaveBeenCalled();
		expect(container.querySelector('[data-settings-content="colors"]')).not.toBeNull();

		await act(async () => {
			search.dispatchEvent(
				new KeyboardEvent('keydown', { bubbles: true, cancelable: true, key: 'Escape' }),
			);
		});
		expect(close).toHaveBeenCalledOnce();

		await act(async () => root.unmount());
		container.remove();
		controller.dispose();
		keyboardShortcutController.dispose();
	});

	it('exposes a Desktop macOS-only Option-as-Alt preference and applies it live', async () => {
		const container = document.createElement('div');
		document.body.append(container);
		const storage = memoryStorage();
		const controller = new TerminalSettingsController({
			storage,
			themeTarget: document.documentElement,
		});
		const keyboardShortcutController = new KeyboardShortcutSettingsController({
			platform: 'apple',
			storage,
		});
		const macosTerminalInputSettingsController = new MacosTerminalInputSettingsController(storage);
		const inputListener = vi.fn<(configuration: TerminalInputConfiguration) => void>();
		macosTerminalInputSettingsController.subscribe(inputListener);
		controller.open();
		const root = createRoot(container);

		await act(async () => {
			root.render(
				createElement(
					OrchbeeI18nProvider,
					{ locale: 'en-US' },
					createElement(TerminalSettingsSurface, {
						controller,
						keyboardShortcutController,
						keyboardShortcutPlatform: 'apple',
						macosTerminalInputSettingsController,
						onRequestClose: () => undefined,
					}),
				),
			);
		});

		const macosInputCategory = requiredElement<HTMLButtonElement>(
			container.querySelector("button[data-settings-category='cursor-input']"),
		);
		await act(async () => macosInputCategory.click());
		const optionAsAlt = requiredElement<HTMLSelectElement>(
			container.querySelector("select[aria-label='Use Option as Alt']"),
		);
		expect(optionAsAlt.value).toBe('off');
		await act(async () => {
			optionAsAlt.value = 'right';
			optionAsAlt.dispatchEvent(new Event('change', { bubbles: true }));
		});
		expect(macosTerminalInputSettingsController.current()).toEqual({ optionAsAlt: 'right' });
		expect(inputListener).toHaveBeenCalledWith({ optionAsAlt: 'right' });

		await act(async () => root.unmount());
		container.remove();
		controller.dispose();
		keyboardShortcutController.dispose();
		macosTerminalInputSettingsController.dispose();
	});

	it('validates draft values and opens the Desktop-only Browser access capability', async () => {
		const container = document.createElement('div');
		document.body.append(container);
		const storage = memoryStorage();
		const controller = new TerminalSettingsController({
			storage,
			themeTarget: document.documentElement,
		});
		const keyboardShortcutController = new KeyboardShortcutSettingsController({
			platform: 'apple',
			storage,
		});
		controller.open();
		const requestBrowserAccess = vi.fn<(returnFocus: HTMLElement) => void>();
		const root = createRoot(container);
		await act(async () => {
			root.render(
				createElement(
					OrchbeeI18nProvider,
					{ locale: 'en-US' },
					createElement(TerminalSettingsSurface, {
						controller,
						keyboardShortcutController,
						keyboardShortcutPlatform: 'apple',
						onRequestClose: () => undefined,
						requestBrowserAccess,
					}),
				),
			);
		});

		await act(async () =>
			requiredElement<HTMLButtonElement>(
				container.querySelector("button[data-settings-category='text']"),
			).click(),
		);
		const fontSize = requiredElement<HTMLInputElement>(
			container.querySelector("input[aria-label='Terminal font size']"),
		);
		await act(async () => {
			fontSize.focus();
			setInputValue(fontSize, '49');
			fontSize.dispatchEvent(new Event('input', { bubbles: true }));
			fontSize.blur();
		});
		expect(fontSize.getAttribute('aria-invalid')).toBe('true');
		expect(controller.getSnapshot().preferences.fontSize).toBe(13);

		const browserCategory = requiredElement<HTMLButtonElement>(
			[...container.querySelectorAll('button')].find(
				(button) => button.textContent?.trim() === 'Browser access',
			),
		);
		await act(async () => browserCategory.click());
		const open = requiredElement<HTMLButtonElement>(
			[...container.querySelectorAll('button')].find(
				(button) => button.textContent?.trim() === 'Open Browser access',
			),
		);
		await act(async () => open.click());
		expect(requestBrowserAccess).toHaveBeenCalledOnce();
		expect(controller.getSnapshot().open).toBe(true);

		await act(async () => root.unmount());
		container.remove();
		controller.dispose();
		keyboardShortcutController.dispose();
	});

	it('keeps the shortcut table stable during global search and supports shortcut editing', async () => {
		const container = document.createElement('div');
		document.body.append(container);
		const storage = memoryStorage();
		const controller = new TerminalSettingsController({
			storage,
			themeTarget: document.documentElement,
		});
		const keyboardShortcutController = new KeyboardShortcutSettingsController({
			platform: 'standard',
			storage,
		});
		controller.open();
		const close = vi.fn<() => void>();
		const root = createRoot(container);
		await act(async () => {
			root.render(
				createElement(
					OrchbeeI18nProvider,
					{ locale: 'en-US' },
					createElement(TerminalSettingsSurface, {
						controller,
						keyboardShortcutController,
						keyboardShortcutPlatform: 'standard',
						onRequestClose: close,
					}),
				),
			);
		});

		const keyboardCategory = requiredElement<HTMLButtonElement>(
			[...container.querySelectorAll('button')].find(
				(button) => button.textContent?.trim() === 'Keyboard shortcuts',
			),
		);
		await act(async () => keyboardCategory.click());
		expect(container.querySelectorAll('[data-shortcut-id]')).toHaveLength(13);
		expect(container.textContent).toContain('Ctrl+Shift+T');
		expect(container.textContent).toContain('Ctrl+Shift+W');
		expect(container.textContent).toContain('Focused terminal');

		const search = requiredElement<HTMLInputElement>(
			container.querySelector("input[aria-label='Search settings']"),
		);
		await act(async () => {
			setInputValue(search, 'Focused terminal');
			search.dispatchEvent(new Event('input', { bubbles: true }));
		});
		expect(container.querySelectorAll('[data-shortcut-id]')).toHaveLength(13);
		expect(
			container.querySelectorAll('[data-settings-search-result]').length,
		).toBeGreaterThanOrEqual(3);
		await act(async () => {
			setInputValue(search, '');
			search.dispatchEvent(new Event('input', { bubbles: true }));
		});

		const globalSearchRecorder = requiredElement<HTMLButtonElement>(
			container.querySelector("button[aria-label='Change shortcut for Search everything']"),
		);
		await act(async () => globalSearchRecorder.click());
		await act(async () => {
			globalSearchRecorder.dispatchEvent(
				new KeyboardEvent('keydown', {
					bubbles: true,
					cancelable: true,
					code: 'KeyG',
					ctrlKey: true,
					key: 'G',
					shiftKey: true,
				}),
			);
		});
		expect(keyboardShortcutController.current()['workbench.search.all.open']).toEqual({
			code: 'KeyG',
			modifiers: ['mod', 'shift'],
		});
		expect(
			container.querySelector("[data-shortcut-id='workbench.search.all.open']")?.textContent,
		).toContain('Custom');

		const actionsSearchRecorder = requiredElement<HTMLButtonElement>(
			container.querySelector("button[aria-label='Change shortcut for Search actions']"),
		);
		await act(async () => actionsSearchRecorder.click());
		await act(async () => {
			actionsSearchRecorder.dispatchEvent(
				new KeyboardEvent('keydown', {
					bubbles: true,
					cancelable: true,
					code: 'KeyG',
					ctrlKey: true,
					key: 'G',
					shiftKey: true,
				}),
			);
		});
		const actionsRow = requiredElement<HTMLElement>(
			container.querySelector("[data-shortcut-id='workbench.search.actions.open']"),
		);
		expect(actionsRow.textContent).toContain('Search everything');
		expect(actionsRow.textContent).toContain('Workbench');
		expect(actionsRow.textContent).toContain('Custom');
		expect(keyboardShortcutController.current()['workbench.search.actions.open']).toEqual({
			code: 'KeyP',
			modifiers: ['mod', 'shift'],
		});

		await act(async () => actionsSearchRecorder.click());
		await act(async () => {
			actionsSearchRecorder.dispatchEvent(
				new KeyboardEvent('keydown', {
					bubbles: true,
					cancelable: true,
					code: 'Escape',
					key: 'Escape',
				}),
			);
		});
		expect(controller.getSnapshot().open).toBe(true);
		expect(close).not.toHaveBeenCalled();

		const reset = requiredElement<HTMLButtonElement>(
			[...container.querySelectorAll('button')].find(
				(button) =>
					button.textContent?.trim() === 'Restore default' &&
					button.closest('[data-shortcut-id]')?.getAttribute('data-shortcut-id') ===
						'workbench.search.all.open',
			),
		);
		await act(async () => reset.click());
		expect(keyboardShortcutController.current()['workbench.search.all.open']).toEqual({
			code: 'KeyK',
			modifiers: ['mod', 'shift'],
		});

		await act(async () => root.unmount());
		container.remove();
		controller.dispose();
		keyboardShortcutController.dispose();
	});

	it('explains a pasted Ghostty configuration without applying or persisting it', async () => {
		const container = document.createElement('div');
		document.body.append(container);
		const storage = memoryStorage();
		const controller = new TerminalSettingsController({
			storage,
			themeTarget: document.documentElement,
		});
		const keyboardShortcutController = new KeyboardShortcutSettingsController({
			platform: 'apple',
			storage,
		});
		controller.open();
		const initialPreferences = controller.getSnapshot().preferences;
		const root = createRoot(container);
		await act(async () => {
			root.render(
				createElement(
					OrchbeeI18nProvider,
					{ locale: 'en-US' },
					createElement(TerminalSettingsSurface, {
						controller,
						keyboardShortcutController,
						keyboardShortcutPlatform: 'apple',
						onRequestClose: () => undefined,
					}),
				),
			);
		});

		const migrationCategory = requiredElement<HTMLButtonElement>(
			[...container.querySelectorAll('button')].find(
				(button) => button.textContent?.trim() === 'Ghostty migration',
			),
		);
		await act(async () => migrationCategory.click());
		expect(container.textContent).toContain(
			'This is a dry run. Orchbee will not change or save any settings.',
		);

		const source = requiredElement<HTMLTextAreaElement>(
			container.querySelector('#ghostty-migration-source'),
		);
		await act(async () => {
			setTextareaValue(
				source,
				[
					'font-family = JetBrains Mono',
					'macos-option-as-alt = right',
					'font-size = 13.5',
					'background = #abc',
					'theme = light:Day,dark:Night',
					'cursor-style = block',
					'broken',
				].join('\n'),
			);
			source.dispatchEvent(new Event('input', { bubbles: true }));
		});
		const analyze = requiredElement<HTMLButtonElement>(
			[...container.querySelectorAll('button')].find(
				(button) => button.textContent?.trim() === 'Analyze configuration',
			),
		);
		await act(async () => analyze.click());

		const report = requiredElement<HTMLElement>(
			container.querySelector("section[aria-labelledby='ghostty-migration-report-title']"),
		);
		expect(report.textContent).toContain('Can carry over3');
		expect(report.textContent).toContain('Review manually2');
		expect(report.textContent).toContain('Not migrated1');
		expect(report.textContent).toContain('Needs correction1');
		expect(report.textContent).toContain('Terminal font familyJetBrains Mono');
		expect(report.textContent).toContain('Background color#aabbcc');
		expect(report.textContent).toContain('Use Option as AltRight Option only');
		expect(controller.getSnapshot().preferences).toBe(initialPreferences);
		expect(storage.setItem).not.toHaveBeenCalled();

		const terminalCategory = requiredElement<HTMLButtonElement>(
			[...container.querySelectorAll('button')].find(
				(button) => button.textContent?.trim() === 'Terminal',
			),
		);
		await act(async () => terminalCategory.click());
		await act(async () => migrationCategory.click());
		const remountedSource = requiredElement<HTMLTextAreaElement>(
			container.querySelector('#ghostty-migration-source'),
		);
		expect(remountedSource.value).toBe('');
		expect(container.textContent).not.toContain('Migration report');

		await act(async () => root.unmount());
		container.remove();
		controller.dispose();
		keyboardShortcutController.dispose();
	});

	it('keeps the read-only remote diagnostic stable during global search without side effects', async () => {
		const container = document.createElement('div');
		document.body.append(container);
		const storage = memoryStorage();
		const controller = new TerminalSettingsController({
			storage,
			themeTarget: document.documentElement,
		});
		const keyboardShortcutController = new KeyboardShortcutSettingsController({
			platform: 'apple',
			storage,
		});
		controller.open();
		const root = createRoot(container);
		await act(async () => {
			root.render(
				createElement(
					OrchbeeI18nProvider,
					{ locale: 'en-US' },
					createElement(TerminalSettingsSurface, {
						controller,
						keyboardShortcutController,
						keyboardShortcutPlatform: 'apple',
						onRequestClose: () => undefined,
					}),
				),
			);
		});

		const remoteCategory = requiredElement<HTMLButtonElement>(
			container.querySelector("button[data-settings-category='remote-development']"),
		);
		await act(async () => remoteCategory.click());
		const diagnostic = requiredElement<HTMLTextAreaElement>(
			container.querySelector('#remote-development-diagnostic-command'),
		);
		expect(diagnostic.readOnly).toBe(true);
		expect(diagnostic.value).toBe(REMOTE_DEVELOPMENT_DIAGNOSTIC_COMMAND);
		expect(container.textContent).toContain('xterm-256color');
		expect(container.textContent).toContain('No automatic remote changes');
		expect(container.textContent).toContain('does not wrap SSH');
		expect(storage.setItem).not.toHaveBeenCalled();

		const search = requiredElement<HTMLInputElement>(
			container.querySelector("input[aria-label='Search settings']"),
		);
		await act(async () => {
			setInputValue(search, 'infocmp');
			search.dispatchEvent(new Event('input', { bubbles: true }));
		});
		expect(container.querySelector('#remote-development-diagnostic-command')).not.toBeNull();
		await act(async () => {
			setInputValue(search, 'automatic installation action');
			search.dispatchEvent(new Event('input', { bubbles: true }));
		});
		expect(container.textContent).toContain('No settings found.');
		expect(container.querySelector('#remote-development-diagnostic-command')).not.toBeNull();
		expect(storage.setItem).not.toHaveBeenCalled();

		await act(async () => root.unmount());
		container.remove();
		controller.dispose();
		keyboardShortcutController.dispose();
	});

	it('searches the Chinese catalog without falling back to English', async () => {
		const container = document.createElement('div');
		document.body.append(container);
		const storage = memoryStorage();
		const controller = new TerminalSettingsController({
			storage,
			themeTarget: document.createElement('div'),
		});
		const keyboardShortcutController = new KeyboardShortcutSettingsController({
			platform: 'apple',
			storage,
		});
		controller.open();
		const root = createRoot(container);
		await act(async () => {
			root.render(
				createElement(
					OrchbeeI18nProvider,
					{ locale: 'zh-CN' },
					createElement(TerminalSettingsSurface, {
						controller,
						keyboardShortcutController,
						keyboardShortcutPlatform: 'apple',
						onRequestClose: () => undefined,
					}),
				),
			);
		});

		const search = requiredElement<HTMLInputElement>(
			container.querySelector("input[aria-label='搜索设置']"),
		);
		await act(async () =>
			requiredElement<HTMLButtonElement>(
				container.querySelector("button[data-settings-category='text']"),
			).click(),
		);
		expect(container.textContent).toContain('终端字体族');
		expect(container.textContent).not.toContain('Terminal font family');

		await act(async () => {
			setInputValue(search, '字体');
			search.dispatchEvent(new Event('input', { bubbles: true }));
		});
		expect(container.textContent).toContain('终端字体族');
		expect(container.textContent).not.toContain('Application theme');
		await act(async () => {
			setInputValue(search, '');
			search.dispatchEvent(new Event('input', { bubbles: true }));
		});

		const keyboardCategory = requiredElement<HTMLButtonElement>(
			[...container.querySelectorAll('button')].find(
				(button) => button.textContent?.trim() === '键盘快捷键',
			),
		);
		await act(async () => keyboardCategory.click());
		expect(container.textContent).toContain('搜索操作');
		expect(container.textContent).toContain('已聚焦的终端');
		expect(container.textContent).not.toContain('Search actions');

		const migrationCategory = requiredElement<HTMLButtonElement>(
			[...container.querySelectorAll('button')].find(
				(button) => button.textContent?.trim() === 'Ghostty 迁移',
			),
		);
		await act(async () => migrationCategory.click());
		expect(container.textContent).toContain('从 Ghostty 迁移');
		expect(container.textContent).toContain('这只是预检，Orchbee 不会更改或保存任何设置。');
		expect(container.textContent).not.toContain('This is a dry run');

		const remoteCategory = requiredElement<HTMLButtonElement>(
			container.querySelector("button[data-settings-category='remote-development']"),
		);
		await act(async () => remoteCategory.click());
		expect(container.textContent).toContain('诊断远端 shell');
		expect(container.textContent).toContain('不会自动修改远端');
		expect(container.textContent).not.toContain('No automatic remote changes');

		await act(async () => root.unmount());
		container.remove();
		controller.dispose();
		keyboardShortcutController.dispose();
	});
});

function memoryStorage(): {
	readonly getItem: ReturnType<typeof vi.fn<TerminalAppearanceStorage['getItem']>>;
	readonly setItem: ReturnType<typeof vi.fn<TerminalAppearanceStorage['setItem']>>;
} {
	return {
		getItem: vi.fn<TerminalAppearanceStorage['getItem']>(() => null),
		setItem: vi.fn<TerminalAppearanceStorage['setItem']>(() => undefined),
	};
}

function requiredElement<ElementType extends Element>(
	element: ElementType | null | undefined,
): ElementType {
	if (element === null || element === undefined) {
		throw new Error('Expected element was not rendered.');
	}
	return element;
}

function setInputValue(input: HTMLInputElement, value: string): void {
	const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')?.set;
	if (setter === undefined) throw new Error('The input value setter is unavailable.');
	setter.call(input, value);
}

function setTextareaValue(input: HTMLTextAreaElement, value: string): void {
	const setter = Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, 'value')?.set;
	if (setter === undefined) throw new Error('The textarea value setter is unavailable.');
	setter.call(input, value);
}
