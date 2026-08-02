import type { IpcRendererEvent } from 'electron';

import {
	TERMINAL_PROJECTION_CONTROL_IPC_CHANNEL,
	TERMINAL_PROJECTION_CONTROL_WINDOW_MESSAGE,
	TerminalProjectionControlBootstrapSchema,
	TerminalProjectionControlWindowMessageSchema,
} from '../shared';

export interface TerminalProjectionControlIpc {
	once(channel: string, listener: (event: IpcRendererEvent, value: unknown) => void): void;
}

export interface TerminalProjectionControlWindow {
	readonly location: Pick<Location, 'origin'>;
	postMessage(message: unknown, targetOrigin: string, transfer: Transferable[]): void;
}

export function installTerminalProjectionControlBridge(
	ipc: TerminalProjectionControlIpc,
	targetWindow: TerminalProjectionControlWindow,
	security: { readonly contextIsolated: boolean; readonly sandboxed: boolean },
): void {
	ipc.once(TERMINAL_PROJECTION_CONTROL_IPC_CHANNEL, (event, value) => {
		const bootstrap = TerminalProjectionControlBootstrapSchema.safeParse(value);
		if (!bootstrap.success || event.ports.length !== 1) return;
		const port = event.ports[0];
		if (port === undefined) return;
		targetWindow.postMessage(
			TerminalProjectionControlWindowMessageSchema.parse({
				security,
				type: TERMINAL_PROJECTION_CONTROL_WINDOW_MESSAGE,
			}),
			targetWindow.location.origin === 'null' ? '*' : targetWindow.location.origin,
			[port],
		);
	});
}
