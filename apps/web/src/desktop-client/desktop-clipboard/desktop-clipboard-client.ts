import {
	DesktopClipboardMainEventSchema,
	DesktopClipboardRendererCommandSchema,
	DesktopClipboardRequestIdSchema,
	type DesktopClipboardReadResult,
	type DesktopClipboardRequestId,
} from '@orchbee/desktop-client-protocol';

import { createBrowserRandomIdentifier } from '../../browser-random-identifier';
import type { ProjectedTerminalPlatformEffects } from '../../workbench-client';

interface PendingClipboardRead {
	resolve(result: DesktopClipboardReadResult): void;
}

interface DesktopClipboardClientPort {
	dispose(): void;
	readText(): Promise<DesktopClipboardReadResult>;
}

export class DesktopClipboardClient implements DesktopClipboardClientPort {
	readonly #createRequestId: () => DesktopClipboardRequestId;
	#disposed = false;
	readonly #pending = new Map<DesktopClipboardRequestId, PendingClipboardRead>();
	readonly #port: MessagePort;
	#sequence = 0;

	constructor(port: MessagePort, createRequestId: () => string = defaultRequestId) {
		this.#port = port;
		this.#createRequestId = () =>
			DesktopClipboardRequestIdSchema.parse(`${createRequestId()}:${++this.#sequence}`);
		port.addEventListener('message', this.#handleMessage);
		port.addEventListener('messageerror', this.#handleMessageError);
		port.start();
	}

	readText(): Promise<DesktopClipboardReadResult> {
		if (this.#disposed) {
			return Promise.resolve({ code: 'desktop-quiescing', kind: 'failed' });
		}
		const requestId = this.#createRequestId();
		const command = DesktopClipboardRendererCommandSchema.parse({
			requestId,
			type: 'desktop-clipboard-read-text-requested',
		});
		const result = new Promise<DesktopClipboardReadResult>((resolve) => {
			this.#pending.set(requestId, { resolve });
		});
		try {
			this.#port.postMessage(command);
		} catch {
			this.#retire(true);
		}
		return result;
	}

	dispose(): void {
		this.#retire(true);
	}

	readonly #handleMessage = (event: MessageEvent<unknown>): void => {
		if (this.#disposed || event.ports.length !== 0) {
			this.#retire(true);
			return;
		}
		const parsed = DesktopClipboardMainEventSchema.safeParse(event.data);
		if (!parsed.success) {
			this.#retire(true);
			return;
		}
		const pending = this.#pending.get(parsed.data.requestId);
		if (pending === undefined) {
			this.#retire(true);
			return;
		}
		this.#pending.delete(parsed.data.requestId);
		pending.resolve(parsed.data.result);
	};

	readonly #handleMessageError = (): void => {
		this.#retire(true);
	};

	#retire(closePort: boolean): void {
		if (this.#disposed) return;
		this.#disposed = true;
		this.#port.removeEventListener('message', this.#handleMessage);
		this.#port.removeEventListener('messageerror', this.#handleMessageError);
		if (closePort) this.#port.close();
		for (const pending of this.#pending.values()) {
			pending.resolve({ code: 'desktop-clipboard-unavailable', kind: 'failed' });
		}
		this.#pending.clear();
	}
}

export function createDesktopTerminalPlatformEffects(
	clipboardClient: DesktopClipboardClientPort,
): ProjectedTerminalPlatformEffects {
	return {
		readClipboardText: async () => {
			const result = await clipboardClient.readText();
			return result.kind === 'completed' ? result : { ...result, retryable: false };
		},
	};
}

function defaultRequestId(): string {
	return `clipboard:${createBrowserRandomIdentifier()}`;
}
