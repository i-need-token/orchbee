import type { IpcRendererEvent } from 'electron';

import {
	DESKTOP_CLIENT_SESSION_IPC_CHANNEL,
	DESKTOP_CLIENT_SESSION_WINDOW_MESSAGE,
	DesktopClientSessionBootstrapSchema,
	DesktopClientSessionWindowMessageSchema,
} from '../shared';

export interface DesktopClientSessionIpc {
	once(channel: string, listener: (event: IpcRendererEvent, value: unknown) => void): void;
}

export interface DesktopClientSessionWindow {
	readonly location: Pick<Location, 'origin'>;
	postMessage(message: unknown, targetOrigin: string, transfer: Transferable[]): void;
}

export function installDesktopClientSessionBridge(
	ipc: DesktopClientSessionIpc,
	targetWindow: DesktopClientSessionWindow,
	security: { readonly contextIsolated: boolean; readonly sandboxed: boolean },
): void {
	ipc.once(DESKTOP_CLIENT_SESSION_IPC_CHANNEL, (event, value) => {
		const bootstrap = DesktopClientSessionBootstrapSchema.safeParse(value);
		if (!bootstrap.success || event.ports.length !== 1) return;
		const port = event.ports[0];
		if (port === undefined) return;
		targetWindow.postMessage(
			DesktopClientSessionWindowMessageSchema.parse({
				bootstrap: bootstrap.data,
				security,
				type: DESKTOP_CLIENT_SESSION_WINDOW_MESSAGE,
			}),
			targetWindow.location.origin === 'null' ? '*' : targetWindow.location.origin,
			[port],
		);
	});
}
