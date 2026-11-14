import type { ActionAvailability, ActionDisposable } from '@orchbee/actions';
import { Menu, type KeyboardEvent, type MenuItem, type MenuItemConstructorOptions } from 'electron';

import type { DesktopDiagnosticsActionSource } from '../diagnostics';
import type { DesktopSecureKeyboardEntryActionSource } from '../secure-keyboard-entry';
import type { DesktopWindowActionSource } from '../window-actions';

const DESKTOP_APPLICATION_MENU_ITEM_IDS = {
	closeWindow: 'desktop-window-close',
	forceReload: 'desktop-view-force-reload',
	reload: 'desktop-view-reload',
	saveDiagnosticBundle: 'desktop-help-save-diagnostic-bundle',
	secureKeyboardEntry: 'desktop-secure-keyboard-entry',
	toggleFullScreen: 'desktop-window-toggle-full-screen',
	toggleMaximized: 'desktop-window-toggle-maximized',
} as const;

export interface DesktopApplicationMenuLabels {
	readonly secureKeyboardEntry: string;
	readonly securityCategory: string;
	readonly windowCategory: string;
	readonly toggleFullScreen: string;
	readonly toggleMaximized: string;
}

/** The Windows diagnostic bundle Action, placed in the Help menu. */
interface DesktopApplicationMenuDiagnostics {
	readonly categoryLabel: string;
	readonly label: string;
	getAvailability(): ActionAvailability;
	invoke(source: DesktopDiagnosticsActionSource): boolean;
}

export interface DesktopApplicationMenuTarget {
	getSecureKeyboardEntryAvailability(): ActionAvailability;
	getToggleFullScreenAvailability(): ActionAvailability;
	getToggleMaximizedAvailability(): ActionAvailability;
	invokeToggleFullScreen(source: DesktopWindowActionSource): boolean;
	invokeToggleMaximized(source: DesktopWindowActionSource): boolean;
	invokeToggleSecureKeyboardEntry(source: DesktopSecureKeyboardEntryActionSource): boolean;
	readonly saveDiagnosticBundle?: DesktopApplicationMenuDiagnostics | undefined;
}

interface DesktopApplicationMenuApi {
	buildFromTemplate(template: Array<MenuItemConstructorOptions | MenuItem>): Menu;
	getApplicationMenu(): Menu | null;
	setApplicationMenu(menu: Menu | null): void;
}

const defaultMenuApi: DesktopApplicationMenuApi = {
	buildFromTemplate: (template) => Menu.buildFromTemplate(template),
	getApplicationMenu: () => Menu.getApplicationMenu(),
	setApplicationMenu: (menu) => Menu.setApplicationMenu(menu),
};

export function installDesktopApplicationMenu(
	actions: DesktopApplicationMenuTarget,
	labels: DesktopApplicationMenuLabels,
	platform: NodeJS.Platform,
	menuApi: DesktopApplicationMenuApi = defaultMenuApi,
): ActionDisposable {
	const previousMenu = menuApi.getApplicationMenu();
	const baseMenu =
		previousMenu ?? menuApi.buildFromTemplate(defaultApplicationMenuTemplate(platform));
	const fileRoot = findRole(baseMenu, 'fileMenu');
	const viewRoot = findRole(baseMenu, 'viewMenu');
	const windowRoot = findRole(baseMenu, 'windowMenu');
	const fullScreenItem: MenuItemConstructorOptions & { readonly id: string } = {
		accelerator: platform === 'darwin' ? 'Control+Command+F' : 'F11',
		click: (_menuItem, _window, event) => {
			actions.invokeToggleFullScreen(invocationSource(event));
		},
		id: DESKTOP_APPLICATION_MENU_ITEM_IDS.toggleFullScreen,
		label: labels.toggleFullScreen,
		type: 'checkbox',
	};
	const maximizedItem: MenuItemConstructorOptions & { readonly id: string } = {
		click: (_menuItem, _window, event) => {
			actions.invokeToggleMaximized(invocationSource(event));
		},
		id: DESKTOP_APPLICATION_MENU_ITEM_IDS.toggleMaximized,
		label: labels.toggleMaximized,
		type: 'checkbox',
	};
	const rootPlans = new Map<MenuItem, RootPlan>();
	const appendedRoots: MenuItemConstructorOptions[] = [];
	const viewMenu = viewRoot?.submenu;
	if (viewRoot !== undefined && viewMenu !== null && viewMenu !== undefined) {
		for (const [role, id] of [
			['reload', DESKTOP_APPLICATION_MENU_ITEM_IDS.reload],
			['forceReload', DESKTOP_APPLICATION_MENU_ITEM_IDS.forceReload],
		] as const) {
			const nativeItem = findRole(viewMenu, role);
			if (nativeItem === undefined) continue;
			planRoot(
				rootPlans,
				viewRoot,
				{
					...(nativeItem.accelerator === null ? {} : { accelerator: nativeItem.accelerator }),
					enabled: nativeItem.enabled,
					id,
					label: nativeItem.label,
					registerAccelerator: false,
					role,
					type: nativeItem.type,
					visible: nativeItem.visible,
				},
				role,
			);
		}
	}
	const fileMenu = fileRoot?.submenu;
	if (
		platform === 'darwin' &&
		fileRoot !== undefined &&
		fileMenu !== null &&
		fileMenu !== undefined
	) {
		const nativeCloseWindowItem = findRole(fileMenu, 'close');
		if (nativeCloseWindowItem !== undefined) {
			planRoot(
				rootPlans,
				fileRoot,
				{
					...(nativeCloseWindowItem.accelerator === null
						? {}
						: { accelerator: nativeCloseWindowItem.accelerator }),
					enabled: nativeCloseWindowItem.enabled,
					id: DESKTOP_APPLICATION_MENU_ITEM_IDS.closeWindow,
					label: nativeCloseWindowItem.label,
					registerAccelerator: false,
					role: 'close',
					type: nativeCloseWindowItem.type,
					visible: nativeCloseWindowItem.visible,
				},
				'close',
			);
		}
	}
	if (viewRoot === undefined && windowRoot === undefined) {
		appendedRoots.push({
			id: 'desktop-window-menu',
			label: labels.windowCategory,
			submenu: [fullScreenItem, maximizedItem],
		});
	} else {
		const fullScreenRoot = viewRoot ?? windowRoot;
		const maximizedRoot = windowRoot ?? viewRoot;
		if (fullScreenRoot === undefined || maximizedRoot === undefined) {
			throw new Error('Desktop window actions require a native application menu root.');
		}
		planRoot(rootPlans, fullScreenRoot, fullScreenItem, 'togglefullscreen');
		planRoot(rootPlans, maximizedRoot, maximizedItem, 'zoom');
	}

	if (platform === 'darwin') {
		const secureKeyboardEntryItem: MenuItemConstructorOptions & { readonly id: string } = {
			click: () => actions.invokeToggleSecureKeyboardEntry('menu'),
			id: DESKTOP_APPLICATION_MENU_ITEM_IDS.secureKeyboardEntry,
			label: labels.secureKeyboardEntry,
			type: 'checkbox',
		};
		const securityRoot = findRole(baseMenu, 'appMenu') ?? findRole(baseMenu, 'editMenu');
		if (securityRoot === undefined) {
			appendedRoots.push({
				id: 'desktop-input-security-menu',
				label: labels.securityCategory,
				submenu: [secureKeyboardEntryItem],
			});
		} else {
			planRoot(rootPlans, securityRoot, secureKeyboardEntryItem);
		}
	}

	const diagnostics = platform === 'win32' ? actions.saveDiagnosticBundle : undefined;
	if (diagnostics !== undefined) {
		const saveDiagnosticBundleItem: MenuItemConstructorOptions & { readonly id: string } = {
			click: () => diagnostics.invoke('menu'),
			id: DESKTOP_APPLICATION_MENU_ITEM_IDS.saveDiagnosticBundle,
			label: diagnostics.label,
		};
		const helpRoot = findRole(baseMenu, 'help');
		if (helpRoot === undefined) {
			appendedRoots.push({
				id: 'desktop-help-menu',
				label: diagnostics.categoryLabel,
				submenu: [saveDiagnosticBundleItem],
			});
		} else {
			planRoot(rootPlans, helpRoot, saveDiagnosticBundleItem);
		}
	}

	let replacementOrdinal = 0;
	const replacementRootIds: string[] = [];
	const template: Array<MenuItemConstructorOptions | MenuItem> = [
		...baseMenu.items.map<MenuItem | MenuItemConstructorOptions>((item) => {
			const plan = rootPlans.get(item);
			if (plan === undefined) return item;
			const replacementRootId = `desktop-application-menu-root-${replacementOrdinal++}`;
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
				type: 'submenu' as const,
				visible: item.visible,
			};
		}),
		...appendedRoots.map((root) => {
			if (root.id !== undefined) replacementRootIds.push(root.id);
			return root;
		}),
	];
	const menu = menuApi.buildFromTemplate(template);
	const observedSubmenus = replacementRootIds.flatMap((id) => {
		const submenu = menu.getMenuItemById(id)?.submenu;
		return submenu === undefined ? [] : [submenu];
	});

	const sync = (): void => {
		projectAvailability(
			menu.getMenuItemById(DESKTOP_APPLICATION_MENU_ITEM_IDS.toggleFullScreen),
			actions.getToggleFullScreenAvailability(),
		);
		projectAvailability(
			menu.getMenuItemById(DESKTOP_APPLICATION_MENU_ITEM_IDS.toggleMaximized),
			actions.getToggleMaximizedAvailability(),
		);
		if (platform === 'darwin') {
			projectAvailability(
				menu.getMenuItemById(DESKTOP_APPLICATION_MENU_ITEM_IDS.secureKeyboardEntry),
				actions.getSecureKeyboardEntryAvailability(),
			);
		}
		if (diagnostics !== undefined) {
			projectAvailability(
				menu.getMenuItemById(DESKTOP_APPLICATION_MENU_ITEM_IDS.saveDiagnosticBundle),
				diagnostics.getAvailability(),
			);
		}
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
	item: MenuItemConstructorOptions & { readonly id: string },
	replacedRole?: MenuItem['role'],
): void {
	const plan = plans.get(root) ?? {
		items: [],
		replacedRoles: new Set<MenuItem['role']>(),
		replacementIds: new Set<string>(),
	};
	if (replacedRole !== undefined) plan.replacedRoles.add(replacedRole);
	plan.replacementIds.add(item.id);
	plan.items.push(item);
	plans.set(root, plan);
}

function projectAvailability(item: MenuItem | null, availability: ActionAvailability): void {
	if (item === null) return;
	item.enabled = availability.kind === 'enabled';
	item.checked = availability.kind !== 'hidden' && availability.checked === true;
	item.visible = availability.kind !== 'hidden';
}
