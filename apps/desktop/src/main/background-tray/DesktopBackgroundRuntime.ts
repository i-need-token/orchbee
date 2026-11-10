import type {
	DesktopBackgroundStatusSummary,
	DesktopSessionStopAnswer,
} from '@orchbee/desktop-client-protocol';
import type { BrowserWindow } from 'electron';

import type { ServerStopStatus } from '../server-connection';
import {
	createDesktopBackgroundState,
	projectDesktopTray,
	transitionDesktopBackground,
	type DesktopBackgroundEvent,
	type DesktopBackgroundState,
} from './desktop-background-lifecycle';
import { DesktopBackgroundTray, type DesktopBackgroundTrayOptions } from './DesktopBackgroundTray';
import { DesktopSessionStop } from './DesktopSessionStop';
import type { FileDesktopTrayRecord } from './FileDesktopTrayRecord';
import { confirmDesktopQuitAll, reportDesktopQuitAllFailure } from './quit-all-dialogs';

/** What a document's background-status port delivers. */
export interface DesktopBackgroundStatusSink {
	/** The page's answer to the Session stop Quit All asked it for. */
	sessionStopAnswered(stopId: string, answer: DesktopSessionStopAnswer): void;
	summaryChanged(summary: DesktopBackgroundStatusSummary): void;
	/** The page asks for the window, as a notification click does while it is hidden. */
	windowRequested(): void;
}

/** What the window tells the background lifecycle, and how its close is decided. */
export interface DesktopWindowBackground extends DesktopBackgroundStatusSink {
	/** Returns whether the close becomes a hide; the window then cancels it and hides. */
	closeRequested(): boolean;
	sessionEnding(): void;
	windowClosed(): void;
	windowShown(): void;
}

export interface DesktopBackgroundRuntimeOptions extends Omit<
	DesktopBackgroundTrayOptions,
	'onNoticeShown' | 'onQuit' | 'onQuitAll' | 'onReopen'
> {
	readonly noticeShown: boolean;
	readonly quit: () => void;
	/** Reads a Session stop's progress with `server stop-status`. */
	readonly readStopStatus: (stopId: string) => Promise<ServerStopStatus>;
	readonly record: Pick<FileDesktopTrayRecord, 'recordNoticeShown'>;
	/** Opens the window, or shows and focuses the hidden one. */
	readonly reopen: () => void;
	/** Asks the window's page to stop the Session under `stopId`; false when there is none. */
	readonly requestSessionStop: (stopId: string) => boolean;
	/** Opens or shows the window and settles with it once it shows, for a report over it. */
	readonly showWindow: () => Promise<BrowserWindow | undefined>;
}

/**
 * Runs the background lifecycle for the Windows window: applies each event, keeps the
 * notification-area icon equal to the lifecycle's view, and performs the commands it returns.
 */
export class DesktopBackgroundRuntime implements DesktopWindowBackground {
	readonly #options: DesktopBackgroundRuntimeOptions;
	readonly #sessionStop: DesktopSessionStop;
	#state: DesktopBackgroundState;
	readonly #tray: DesktopBackgroundTray;

	constructor(options: DesktopBackgroundRuntimeOptions) {
		this.#options = options;
		this.#sessionStop = new DesktopSessionStop({
			readStatus: options.readStopStatus,
			request: options.requestSessionStop,
		});
		this.#state = createDesktopBackgroundState(options.noticeShown);
		this.#tray = new DesktopBackgroundTray({
			...options,
			onNoticeShown: () => this.#dispatch({ kind: 'notice-shown' }),
			onQuit: options.quit,
			onQuitAll: () => this.#dispatch({ kind: 'quit-all-requested' }),
			onReopen: options.reopen,
		});
	}

	closeRequested(): boolean {
		return this.#dispatch({ kind: 'close-requested' });
	}

	/** Every quit passes here first, the development runtime's restart included. */
	quitRequested(): void {
		this.#dispatch({ kind: 'quit-requested' });
	}

	sessionEnding(): void {
		this.#dispatch({ kind: 'session-ending' });
	}

	sessionStopAnswered(stopId: string, answer: DesktopSessionStopAnswer): void {
		this.#sessionStop.answered(stopId, answer);
	}

	summaryChanged(summary: DesktopBackgroundStatusSummary): void {
		this.#dispatch({ kind: 'summary-changed', summary });
	}

	windowClosed(): void {
		this.#dispatch({ kind: 'window-closed' });
	}

	windowRequested(): void {
		this.#options.reopen();
	}

	windowShown(): void {
		this.#dispatch({ kind: 'window-shown' });
	}

	dispose(): void {
		this.#tray.dispose();
	}

	/** Returns whether the event hid the window. */
	#dispatch(event: DesktopBackgroundEvent): boolean {
		const { commands, state } = transitionDesktopBackground(this.#state, event);
		this.#state = state;
		this.#tray.update(projectDesktopTray(state));
		let hidden = false;
		const { applicationName, intl } = this.#options;
		for (const command of commands) {
			switch (command.kind) {
				case 'confirm-quit-all':
					void confirmDesktopQuitAll(intl, applicationName, state.summary).then((confirmed) =>
						this.#dispatch({ kind: confirmed ? 'quit-all-confirmed' : 'quit-all-cancelled' }),
					);
					break;
				case 'hide-window':
					hidden = true;
					break;
				case 'quit':
					this.#options.quit();
					break;
				case 'record-notice':
					void this.#options.record.recordNoticeShown();
					break;
				case 'report-quit-all-failure': {
					const { reason, stopId } = command;
					void this.#options
						.showWindow()
						.then((window) =>
							reportDesktopQuitAllFailure(intl, applicationName, window, reason, stopId),
						)
						.then(() => this.#dispatch({ kind: 'quit-all-failure-dismissed' }));
					break;
				}
				case 'show-notice':
					this.#tray.showNotice(state.summary.runningTerminals);
					break;
				case 'stop-session':
					void this.#sessionStop
						.stop()
						.then((result) =>
							this.#dispatch(
								result.kind === 'stopped'
									? { kind: 'quit-all-stopped' }
									: { kind: 'quit-all-failed', reason: result.reason, stopId: result.stopId },
							),
						);
					break;
			}
		}
		return hidden;
	}
}
