import { EventEmitter } from 'node:events';

import type { ActionAvailability } from '@orchbee/actions';
import type { KeyboardEvent, Menu, MenuItem, MenuItemConstructorOptions } from 'electron';
import { describe, expect, it, vi } from 'vitest';

import {
	installDesktopApplicationMenu,
	type DesktopApplicationMenuTarget,
} from '../installDesktopApplicationMenu';

const ITEM_IDS = {
	secureKeyboardEntry: 'desktop-secure-keyboard-entry',
	toggleFullScreen: 'desktop-window-toggle-full-screen',
	toggleMaximized: 'desktop-window-toggle-maximized',
} as const;

const labels = {
	secureKeyboardEntry: 'Secure Keyboard Entry',
	securityCategory: 'Input Security',
	toggleFullScreen: 'Enter / Exit Full Screen',
	toggleMaximized: 'Maximize / Restore Window',
	windowCategory: 'Window',
} as const;

describe('installDesktopApplicationMenu', () => {
	it('preserves native menu items and composes checked window and macOS security actions', () => {
		const appRoot = fakeMenuItem({
			role: 'appMenu',
			submenu: fakeMenu([fakeMenuItem({ role: 'about' })]),
		});
		const viewRoot = fakeMenuItem({
			role: 'viewMenu',
			submenu: fakeMenu([
				fakeMenuItem({ role: 'reload' }),
				fakeMenuItem({ role: 'togglefullscreen' }),
			]),
		});
		const windowRoot = fakeMenuItem({
			role: 'windowMenu',
			submenu: fakeMenu([fakeMenuItem({ role: 'minimize' }), fakeMenuItem({ role: 'zoom' })]),
		});
		const previousMenu = fakeMenu([appRoot, viewRoot, windowRoot]);
		const actions = createActions();
		const menuApi = createMenuApi(previousMenu);

		const installation = installDesktopApplicationMenu(actions, labels, 'darwin', menuApi);
		const installedMenu = menuApi.setApplicationMenu.mock.calls[0]?.[0];
		const secureItem = installedMenu?.getMenuItemById(ITEM_IDS.secureKeyboardEntry);
		const fullScreenItem = installedMenu?.getMenuItemById(ITEM_IDS.toggleFullScreen);
		const maximizedItem = installedMenu?.getMenuItemById(ITEM_IDS.toggleMaximized);
		expect(secureItem?.label).toBe('Secure Keyboard Entry');
		expect(installedMenu?.items[0]?.submenu?.items.map((item) => item.role ?? item.id)).toEqual([
			'about',
			ITEM_IDS.secureKeyboardEntry,
		]);
		expect(fullScreenItem?.accelerator).toBe('Control+Command+F');
		expect(maximizedItem?.accelerator).toBe('CommandOrControl+Shift+M');
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

		installDesktopApplicationMenu(actions, labels, 'linux', menuApi);

		const firstTemplate = menuApi.buildFromTemplate.mock.calls[0]?.[0];
		expect(firstTemplate).toEqual([
			{ role: 'fileMenu' },
			{ role: 'editMenu' },
			{ role: 'viewMenu' },
			{ role: 'windowMenu' },
		]);
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
