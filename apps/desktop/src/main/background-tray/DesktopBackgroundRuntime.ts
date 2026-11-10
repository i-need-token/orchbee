import type { DesktopBackgroundStatusSummary } from '@orchbee/desktop-client-protocol';

import {
	createDesktopBackgroundState,
	projectDesktopTray,
	transitionDesktopBackground,
	type DesktopBackgroundEvent,
	type DesktopBackgroundState,
} from './desktop-background-lifecycle';
import { DesktopBackgroundTray, type DesktopBackgroundTrayOptions } from './DesktopBackgroundTray';
import type { FileDesktopTrayRecord } from './FileDesktopTrayRecord';

/** What a document's background-status port delivers. */
export interface DesktopBackgroundStatusSink {
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
	'onNoticeShown' | 'onQuit' | 'onReopen'
> {
	readonly noticeShown: boolean;
	readonly quit: () => void;
	readonly record: Pick<FileDesktopTrayRecord, 'recordNoticeShown'>;
	/** Opens the window, or shows and focuses the hidden one. */
	readonly reopen: () => void;
}

/**
 * Runs the background lifecycle for the Windows window: applies each event, keeps the
 * notification-area icon equal to the lifecycle's view, and performs the commands it returns.
 */
export class DesktopBackgroundRuntime implements DesktopWindowBackground {
	readonly #options: DesktopBackgroundRuntimeOptions;
	#state: DesktopBackgroundState;
	readonly #tray: DesktopBackgroundTray;

	constructor(options: DesktopBackgroundRuntimeOptions) {
		this.#options = options;
		this.#state = createDesktopBackgroundState(options.noticeShown);
		this.#tray = new DesktopBackgroundTray({
			...options,
			onNoticeShown: () => this.#dispatch({ kind: 'notice-shown' }),
			onQuit: options.quit,
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
		for (const command of commands) {
			switch (command.kind) {
				case 'hide-window':
					hidden = true;
					break;
				case 'quit':
					this.#options.quit();
					break;
				case 'record-notice':
					void this.#options.record.recordNoticeShown();
					break;
				case 'show-notice':
					this.#tray.showNotice(state.summary.runningTerminals);
					break;
			}
		}
		return hidden;
	}
}
