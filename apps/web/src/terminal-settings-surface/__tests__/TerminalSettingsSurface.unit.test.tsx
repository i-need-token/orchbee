// @vitest-environment jsdom

import { OrchbeeI18nProvider, type OrchbeeLocale } from '@orchbee/i18n';
import { TooltipProvider } from '@orchbee/ui/components';
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
import { colorThemesWithFixtures } from '../../__tests__/color-theme-fixtures';
import { memoryUserSettings, type MemoryUserSettings } from '../../__tests__/memoryUserSettings';
import { chooseSelectOption, selectedText, selectTrigger } from '../../__tests__/select-choice';

/** The registry's page mark for `update.automatic` as a test sets it; null keeps the real mark. */
const automaticUpdatesPage = vi.hoisted((): { listed: boolean | null } => ({ listed: null }));

vi.mock(import('@orchbee/workbench-client-protocol'), async (importOriginal) => {
	const actual = await importOriginal();
	return {
		...actual,
		isSettingOnPage: (key) =>
			key === 'update.automatic' && automaticUpdatesPage.listed !== null
				? automaticUpdatesPage.listed
				: actual.isSettingOnPage(key),
	};
});

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
		automaticUpdatesPage.listed = null;
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
		readonly locale?: OrchbeeLocale;
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
					TooltipProvider,
					null,
					createElement(
						OrchbeeI18nProvider,
						{ locale: options.locale ?? 'en-US' },
						createElement(TerminalSettingsSurface, {
							applicationLocaleController: trackDisposable(createApplicationLocaleController()),
							controller,
							keyboardShortcutController,
							keyboardShortcutPlatform: platform,
							onRequestClose: close,
							...options.props,
						}),
					),
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

		const chineseSelect = selectTrigger(container, '语言');
		expect(selectedText(chineseSelect)).toBe('跟随系统');
		expect(edit).not.toHaveBeenCalled();

		await chooseSelectOption(chineseSelect, 'English');

		expect(document.documentElement.lang).toBe('en-US');
		expect(selectTrigger(container, 'Language')).not.toBeNull();
		expect(userSettings.getSnapshot().snapshot?.values['general.language']).toBe('en-US');
	});

	const automaticUpdatesCopy = {
		'en-US': ['Automatic updates', 'Download and install new versions in the background.'],
		'zh-CN': ['自动更新', '在后台下载并安装新版本。'],
	} as const;

	it('keeps Automatic updates off General while the registry does not list it', async () => {
		for (const locale of ['en-US', 'zh-CN'] as const) {
			automaticUpdatesPage.listed = false;
			const { container } = await renderSurface({ locale });
			const general = requiredElement(container.querySelector('[data-settings-content="general"]'));
			expect(general.querySelector('[data-setting-id="general.automatic-updates"]')).toBeNull();
			expect(general.textContent).not.toContain(automaticUpdatesCopy[locale][0]);
		}
	});

	it('shows Automatic updates under General once the registry lists it, and applies it', async () => {
		for (const locale of ['en-US', 'zh-CN'] as const) {
			automaticUpdatesPage.listed = true;
			const settings = memoryUserSettings();
			const { container } = await renderSurface({ locale, settings });
			const [label, description] = automaticUpdatesCopy[locale];
			const row = requiredElement(
				container.querySelector(
					'[data-settings-content="general"] [data-setting-id="general.automatic-updates"]',
				),
			);
			expect(row.textContent).toContain(description);
			const toggle = requiredElement<HTMLElement>(row.querySelector(`[aria-label='${label}']`));
			expect(toggle.getAttribute('aria-checked')).toBe('true');
			await act(async () => toggle.click());
			expect(settings.getSnapshot().snapshot?.values['update.automatic']).toBe(false);
		}
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

	it('writes each new terminal folder entry and shows a stored value as its entry', async () => {
		const folderRow = async (
			stored: string | undefined,
			folderPicker?: TerminalSettingsSurfaceProps['folderPicker'],
		) => {
			const settings = memoryUserSettings({
				values: stored === undefined ? {} : { 'terminal.new_cwd': stored },
			});
			const { container, openCategory } = await renderSurface({
				props: {
					folderPicker,
					terminalDefaultSettingsControl: createTerminalDefaultSettingsControl(),
				},
				settings,
			});
			await openCategory('terminal');
			const choice = selectTrigger(container, 'New terminal folder');
			const field = (): HTMLInputElement | null =>
				container.querySelector("input[aria-label='Custom folder path']");
			const choose = async (entry: keyof typeof FOLDER_ENTRY_LABELS): Promise<void> => {
				await chooseSelectOption(choice, FOLDER_ENTRY_LABELS[entry]);
			};
			const file = (): { set: boolean; value: unknown } => {
				const snapshot = settings.getSnapshot().snapshot;
				return {
					set: snapshot?.set.includes('terminal.new_cwd') ?? false,
					value: snapshot?.values['terminal.new_cwd'],
				};
			};
			return { choice, choose, field, file };
		};

		for (const [stored, entry, folder] of [
			[undefined, 'follow', null],
			['', 'follow', null],
			['follow', 'follow', null],
			['home', 'home', null],
			['workspace', 'workspace', null],
			['current', 'custom', 'current'],
			['~/项目', 'custom', '~/项目'],
		] as const) {
			const row = await folderRow(stored);
			expect([stored, selectedText(row.choice), row.field()?.value ?? null]).toEqual([
				stored,
				FOLDER_ENTRY_LABELS[entry],
				folder,
			]);
		}

		for (const entry of ['workspace', 'home', 'follow'] as const) {
			const row = await folderRow('C:\\old');
			await row.choose(entry);
			expect(row.file()).toEqual({ set: true, value: entry });
		}
		const unset = await folderRow(undefined);
		await unset.choose('follow');
		expect(unset.file().set).toBe(false);

		const picked = await folderRow(undefined, {
			selectExistingFolder: async () => ({ kind: 'selected', path: 'D:\\picked' }),
		});
		await picked.choose('custom');
		expect(picked.file()).toEqual({ set: true, value: 'D:\\picked' });
		expect(picked.field()?.value).toBe('D:\\picked');

		const typed = await folderRow(undefined);
		await typed.choose('custom');
		await commitInput(requiredElement(typed.field()), 'D:\\typed');
		expect(typed.file()).toEqual({ set: true, value: 'D:\\typed' });
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
		const optionAsAlt = selectTrigger(container, 'Use Option as Alt');
		expect(selectedText(optionAsAlt)).toBe('Off');
		await chooseSelectOption(optionAsAlt, 'Right Option');
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
		expect(container.querySelector('[data-slot="settings-notice"]')?.textContent).toBe(
			"Couldn't save the change: the settings file is read-only.",
		);
		expect(settings.getSnapshot().snapshot?.values['terminal.font_size']).toBe(13);
	});

	it('says a file that cannot be read cannot be read, not that it is read-only', async () => {
		const settings = memoryUserSettings();
		settings.refuseEdits({ detail: 'not-file', outcome: 'refused', reason: 'unusable-file' });
		const { container, openCategory } = await renderSurface({ settings });
		await openCategory('appearance');
		const fontSize = requiredElement<HTMLInputElement>(
			container.querySelector("input[aria-label='Font size']"),
		);
		await commitInput(fontSize, '15');
		await vi.waitFor(() =>
			expect(container.querySelector('[data-slot="settings-notice"]')?.textContent).toBe(
				"Couldn't save the change: the settings file can't be read.",
			),
		);
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
		const theme = selectTrigger(container, 'Application theme');
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

	it('uses shared tooltips without native titles on Settings banners, indicators, and controls', async () => {
		const baseSettings = memoryUserSettings({
			values: { 'appearance.mode': 'light', 'terminal.font_size': 16 },
		});
		const unavailableSnapshot = { ...baseSettings.getSnapshot(), available: false };
		const userSettings: MemoryUserSettings = {
			...baseSettings,
			getSnapshot: () => unavailableSnapshot,
		};
		const controller = trackDisposable(
			new TerminalSettingsController({
				colorThemes: colorThemesWithFixtures,
				themeTarget: document.createElement('div'),
				userSettings,
			}),
		);
		const { container, openCategory } = await renderSurface({ controller });

		expect(container.querySelectorAll('[title]')).toHaveLength(0);

		const dismissButton = requiredElement<HTMLButtonElement>(
			container.querySelector("button[aria-label='Dismiss']"),
		);
		await act(async () => dismissButton.focus());
		await vi.waitFor(() => {
			expect(document.querySelector('[data-slot="tooltip-content"]')?.textContent).toBe('Dismiss');
		});
		await act(async () => dismissButton.blur());

		await openCategory('appearance');
		expect(container.querySelectorAll('[title]')).toHaveLength(0);

		const restoreButton = requiredElement<HTMLButtonElement>(
			container.querySelector("button[aria-label^='Restore default:']"),
		);
		await act(async () => restoreButton.focus());
		await vi.waitFor(() => {
			expect(document.querySelector('[data-slot="tooltip-content"]')?.textContent).toBe(
				'Restore default',
			);
		});
		await act(async () => restoreButton.blur());

		await openCategory('themes');
		expect(container.querySelectorAll('[title]')).toHaveLength(0);
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

/** The New terminal folder entries' English names. */
const FOLDER_ENTRY_LABELS = {
	custom: 'Custom folder…',
	follow: 'Same as the current terminal',
	home: 'Home folder',
	workspace: 'Workspace folder',
} as const;
