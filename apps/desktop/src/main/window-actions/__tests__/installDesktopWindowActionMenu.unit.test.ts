import { EventEmitter } from 'node:events';

import type { ActionAvailability } from '@orchbee/actions';
import type { KeyboardEvent, Menu, MenuItem, MenuItemConstructorOptions } from 'electron';
import { describe, expect, it, vi } from 'vitest';

import {
	installDesktopWindowActionMenu,
	type DesktopWindowActionMenuTarget,
} from '../installDesktopWindowActionMenu';

const DESKTOP_WINDOW_MENU_ITEM_IDS = {
	toggleFullScreen: 'desktop-window-toggle-full-screen',
	toggleMaximized: 'desktop-window-toggle-maximized',
} as const;

describe('installDesktopWindowActionMenu', () => {
	it('preserves native menu items while replacing window roles with semantic actions', () => {
		const originalViewSubmenu = fakeMenu([
			fakeMenuItem({ role: 'reload' }),
			fakeMenuItem({ role: 'togglefullscreen' }),
		]);
		const originalWindowSubmenu = fakeMenu([
			fakeMenuItem({ role: 'minimize' }),
			fakeMenuItem({ role: 'zoom' }),
		]);
		const viewRoot = fakeMenuItem({ role: 'viewMenu', submenu: originalViewSubmenu });
		const windowRoot = fakeMenuItem({ role: 'windowMenu', submenu: originalWindowSubmenu });
		const previousMenu = fakeMenu([viewRoot, windowRoot]);
		const actions = createActions();
		const menuApi = createMenuApi(previousMenu);

		const installation = installDesktopWindowActionMenu(
			actions,
			{
				category: 'Window',
				toggleFullScreen: 'Enter / Exit Full Screen',
				toggleMaximized: 'Maximize / Restore Window',
			},
			'darwin',
			menuApi,
		);

		const installedMenu = menuApi.setApplicationMenu.mock.calls[0]?.[0];
		const installedViewRoot = installedMenu?.items.find((item) =>
			Boolean(item.submenu?.getMenuItemById(DESKTOP_WINDOW_MENU_ITEM_IDS.toggleFullScreen)),
		);
		const installedWindowRoot = installedMenu?.items.find((item) =>
			Boolean(item.submenu?.getMenuItemById(DESKTOP_WINDOW_MENU_ITEM_IDS.toggleMaximized)),
		);
		expect(installedViewRoot?.submenu?.items.map((item) => item.role ?? item.id)).toEqual([
			'reload',
			DESKTOP_WINDOW_MENU_ITEM_IDS.toggleFullScreen,
		]);
		expect(installedWindowRoot?.submenu?.items.map((item) => item.role ?? item.id)).toEqual([
			'minimize',
			DESKTOP_WINDOW_MENU_ITEM_IDS.toggleMaximized,
		]);
		const fullScreenItem = installedMenu?.getMenuItemById(
			DESKTOP_WINDOW_MENU_ITEM_IDS.toggleFullScreen,
		);
		const maximizedItem = installedMenu?.getMenuItemById(
			DESKTOP_WINDOW_MENU_ITEM_IDS.toggleMaximized,
		);
		expect(fullScreenItem?.accelerator).toBe('Control+Command+F');
		expect(maximizedItem?.accelerator).toBe('CommandOrControl+Shift+M');
		fullScreenItem?.click({ triggeredByAccelerator: true }, undefined, undefined);
		maximizedItem?.click({ triggeredByAccelerator: false }, undefined, undefined);
		expect(actions.invokeToggleFullScreen).toHaveBeenCalledWith('keyboard');
		expect(actions.invokeToggleMaximized).toHaveBeenCalledWith('menu');

		actions.getToggleFullScreenAvailability.mockReturnValue({
			checked: true,
			kind: 'enabled',
		});
		actions.getToggleMaximizedAvailability.mockReturnValue({
			kind: 'disabled',
			reason: { id: 'full-screen' },
		});
		installedViewRoot?.submenu?.emit('menu-will-show', {});
		expect(fullScreenItem).toMatchObject({ checked: true, enabled: true });
		expect(maximizedItem).toMatchObject({ checked: false, enabled: false });

		installation.dispose();
		expect(viewRoot.submenu).toBe(originalViewSubmenu);
		expect(windowRoot.submenu).toBe(originalWindowSubmenu);
		expect(menuApi.setApplicationMenu).toHaveBeenLastCalledWith(previousMenu);
	});

	it('creates platform-native role menus when Electron has not installed one yet', () => {
		const actions = createActions();
		const menuApi = createMenuApi(null);

		installDesktopWindowActionMenu(
			actions,
			{
				category: '窗口',
				toggleFullScreen: '进入 / 退出全屏',
				toggleMaximized: '最大化 / 还原窗口',
			},
			'linux',
			menuApi,
		);

		const firstTemplate = menuApi.buildFromTemplate.mock.calls[0]?.[0];
		expect(firstTemplate).toEqual([
			{ role: 'fileMenu' },
			{ role: 'editMenu' },
			{ role: 'viewMenu' },
			{ role: 'windowMenu' },
		]);
		const installedMenu = menuApi.setApplicationMenu.mock.calls[0]?.[0];
		expect(
			installedMenu?.getMenuItemById(DESKTOP_WINDOW_MENU_ITEM_IDS.toggleFullScreen)?.accelerator,
		).toBe('F11');
	});
});

function createActions() {
	return {
		getToggleFullScreenAvailability: vi
			.fn<DesktopWindowActionMenuTarget['getToggleFullScreenAvailability']>()
			.mockReturnValue(enabledAvailability()),
		getToggleMaximizedAvailability: vi
			.fn<DesktopWindowActionMenuTarget['getToggleMaximizedAvailability']>()
			.mockReturnValue(enabledAvailability()),
		invokeToggleFullScreen: vi
			.fn<DesktopWindowActionMenuTarget['invokeToggleFullScreen']>()
			.mockReturnValue(true),
		invokeToggleMaximized: vi
			.fn<DesktopWindowActionMenuTarget['invokeToggleMaximized']>()
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
	} as unknown as MenuItem;
	return item;
}

function isFakeMenuItem(item: MenuItemConstructorOptions | MenuItem): item is MenuItem {
	return '__fakeMenuItem' in item;
}
