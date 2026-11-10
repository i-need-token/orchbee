import path from 'node:path';

import { desktopTrayMessages, type IntlShape } from '@orchbee/i18n';
import { Menu, Tray, type MenuItemConstructorOptions } from 'electron';

import type { DesktopTrayStatus, DesktopTrayView } from './desktop-background-lifecycle';

/**
 * Lets Windows remember the person's "show in the taskbar" choice for the release icon across runs.
 * Test and Dev run from paths that change, and Windows ties a GUID to an unsigned executable's path,
 * so they set none rather than fail to create the icon. Never change it: Windows keys the choice to it.
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

export interface DesktopBackgroundTrayOptions {
	/** The application name, such as `Orchbee Test`, which begins the tooltip and the notice. */
	readonly applicationName: string;
	readonly distribution: 'development' | 'release' | 'test';
	/** The folder holding the generated `resources/brand/ico/tray/` icons. */
	readonly iconDirectory: string;
	readonly intl: IntlShape;
	readonly onNoticeShown: () => void;
	readonly onQuit: () => void;
	readonly onQuitAll: () => void;
	readonly onReopen: () => void;
}

/**
 * The Windows notification-area icon, which exists only while the lifecycle's view says so: its
 * image marks the most urgent Agent state and, outside a release, the environment; its tooltip and
 * its menu's first line say how many terminals run, or that Quit All is stopping them. Left click
 * and double-click reopen the window.
 */
export class DesktopBackgroundTray {
	readonly #options: DesktopBackgroundTrayOptions;
	#tray: Tray | undefined;

	constructor(options: DesktopBackgroundTrayOptions) {
		this.#options = options;
	}

	/** Creates, updates, or removes the icon to match `view`. */
	update(view: DesktopTrayView | null): void {
		if (view === null) {
			this.dispose();
			return;
		}
		const image = this.#iconPath(view.status);
		let tray = this.#tray;
		if (tray === undefined || tray.isDestroyed()) {
			tray = this.#create(image);
			this.#tray = tray;
		} else {
			tray.setImage(image);
		}
		const { intl } = this.#options;
		const status = view.stopping
			? intl.formatMessage(desktopTrayMessages.quitAllStopping)
			: intl.formatMessage(desktopTrayMessages.status, {
					attention: view.summary.attention,
					done: view.summary.doneUnseen,
					failed: view.summary.failed,
					terminals: view.summary.runningTerminals,
				});
		tray.setToolTip(
			intl.formatMessage(desktopTrayMessages.tooltip, {
				application: this.#options.applicationName,
				status,
			}),
		);
		const statusAndReopen: MenuItemConstructorOptions[] = [
			{ enabled: false, label: status },
			{ type: 'separator' },
			{
				click: () => this.#options.onReopen(),
				label: intl.formatMessage(desktopTrayMessages.reopen),
			},
		];
		// While Quit All stops the Session, there is nothing left to quit but the window to reopen.
		tray.setContextMenu(
			Menu.buildFromTemplate(
				view.stopping
					? statusAndReopen
					: [
							...statusAndReopen,
							{ type: 'separator' },
							{
								click: () => this.#options.onQuit(),
								label: intl.formatMessage(desktopTrayMessages.quitKeepRunning),
							},
							{
								click: () => this.#options.onQuitAll(),
								label: intl.formatMessage(desktopTrayMessages.quitAll),
							},
						],
			),
		);
	}

	/**
	 * Shows the one-time notice from the icon. Windows withholds it during Do Not Disturb, and only
	 * one it shows reports `balloon-show`, so a withheld notice is shown again at the next close.
	 */
	showNotice(runningTerminals: number): void {
		const tray = this.#tray;
		if (tray === undefined || tray.isDestroyed()) return;
		const { intl } = this.#options;
		tray.displayBalloon({
			content: intl.formatMessage(desktopTrayMessages.noticeBody, { terminals: runningTerminals }),
			icon: this.#iconPath('none'),
			respectQuietTime: true,
			title: intl.formatMessage(desktopTrayMessages.noticeTitle, {
				application: this.#options.applicationName,
			}),
		});
	}

	dispose(): void {
		const tray = this.#tray;
		this.#tray = undefined;
		if (tray !== undefined && !tray.isDestroyed()) tray.destroy();
	}

	#create(image: string): Tray {
		const tray =
			this.#options.distribution === 'release'
				? new Tray(image, RELEASE_TRAY_GUID)
				: new Tray(image);
		const reopen = (): void => this.#options.onReopen();
		tray.on('click', reopen);
		tray.on('double-click', reopen);
		tray.on('balloon-click', reopen);
		tray.on('balloon-show', () => this.#options.onNoticeShown());
		return tray;
	}

	#iconPath(status: DesktopTrayStatus): string {
		const environment = ENVIRONMENT_FILE_SUFFIXES[this.#options.distribution];
		return path.join(
			this.#options.iconDirectory,
			`orchbee${environment}${STATUS_FILE_SUFFIXES[status]}.ico`,
		);
	}
}
