import type {
	DesktopBackgroundStatusSummary,
	DesktopSessionStopAnswer,
} from '@orchbee/desktop-client-protocol';
import type { IntlShape } from '@orchbee/i18n';
import type { BrowserWindow } from 'electron';

import type { DesktopDiagnosticRecorder } from '../diagnostics';
import type { ServerStopStatus } from '../server-connection';
import {
	createDesktopBackgroundState,
	projectDesktopDock,
	transitionDesktopBackground,
	type DesktopBackgroundEvent,
	type DesktopBackgroundRelease,
	type DesktopBackgroundState,
	type DesktopDockView,
} from './desktop-background-lifecycle';
import type {
	DesktopBackgroundSurface,
	DesktopBackgroundSurfaceActions,
} from './desktop-background-surface';
import { DesktopSessionStop } from './DesktopSessionStop';
import type { FileDesktopTrayRecord } from './FileDesktopTrayRecord';
import { confirmDesktopQuitAll, reportDesktopQuitAllFailure } from './quit-all-dialogs';

/** What a document's background-status port delivers. */
export interface DesktopBackgroundStatusSink {
	/** Opens Quit All's confirmation, as its menu item does; the page asks once its Server gave up. */
	quitAllRequested(): void;
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

export interface DesktopBackgroundRuntimeOptions {
	/** The application name, such as `Orchbee Test`, which titles Quit All's dialogs. */
	readonly applicationName: string;
	/** Closes the hidden window, where the platform's release keeps Desktop running without one. */
	readonly closeWindow: () => void;
	/** Creates the platform's surface, which reports the person's requests through `actions`. */
	readonly createSurface: (actions: DesktopBackgroundSurfaceActions) => DesktopBackgroundSurface;
	readonly intl: IntlShape;
	readonly noticeShown: boolean;
	readonly quit: () => void;
	/** Reads a Session stop's progress with `operation status`. */
	readonly readStopStatus: (stopId: string) => Promise<ServerStopStatus>;
	readonly recordEvent: DesktopDiagnosticRecorder;
	readonly record: Pick<FileDesktopTrayRecord, 'recordNoticeShown'>;
	/** What ends a hidden window that nothing is left to reopen for. */
	readonly release: DesktopBackgroundRelease;
	/** Opens the window, or shows and focuses the hidden one. */
	readonly reopen: () => void;
	/** Asks the window's page to stop the Session under `stopId`; false when there is none. */
	readonly requestSessionStop: (stopId: string) => boolean;
	/** Opens or shows the window and settles with it once it shows, for a report over it. */
	readonly showWindow: () => Promise<BrowserWindow | undefined>;
}

/**
 * Runs the background lifecycle for the window: applies each event, keeps the platform's surface,
 * the Windows notification-area icon or the macOS Dock menu, equal to the lifecycle's state, and
 * performs the commands it returns.
 */
export class DesktopBackgroundRuntime implements DesktopWindowBackground {
	readonly #options: DesktopBackgroundRuntimeOptions;
	readonly #sessionStop: DesktopSessionStop;
	#state: DesktopBackgroundState;
	readonly #surface: DesktopBackgroundSurface;

	constructor(options: DesktopBackgroundRuntimeOptions) {
		this.#options = options;
		this.#sessionStop = new DesktopSessionStop({
			readStatus: options.readStopStatus,
			request: options.requestSessionStop,
		});
		this.#state = createDesktopBackgroundState(options.noticeShown, options.release);
		this.#surface = options.createSurface({
			onNoticeShown: () => this.#dispatch({ kind: 'notice-shown' }),
			onQuit: options.quit,
			onQuitAll: () => this.quitAllRequested(),
			onReopen: options.reopen,
		});
		this.#surface.update(this.#state);
	}

	/** What the Dock menu offers, which the macOS application menu's Reopen and Quit All follow. */
	get dockView(): DesktopDockView {
		return projectDesktopDock(this.#state);
	}

	closeRequested(): boolean {
		return this.#dispatch({ kind: 'close-requested' });
	}

	quitAllRequested(): void {
		this.#dispatch({ kind: 'quit-all-requested' });
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
		this.#surface.dispose();
	}

	/** Returns whether the event hid the window. */
	#dispatch(event: DesktopBackgroundEvent): boolean {
		const { commands, state } = transitionDesktopBackground(this.#state, event);
		this.#state = state;
		this.#surface.update(state);
		let hidden = false;
		const { applicationName, intl } = this.#options;
		for (const command of commands) {
			switch (command.kind) {
				case 'confirm-quit-all':
					void confirmDesktopQuitAll(intl, applicationName, state.summary).then((confirmed) =>
						this.#dispatch({ kind: confirmed ? 'quit-all-confirmed' : 'quit-all-cancelled' }),
					);
					break;
				case 'close-window':
					this.#options.closeWindow();
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
					const { reason } = command;
					// The words say only what happened; the reason goes to the diagnostic log.
					this.#options.recordEvent('warning', 'quit-all-failed', { reason });
					const report = (window: BrowserWindow | undefined): Promise<void> =>
						reportDesktopQuitAllFailure(intl, applicationName, window, reason);
					// Whatever becomes of the report, Quit All is offered again afterwards.
					void this.#options
						.showWindow()
						.then(report, () => report(undefined))
						.catch(() => undefined)
						.then(() => this.#dispatch({ kind: 'quit-all-failure-dismissed' }));
					break;
				}
				case 'show-notice':
					this.#surface.showNotice(state.summary.runningTerminals);
					break;
				case 'stop-session':
					void this.#sessionStop
						.stop()
						// A stop that failed in Main may still have reached the page, so it is unconfirmed.
						.catch(() => ({ kind: 'failed', reason: 'unconfirmed' }) as const)
						.then((result) =>
							this.#dispatch(
								result.kind === 'stopped'
									? { kind: 'quit-all-stopped' }
									: { kind: 'quit-all-failed', reason: result.reason },
							),
						);
					break;
			}
		}
		return hidden;
	}
}
