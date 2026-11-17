import {
	DESKTOP_WINDOW_FAULT_IPC_CHANNEL,
	DesktopWindowFaultWindowMessageSchema,
} from '@orchbee/desktop-client-protocol';

interface DesktopWindowFaultIpc {
	send(channel: string, site: string): void;
}

interface DesktopWindowFaultWindow {
	addEventListener(type: 'message', listener: (event: MessageEvent<unknown>) => void): void;
}

/** Forwards the site of each fault the page posts to its own window, and nothing else, to Main. */
export function installDesktopWindowFaultBridge(
	ipc: DesktopWindowFaultIpc,
	targetWindow: DesktopWindowFaultWindow,
): void {
	targetWindow.addEventListener('message', (event) => {
		if (event.source !== targetWindow) return;
		const message = DesktopWindowFaultWindowMessageSchema.safeParse(event.data);
		if (message.success) ipc.send(DESKTOP_WINDOW_FAULT_IPC_CHANNEL, message.data.site);
	});
}
