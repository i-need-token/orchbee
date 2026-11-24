import {
	DESKTOP_MACHINE_SESSION_IPC_CHANNEL,
	DESKTOP_MACHINE_SESSION_REQUEST_IPC_CHANNEL,
	DESKTOP_MACHINE_SESSION_WINDOW_MESSAGE,
	DesktopMachineSessionReplySchema,
	DesktopMachineSessionWindowRequestSchema,
	type DesktopMachineSessionWindowMessage,
} from '@orchbee/desktop-client-protocol';
import type { IpcRendererEvent } from 'electron';

export interface DesktopMachineSessionIpc {
	on(channel: string, listener: (event: IpcRendererEvent, reply: unknown) => void): void;
	send(channel: string, request: unknown): void;
}

export interface DesktopMachineSessionWindow {
	addEventListener(type: 'message', listener: (event: MessageEvent<unknown>) => void): void;
	readonly location: Pick<Location, 'origin'>;
	postMessage(message: unknown, targetOrigin: string, transfer: Transferable[]): void;
}

/**
 * Carries the page's remote Session requests to Main, naming the document Main connected, which
 * the page never sees, and Main's answers back. An answer for another document, or whose ports do
 * not match its outcome, is dropped and its ports closed.
 */
export function installDesktopMachineSessionBridge(
	ipc: DesktopMachineSessionIpc,
	targetWindow: DesktopMachineSessionWindow,
	connectedDocument: () => string | undefined,
): void {
	targetWindow.addEventListener('message', (event) => {
		const document = connectedDocument();
		if (event.source !== targetWindow || document === undefined) return;
		const request = DesktopMachineSessionWindowRequestSchema.safeParse(event.data);
		if (!request.success) return;
		ipc.send(DESKTOP_MACHINE_SESSION_REQUEST_IPC_CHANNEL, {
			document,
			machine: request.data.machine,
			request: request.data.request,
		});
	});
	ipc.on(DESKTOP_MACHINE_SESSION_IPC_CHANNEL, (event, value) => {
		const reply = DesktopMachineSessionReplySchema.safeParse(value);
		if (
			!reply.success ||
			reply.data.document !== connectedDocument() ||
			event.ports.length !== (reply.data.outcome === 'admitted' ? 1 : 0)
		) {
			for (const unused of event.ports) unused.close();
			return;
		}
		const type = DESKTOP_MACHINE_SESSION_WINDOW_MESSAGE;
		const { request } = reply.data;
		const message: DesktopMachineSessionWindowMessage =
			reply.data.outcome === 'admitted'
				? { bootstrap: reply.data.bootstrap, outcome: 'admitted', request, type }
				: { outcome: 'refused', reason: reply.data.reason, request, type };
		targetWindow.postMessage(
			message,
			targetWindow.location.origin === 'null' ? '*' : targetWindow.location.origin,
			[...event.ports],
		);
	});
}
