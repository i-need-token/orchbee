import { desktopTrayMessages, type IntlShape } from '@orchbee/i18n';
import { Menu, Notification, type Dock, type MenuItemConstructorOptions } from 'electron';

import {
	projectDesktopDock,
	type DesktopBackgroundState,
	type DesktopDockView,
} from './desktop-background-lifecycle';
import type {
	DesktopBackgroundSurface,
	DesktopBackgroundSurfaceActions,
} from './desktop-background-surface';

export interface DesktopBackgroundDockOptions extends DesktopBackgroundSurfaceActions {
	/** The application name, such as `Orchbee Dev`, which the notice's title names. */
	readonly applicationName: string;
	readonly dock: Pick<Dock, 'setMenu'>;
	readonly intl: IntlShape;
}

/**
 * The macOS Dock menu, which the application keeps for as long as it runs, as macOS applications
 * do: its first line counts the running terminals and the Agents that need the person while a
 * window's page reports them; Reopen Orchbee shows while no window does; Quit Orchbee, Keep
 * Terminals Running and Quit All… follow. The system adds its own window list and Quit below.
 * Clicking the Dock icon reopens the window through the application's `activate`.
 */
export class DesktopBackgroundDock implements DesktopBackgroundSurface {
	#menuKey: string | undefined;
	/** Held so that its click and show events reach Main for as long as the notice is listed. */
	#notice: Notification | undefined;
	readonly #options: DesktopBackgroundDockOptions;

	constructor(options: DesktopBackgroundDockOptions) {
		this.#options = options;
	}

	/** Replaces the Dock menu when what it offers changed. */
	update(state: DesktopBackgroundState): void {
		const view = projectDesktopDock(state);
		const key = JSON.stringify(view);
		if (key === this.#menuKey) return;
		this.#menuKey = key;
		this.#options.dock.setMenu(Menu.buildFromTemplate(this.#template(view)));
	}

	/**
	 * Shows the one-time notice as a notification, as the Windows icon's balloon is; clicking it
	 * reopens the window. Only a notification macOS reports as shown counts as the notice.
	 */
	showNotice(runningTerminals: number): void {
		if (!Notification.isSupported()) return;
		const { intl } = this.#options;
		const notice = new Notification({
			body: intl.formatMessage(desktopTrayMessages.noticeDockBody, { terminals: runningTerminals }),
			title: intl.formatMessage(desktopTrayMessages.noticeTitle, {
				application: this.#options.applicationName,
			}),
		});
		notice.once('show', () => this.#options.onNoticeShown());
		notice.on('click', () => this.#options.onReopen());
		this.#notice?.close();
		this.#notice = notice;
		notice.show();
	}

	dispose(): void {
		this.#notice?.close();
		this.#notice = undefined;
	}

	#template(view: DesktopDockView): MenuItemConstructorOptions[] {
		const { intl } = this.#options;
		const sections: MenuItemConstructorOptions[][] = [];
		if (view.status.kind === 'stopping') {
			sections.push([
				{ enabled: false, label: intl.formatMessage(desktopTrayMessages.quitAllStopping) },
			]);
		} else if (view.status.kind === 'summary') {
			const { summary } = view.status;
			sections.push([
				{
					enabled: false,
					label: intl.formatMessage(desktopTrayMessages.status, {
						attention: summary.attention,
						done: summary.doneUnseen,
						failed: summary.failed,
						terminals: summary.runningTerminals,
					}),
				},
			]);
		}
		if (view.reopen) {
			sections.push([
				{
					click: () => this.#options.onReopen(),
					label: intl.formatMessage(desktopTrayMessages.reopen),
				},
			]);
		}
		const quits: MenuItemConstructorOptions[] = [];
		if (view.quit) {
			quits.push({
				click: () => this.#options.onQuit(),
				label: intl.formatMessage(desktopTrayMessages.quitKeepRunning),
			});
		}
		if (view.quitAll) {
			quits.push({
				click: () => this.#options.onQuitAll(),
				label: intl.formatMessage(desktopTrayMessages.quitAll),
			});
		}
		if (quits.length > 0) sections.push(quits);
		return sections.flatMap((section, index) =>
			index === 0 ? section : [{ type: 'separator' as const }, ...section],
		);
	}
}
