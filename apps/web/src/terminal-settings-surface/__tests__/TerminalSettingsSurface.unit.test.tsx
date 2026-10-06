// @vitest-environment jsdom

import { OrchbeeI18nProvider } from '@orchbee/i18n';
import type {
	TerminalInputConfiguration,
	TerminalInteractionConfiguration,
} from '@orchbee/terminal';
import { act, createElement } from 'react';
import { createRoot } from 'react-dom/client';
import { afterAll, afterEach, beforeAll, describe, expect, it, vi } from 'vitest';

import { TerminalSettingsSurface } from '../TerminalSettingsSurface';
import { ApplicationLocaleController } from '../../application-locale';
import { ApplicationLocaleProvider } from '../../application-locale-provider';
import { KeyboardShortcutSettingsController } from '../../keyboard-shortcuts';
import { MacosTerminalInputSettingsController } from '../../macos-terminal-input-settings';
import { TerminalInteractionSettingsController } from '../../terminal-interaction-settings';
import {
	DEFAULT_TERMINAL_APPEARANCE_PREFERENCES,
	TerminalSettingsController,
} from '../../terminal-settings';
import { REMOTE_DEVELOPMENT_DIAGNOSTIC_COMMAND } from '../constants/remote-development-diagnostic';
import type {
	DesktopTerminalDefaultSettingsClientSnapshot,
	DesktopTerminalDefaultSettingsControl,
} from '../../terminal-default-settings-control';

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
	const testResources = {
		containers: [] as HTMLElement[],
		disposables: [] as { dispose(): void }[],
		roots: [] as ReturnType<typeof createRoot>[],
	};

	function trackContainer<ElementType extends HTMLElement>(element: ElementType): ElementType {
		testResources.containers.push(element);
		return element;
	}
	function trackDisposable<DisposableType extends { dispose(): void }>(
		disposable: DisposableType,
	): DisposableType {
		testResources.disposables.push(disposable);
		return disposable;
	}
	function trackRoot(root: ReturnType<typeof createRoot>): ReturnType<typeof createRoot> {
		testResources.roots.push(root);
		return root;
	}

	afterEach(async () => {
		const failures: unknown[] = [];
		for (const root of testResources.roots.splice(0)) {
			try {
				await act(async () => root.unmount());
			} catch (error) {
				failures.push(error);
			}
		}
		for (const container of testResources.containers.splice(0)) {
			try {
				container.remove();
			} catch (error) {
				failures.push(error);
			}
		}
		for (const disposable of testResources.disposables.splice(0)) {
			try {
				disposable.dispose();
			} catch (error) {
				failures.push(error);
			}
		}
		if (failures.length === 1) throw failures[0];
		if (failures.length > 1) throw new AggregateError(failures, 'test cleanup failed');
	});

	it('focuses global search, navigates categories, and routes theme choices outside presentation configuration', async () => {
		const container = trackContainer(document.createElement('div'));
		document.body.append(container);
		const storage = memoryStorage();
		const controller = trackDisposable(
			new TerminalSettingsController({
				storage,
				themeTarget: document.documentElement,
			}),
		);
		const keyboardShortcutController = trackDisposable(
			new KeyboardShortcutSettingsController({
				platform: 'apple',
				storage,
			}),
		);
		const presentationListener =
			vi.fn<(configuration: ReturnType<typeof controller.current>) => void>();
		controller.subscribe(presentationListener);
		controller.open();
		const close = vi.fn<() => void>();
		const root = trackRoot(createRoot(container));

		await act(async () => {
			root.render(
				createElement(
					OrchbeeI18nProvider,
					{ locale: 'en-US' },
					createElement(TerminalSettingsSurface, {
						applicationLocaleController: trackDisposable(createApplicationLocaleController()),
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
		expect(
			container.querySelector("[data-settings-layout='bounded'][role='dialog']"),
		).not.toBeNull();
		const closeSettings = requiredElement<HTMLButtonElement>(
			container.querySelector('button[data-settings-close]'),
		);
		expect(closeSettings.getAttribute('aria-label')).toBe('Close settings');
		expect(container.textContent).toContain('Settings');
		await act(async () => closeSettings.click());
		expect(close).toHaveBeenCalledOnce();

		expect(container.textContent).toContain('Display language');
		expect(container.textContent).not.toContain('Planned');
		expect(container.querySelector("button[data-settings-category='windows-tabs']")).toBeNull();
		expect(container.querySelectorAll('[data-settings-group]')).toHaveLength(5);
		expect(
			Array.from(
				container.querySelectorAll('[data-settings-group-label]'),
				(groupLabel) => groupLabel.textContent,
			),
		).toEqual(['Application', 'Terminal', 'Workspace', 'Integrations', 'Data & Access']);

		await act(async () => {
			setInputValue(search, 'does not exist');
			search.dispatchEvent(new Event('input', { bubbles: true }));
		});
		expect(container.textContent).toContain('No settings found.');
		expect(container.textContent).toContain('Display language');
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
		expect(container.textContent).toContain('Display language');
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
	});

	it('switches the mounted Settings copy and persists an explicit display language', async () => {
		const container = trackContainer(document.createElement('div'));
		document.body.append(container);
		const storage = memoryStorage();
		const applicationLocaleController = trackDisposable(
			new ApplicationLocaleController({
				documentElement: document.documentElement,
				preferredLocales: ['zh-CN'],
				storage,
			}),
		);
		const controller = trackDisposable(
			new TerminalSettingsController({
				storage,
				themeTarget: document.documentElement,
			}),
		);
		const keyboardShortcutController = trackDisposable(
			new KeyboardShortcutSettingsController({
				platform: 'apple',
				storage,
			}),
		);
		controller.open();
		const root = trackRoot(createRoot(container));

		await act(async () => {
			root.render(
				createElement(
					ApplicationLocaleProvider,
					{ source: applicationLocaleController },
					createElement(TerminalSettingsSurface, {
						applicationLocaleController,
						controller,
						keyboardShortcutController,
						keyboardShortcutPlatform: 'apple',
						onRequestClose: () => undefined,
					}),
				),
			);
		});

		const chineseSelect = requiredElement<HTMLSelectElement>(
			container.querySelector("select[aria-label='显示语言']"),
		);
		expect(chineseSelect.value).toBe('zh-CN');
		expect(chineseSelect.options).toHaveLength(2);
		expect(storage.setItem).not.toHaveBeenCalled();
		expect(container.querySelector('[data-setting-id="general.language"]')).not.toBeNull();

		await act(async () => {
			setSelectValue(chineseSelect, 'en-US');
			chineseSelect.dispatchEvent(new Event('change', { bubbles: true }));
		});

		expect(document.documentElement.lang).toBe('en-US');
		expect(container.querySelector("select[aria-label='Display language']")).not.toBeNull();
		expect(container.textContent).toContain('Settings');
		expect(storage.setItem).toHaveBeenCalledWith(
			'orchbee:web:application-locale:v1',
			'{"locale":"en-US"}',
		);
	});

	it('supports deterministic result selection, destination focus, and layered Escape', async () => {
		const container = trackContainer(document.createElement('div'));
		document.body.append(container);
		const storage = memoryStorage();
		const controller = trackDisposable(
			new TerminalSettingsController({
				storage,
				themeTarget: document.documentElement,
			}),
		);
		const keyboardShortcutController = trackDisposable(
			new KeyboardShortcutSettingsController({
				platform: 'apple',
				storage,
			}),
		);
		const close = vi.fn<() => void>();
		controller.open();
		const root = trackRoot(createRoot(container));
		await act(async () => {
			root.render(
				createElement(
					OrchbeeI18nProvider,
					{ locale: 'en-US' },
					createElement(TerminalSettingsSurface, {
						applicationLocaleController: trackDisposable(createApplicationLocaleController()),
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
		expect(container.textContent).toContain('Display language');
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
		expect(document.activeElement?.id).toBe('colors.display-theme');
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
	});

	it('applies typography, padding, Agent tail, and copy settings live', async () => {
		const container = trackContainer(document.createElement('div'));
		document.body.append(container);
		const storage = memoryStorage();
		const controller = trackDisposable(
			new TerminalSettingsController({
				storage,
				themeTarget: document.documentElement,
			}),
		);
		const interactionController = trackDisposable(
			new TerminalInteractionSettingsController(storage),
		);
		const interactionListener = vi.fn<(configuration: TerminalInteractionConfiguration) => void>();
		interactionController.subscribe(interactionListener);
		const keyboardShortcutController = trackDisposable(
			new KeyboardShortcutSettingsController({
				platform: 'apple',
				storage,
			}),
		);
		controller.open();
		const root = trackRoot(createRoot(container));
		await act(async () => {
			root.render(
				createElement(
					OrchbeeI18nProvider,
					{ locale: 'en-US' },
					createElement(TerminalSettingsSurface, {
						applicationLocaleController: trackDisposable(createApplicationLocaleController()),
						controller,
						keyboardShortcutController,
						keyboardShortcutPlatform: 'apple',
						onRequestClose: () => undefined,
						terminalInteractionSettingsController: interactionController,
					}),
				),
			);
		});

		await act(async () =>
			requiredElement<HTMLButtonElement>(
				container.querySelector("button[data-settings-category='appearance']"),
			).click(),
		);
		const padding = requiredElement<HTMLInputElement>(
			container.querySelector("input[aria-label='Terminal padding']"),
		);
		await act(async () => {
			padding.focus();
			setInputValue(padding, '12');
			padding.dispatchEvent(new Event('input', { bubbles: true }));
			padding.blur();
		});

		await act(async () =>
			requiredElement<HTMLButtonElement>(
				container.querySelector("button[data-settings-category='text']"),
			).click(),
		);
		const regularWeight = requiredElement<HTMLSelectElement>(
			container.querySelector("select[aria-label='Regular font weight']"),
		);
		const boldWeight = requiredElement<HTMLSelectElement>(
			container.querySelector("select[aria-label='Bold font weight']"),
		);
		const ligatures = requiredElement<HTMLElement>(
			container.querySelector("[aria-label='Font ligatures']"),
		);
		await act(async () => {
			regularWeight.value = '300';
			regularWeight.dispatchEvent(new Event('change', { bubbles: true }));
			boldWeight.value = '800';
			boldWeight.dispatchEvent(new Event('change', { bubbles: true }));
			ligatures.click();
		});
		expect(controller.current()).toMatchObject({
			fontLigatures: false,
			fontWeight: 300,
			fontWeightBold: 800,
			terminalPadding: 12,
		});

		await act(async () =>
			requiredElement<HTMLButtonElement>(
				container.querySelector("button[data-settings-category='terminal']"),
			).click(),
		);
		const trimTrailingBlankRows = requiredElement<HTMLElement>(
			container.querySelector("[aria-label='Trim trailing blank rows for CLI agents']"),
		);
		expect(trimTrailingBlankRows.getAttribute('aria-checked')).toBe('true');
		await act(async () => trimTrailingBlankRows.click());
		expect(interactionController.getSnapshot().preferences).toEqual({
			copyOnSelect: true,
			trimTrailingBlankRowsForAgents: false,
		});
		expect(interactionListener).not.toHaveBeenCalled();

		await act(async () =>
			requiredElement<HTMLButtonElement>(
				container.querySelector("button[data-settings-category='interaction']"),
			).click(),
		);
		const copyOnSelect = requiredElement<HTMLElement>(
			container.querySelector("[aria-label='Copy on select']"),
		);
		await act(async () => copyOnSelect.click());
		expect(interactionController.current()).toEqual({ copyOnSelect: false });
		expect(interactionListener).toHaveBeenCalledWith({ copyOnSelect: false });
	});

	it('commits Desktop scrollback defaults for future terminals and keeps failures visible', async () => {
		const container = trackContainer(document.createElement('div'));
		document.body.append(container);
		const storage = memoryStorage();
		const controller = trackDisposable(
			new TerminalSettingsController({
				storage,
				themeTarget: document.documentElement,
			}),
		);
		const keyboardShortcutController = trackDisposable(
			new KeyboardShortcutSettingsController({
				platform: 'apple',
				storage,
			}),
		);
		const terminalDefaults = createTerminalDefaultSettingsControl();
		controller.open();
		const root = trackRoot(createRoot(container));
		await act(async () => {
			root.render(
				createElement(
					OrchbeeI18nProvider,
					{ locale: 'en-US' },
					createElement(TerminalSettingsSurface, {
						applicationLocaleController: trackDisposable(createApplicationLocaleController()),
						controller,
						keyboardShortcutController,
						keyboardShortcutPlatform: 'apple',
						onRequestClose: () => undefined,
						terminalDefaultSettingsControl: terminalDefaults.control,
					}),
				),
			);
		});

		await act(async () =>
			requiredElement<HTMLButtonElement>(
				container.querySelector("button[data-settings-category='terminal']"),
			).click(),
		);
		expect(container.textContent).toContain('Running terminals keep their current history limit.');
		const scrollback = requiredElement<HTMLInputElement>(
			container.querySelector("input[aria-label='Scrollback limit']"),
		);
		expect(scrollback.value).toBe('10000000');

		await act(async () => {
			scrollback.focus();
			setInputValue(scrollback, '1000000001');
			scrollback.dispatchEvent(new Event('input', { bubbles: true }));
			scrollback.blur();
		});
		expect(scrollback.getAttribute('aria-invalid')).toBe('true');
		expect(terminalDefaults.update).not.toHaveBeenCalled();

		await act(async () => {
			scrollback.focus();
			setInputValue(scrollback, '25000000');
			scrollback.dispatchEvent(new Event('input', { bubbles: true }));
			scrollback.blur();
		});
		expect(terminalDefaults.update).toHaveBeenCalledWith({ scrollbackBytes: 25_000_000 });
		expect(container.textContent).toContain(
			'This scrollback value is active for this session, but could not be saved.',
		);

		const search = requiredElement<HTMLInputElement>(
			container.querySelector("input[aria-label='Search settings']"),
		);
		await act(async () => {
			setInputValue(search, 'scrollback');
			search.dispatchEvent(new Event('input', { bubbles: true }));
		});
		const searchResult = requiredElement<HTMLElement>(
			container.querySelector("[data-settings-search-result='terminal.scrollback']"),
		);
		expect(searchResult.textContent).toContain('Scrollback limit');
	});

	it('exposes a Desktop macOS-only Option-as-Alt preference and applies it live', async () => {
		const container = trackContainer(document.createElement('div'));
		document.body.append(container);
		const storage = memoryStorage();
		const controller = trackDisposable(
			new TerminalSettingsController({
				storage,
				themeTarget: document.documentElement,
			}),
		);
		const keyboardShortcutController = trackDisposable(
			new KeyboardShortcutSettingsController({
				platform: 'apple',
				storage,
			}),
		);
		const macosTerminalInputSettingsController = trackDisposable(
			new MacosTerminalInputSettingsController(storage),
		);
		const inputListener = vi.fn<(configuration: TerminalInputConfiguration) => void>();
		macosTerminalInputSettingsController.subscribe(inputListener);
		controller.open();
		const root = trackRoot(createRoot(container));

		await act(async () => {
			root.render(
				createElement(
					OrchbeeI18nProvider,
					{ locale: 'en-US' },
					createElement(TerminalSettingsSurface, {
						applicationLocaleController: trackDisposable(createApplicationLocaleController()),
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
	});

	it('validates draft values while keeping Settings open', async () => {
		const container = trackContainer(document.createElement('div'));
		document.body.append(container);
		const storage = memoryStorage();
		const controller = trackDisposable(
			new TerminalSettingsController({
				storage,
				themeTarget: document.documentElement,
			}),
		);
		const keyboardShortcutController = trackDisposable(
			new KeyboardShortcutSettingsController({
				platform: 'apple',
				storage,
			}),
		);
		controller.open();

		const root = trackRoot(createRoot(container));
		await act(async () => {
			root.render(
				createElement(
					OrchbeeI18nProvider,
					{ locale: 'en-US' },
					createElement(TerminalSettingsSurface, {
						applicationLocaleController: trackDisposable(createApplicationLocaleController()),
						controller,
						keyboardShortcutController,
						keyboardShortcutPlatform: 'apple',
						onRequestClose: () => undefined,
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

		expect(controller.getSnapshot().open).toBe(true);
	});

	it('keeps the shortcut table stable during global search and supports shortcut editing', async () => {
		const container = trackContainer(document.createElement('div'));
		document.body.append(container);
		const storage = memoryStorage();
		const controller = trackDisposable(
			new TerminalSettingsController({
				storage,
				themeTarget: document.documentElement,
			}),
		);
		const keyboardShortcutController = trackDisposable(
			new KeyboardShortcutSettingsController({
				platform: 'standard',
				storage,
			}),
		);
		controller.open();
		const close = vi.fn<() => void>();
		const root = trackRoot(createRoot(container));
		await act(async () => {
			root.render(
				createElement(
					OrchbeeI18nProvider,
					{ locale: 'en-US' },
					createElement(TerminalSettingsSurface, {
						applicationLocaleController: trackDisposable(createApplicationLocaleController()),
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
		expect(container.querySelectorAll('[data-shortcut-id]')).toHaveLength(69);
		expect(container.textContent).toContain('Ctrl+Shift+T');
		expect(container.textContent).toContain('New terminal in current directory');
		expect(container.textContent).toContain('Ctrl+Alt+Enter');
		expect(container.textContent).toContain('Ctrl+Shift+W');
		expect(container.textContent).toContain('Clear terminal');
		expect(container.textContent).toContain('Focus panel left');
		expect(container.textContent).toContain('Focus next Workbench region');
		expect(container.textContent).toContain('Ctrl+F6');
		expect(container.textContent).toContain('Ctrl+Alt+←');
		expect(container.textContent).toContain('Focus panel 1');
		expect(container.textContent).toContain('Ctrl+Shift+1');
		expect(container.textContent).toContain('Focus panel 10');
		expect(container.textContent).toContain('Ctrl+Shift+0');
		expect(container.textContent).toContain('Visually focus panel');
		expect(container.textContent).toContain('Ctrl+Shift+F7');
		expect(container.textContent).toContain('Visually swap panel');
		expect(container.textContent).toContain('Ctrl+Shift+F8');
		expect(container.textContent).toContain('Equalize panels');
		expect(container.textContent).toContain('Ctrl+Alt+=');
		expect(container.textContent).toContain('Select next tab');
		expect(container.textContent).toContain('Ctrl+Page Down');
		expect(container.textContent).toContain('Move active tab forward');
		expect(container.textContent).toContain('Ctrl+Shift+.');
		expect(container.textContent).toContain('Scroll up one page');
		expect(container.textContent).toContain('Ctrl+Shift+Page Up');
		expect(container.textContent).toContain('Show terminal hints');
		expect(container.textContent).toContain('Ctrl+Shift+O');
		expect(container.textContent).toContain('Show last command output');
		expect(container.textContent).toContain('Ctrl+Shift+G');
		expect(container.textContent).toContain('Focused terminal');

		const search = requiredElement<HTMLInputElement>(
			container.querySelector("input[aria-label='Search settings']"),
		);
		await act(async () => {
			setInputValue(search, 'Focused terminal');
			search.dispatchEvent(new Event('input', { bubbles: true }));
		});
		expect(container.querySelectorAll('[data-shortcut-id]')).toHaveLength(69);
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
					code: 'KeyY',
					ctrlKey: true,
					key: 'Y',
					shiftKey: true,
				}),
			);
		});
		expect(keyboardShortcutController.current()['workbench.search.all.open']).toEqual({
			code: 'KeyY',
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
					code: 'KeyY',
					ctrlKey: true,
					key: 'Y',
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
	});

	it('explains a pasted Ghostty configuration without applying or persisting it', async () => {
		const container = trackContainer(document.createElement('div'));
		document.body.append(container);
		const storage = memoryStorage();
		const controller = trackDisposable(
			new TerminalSettingsController({
				storage,
				themeTarget: document.documentElement,
			}),
		);
		const keyboardShortcutController = trackDisposable(
			new KeyboardShortcutSettingsController({
				platform: 'apple',
				storage,
			}),
		);
		controller.open();
		const initialPreferences = controller.getSnapshot().preferences;
		const root = trackRoot(createRoot(container));
		await act(async () => {
			root.render(
				createElement(
					OrchbeeI18nProvider,
					{ locale: 'en-US' },
					createElement(TerminalSettingsSurface, {
						applicationLocaleController: trackDisposable(createApplicationLocaleController()),
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

		const appearanceCategory = requiredElement<HTMLButtonElement>(
			[...container.querySelectorAll('button')].find(
				(button) => button.textContent?.trim() === 'Appearance',
			),
		);
		await act(async () => appearanceCategory.click());
		await act(async () => migrationCategory.click());
		const remountedSource = requiredElement<HTMLTextAreaElement>(
			container.querySelector('#ghostty-migration-source'),
		);
		expect(remountedSource.value).toBe('');
		expect(container.textContent).not.toContain('Migration report');
	});

	it('keeps the read-only remote diagnostic stable during global search without side effects', async () => {
		const container = trackContainer(document.createElement('div'));
		document.body.append(container);
		const storage = memoryStorage();
		const controller = trackDisposable(
			new TerminalSettingsController({
				storage,
				themeTarget: document.documentElement,
			}),
		);
		const keyboardShortcutController = trackDisposable(
			new KeyboardShortcutSettingsController({
				platform: 'apple',
				storage,
			}),
		);
		controller.open();
		const root = trackRoot(createRoot(container));
		await act(async () => {
			root.render(
				createElement(
					OrchbeeI18nProvider,
					{ locale: 'en-US' },
					createElement(TerminalSettingsSurface, {
						applicationLocaleController: trackDisposable(createApplicationLocaleController()),
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
		expect(container.textContent).toContain('private reverse Unix-socket forward');
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
	});

	it('searches the Chinese catalog without falling back to English', async () => {
		const container = trackContainer(document.createElement('div'));
		document.body.append(container);
		const storage = memoryStorage();
		const controller = trackDisposable(
			new TerminalSettingsController({
				storage,
				themeTarget: document.createElement('div'),
			}),
		);
		const keyboardShortcutController = trackDisposable(
			new KeyboardShortcutSettingsController({
				platform: 'apple',
				storage,
			}),
		);
		controller.open();
		const root = trackRoot(createRoot(container));
		await act(async () => {
			root.render(
				createElement(
					OrchbeeI18nProvider,
					{ locale: 'zh-CN' },
					createElement(TerminalSettingsSurface, {
						applicationLocaleController: trackDisposable(createApplicationLocaleController()),
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
	});
});

function createApplicationLocaleController(): ApplicationLocaleController {
	return new ApplicationLocaleController({
		documentElement: document.documentElement,
		preferredLocales: ['en-US'],
		storage: memoryStorage(),
	});
}

function memoryStorage(): {
	readonly getItem: ReturnType<typeof vi.fn<TerminalAppearanceStorage['getItem']>>;
	readonly setItem: ReturnType<typeof vi.fn<TerminalAppearanceStorage['setItem']>>;
} {
	return {
		getItem: vi.fn<TerminalAppearanceStorage['getItem']>(() => null),
		setItem: vi.fn<TerminalAppearanceStorage['setItem']>(() => undefined),
	};
}

function createTerminalDefaultSettingsControl(): {
	readonly control: DesktopTerminalDefaultSettingsControl;
	readonly update: ReturnType<typeof vi.fn<DesktopTerminalDefaultSettingsControl['update']>>;
} {
	let snapshot: DesktopTerminalDefaultSettingsClientSnapshot = {
		configuration: { scrollbackBytes: 10_000_000 },
		failureCode: null,
		pending: false,
		persistenceIssue: null,
		source: 'stored',
	};
	const listeners = new Set<() => void>();
	const update = vi.fn<DesktopTerminalDefaultSettingsControl['update']>(async (configuration) => {
		snapshot = {
			configuration,
			failureCode: null,
			pending: false,
			persistenceIssue: 'write-failed',
			source: 'session',
		};
		for (const listener of listeners) listener();
		return { kind: 'completed', snapshot };
	});
	return {
		control: {
			getSnapshot: () => snapshot,
			subscribe: (listener) => {
				listeners.add(listener);
				return () => listeners.delete(listener);
			},
			update,
		},
		update,
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

function setSelectValue(input: HTMLSelectElement, value: string): void {
	const setter = Object.getOwnPropertyDescriptor(HTMLSelectElement.prototype, 'value')?.set;
	if (setter === undefined) throw new Error('The select value setter is unavailable.');
	setter.call(input, value);
}

function setTextareaValue(input: HTMLTextAreaElement, value: string): void {
	const setter = Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, 'value')?.set;
	if (setter === undefined) throw new Error('The textarea value setter is unavailable.');
	setter.call(input, value);
}
