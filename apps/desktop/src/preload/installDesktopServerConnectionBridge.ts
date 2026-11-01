import {
	DESKTOP_SERVER_CONNECTION_IPC_CHANNEL,
	DESKTOP_SERVER_CONNECTION_WINDOW_MESSAGE,
	DesktopServerConnectionWindowMessageSchema,
} from '@orchbee/desktop-client-protocol';
import type { IpcRendererEvent } from 'electron';

export interface DesktopServerConnectionIpc {
	once(channel: string, listener: (event: IpcRendererEvent) => void): void;
}

export interface DesktopServerConnectionWindow {
	readonly location: Pick<Location, 'origin'>;
	postMessage(message: unknown, targetOrigin: string, transfer: Transferable[]): void;
}

/** Hands Main's connection port to the page's world; each document receives exactly one. */
export function installDesktopServerConnectionBridge(
	ipc: DesktopServerConnectionIpc,
	targetWindow: DesktopServerConnectionWindow,
	security: { readonly contextIsolated: boolean; readonly sandboxed: boolean },
): void {
	ipc.once(DESKTOP_SERVER_CONNECTION_IPC_CHANNEL, (event) => {
		const port = event.ports[0];
		const message = DesktopServerConnectionWindowMessageSchema.safeParse({
			security,
			type: DESKTOP_SERVER_CONNECTION_WINDOW_MESSAGE,
		});
		if (port === undefined || event.ports.length !== 1 || !message.success) {
			for (const unused of event.ports) unused.close();
			return;
		}
		targetWindow.postMessage(
			message.data,
			targetWindow.location.origin === 'null' ? '*' : targetWindow.location.origin,
			[port],
		);
	});
}
