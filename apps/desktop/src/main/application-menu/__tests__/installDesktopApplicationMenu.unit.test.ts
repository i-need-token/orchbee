import { EventEmitter } from 'node:events';

import type { ActionAvailability } from '@orchbee/actions';
import { createOrchbeeIntl } from '@orchbee/i18n';
import type { KeyboardEvent, Menu, MenuItem, MenuItemConstructorOptions } from 'electron';
import { describe, expect, it, vi } from 'vitest';

import {
	installDesktopApplicationMenu,
	type DesktopApplicationMenuTarget,
} from '../installDesktopApplicationMenu';

const ITEM_IDS = {
	closeWindow: 'desktop-window-close',
	forceReload: 'desktop-view-force-reload',
	reload: 'desktop-view-reload',
	secureKeyboardEntry: 'desktop-secure-keyboard-entry',
	toggleFullScreen: 'desktop-window-toggle-full-screen',
	toggleMaximized: 'desktop-window-toggle-maximized',
} as const;

const englishText = { applicationName: 'Orchbee Test', intl: createOrchbeeIntl('en-US') };

describe('installDesktopApplicationMenu', () => {
	it('labels every menu and item in the interface language', () => {
		for (const platform of ['darwin', 'win32'] as const) {
			const menuApi = createMenuApi(null);
			installDesktopApplicationMenu(
				{
					...createActions(),
					background: {
						getQuitAllAvailability: () => enabledAvailability(),
						getReopenAvailability: () => enabledAvailability(),
						invokeQuitAll: vi.fn<() => void>(),
						invokeReopen: vi.fn<() => void>(),
					},
					saveDiagnosticBundle: {
						getAvailability: () => enabledAvailability(),
						invoke: vi.fn<() => boolean>(() => true),
					},
				},
				{ applicationName: 'Orchbee Test', intl: createOrchbeeIntl('zh-CN') },
				platform,
				menuApi,
			);

			const roots = menuApi.setApplicationMenu.mock.calls[0]?.[0]?.items ?? [];
			// The macOS application menu is titled with the application's name.
			const [applicationMenu, ...otherRoots] = roots;
			const labels =
				platform === 'darwin'
					? [...menuLabels(applicationMenu?.submenu?.items ?? []), ...menuLabels(otherRoots)]
					: menuLabels(roots);
			expect(labels.length).toBeGreaterThan(roots.length);
			expect(labels.filter((label) => !/\p{Script=Han}/u.test(label))).toEqual([]);
		}
	});

	it('composes checked window and macOS security actions and restores the previous menu', () => {
		const previousMenu = fakeMenu([]);
		const actions = createActions();
		const menuApi = createMenuApi(previousMenu);

		const installation = installDesktopApplicationMenu(actions, englishText, 'darwin', menuApi);
		const installedMenu = menuApi.setApplicationMenu.mock.calls[0]?.[0];
		const secureItem = installedMenu?.getMenuItemById(ITEM_IDS.secureKeyboardEntry);
		const fullScreenItem = installedMenu?.getMenuItemById(ITEM_IDS.toggleFullScreen);
		const maximizedItem = installedMenu?.getMenuItemById(ITEM_IDS.toggleMaximized);
		expect(fullScreenItem?.accelerator).toBe('Control+Command+F');
		// Workbench resolves these keys first; the native items stay clickable.
		for (const [id, role] of [
			[ITEM_IDS.reload, 'reload'],
			[ITEM_IDS.forceReload, 'forceReload'],
			[ITEM_IDS.closeWindow, 'close'],
		] as const) {
			expect(installedMenu?.getMenuItemById(id)).toMatchObject({
				registerAccelerator: false,
				role,
			});
		}
		secureItem?.click({ triggeredByAccelerator: false }, undefined, undefined);
		fullScreenItem?.click({ triggeredByAccelerator: true }, undefined, undefined);
		maximizedItem?.click({ triggeredByAccelerator: false }, undefined, undefined);
		expect(actions.invokeToggleSecureKeyboardEntry).toHaveBeenCalledWith('menu');
		expect(actions.invokeToggleFullScreen).toHaveBeenCalledWith('keyboard');
		expect(actions.invokeToggleMaximized).toHaveBeenCalledWith('menu');

		actions.getSecureKeyboardEntryAvailability.mockReturnValue({
			checked: true,
			kind: 'enabled',
		});
		installedMenu?.items[0]?.submenu?.emit('menu-will-show', {});
		expect(secureItem).toMatchObject({ checked: true, enabled: true, visible: true });

		installation.dispose();
		expect(menuApi.setApplicationMenu).toHaveBeenLastCalledWith(previousMenu);
	});

	it('omits Secure Keyboard Entry outside macOS', () => {
		const actions = createActions();
		const menuApi = createMenuApi(null);

		installDesktopApplicationMenu(actions, englishText, 'linux', menuApi);

		const installedMenu = menuApi.setApplicationMenu.mock.calls[0]?.[0];
		expect(installedMenu?.getMenuItemById(ITEM_IDS.secureKeyboardEntry)).toBeNull();
		expect(actions.getSecureKeyboardEntryAvailability).not.toHaveBeenCalled();
		expect(installedMenu?.getMenuItemById(ITEM_IDS.toggleFullScreen)?.accelerator).toBe('F11');
	});
});

function createActions() {
	return {
		getSecureKeyboardEntryAvailability: vi
			.fn<DesktopApplicationMenuTarget['getSecureKeyboardEntryAvailability']>()
			.mockReturnValue(enabledAvailability()),
		getToggleFullScreenAvailability: vi
			.fn<DesktopApplicationMenuTarget['getToggleFullScreenAvailability']>()
			.mockReturnValue(enabledAvailability()),
		getToggleMaximizedAvailability: vi
			.fn<DesktopApplicationMenuTarget['getToggleMaximizedAvailability']>()
			.mockReturnValue(enabledAvailability()),
		invokeToggleFullScreen: vi
			.fn<DesktopApplicationMenuTarget['invokeToggleFullScreen']>()
			.mockReturnValue(true),
		invokeToggleMaximized: vi
			.fn<DesktopApplicationMenuTarget['invokeToggleMaximized']>()
			.mockReturnValue(true),
		invokeToggleSecureKeyboardEntry: vi
			.fn<DesktopApplicationMenuTarget['invokeToggleSecureKeyboardEntry']>()
			.mockReturnValue(true),
	};
}

function createMenuApi(applicationMenu: Menu | null) {
	return {
		buildFromTemplate: vi.fn<(template: Array<MenuItemConstructorOptions | MenuItem>) => Menu>(
			(template) => {
				return fakeMenu(
					template.map((item) =>
						isFakeMenuItem(item) ? item : fakeMenuItem(item as MenuItemConstructorOptions),
					),
				);
			},
		),
		getApplicationMenu: vi.fn<() => Menu | null>(() => applicationMenu),
		setApplicationMenu: vi.fn<(menu: Menu | null) => void>(),
	};
}

function enabledAvailability(): ActionAvailability {
	return { checked: false, kind: 'enabled' };
}

/** Every label a menu shows, its submenus' included; separators show none. */
function menuLabels(items: readonly MenuItem[]): string[] {
	return items.flatMap((item) => [
		...(item.type === 'separator' ? [] : [item.label]),
		...menuLabels(item.submenu?.items ?? []),
	]);
}

function fakeMenu(items: MenuItem[]): Menu {
	const emitter = new EventEmitter();
	const menu = Object.assign(emitter, {
		getMenuItemById: (id: string): MenuItem | null => {
			for (const item of menu.items) {
				if (item.id === id) return item;
				const nested = item.submenu?.getMenuItemById(id);
				if (nested !== null && nested !== undefined) return nested;
			}
			return null;
		},
		items,
	}) as unknown as Menu;
	return menu;
}

function fakeMenuItem(options: MenuItemConstructorOptions): MenuItem {
	const submenu = Array.isArray(options.submenu)
		? fakeMenu(options.submenu.map((item) => fakeMenuItem(item)))
		: options.submenu;
	let item: MenuItem;
	item = {
		__fakeMenuItem: true,
		accelerator: options.accelerator ?? null,
		checked: options.checked ?? false,
		click: (event: KeyboardEvent) => options.click?.(item, undefined, event),
		enabled: options.enabled ?? true,
		id: options.id ?? '',
		label: options.label ?? '',
		registerAccelerator: options.registerAccelerator ?? true,
		role: options.role,
		submenu,
		type: options.type ?? 'normal',
		visible: options.visible ?? true,
	} as unknown as MenuItem;
	return item;
}

function isFakeMenuItem(item: MenuItemConstructorOptions | MenuItem): item is MenuItem {
	return '__fakeMenuItem' in item;
}
