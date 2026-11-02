import {
	DESKTOP_SERVER_CONNECTION_IPC_CHANNEL,
	DESKTOP_SERVER_CONNECTION_PORT_ROLES,
	DESKTOP_SERVER_CONNECTION_REQUEST_IPC_CHANNEL,
	DESKTOP_SERVER_CONNECTION_WINDOW_MESSAGE,
	DesktopServerConnectionReplySchema,
	DesktopServerConnectionWindowMessageSchema,
	DesktopServerConnectionWindowRequestSchema,
} from '@orchbee/desktop-client-protocol';
import type { IpcRendererEvent } from 'electron';

export interface DesktopServerConnectionIpc {
	on(channel: string, listener: (event: IpcRendererEvent, reply: unknown) => void): void;
	send(channel: string, document: string): void;
}

export interface DesktopServerConnectionWindow {
	addEventListener(type: 'message', listener: (event: MessageEvent<unknown>) => void): void;
	readonly location: Pick<Location, 'origin'>;
	postMessage(message: unknown, targetOrigin: string, transfer: Transferable[]): void;
}

/**
 * Carries the page's request to Main and Main's ports back; each document asks once and receives
 * exactly one set, one port for each role in order.
 */
export function installDesktopServerConnectionBridge(
	ipc: DesktopServerConnectionIpc,
	targetWindow: DesktopServerConnectionWindow,
	security: { readonly contextIsolated: boolean; readonly sandboxed: boolean },
	createDocumentId: () => string = () => crypto.randomUUID(),
): void {
	let asked = false;
	let awaitedDocument: string | undefined;
	targetWindow.addEventListener('message', (event) => {
		if (asked || event.source !== targetWindow) return;
		if (!DesktopServerConnectionWindowRequestSchema.safeParse(event.data).success) return;
		asked = true;
		awaitedDocument = createDocumentId();
		ipc.send(DESKTOP_SERVER_CONNECTION_REQUEST_IPC_CHANNEL, awaitedDocument);
	});
	ipc.on(DESKTOP_SERVER_CONNECTION_IPC_CHANNEL, (event, value) => {
		const reply = DesktopServerConnectionReplySchema.safeParse(value);
		const message = DesktopServerConnectionWindowMessageSchema.safeParse({
			bootstrap: reply.data?.bootstrap,
			security,
			type: DESKTOP_SERVER_CONNECTION_WINDOW_MESSAGE,
		});
		if (
			event.ports.length !== DESKTOP_SERVER_CONNECTION_PORT_ROLES.length ||
			!message.success ||
			reply.data?.document !== awaitedDocument
		) {
			for (const unused of event.ports) unused.close();
			return;
		}
		awaitedDocument = undefined;
		targetWindow.postMessage(
			message.data,
			targetWindow.location.origin === 'null' ? '*' : targetWindow.location.origin,
			[...event.ports],
		);
	});
}
