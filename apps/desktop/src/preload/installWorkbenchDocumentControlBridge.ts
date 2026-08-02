import type { IpcRendererEvent } from 'electron';

import {
	WORKBENCH_DOCUMENT_CONTROL_IPC_CHANNEL,
	WORKBENCH_DOCUMENT_CONTROL_WINDOW_MESSAGE,
	WorkbenchDocumentControlBootstrapSchema,
	WorkbenchDocumentControlWindowMessageSchema,
} from '../shared';

export interface WorkbenchDocumentControlIpc {
	once(channel: string, listener: (event: IpcRendererEvent, value: unknown) => void): void;
}

export interface WorkbenchDocumentControlWindow {
	readonly location: Pick<Location, 'origin'>;
	postMessage(message: unknown, targetOrigin: string, transfer: Transferable[]): void;
}

export function installWorkbenchDocumentControlBridge(
	ipc: WorkbenchDocumentControlIpc,
	targetWindow: WorkbenchDocumentControlWindow,
	security: { readonly contextIsolated: boolean; readonly sandboxed: boolean },
): void {
	ipc.once(WORKBENCH_DOCUMENT_CONTROL_IPC_CHANNEL, (event, value) => {
		const bootstrap = WorkbenchDocumentControlBootstrapSchema.safeParse(value);
		if (!bootstrap.success || event.ports.length !== 1) return;
		const port = event.ports[0];
		if (port === undefined) return;
		targetWindow.postMessage(
			WorkbenchDocumentControlWindowMessageSchema.parse({
				security,
				type: WORKBENCH_DOCUMENT_CONTROL_WINDOW_MESSAGE,
			}),
			targetWindow.location.origin === 'null' ? '*' : targetWindow.location.origin,
			[port],
		);
	});
}
