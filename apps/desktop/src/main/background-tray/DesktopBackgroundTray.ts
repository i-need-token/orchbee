import { readFileSync } from 'node:fs';
import path from 'node:path';

import { desktopTrayMessages, type IntlShape } from '@orchbee/i18n';
import {
	Menu,
	Notification,
	Tray,
	nativeImage,
	type MenuItemConstructorOptions,
	type NativeImage,
} from 'electron';

import {
	projectDesktopTray,
	type DesktopBackgroundMenuView,
	type DesktopBackgroundState,
	type DesktopBackgroundStatusLine,
	type DesktopTrayStatus,
	type DesktopTrayView,
} from './desktop-background-lifecycle';
import type {
	DesktopBackgroundSurface,
	DesktopBackgroundSurfaceActions,
} from './desktop-background-surface';

/**
 * Lets Windows remember the person's "show in the taskbar" choice for the release icon across runs,
 * and macOS the place the person dragged it to in the menu bar; Linux ignores it. Test and Dev run from paths that
 * change, and Windows ties a GUID to an unsigned executable's path, so they set none rather than
 * fail to create the icon. Never change it: both systems key the person's choice to it.
 */
const RELEASE_TRAY_GUID = '4b85c879-5ad2-453c-8de2-5669f38c0b5f';

const ENVIRONMENT_FILE_SUFFIXES: Readonly<
	Record<DesktopBackgroundTrayOptions['distribution'], string>
> = {
	development: '-dev',
	release: '',
	test: '-test',
};

const STATUS_FILE_SUFFIXES: Readonly<Record<DesktopTrayStatus, string>> = {
	attention: '-attention',
	done: '-done',
	failed: '-failed',
	none: '',
};

export interface DesktopBackgroundTrayOptions extends DesktopBackgroundSurfaceActions {
	/** The application name, such as `Orchbee Test`, which begins the tooltip and the notice. */
	readonly applicationName: string;
	readonly distribution: 'development' | 'release' | 'test';
	/**
	 * The folder holding the generated icons: `resources/brand/ico/tray/` on Windows,
	 * `resources/brand/png/tray/` on macOS and `resources/brand/png/linux/tray/` on Linux.
	 */
	readonly iconDirectory: string;
	readonly intl: IntlShape;
	readonly platform: 'darwin' | 'linux' | 'win32';
}

/**
 * The icon Desktop shows from start until quit: in the notification area on Windows, in the menu bar
 * on macOS, and on Linux as a StatusNotifierItem, which the panel shows where the desktop hosts
 * them. Its image marks the environment outside a release; on Windows and Linux it also marks the
 * most urgent Agent state, and on macOS the item shows beside it how many Agents need the person.
 * Its tooltip and its menu's first line say how many terminals run, or that Quit All is stopping
 * them. On Windows a click or double-click shows and focuses the window, and on Linux the
 * activation the panel sends, a click on most; on macOS a click opens the menu, as menu-bar icons
 * do.
 */
export class DesktopBackgroundTray implements DesktopBackgroundSurface {
	#image: string | undefined;
	#menuKey: string | undefined;
	/** Held so that its click and show events reach Main for as long as the notice is listed. */
	#notice: Notification | undefined;
	readonly #options: DesktopBackgroundTrayOptions;
	#title: string | undefined;
	#toolTip: string | undefined;
	#tray: Tray | undefined;

	constructor(options: DesktopBackgroundTrayOptions) {
		this.#options = options;
	}

	/** Creates, updates, or removes the icon to match the lifecycle's view of `state`. */
	update(state: DesktopBackgroundState): void {
		this.#render(projectDesktopTray(state));
	}

	/**
	 * Shows the one-time notice: the icon's balloon on Windows, which withholds it during Do Not
	 * Disturb and reports `balloon-show` only for one it shows, and a notification on macOS and
	 * Linux, which counts only once the system reports it shown. A withheld notice is shown again at
	 * the next hide.
	 */
	showNotice(runningTerminals: number): void {
		const { applicationName, intl, platform } = this.#options;
		const title = intl.formatMessage(desktopTrayMessages.noticeTitle, {
			application: applicationName,
		});
		if (platform !== 'win32') {
			if (!Notification.isSupported()) return;
			const body =
				platform === 'darwin'
					? desktopTrayMessages.noticeMenuBarBody
					: desktopTrayMessages.noticePanelBody;
			const notice = new Notification({
				body: intl.formatMessage(body, { terminals: runningTerminals }),
				title,
			});
			notice.once('show', () => this.#options.onNoticeShown());
			notice.on('click', () => this.#options.onReopen());
			this.#notice?.close();
			this.#notice = notice;
			notice.show();
			return;
		}
		const tray = this.#tray;
		if (tray === undefined || tray.isDestroyed()) return;
		tray.displayBalloon({
			content: intl.formatMessage(desktopTrayMessages.noticeBody, { terminals: runningTerminals }),
			icon: this.#iconPath('none'),
			respectQuietTime: true,
			title,
		});
	}

	dispose(): void {
		this.#notice?.close();
		this.#notice = undefined;
		const tray = this.#tray;
		this.#tray = undefined;
		this.#image = undefined;
		this.#menuKey = undefined;
		this.#title = undefined;
		this.#toolTip = undefined;
		if (tray !== undefined && !tray.isDestroyed()) tray.destroy();
	}

	#render(view: DesktopTrayView | null): void {
		if (view === null) {
			this.dispose();
			return;
		}
		const image = this.#iconPath(view.status);
		let tray = this.#tray;
		if (tray === undefined || tray.isDestroyed()) {
			tray = this.#create(this.#trayImage(image));
			this.#tray = tray;
			this.#image = image;
			this.#menuKey = undefined;
			this.#title = undefined;
			this.#toolTip = undefined;
		} else if (image !== this.#image) {
			tray.setImage(this.#trayImage(image));
			this.#image = image;
		}
		const { applicationName, intl, platform } = this.#options;
		if (platform === 'darwin') {
			// A count rather than a mark: the template image has one color, and the menu names the states.
			const title = view.agents > 0 ? intl.formatNumber(view.agents) : '';
			if (title !== this.#title) {
				tray.setTitle(title, { fontType: 'monospacedDigit' });
				this.#title = title;
			}
		}
		const status = this.#statusLine(view.menu.status);
		const toolTip =
			status === undefined
				? applicationName
				: intl.formatMessage(desktopTrayMessages.tooltip, { application: applicationName, status });
		if (toolTip !== this.#toolTip) {
			tray.setToolTip(toolTip);
			this.#toolTip = toolTip;
		}
		// An unchanged menu is kept, so a menu the person has open is not replaced under them.
		const menuKey = JSON.stringify(view.menu);
		if (menuKey !== this.#menuKey) {
			tray.setContextMenu(Menu.buildFromTemplate(this.#template(view.menu, status)));
			this.#menuKey = menuKey;
		}
	}

	/**
	 * The status line, open, then the quits that are offered, Stop Session… before Quit Orchbee so the
	 * menu ends with the quit; one left out is not disabled.
	 */
	#template(
		menu: DesktopBackgroundMenuView,
		status: string | undefined,
	): MenuItemConstructorOptions[] {
		const { intl } = this.#options;
		const sections: MenuItemConstructorOptions[][] = [];
		if (status !== undefined) sections.push([{ enabled: false, label: status }]);
		sections.push([
			{
				click: () => this.#options.onReopen(),
				label: intl.formatMessage(desktopTrayMessages.open),
			},
		]);
		const quits: MenuItemConstructorOptions[] = [];
		if (menu.quitAll) {
			quits.push({
				click: () => this.#options.onQuitAll(),
				label: intl.formatMessage(desktopTrayMessages.quitAll),
			});
		}
		if (menu.quit) {
			quits.push({
				click: () => this.#options.onQuit(),
				label: intl.formatMessage(desktopTrayMessages.quitKeepRunning),
			});
		}
		if (quits.length > 0) sections.push(quits);
		return sections.flatMap((section, index) =>
			index === 0 ? section : [{ type: 'separator' as const }, ...section],
		);
	}

	#statusLine(line: DesktopBackgroundStatusLine): string | undefined {
		const { intl } = this.#options;
		switch (line.kind) {
			case 'none':
				return undefined;
			case 'stopping':
				return intl.formatMessage(desktopTrayMessages.quitAllStopping);
			case 'summary':
				return intl.formatMessage(desktopTrayMessages.status, {
					attention: line.summary.attention,
					done: line.summary.doneUnseen,
					failed: line.summary.failed,
					terminals: line.summary.runningTerminals,
				});
		}
	}

	#create(image: NativeImage | string): Tray {
		const tray =
			this.#options.distribution === 'release'
				? new Tray(image, RELEASE_TRAY_GUID)
				: new Tray(image);
		const reopen = (): void => this.#options.onReopen();
		// A macOS menu-bar icon opens its menu when clicked and reports no click of its own.
		if (this.#options.platform === 'win32') {
			tray.on('click', reopen);
			tray.on('double-click', reopen);
			tray.on('balloon-click', reopen);
			tray.on('balloon-show', () => this.#options.onNoticeShown());
		} else if (this.#options.platform === 'linux') {
			// The panel's activation, which the StatusNotifierItem specification leaves to each desktop.
			tray.on('click', reopen);
		}
		return tray;
	}

	/**
	 * The icon as the tray takes it: the `.ico` file on Windows, and elsewhere one image of the picture
	 * and its `-2x` double, on macOS the 18 point template image. Electron would join a `@2x` file by
	 * itself, but that name is outside the window file list's path grammar, which every reader of an
	 * update checks.
	 */
	#trayImage(iconPath: string): NativeImage | string {
		const { platform } = this.#options;
		if (platform === 'win32') return iconPath;
		const image = nativeImage.createEmpty();
		image.addRepresentation({ buffer: readFileSync(iconPath), scaleFactor: 1 });
		image.addRepresentation({
			buffer: readFileSync(iconPath.replace(/\.png$/u, '-2x.png')),
			scaleFactor: 2,
		});
		if (platform === 'darwin') image.setTemplateImage(true);
		return image;
	}

	/**
	 * The Windows and Linux icons mark each Agent state in its color. The macOS icon is a template
	 * image, which macOS draws in the menu bar's own color, so it marks no state; the item's title
	 * counts the Agents instead.
	 */
	#iconPath(status: DesktopTrayStatus): string {
		const { distribution, iconDirectory, platform } = this.#options;
		const environment = ENVIRONMENT_FILE_SUFFIXES[distribution];
		if (platform === 'darwin') {
			return path.join(iconDirectory, `orchbee${environment}Template.png`);
		}
		const extension = platform === 'linux' ? 'png' : 'ico';
		return path.join(
			iconDirectory,
			`orchbee${environment}${STATUS_FILE_SUFFIXES[status]}.${extension}`,
		);
	}
}
