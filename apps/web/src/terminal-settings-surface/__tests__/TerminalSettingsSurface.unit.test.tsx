// @vitest-environment jsdom

import { OrchbeeI18nProvider } from '@orchbee/i18n';
import type {
	TerminalInputConfiguration,
	TerminalInteractionConfiguration,
} from '@orchbee/terminal';
import { act, createElement } from 'react';
import { createRoot } from 'react-dom/client';
import { afterAll, afterEach, beforeAll, describe, expect, it, vi } from 'vitest';

import {
	TerminalSettingsSurface,
	type TerminalSettingsSurfaceProps,
} from '../TerminalSettingsSurface';
import { ApplicationLocaleController } from '../../application-locale';
import { ApplicationLocaleProvider } from '../../application-locale-provider';
import { KeyboardShortcutSettingsController } from '../../keyboard-shortcuts';
import { MacosTerminalInputSettingsController } from '../../macos-terminal-input-settings';
import { TerminalInteractionSettingsController } from '../../terminal-interaction-settings';
import { TerminalSettingsController } from '../../terminal-settings';
import type {
	TerminalDefaultSettingsClientSnapshot,
	TerminalDefaultSettingsControl,
} from '../../terminal-default-settings-control';
import { colorThemesWithFixtures, fixtureDarkTheme } from '../../__tests__/color-theme-fixtures';
import { memoryUserSettings, type MemoryUserSettings } from '../../__tests__/memoryUserSettings';

interface PreferenceStorage {
	getItem(key: string): string | null;
	setItem(key: string, value: string): void;
}

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

	function trackDisposable<DisposableType extends { dispose(): void }>(
		disposable: DisposableType,
	): DisposableType {
		testResources.disposables.push(disposable);
		return disposable;
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
		for (const container of testResources.containers.splice(0)) container.remove();
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

	interface HarnessOptions {
		readonly controller?: TerminalSettingsController;
		readonly platform?: 'apple' | 'standard';
		readonly props?: Partial<TerminalSettingsSurfaceProps>;
		readonly settings?: MemoryUserSettings;
	}

	async function renderSurface(options: HarnessOptions = {}) {
		const container = document.createElement('div');
		testResources.containers.push(container);
		document.body.append(container);
		const storage = memoryStorage();
		const controller =
			options.controller ??
			trackDisposable(
				new TerminalSettingsController({
					themeTarget: document.documentElement,
					userSettings: options.settings ?? memoryUserSettings(),
				}),
			);
		const platform = options.platform ?? 'apple';
		const keyboardShortcutController = trackDisposable(
			new KeyboardShortcutSettingsController({ platform, storage }),
		);
		const close = vi.fn<() => void>();
		controller.open();
		const root = createRoot(container);
		testResources.roots.push(root);
		await act(async () => {
			root.render(
				createElement(
					OrchbeeI18nProvider,
					{ locale: 'en-US' },
					createElement(TerminalSettingsSurface, {
						applicationLocaleController: trackDisposable(createApplicationLocaleController()),
						controller,
						keyboardShortcutController,
						keyboardShortcutPlatform: platform,
						onRequestClose: close,
						...options.props,
					}),
				),
			);
		});
		const openCategory = async (category: string): Promise<void> => {
			await act(async () =>
				requiredElement<HTMLButtonElement>(
					container.querySelector(`button[data-settings-category='${category}']`),
				).click(),
			);
		};
		return { close, container, controller, keyboardShortcutController, openCategory };
	}

	it('groups search matches under their category, then Escape clears search before leaving', async () => {
		const { close, container } = await renderSurface();
		const search = requiredElement<HTMLInputElement>(
			container.querySelector("input[aria-label='Search settings']"),
		);
		expect(document.activeElement).toBe(search);

		await changeInput(search, 'font size');
		expect(container.querySelector('[data-settings-content="search"]')).not.toBeNull();
		const appearance = requiredElement(container.querySelector("section[aria-label='Appearance']"));
		expect(appearance.querySelector('[data-setting-id="appearance.font-size"]')).not.toBeNull();
		expect(container.querySelector('[data-setting-id="general.language"]')).toBeNull();

		await changeInput(search, 'no such setting anywhere');
		expect(
			container.querySelector('[data-settings-content="search"] [data-settings-row]'),
		).toBeNull();
		expect(container.textContent).toContain('No matching settings.');

		await act(async () => {
			search.dispatchEvent(
				new KeyboardEvent('keydown', { bubbles: true, cancelable: true, key: 'Escape' }),
			);
		});
		expect(search.value).toBe('');
		expect(close).not.toHaveBeenCalled();
		expect(container.querySelector('[data-settings-content="general"]')).not.toBeNull();

		await act(async () => {
			search.dispatchEvent(
				new KeyboardEvent('keydown', { bubbles: true, cancelable: true, key: 'Escape' }),
			);
		});
		expect(close).toHaveBeenCalledOnce();
	});

	it('previews a theme while moving through a list, restores it on Escape, and commits on Enter', async () => {
		const themeTarget = document.createElement('div');
		const userSettings = memoryUserSettings({ values: { 'appearance.mode': 'light' } });
		const controller = trackDisposable(
			new TerminalSettingsController({
				colorThemes: colorThemesWithFixtures,
				themeTarget,
				userSettings,
			}),
		);
		const { close, container, openCategory } = await renderSurface({ controller });
		await openCategory('themes');
		const darkThemes = requiredElement<HTMLElement>(
			container.querySelector("[role='listbox'][aria-label='Dark theme']"),
		);
		const press = async (key: string): Promise<void> => {
			await act(async () => {
				darkThemes.dispatchEvent(new KeyboardEvent('keydown', { bubbles: true, key }));
			});
		};
		const savedValues = () => userSettings.getSnapshot().snapshot?.values;
		await act(async () => darkThemes.focus());

		await press('ArrowDown');
		expect({ ...themeTarget.dataset }).toEqual({ colorTheme: fixtureDarkTheme.id, theme: 'dark' });
		expect(savedValues()).toMatchObject({
			'appearance.dark_theme': 'orchbee-dark',
			'appearance.mode': 'light',
		});

		await press('Escape');
		expect({ ...themeTarget.dataset }).toEqual({ theme: 'light' });
		expect(close).not.toHaveBeenCalled();

		await press('ArrowDown');
		await press('Enter');
		await vi.waitFor(() =>
			expect(savedValues()).toMatchObject({
				'appearance.dark_theme': fixtureDarkTheme.id,
				'appearance.mode': 'dark',
			}),
		);
		expect(
			darkThemes
				.querySelector("[role='option'][aria-selected='true']")
				?.id.endsWith(fixtureDarkTheme.id),
		).toBe(true);
	});

	it('switches the mounted Settings copy and persists an explicit display language', async () => {
		const container = document.createElement('div');
		testResources.containers.push(container);
		document.body.append(container);
		const storage = memoryStorage();
		const userSettings = memoryUserSettings();
		const edit = vi.spyOn(userSettings, 'edit');
		const applicationLocaleController = trackDisposable(
			new ApplicationLocaleController({
				documentElement: document.documentElement,
				preferredLocales: ['zh-CN'],
				userSettings,
			}),
		);
		const controller = trackDisposable(
			new TerminalSettingsController({
				themeTarget: document.documentElement,
				userSettings,
			}),
		);
		controller.open();
		const root = createRoot(container);
		testResources.roots.push(root);
		await act(async () => {
			root.render(
				createElement(
					ApplicationLocaleProvider,
					{ source: applicationLocaleController },
					createElement(TerminalSettingsSurface, {
						applicationLocaleController,
						controller,
						keyboardShortcutController: trackDisposable(
							new KeyboardShortcutSettingsController({ platform: 'apple', storage }),
						),
						keyboardShortcutPlatform: 'apple',
						onRequestClose: () => undefined,
					}),
				),
			);
		});

		const chineseSelect = requiredElement<HTMLSelectElement>(
			container.querySelector("select[aria-label='语言']"),
		);
		expect(chineseSelect.value).toBe('system');
		expect(edit).not.toHaveBeenCalled();

		await act(async () => {
			setSelectValue(chineseSelect, 'en-US');
			chineseSelect.dispatchEvent(new Event('change', { bubbles: true }));
		});

		expect(document.documentElement.lang).toBe('en-US');
		expect(container.querySelector("select[aria-label='Language']")).not.toBeNull();
		expect(userSettings.getSnapshot().snapshot?.values['general.language']).toBe('en-US');
	});

	it('applies copy on select live', async () => {
		const controller = trackDisposable(
			new TerminalSettingsController({
				themeTarget: document.documentElement,
				userSettings: memoryUserSettings({ values: { 'terminal.copy_on_select': true } }),
			}),
		);
		const interactionController = trackDisposable(
			new TerminalInteractionSettingsController(controller.userSettings),
		);
		const listener = vi.fn<(configuration: TerminalInteractionConfiguration) => void>();
		interactionController.subscribe(listener);
		const { container, openCategory } = await renderSurface({
			controller,
			props: { terminalInteractionSettingsController: interactionController },
		});
		await openCategory('terminal');
		await act(async () =>
			requiredElement<HTMLElement>(
				container.querySelector("[aria-label='Copy on select']"),
			).click(),
		);
		expect(interactionController.current()).toEqual({ copyOnSelect: false });
		expect(listener).toHaveBeenCalledWith({ copyOnSelect: false });
	});

	it('saves terminal history in megabytes and rejects values outside the range', async () => {
		const { container, controller, openCategory } = await renderSurface({
			props: { terminalDefaultSettingsControl: createTerminalDefaultSettingsControl() },
		});
		await openCategory('terminal');
		const history = requiredElement<HTMLInputElement>(
			container.querySelector("input[aria-label='Terminal history']"),
		);
		expect(history.value).toBe('10');

		await commitInput(history, '1001');
		expect(history.getAttribute('aria-invalid')).toBe('true');
		expect(
			controller.userSettings.getSnapshot().snapshot?.values['terminal.scrollback_bytes'],
		).toBe(10_000_000);

		await commitInput(history, '25');
		expect(
			controller.userSettings.getSnapshot().snapshot?.values['terminal.scrollback_bytes'],
		).toBe(25_000_000);
	});

	it('shows Option as Alt only with its controller and applies it live', async () => {
		const settings = memoryUserSettings();
		const macosController = trackDisposable(new MacosTerminalInputSettingsController(settings));
		const listener = vi.fn<(configuration: TerminalInputConfiguration) => void>();
		macosController.subscribe(listener);
		const { container, openCategory } = await renderSurface({
			props: { macosTerminalInputSettingsController: macosController },
			settings,
		});
		await openCategory('terminal');
		const optionAsAlt = requiredElement<HTMLSelectElement>(
			container.querySelector("select[aria-label='Use Option as Alt']"),
		);
		expect(optionAsAlt.value).toBe('off');
		await act(async () => {
			setSelectValue(optionAsAlt, 'right');
			optionAsAlt.dispatchEvent(new Event('change', { bubbles: true }));
		});
		expect(macosController.current()).toEqual({ optionAsAlt: 'right' });
		expect(listener).toHaveBeenCalledWith({ optionAsAlt: 'right' });
	});

	it('keeps an invalid font size as a draft without applying it', async () => {
		const { container, controller, openCategory } = await renderSurface();
		await openCategory('appearance');
		const fontSize = requiredElement<HTMLInputElement>(
			container.querySelector("input[aria-label='Font size']"),
		);
		await commitInput(fontSize, '49');
		expect(fontSize.getAttribute('aria-invalid')).toBe('true');
		expect(controller.getSnapshot().preferences.fontSize).toBe(13);
		expect(controller.getSnapshot().open).toBe(true);
	});

	it('puts a refused change back on the file value and says why', async () => {
		const settings = memoryUserSettings();
		settings.refuseEdits({ outcome: 'refused', reason: 'read-only' });
		const { container, openCategory } = await renderSurface({ settings });
		await openCategory('appearance');
		const fontSize = requiredElement<HTMLInputElement>(
			container.querySelector("input[aria-label='Font size']"),
		);
		await commitInput(fontSize, '15');
		await vi.waitFor(() => expect(fontSize.value).toBe('13'));
		const row = requiredElement(
			container.querySelector('[data-setting-id="appearance.font-size"]'),
		);
		expect(row.querySelector('[role="alert"]')?.textContent).toBe(
			'The settings file is read-only · make it writable and try again',
		);
		expect(settings.getSnapshot().snapshot?.values['terminal.font_size']).toBe(13);
	});

	it('shows the last valid values read-only while the file has a syntax error', async () => {
		const settings = memoryUserSettings({
			file: { column: 3, line: 7, reason: 'invalid-toml', status: 'syntax-error' },
			filePath: '/home/person/.orchbee/config.toml',
			values: { 'terminal.font_size': 17 },
		});
		const { container, openCategory } = await renderSurface({ settings });
		expect(container.querySelector('[data-slot="settings-banner"]')?.textContent).toContain(
			'/home/person/.orchbee/config.toml has a syntax error at line 7, column 3',
		);
		await openCategory('appearance');
		const fontSize = requiredElement<HTMLInputElement>(
			container.querySelector("input[aria-label='Font size']"),
		);
		expect(fontSize.value).toBe('17');
		expect(fontSize.readOnly).toBe(true);
		expect(container.querySelector("button[aria-label='Restore default: Font size']")).toBeNull();
		const theme = requiredElement<HTMLSelectElement>(
			container.querySelector("select[aria-label='Application theme']"),
		);
		expect(theme.disabled).toBe(true);
	});

	it('records a shortcut, keeps Settings open on a recorder Escape, and restores the default', async () => {
		const { close, container, controller, keyboardShortcutController, openCategory } =
			await renderSurface({ platform: 'standard' });
		await openCategory('keyboard');
		const recorder = requiredElement<HTMLButtonElement>(
			container.querySelector("button[aria-label='Change shortcut for Search everything']"),
		);
		const pressOnRecorder = async (init: KeyboardEventInit): Promise<void> => {
			await act(async () => {
				recorder.dispatchEvent(
					new KeyboardEvent('keydown', { bubbles: true, cancelable: true, ...init }),
				);
			});
		};
		await act(async () => recorder.click());
		await pressOnRecorder({ code: 'KeyY', ctrlKey: true, key: 'Y', shiftKey: true });
		expect(keyboardShortcutController.current()['workbench.search.all.open']).toEqual({
			code: 'KeyY',
			modifiers: ['mod', 'shift'],
		});

		await act(async () => recorder.click());
		await pressOnRecorder({ code: 'Escape', key: 'Escape' });
		expect(controller.getSnapshot().open).toBe(true);
		expect(close).not.toHaveBeenCalled();

		await act(async () =>
			requiredElement<HTMLButtonElement>(
				container.querySelector("button[aria-label='Restore default: Search everything']"),
			).click(),
		);
		expect(keyboardShortcutController.current()['workbench.search.all.open']).toEqual({
			code: 'KeyK',
			modifiers: ['mod', 'shift'],
		});
	});
});

function createApplicationLocaleController(): ApplicationLocaleController {
	return new ApplicationLocaleController({
		documentElement: document.documentElement,
		preferredLocales: ['en-US'],
		userSettings: memoryUserSettings(),
	});
}

function memoryStorage(): PreferenceStorage {
	return { getItem: () => null, setItem: () => undefined };
}

function createTerminalDefaultSettingsControl(): TerminalDefaultSettingsControl {
	const snapshot: TerminalDefaultSettingsClientSnapshot = {
		configuration: { scrollbackBytes: 10_000_000 },
		failureCode: null,
	};
	return { getSnapshot: () => snapshot, subscribe: () => () => undefined };
}

function requiredElement<ElementType extends Element>(
	element: ElementType | null | undefined,
): ElementType {
	if (element === null || element === undefined) {
		throw new Error('Expected element was not rendered.');
	}
	return element;
}

async function changeInput(input: HTMLInputElement, value: string): Promise<void> {
	await act(async () => {
		setInputValue(input, value);
		input.dispatchEvent(new Event('input', { bubbles: true }));
	});
}

async function commitInput(input: HTMLInputElement, value: string): Promise<void> {
	await act(async () => {
		input.focus();
		setInputValue(input, value);
		input.dispatchEvent(new Event('input', { bubbles: true }));
		input.blur();
	});
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
