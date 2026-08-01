import type { IpcRendererEvent } from 'electron';

import {
	PROJECTED_ATTACHMENT_IPC_CHANNEL,
	PROJECTED_ATTACHMENT_WINDOW_MESSAGE,
	ProjectedAttachmentBootstrapSchema,
} from '../shared';

export interface ProjectedAttachmentIpc {
	once(channel: string, listener: (event: IpcRendererEvent, value: unknown) => void): void;
}

export interface ProjectedAttachmentWindow {
	readonly location: Pick<Location, 'origin'>;
	postMessage(message: unknown, targetOrigin: string, transfer: Transferable[]): void;
}

export function installProjectedAttachmentBridge(
	ipc: ProjectedAttachmentIpc,
	targetWindow: ProjectedAttachmentWindow,
	security: { readonly contextIsolated: boolean; readonly sandboxed: boolean },
): void {
	ipc.once(PROJECTED_ATTACHMENT_IPC_CHANNEL, (event, value) => {
		const bootstrap = ProjectedAttachmentBootstrapSchema.safeParse(value);
		if (!bootstrap.success || event.ports.length !== 1) return;
		const port = event.ports[0];
		if (port === undefined) return;
		targetWindow.postMessage(
			{ bootstrap: bootstrap.data, security, type: PROJECTED_ATTACHMENT_WINDOW_MESSAGE },
			targetWindow.location.origin === 'null' ? '*' : targetWindow.location.origin,
			[port],
		);
	});
}
