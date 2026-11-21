import type { ActionAvailability, ActionDisposable } from '@orchbee/actions';
import {
	desktopDiagnosticsMessages,
	desktopMenuMessages,
	desktopSecureKeyboardEntryMessages,
	desktopTrayMessages,
	desktopWindowMessages,
	type IntlShape,
	type MessageDescriptor,
} from '@orchbee/i18n';
import { Menu, type KeyboardEvent, type MenuItem, type MenuItemConstructorOptions } from 'electron';

import type { DesktopDiagnosticsActionSource } from '../diagnostics';
import type { DesktopSecureKeyboardEntryActionSource } from '../secure-keyboard-entry';
import type { DesktopWindowActionSource } from '../window-actions';

const DESKTOP_APPLICATION_MENU_ITEM_IDS = {
	closeWindow: 'desktop-window-close',
	forceReload: 'desktop-view-force-reload',
	quitAll: 'desktop-app-quit-all',
	quitKeepRunning: 'desktop-app-quit-keep-running',
	reload: 'desktop-view-reload',
	reopen: 'desktop-window-reopen',
	saveDiagnosticBundle: 'desktop-help-save-diagnostic-bundle',
	secureKeyboardEntry: 'desktop-secure-keyboard-entry',
	toggleFullScreen: 'desktop-window-toggle-full-screen',
	toggleMaximized: 'desktop-window-toggle-maximized',
} as const;

const DESKTOP_APPLICATION_MENU_ROOT_IDS = {
	application: 'desktop-application-menu',
	edit: 'desktop-edit-menu',
	file: 'desktop-file-menu',
	help: 'desktop-help-menu',
	view: 'desktop-view-menu',
	window: 'desktop-window-menu',
} as const;

/**
 * What the menu's labels are formatted with. Every root and item carries a label of its own, so
 * none shows Electron's English default.
 */
export interface DesktopApplicationMenuText {
	readonly applicationName: string;
	readonly intl: IntlShape;
}

/** The Windows and macOS diagnostic bundle Action, placed in the Help menu. */
interface DesktopApplicationMenuDiagnostics {
	getAvailability(): ActionAvailability;
	invoke(source: DesktopDiagnosticsActionSource): boolean;
}

/**
 * The macOS background items: Command-Q named for what it does, Quit All… after it in the
 * application menu, and Open Orchbee in the Window menu while no window shows.
 */
interface DesktopApplicationMenuBackground {
	getQuitAllAvailability(): ActionAvailability;
	getReopenAvailability(): ActionAvailability;
	invokeQuitAll(): void;
	invokeReopen(): void;
}

export interface DesktopApplicationMenuTarget {
	readonly background?: DesktopApplicationMenuBackground | undefined;
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

/**
 * Installs the menu Electron's default menu would be, every root and item labeled from `text`, with
 * the window Actions, macOS Secure Keyboard Entry, the macOS background items and the diagnostic
 * bundle in it. The standard items keep their roles, so the system still performs them, and macOS
 * still fills some of them, such as Services, in its own language.
 */
export function installDesktopApplicationMenu(
	actions: DesktopApplicationMenuTarget,
	text: DesktopApplicationMenuText,
	platform: NodeJS.Platform,
	menuApi: DesktopApplicationMenuApi = defaultMenuApi,
): ActionDisposable {
	const previousMenu = menuApi.getApplicationMenu();
	const format = (descriptor: MessageDescriptor): string => text.intl.formatMessage(descriptor);
	const formatNamed = (descriptor: MessageDescriptor): string =>
		text.intl.formatMessage(descriptor, { application: text.applicationName });
	const standard = (
		role: NonNullable<MenuItemConstructorOptions['role']>,
		descriptor: MessageDescriptor,
	): MenuItemConstructorOptions => ({ label: format(descriptor), role });
	const darwin = platform === 'darwin';
	const background = darwin ? actions.background : undefined;
	const diagnostics = platform === 'win32' || darwin ? actions.saveDiagnosticBundle : undefined;

	const separator = (): MenuItemConstructorOptions => ({ type: 'separator' });
	const root = (
		id: string,
		label: string,
		submenu: MenuItemConstructorOptions[],
	): MenuItemConstructorOptions => ({ id, label, submenu });

	const applicationMenu: MenuItemConstructorOptions[] = darwin
		? [
				{ label: formatNamed(desktopMenuMessages.about), role: 'about' },
				separator(),
				standard('services', desktopMenuMessages.services),
				separator(),
				{ label: formatNamed(desktopMenuMessages.hide), role: 'hide' },
				standard('hideOthers', desktopMenuMessages.hideOthers),
				standard('unhide', desktopMenuMessages.showAll),
				separator(),
				{
					click: () => actions.invokeToggleSecureKeyboardEntry('menu'),
					id: DESKTOP_APPLICATION_MENU_ITEM_IDS.secureKeyboardEntry,
					label: format(desktopSecureKeyboardEntryMessages.toggle),
					type: 'checkbox',
				},
				separator(),
				...(background === undefined
					? [{ label: formatNamed(desktopMenuMessages.quit), role: 'quit' as const }]
					: [
							// Command-Q stays the native quit, which leaves the Session running; only its
							// name says so.
							{
								accelerator: 'Command+Q',
								id: DESKTOP_APPLICATION_MENU_ITEM_IDS.quitKeepRunning,
								label: format(desktopTrayMessages.quitKeepRunning),
								role: 'quit' as const,
							},
							{
								click: () => background.invokeQuitAll(),
								id: DESKTOP_APPLICATION_MENU_ITEM_IDS.quitAll,
								label: format(desktopTrayMessages.quitAll),
							},
						]),
			]
		: [];
	const fileMenu: MenuItemConstructorOptions[] = darwin
		? [
				// Workbench owns Command-W as Close Terminal; the item registers no key, so that key
				// cannot fall through and close the window.
				{
					id: DESKTOP_APPLICATION_MENU_ITEM_IDS.closeWindow,
					label: format(desktopMenuMessages.closeWindow),
					registerAccelerator: false,
					role: 'close',
				},
			]
		: [
				platform === 'win32'
					? standard('quit', desktopMenuMessages.exit)
					: { label: formatNamed(desktopMenuMessages.quit), role: 'quit' },
			];
	const editMenu: MenuItemConstructorOptions[] = [
		standard('undo', desktopMenuMessages.undo),
		standard('redo', desktopMenuMessages.redo),
		separator(),
		standard('cut', desktopMenuMessages.cut),
		standard('copy', desktopMenuMessages.copy),
		standard('paste', desktopMenuMessages.paste),
		...(darwin
			? [
					standard('pasteAndMatchStyle', desktopMenuMessages.pasteAndMatchStyle),
					standard('delete', desktopMenuMessages.delete),
					standard('selectAll', desktopMenuMessages.selectAll),
					separator(),
					{
						label: format(desktopMenuMessages.speech),
						submenu: [
							standard('startSpeaking', desktopMenuMessages.startSpeaking),
							standard('stopSpeaking', desktopMenuMessages.stopSpeaking),
						],
					},
				]
			: [
					standard('delete', desktopMenuMessages.delete),
					separator(),
					standard('selectAll', desktopMenuMessages.selectAll),
				]),
	];
	const viewMenu: MenuItemConstructorOptions[] = [
		// Reload and Force Reload stay clickable but register no key, so the page's own binding for
		// the key (panel resizing among them) resolves first.
		{
			id: DESKTOP_APPLICATION_MENU_ITEM_IDS.reload,
			label: format(desktopMenuMessages.reload),
			registerAccelerator: false,
			role: 'reload',
		},
		{
			id: DESKTOP_APPLICATION_MENU_ITEM_IDS.forceReload,
			label: format(desktopMenuMessages.forceReload),
			registerAccelerator: false,
			role: 'forceReload',
		},
		standard('toggleDevTools', desktopMenuMessages.toggleDeveloperTools),
		separator(),
		standard('resetZoom', desktopMenuMessages.actualSize),
		standard('zoomIn', desktopMenuMessages.zoomIn),
		standard('zoomOut', desktopMenuMessages.zoomOut),
		separator(),
		{
			accelerator: darwin ? 'Control+Command+F' : 'F11',
			click: (_menuItem, _window, event) => {
				actions.invokeToggleFullScreen(invocationSource(event));
			},
			id: DESKTOP_APPLICATION_MENU_ITEM_IDS.toggleFullScreen,
			label: format(desktopWindowMessages.toggleFullScreen),
			type: 'checkbox',
		},
	];
	const windowMenu: MenuItemConstructorOptions[] = [
		standard('minimize', desktopMenuMessages.minimize),
		{
			click: (_menuItem, _window, event) => {
				actions.invokeToggleMaximized(invocationSource(event));
			},
			id: DESKTOP_APPLICATION_MENU_ITEM_IDS.toggleMaximized,
			label: format(desktopWindowMessages.toggleMaximized),
			type: 'checkbox',
		},
		...(darwin
			? [separator(), standard('front', desktopMenuMessages.bringAllToFront)]
			: [standard('close', desktopMenuMessages.close)]),
		...(background === undefined
			? []
			: [
					{
						click: () => background.invokeReopen(),
						id: DESKTOP_APPLICATION_MENU_ITEM_IDS.reopen,
						label: format(desktopTrayMessages.open),
					},
				]),
	];

	const roots: MenuItemConstructorOptions[] = [
		...(darwin
			? [root(DESKTOP_APPLICATION_MENU_ROOT_IDS.application, text.applicationName, applicationMenu)]
			: []),
		root(DESKTOP_APPLICATION_MENU_ROOT_IDS.file, format(desktopMenuMessages.file), fileMenu),
		root(DESKTOP_APPLICATION_MENU_ROOT_IDS.edit, format(desktopMenuMessages.edit), editMenu),
		root(DESKTOP_APPLICATION_MENU_ROOT_IDS.view, format(desktopMenuMessages.view), viewMenu),
		root(DESKTOP_APPLICATION_MENU_ROOT_IDS.window, format(desktopMenuMessages.window), windowMenu),
		...(diagnostics === undefined
			? []
			: [
					{
						...root(DESKTOP_APPLICATION_MENU_ROOT_IDS.help, format(desktopMenuMessages.help), [
							{
								click: () => diagnostics.invoke('menu'),
								id: DESKTOP_APPLICATION_MENU_ITEM_IDS.saveDiagnosticBundle,
								label: format(desktopDiagnosticsMessages.saveBundle),
							},
						]),
						// The system's Help menu, to which macOS adds its search field.
						...(darwin ? { role: 'help' as const } : {}),
					},
				]),
	];
	const menu = menuApi.buildFromTemplate(roots);
	const observedSubmenus = Object.values(DESKTOP_APPLICATION_MENU_ROOT_IDS).flatMap((id) => {
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
		if (darwin) {
			projectAvailability(
				menu.getMenuItemById(DESKTOP_APPLICATION_MENU_ITEM_IDS.secureKeyboardEntry),
				actions.getSecureKeyboardEntryAvailability(),
			);
		}
		if (background !== undefined) {
			projectAvailability(
				menu.getMenuItemById(DESKTOP_APPLICATION_MENU_ITEM_IDS.quitAll),
				background.getQuitAllAvailability(),
			);
			projectAvailability(
				menu.getMenuItemById(DESKTOP_APPLICATION_MENU_ITEM_IDS.reopen),
				background.getReopenAvailability(),
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

function invocationSource(event: KeyboardEvent): DesktopWindowActionSource {
	return event.triggeredByAccelerator === true ? 'keyboard' : 'menu';
}

function projectAvailability(item: MenuItem | null, availability: ActionAvailability): void {
	if (item === null) return;
	item.enabled = availability.kind === 'enabled';
	item.checked = availability.kind !== 'hidden' && availability.checked === true;
	item.visible = availability.kind !== 'hidden';
}
