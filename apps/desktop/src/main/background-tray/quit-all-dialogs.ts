import type { DesktopBackgroundStatusSummary } from '@orchbee/desktop-client-protocol';
import { desktopTrayMessages, type IntlShape } from '@orchbee/i18n';
import { dialog, type BrowserWindow, type MessageBoxOptions } from 'electron';

import type { DesktopQuitAllFailure } from './desktop-background-lifecycle';

const FAILURE_MESSAGES = {
	busy: desktopTrayMessages.quitAllFailureBusy,
	'not-sent': desktopTrayMessages.quitAllFailureNotSent,
	refused: desktopTrayMessages.quitAllFailureRefused,
	'stop-failed': desktopTrayMessages.quitAllFailureStopFailed,
	unconfirmed: desktopTrayMessages.quitAllFailureUnconfirmed,
	unmanaged: desktopTrayMessages.quitAllFailureUnmanaged,
} as const satisfies Readonly<Record<DesktopQuitAllFailure, unknown>>;

/**
 * Asks, in a native dialog that needs no window, whether to stop every terminal and quit. Cancel has
 * the focus and answers Esc. Resolves whether the person confirmed.
 */
export function confirmDesktopQuitAll(
	intl: IntlShape,
	applicationName: string,
	summary: DesktopBackgroundStatusSummary,
): Promise<boolean> {
	return dialog
		.showMessageBox({
			buttons: [
				intl.formatMessage(desktopTrayMessages.quitAllCancel),
				intl.formatMessage(desktopTrayMessages.quitAllConfirm),
			],
			cancelId: 0,
			defaultId: 0,
			detail: intl.formatMessage(desktopTrayMessages.quitAllDetail, {
				agents: summary.workingAgents,
				terminals: summary.runningTerminals,
			}),
			message: intl.formatMessage(desktopTrayMessages.quitAllTitle),
			noLink: true,
			title: applicationName,
			type: 'warning',
		})
		.then(
			({ response }) => response === 1,
			() => false,
		);
}

/** Reports over `window` why Quit All left everything running; resolves once it is dismissed. */
export async function reportDesktopQuitAllFailure(
	intl: IntlShape,
	applicationName: string,
	window: BrowserWindow | undefined,
	reason: DesktopQuitAllFailure,
	stopId: string,
): Promise<void> {
	const options: MessageBoxOptions = {
		buttons: [intl.formatMessage(desktopTrayMessages.quitAllFailureClose)],
		detail: intl.formatMessage(FAILURE_MESSAGES[reason], { stopId }),
		message: intl.formatMessage(desktopTrayMessages.quitAllFailureTitle),
		noLink: true,
		title: applicationName,
		type: 'warning',
	};
	await (
		window === undefined ? dialog.showMessageBox(options) : dialog.showMessageBox(window, options)
	).catch(() => undefined);
}
