import { DesktopFileDropWindowMessageSchema } from '@orchbee/desktop-client-protocol';

/** How long a terminal waits for the preload's text after a drop; it arrives in the next task. */
const FILE_DROP_WAIT_MILLISECONDS = 1000;

/**
 * Gives each terminal that takes a drop of files the text the preload posted for that drop. The
 * preload posts before the page's drop listener runs, and the message arrives once that listener
 * returns, so each wait takes the next message; one nobody waits for came from a drop no terminal
 * took.
 */
export function receiveDesktopFileDrops(targetWindow: Window): () => Promise<string | null> {
	const waiting: ((text: string | null) => void)[] = [];
	targetWindow.addEventListener('message', (event) => {
		if (event.source !== targetWindow) return;
		const message = DesktopFileDropWindowMessageSchema.safeParse(event.data);
		if (message.success) waiting.shift()?.(message.data.text);
	});
	return () =>
		new Promise((resolve) => {
			const settle = (text: string | null): void => {
				targetWindow.clearTimeout(timer);
				resolve(text);
			};
			const timer = targetWindow.setTimeout(() => {
				const index = waiting.indexOf(settle);
				if (index !== -1) waiting.splice(index, 1);
				resolve(null);
			}, FILE_DROP_WAIT_MILLISECONDS);
			waiting.push(settle);
		});
}
