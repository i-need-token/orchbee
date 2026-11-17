import { createOrchbeeIntl, desktopWindowMessages, resolveOrchbeeLocale } from '@orchbee/i18n';
import { app, dialog, type BrowserWindow } from 'electron';

import type { DesktopDiagnosticRecorder } from '../diagnostics';

/** A second crash this soon after the last one is a loop, which reloading would only repeat. */
const CRASH_LOOP_MILLISECONDS = 60_000;
const CLOSE_BUTTON = 0;
const REOPEN_BUTTON = 1;

/**
 * Follows the window's page process. A crash ends the document's connection and reloads the page,
 * once; a second crash within a minute of the last stops reloading and asks, in a native dialog
 * like VS Code's, whether to reopen the window or close it. Every crash is recorded with its reason.
 * The Session's terminals keep running either way.
 */
export function followRendererCrashes(
	window: BrowserWindow,
	recordEvent: DesktopDiagnosticRecorder,
	endDocumentConnection: () => void,
): void {
	let lastCrash: number | undefined;
	window.webContents.on('render-process-gone', (_event, details) => {
		endDocumentConnection();
		if (details.reason === 'clean-exit' || window.isDestroyed()) {
			recordEvent('warning', 'renderer-gone', { outcome: 'none', reason: details.reason });
			return;
		}
		const now = Date.now();
		const looping = lastCrash !== undefined && now - lastCrash < CRASH_LOOP_MILLISECONDS;
		lastCrash = now;
		recordEvent('warning', 'renderer-gone', {
			outcome: looping ? 'asked' : 'reloaded',
			reason: details.reason,
		});
		if (!looping) {
			window.reload();
			return;
		}
		void askAfterCrashLoop(window).then((choice) => {
			recordEvent('info', 'renderer-crash-answered', { choice });
			if (window.isDestroyed()) return;
			// A closed window opens afresh from the notification area or the Dock, never as this dead page.
			if (choice === 'reopen') window.reload();
			else window.destroy();
		});
	});
}

async function askAfterCrashLoop(window: BrowserWindow): Promise<'close' | 'reopen'> {
	const intl = createOrchbeeIntl(resolveOrchbeeLocale(app.getPreferredSystemLanguages()));
	try {
		const { response } = await dialog.showMessageBox(window, {
			buttons: [
				intl.formatMessage(desktopWindowMessages.crashedClose),
				intl.formatMessage(desktopWindowMessages.crashedReopen),
			],
			cancelId: CLOSE_BUTTON,
			defaultId: REOPEN_BUTTON,
			detail: intl.formatMessage(desktopWindowMessages.crashedDetail),
			message: intl.formatMessage(desktopWindowMessages.crashedTitle),
			noLink: true,
			type: 'warning',
		});
		return response === REOPEN_BUTTON ? 'reopen' : 'close';
	} catch {
		// Without a dialog to ask through, a window that keeps crashing is closed.
		return 'close';
	}
}
