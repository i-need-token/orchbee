import type { IpcRendererEvent } from 'electron';

import {
	WORKBENCH_TOPOLOGY_CONTROL_IPC_CHANNEL,
	WORKBENCH_TOPOLOGY_CONTROL_WINDOW_MESSAGE,
	WorkbenchTopologyControlBootstrapSchema,
	WorkbenchTopologyControlWindowMessageSchema,
} from '../shared';

export interface WorkbenchTopologyControlIpc {
	once(channel: string, listener: (event: IpcRendererEvent, value: unknown) => void): void;
}

export interface WorkbenchTopologyControlWindow {
	readonly location: Pick<Location, 'origin'>;
	postMessage(message: unknown, targetOrigin: string, transfer: Transferable[]): void;
}

export function installWorkbenchTopologyControlBridge(
	ipc: WorkbenchTopologyControlIpc,
	targetWindow: WorkbenchTopologyControlWindow,
	security: { readonly contextIsolated: boolean; readonly sandboxed: boolean },
): void {
	ipc.once(WORKBENCH_TOPOLOGY_CONTROL_IPC_CHANNEL, (event, value) => {
		const bootstrap = WorkbenchTopologyControlBootstrapSchema.safeParse(value);
		if (!bootstrap.success || event.ports.length !== 1) return;
		const port = event.ports[0];
		if (port === undefined) return;
		targetWindow.postMessage(
			WorkbenchTopologyControlWindowMessageSchema.parse({
				security,
				type: WORKBENCH_TOPOLOGY_CONTROL_WINDOW_MESSAGE,
			}),
			targetWindow.location.origin === 'null' ? '*' : targetWindow.location.origin,
			[port],
		);
	});
}
