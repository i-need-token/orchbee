import type { ActionAvailability, ActionDisposable } from '@orchbee/actions';
import { Menu, type KeyboardEvent, type MenuItem, type MenuItemConstructorOptions } from 'electron';

import type { DesktopWindowActionSource } from './DesktopWindowActions';

const DESKTOP_WINDOW_MENU_ITEM_IDS = {
	toggleFullScreen: 'desktop-window-toggle-full-screen',
	toggleMaximized: 'desktop-window-toggle-maximized',
} as const;

export interface DesktopWindowActionMenuLabels {
	readonly category: string;
	readonly toggleFullScreen: string;
	readonly toggleMaximized: string;
}

export interface DesktopWindowActionMenuTarget {
	getToggleFullScreenAvailability(): ActionAvailability;
	getToggleMaximizedAvailability(): ActionAvailability;
	invokeToggleFullScreen(source: DesktopWindowActionSource): boolean;
	invokeToggleMaximized(source: DesktopWindowActionSource): boolean;
}

interface DesktopWindowActionMenuApi {
	buildFromTemplate(template: Array<MenuItemConstructorOptions | MenuItem>): Menu;
	getApplicationMenu(): Menu | null;
	setApplicationMenu(menu: Menu | null): void;
}

const defaultMenuApi: DesktopWindowActionMenuApi = {
	buildFromTemplate: (template) => Menu.buildFromTemplate(template),
	getApplicationMenu: () => Menu.getApplicationMenu(),
	setApplicationMenu: (menu) => Menu.setApplicationMenu(menu),
};

export function installDesktopWindowActionMenu(
	actions: DesktopWindowActionMenuTarget,
	labels: DesktopWindowActionMenuLabels,
	platform: NodeJS.Platform,
	menuApi: DesktopWindowActionMenuApi = defaultMenuApi,
): ActionDisposable {
	const previousMenu = menuApi.getApplicationMenu();
	const baseMenu =
		previousMenu ?? menuApi.buildFromTemplate(defaultApplicationMenuTemplate(platform));
	const viewRoot = findRole(baseMenu, 'viewMenu');
	const windowRoot = findRole(baseMenu, 'windowMenu');
	const fullScreenItem: MenuItemConstructorOptions & { readonly id: string } = {
		accelerator: platform === 'darwin' ? 'Control+Command+F' : 'F11',
		click: (_menuItem, _window, event) => {
			actions.invokeToggleFullScreen(invocationSource(event));
		},
		id: DESKTOP_WINDOW_MENU_ITEM_IDS.toggleFullScreen,
		label: labels.toggleFullScreen,
		type: 'checkbox',
	};
	const maximizedItem: MenuItemConstructorOptions & { readonly id: string } = {
		accelerator: 'CommandOrControl+Shift+M',
		click: (_menuItem, _window, event) => {
			actions.invokeToggleMaximized(invocationSource(event));
		},
		id: DESKTOP_WINDOW_MENU_ITEM_IDS.toggleMaximized,
		label: labels.toggleMaximized,
		type: 'checkbox',
	};
	const replacementRootIds: string[] = [];
	let template: Array<MenuItemConstructorOptions | MenuItem>;
	if (viewRoot === undefined && windowRoot === undefined) {
		const replacementRootId = 'desktop-window-menu';
		replacementRootIds.push(replacementRootId);
		template = [
			...baseMenu.items,
			{
				id: replacementRootId,
				label: labels.category,
				submenu: [fullScreenItem, maximizedItem],
			},
		];
	} else {
		const fullScreenRoot = viewRoot ?? windowRoot;
		const maximizedRoot = windowRoot ?? viewRoot;
		if (fullScreenRoot === undefined || maximizedRoot === undefined) {
			throw new Error('Desktop window actions require a native application menu root.');
		}
		const rootPlans = new Map<MenuItem, RootPlan>();
		planRoot(rootPlans, fullScreenRoot, 'togglefullscreen', fullScreenItem);
		planRoot(rootPlans, maximizedRoot, 'zoom', maximizedItem);
		let replacementOrdinal = 0;
		template = baseMenu.items.map((item) => {
			const plan = rootPlans.get(item);
			if (plan === undefined) return item;
			const replacementRootId = `desktop-window-action-root-${replacementOrdinal++}`;
			replacementRootIds.push(replacementRootId);
			const submenu = menuApi.buildFromTemplate([
				...(item.submenu?.items ?? []).filter(
					(candidate) =>
						!plan.replacedRoles.has(candidate.role) && !plan.replacementIds.has(candidate.id),
				),
				...plan.items,
			]);
			return {
				enabled: item.enabled,
				id: replacementRootId,
				label: item.label,
				submenu,
				type: 'submenu',
				visible: item.visible,
			};
		});
	}
	const menu = menuApi.buildFromTemplate(template);
	const observedSubmenus = replacementRootIds.flatMap((id) => {
		const submenu = menu.getMenuItemById(id)?.submenu;
		return submenu === undefined ? [] : [submenu];
	});

	const sync = (): void => {
		projectAvailability(
			menu.getMenuItemById(DESKTOP_WINDOW_MENU_ITEM_IDS.toggleFullScreen),
			actions.getToggleFullScreenAvailability(),
		);
		projectAvailability(
			menu.getMenuItemById(DESKTOP_WINDOW_MENU_ITEM_IDS.toggleMaximized),
			actions.getToggleMaximizedAvailability(),
		);
	};
	for (const submenu of observedSubmenus) submenu.on('menu-will-show', sync);
	sync();
	menuApi.setApplicationMenu(menu);

	let active = true;
	return {
		dispose: () => {
			if (!active) return;
			active = false;
			for (const submenu of observedSubmenus) submenu.off('menu-will-show', sync);
			menuApi.setApplicationMenu(previousMenu);
		},
	};
}

interface RootPlan {
	readonly items: MenuItemConstructorOptions[];
	readonly replacedRoles: Set<MenuItem['role']>;
	readonly replacementIds: Set<string>;
}

function defaultApplicationMenuTemplate(platform: NodeJS.Platform): MenuItemConstructorOptions[] {
	return [
		...(platform === 'darwin' ? ([{ role: 'appMenu' }] as const) : []),
		{ role: 'fileMenu' },
		{ role: 'editMenu' },
		{ role: 'viewMenu' },
		{ role: 'windowMenu' },
	];
}

function findRole(menu: Menu, role: MenuItem['role']): MenuItem | undefined {
	const normalizedRole = role?.toLowerCase();
	return menu.items.find((item) => item.role?.toLowerCase() === normalizedRole);
}

function invocationSource(event: KeyboardEvent): DesktopWindowActionSource {
	return event.triggeredByAccelerator === true ? 'keyboard' : 'menu';
}

function planRoot(
	plans: Map<MenuItem, RootPlan>,
	root: MenuItem,
	replacedRole: MenuItem['role'],
	item: MenuItemConstructorOptions & { readonly id: string },
): void {
	const plan = plans.get(root) ?? {
		items: [],
		replacedRoles: new Set<MenuItem['role']>(),
		replacementIds: new Set<string>(),
	};
	plan.replacedRoles.add(replacedRole);
	plan.replacementIds.add(item.id);
	plan.items.push(item);
	plans.set(root, plan);
}

function projectAvailability(item: MenuItem | null, availability: ActionAvailability): void {
	if (item === null) return;
	item.enabled = availability.kind === 'enabled';
	item.checked = availability.kind !== 'hidden' && availability.checked === true;
}
