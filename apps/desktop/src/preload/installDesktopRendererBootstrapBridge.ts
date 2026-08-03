import {
	DESKTOP_RENDERER_BOOTSTRAP_IPC_CHANNEL,
	DESKTOP_RENDERER_BOOTSTRAP_WINDOW_MESSAGE,
	DESKTOP_RENDERER_PORT_ROLES,
	DesktopRendererBootstrapSchema,
	DesktopRendererBootstrapWindowMessageSchema,
} from '@orchbee/desktop-client-protocol';
import type { IpcRendererEvent } from 'electron';

export interface DesktopRendererBootstrapIpc {
	once(channel: string, listener: (event: IpcRendererEvent, value: unknown) => void): void;
}

export interface DesktopRendererBootstrapWindow {
	readonly location: Pick<Location, 'origin'>;
	postMessage(message: unknown, targetOrigin: string, transfer: Transferable[]): void;
}

export function installDesktopRendererBootstrapBridge(
	ipc: DesktopRendererBootstrapIpc,
	targetWindow: DesktopRendererBootstrapWindow,
	security: { readonly contextIsolated: boolean; readonly sandboxed: boolean },
): void {
	ipc.once(DESKTOP_RENDERER_BOOTSTRAP_IPC_CHANNEL, (event, value) => {
		const bootstrap = DesktopRendererBootstrapSchema.safeParse(value);
		if (!bootstrap.success || event.ports.length !== DESKTOP_RENDERER_PORT_ROLES.length) {
			closeTransferredPorts(event.ports);
			return;
		}
		const clientSessionPort = event.ports[0];
		const terminalProjectionPort = event.ports[1];
		const applicationLifetimePort = event.ports[2];
		if (
			clientSessionPort === undefined ||
			terminalProjectionPort === undefined ||
			applicationLifetimePort === undefined
		) {
			closeTransferredPorts(event.ports);
			return;
		}
		try {
			targetWindow.postMessage(
				DesktopRendererBootstrapWindowMessageSchema.parse({
					bootstrap: bootstrap.data,
					security,
					type: DESKTOP_RENDERER_BOOTSTRAP_WINDOW_MESSAGE,
				}),
				targetWindow.location.origin === 'null' ? '*' : targetWindow.location.origin,
				[clientSessionPort, terminalProjectionPort, applicationLifetimePort],
			);
		} catch {
			closeTransferredPorts(event.ports);
		}
	});
}

function closeTransferredPorts(ports: readonly MessagePort[]): void {
	for (const port of ports) port.close();
}
