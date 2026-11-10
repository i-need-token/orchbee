import { DesktopBackgroundStatusRendererMessageSchema } from '@orchbee/desktop-client-protocol';
import type { MessagePortMain } from 'electron';

import type { DesktopBackgroundStatusSink } from './DesktopBackgroundRuntime';

/**
 * Serves one document's background-status port: each message is parsed before it reaches `sink`,
 * and one that does not parse is dropped. Without a sink, where no icon is kept, the port is closed
 * and the page's reports go nowhere. Returns what closes the port.
 */
export function serveDesktopBackgroundStatus(
	port: MessagePortMain,
	sink: DesktopBackgroundStatusSink | undefined,
): () => void {
	if (sink === undefined) {
		port.close();
		return () => undefined;
	}
	let open = true;
	port.on('message', ({ data }) => {
		if (!open) return;
		const message = DesktopBackgroundStatusRendererMessageSchema.safeParse(data);
		if (!message.success) return;
		if (message.data.type === 'desktop-background-status-changed') {
			sink.summaryChanged(message.data.summary);
		} else {
			sink.windowRequested();
		}
	});
	port.start();
	return () => {
		open = false;
		port.close();
	};
}
