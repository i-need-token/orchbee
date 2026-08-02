import type { IpcRendererEvent } from 'electron';

import {
	TERMINAL_DOCUMENT_CONTROL_IPC_CHANNEL,
	TERMINAL_DOCUMENT_CONTROL_WINDOW_MESSAGE,
	TerminalDocumentControlBootstrapSchema,
	TerminalDocumentControlWindowMessageSchema,
} from '../shared';

export interface TerminalDocumentControlIpc {
	once(channel: string, listener: (event: IpcRendererEvent, value: unknown) => void): void;
}

export interface TerminalDocumentControlWindow {
	readonly location: Pick<Location, 'origin'>;
	postMessage(message: unknown, targetOrigin: string, transfer: Transferable[]): void;
}

export function installTerminalDocumentControlBridge(
	ipc: TerminalDocumentControlIpc,
	targetWindow: TerminalDocumentControlWindow,
	security: { readonly contextIsolated: boolean; readonly sandboxed: boolean },
): void {
	ipc.once(TERMINAL_DOCUMENT_CONTROL_IPC_CHANNEL, (event, value) => {
		const bootstrap = TerminalDocumentControlBootstrapSchema.safeParse(value);
		if (!bootstrap.success || event.ports.length !== 1) return;
		const port = event.ports[0];
		if (port === undefined) return;
		targetWindow.postMessage(
			TerminalDocumentControlWindowMessageSchema.parse({
				bootstrap: bootstrap.data,
				security,
				type: TERMINAL_DOCUMENT_CONTROL_WINDOW_MESSAGE,
			}),
			targetWindow.location.origin === 'null' ? '*' : targetWindow.location.origin,
			[port],
		);
	});
}
