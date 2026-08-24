import {
	DesktopClipboardMainEventSchema,
	DesktopClipboardRendererCommandSchema,
	DesktopClipboardTextSchema,
	type DesktopClipboardReadResult,
	type DesktopClipboardRequestId,
} from '@orchbee/desktop-client-protocol';
import type { MessageEvent, MessagePortMain } from 'electron';

interface DesktopClipboardBrokerClipboard {
	readText(): Promise<string> | string;
}

export class DesktopClipboardBroker {
	#activeRequestId: DesktopClipboardRequestId | undefined;
	readonly #clipboard: DesktopClipboardBrokerClipboard;
	#disposed = false;
	readonly #port: MessagePortMain;
	#started = false;

	constructor(port: MessagePortMain, clipboard: DesktopClipboardBrokerClipboard) {
		this.#clipboard = clipboard;
		this.#port = port;
	}

	start(): void {
		if (this.#started || this.#disposed) return;
		this.#started = true;
		this.#port.on('message', this.#handleMessage);
		this.#port.on('close', this.#handleClose);
		this.#port.start();
	}

	dispose(): void {
		this.#retire(true);
	}

	readonly #handleClose = (): void => {
		this.#retire(false);
	};

	readonly #handleMessage = (event: MessageEvent): void => {
		if (event.ports.length !== 0) {
			this.#retire(true);
			return;
		}
		const parsed = DesktopClipboardRendererCommandSchema.safeParse(event.data);
		if (!parsed.success) {
			this.#retire(true);
			return;
		}
		const command = parsed.data;
		if (this.#activeRequestId !== undefined) {
			if (this.#activeRequestId !== command.requestId) {
				this.#post(command.requestId, {
					code: 'desktop-clipboard-unavailable',
					kind: 'failed',
				});
			}
			return;
		}
		this.#activeRequestId = command.requestId;
		void this.#read(command.requestId).finally(() => {
			if (this.#activeRequestId === command.requestId) this.#activeRequestId = undefined;
		});
	};

	async #read(requestId: DesktopClipboardRequestId): Promise<void> {
		let text: string;
		try {
			text = await this.#clipboard.readText();
		} catch {
			this.#post(requestId, { code: 'clipboard-read-failed', kind: 'failed' });
			return;
		}
		const parsed = DesktopClipboardTextSchema.safeParse(text);
		this.#post(
			requestId,
			parsed.success
				? { kind: 'completed', text: parsed.data }
				: { code: 'clipboard-text-invalid', kind: 'failed' },
		);
	}

	#post(requestId: DesktopClipboardRequestId, result: DesktopClipboardReadResult): void {
		if (this.#disposed) return;
		try {
			this.#port.postMessage(
				DesktopClipboardMainEventSchema.parse({
					requestId,
					result,
					type: 'desktop-clipboard-read-text-responded',
				}),
			);
		} catch {
			this.#retire(true);
		}
	}

	#retire(closePort: boolean): void {
		if (this.#disposed) return;
		this.#disposed = true;
		this.#activeRequestId = undefined;
		this.#port.off('message', this.#handleMessage);
		this.#port.off('close', this.#handleClose);
		if (closePort) this.#port.close();
	}
}
