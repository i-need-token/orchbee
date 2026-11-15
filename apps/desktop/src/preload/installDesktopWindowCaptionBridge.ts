import {
	DESKTOP_WINDOW_CAPTION_IPC_CHANNEL,
	DesktopWindowCaptionWindowMessageSchema,
} from '@orchbee/desktop-client-protocol';

interface DesktopWindowCaptionIpc {
	send(channel: string, symbolColor: string): void;
}

interface DesktopWindowCaptionWindow {
	addEventListener(type: 'message', listener: (event: MessageEvent<unknown>) => void): void;
}

/** Forwards each caption symbol color the page posts to its own window, and nothing else, to Main. */
export function installDesktopWindowCaptionBridge(
	ipc: DesktopWindowCaptionIpc,
	targetWindow: DesktopWindowCaptionWindow,
): void {
	targetWindow.addEventListener('message', (event) => {
		if (event.source !== targetWindow) return;
		const message = DesktopWindowCaptionWindowMessageSchema.safeParse(event.data);
		if (message.success) ipc.send(DESKTOP_WINDOW_CAPTION_IPC_CHANNEL, message.data.symbolColor);
	});
}
