import { DesktopBackgroundStatusRendererMessageSchema } from '@orchbee/desktop-client-protocol';
import type { DesktopBackgroundStatusMainMessage } from '@orchbee/desktop-client-protocol';
import type { MessagePortMain } from 'electron';

import type { DesktopDiagnosticRecorder } from '../diagnostics';
import type { DesktopBackgroundStatusSink } from './DesktopBackgroundRuntime';

/** Main's end of one document's background-status port. */
export interface DesktopBackgroundStatusPort {
	close(): void;
	/** Asks the page to stop the Session under `stopId`; false once the port is closed or unserved. */
	requestSessionStop(stopId: string): boolean;
}

/**
 * Serves one document's background-status port: each message is parsed before it reaches `sink`,
 * and one that does not parse is dropped; the first such of the port is recorded. Without a sink,
 * where no icon is kept, the port is closed and the page's reports go nowhere.
 */
export function serveDesktopBackgroundStatus(
	port: MessagePortMain,
	sink: DesktopBackgroundStatusSink | undefined,
	recordEvent: DesktopDiagnosticRecorder,
): DesktopBackgroundStatusPort {
	if (sink === undefined) {
		port.close();
		return { close: () => undefined, requestSessionStop: () => false };
	}
	let open = true;
	let dropRecorded = false;
	port.on('message', ({ data }) => {
		if (!open) return;
		const message = DesktopBackgroundStatusRendererMessageSchema.safeParse(data);
		if (!message.success) {
			if (!dropRecorded) {
				dropRecorded = true;
				recordEvent('warning', 'background-status-message-dropped');
			}
			return;
		}
		switch (message.data.type) {
			case 'desktop-background-status-changed':
				sink.summaryChanged(message.data.summary);
				break;
			case 'desktop-background-window-requested':
				sink.windowRequested();
				break;
			case 'desktop-background-quit-all-requested':
				sink.quitAllRequested();
				break;
			case 'desktop-session-stop-answered':
				sink.sessionStopAnswered(message.data.stopId, message.data.answer);
				break;
		}
	});
	port.start();
	return {
		close: () => {
			open = false;
			port.close();
		},
		requestSessionStop: (stopId) => {
			if (!open) return false;
			const message: DesktopBackgroundStatusMainMessage = {
				stopId,
				type: 'desktop-session-stop-requested',
			};
			port.postMessage(message);
			return true;
		},
	};
}
