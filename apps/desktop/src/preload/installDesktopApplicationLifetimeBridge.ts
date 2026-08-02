import type { IpcRendererEvent } from 'electron';

import {
	DESKTOP_APPLICATION_LIFETIME_IPC_CHANNEL,
	DESKTOP_APPLICATION_LIFETIME_WINDOW_MESSAGE,
	DesktopApplicationLifetimeBootstrapSchema,
	DesktopApplicationLifetimeWindowMessageSchema,
} from '../shared';

export interface DesktopApplicationLifetimeIpc {
	once(channel: string, listener: (event: IpcRendererEvent, value: unknown) => void): void;
}

export interface DesktopApplicationLifetimeWindow {
	readonly location: Pick<Location, 'origin'>;
	postMessage(message: unknown, targetOrigin: string, transfer: Transferable[]): void;
}

export function installDesktopApplicationLifetimeBridge(
	ipc: DesktopApplicationLifetimeIpc,
	targetWindow: DesktopApplicationLifetimeWindow,
	security: { readonly contextIsolated: boolean; readonly sandboxed: boolean },
): void {
	ipc.once(DESKTOP_APPLICATION_LIFETIME_IPC_CHANNEL, (event, value) => {
		const bootstrap = DesktopApplicationLifetimeBootstrapSchema.safeParse(value);
		if (!bootstrap.success || event.ports.length !== 1) return;
		const port = event.ports[0];
		if (port === undefined) return;
		targetWindow.postMessage(
			DesktopApplicationLifetimeWindowMessageSchema.parse({
				security,
				type: DESKTOP_APPLICATION_LIFETIME_WINDOW_MESSAGE,
			}),
			targetWindow.location.origin === 'null' ? '*' : targetWindow.location.origin,
			[port],
		);
	});
}
