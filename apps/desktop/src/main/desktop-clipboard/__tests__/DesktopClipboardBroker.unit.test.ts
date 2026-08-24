import {
	DesktopClipboardMainEventSchema,
	DesktopClipboardRendererCommandSchema,
	type DesktopClipboardRendererCommand,
} from '@orchbee/desktop-client-protocol';
import type { MessageEvent, MessagePortMain } from 'electron';
import { describe, expect, it, vi } from 'vitest';

import { DesktopClipboardBroker } from '../DesktopClipboardBroker';

describe('DesktopClipboardBroker', () => {
	it('reads one bounded plain-text clipboard value in Main', async () => {
		const port = new FakeMessagePort();
		const readText = vi.fn<() => string>(() => 'selected text');
		const broker = new DesktopClipboardBroker(port.asMessagePortMain(), { readText });
		broker.start();

		port.emit(readRequest('clipboard:1'));
		await vi.waitFor(() => expect(port.messages).toHaveLength(1));

		expect(DesktopClipboardMainEventSchema.parse(port.messages[0])).toEqual({
			requestId: 'clipboard:1',
			result: { kind: 'completed', text: 'selected text' },
			type: 'desktop-clipboard-read-text-responded',
		});
		expect(readText).toHaveBeenCalledOnce();
		broker.dispose();
	});

	it('bounds read failures and retires malformed renderer input', async () => {
		const failedPort = new FakeMessagePort();
		const failedBroker = new DesktopClipboardBroker(failedPort.asMessagePortMain(), {
			readText: () => {
				throw new Error('clipboard unavailable');
			},
		});
		failedBroker.start();
		failedPort.emit(readRequest('clipboard:failed'));
		await vi.waitFor(() => expect(failedPort.messages).toHaveLength(1));
		expect(DesktopClipboardMainEventSchema.parse(failedPort.messages[0])).toMatchObject({
			result: { code: 'clipboard-read-failed', kind: 'failed' },
		});

		const malformedPort = new FakeMessagePort();
		const malformedBroker = new DesktopClipboardBroker(malformedPort.asMessagePortMain(), {
			readText: () => 'unused',
		});
		malformedBroker.start();
		malformedPort.emit({ type: 'desktop-clipboard-write-text-requested' });
		expect(malformedPort.close).toHaveBeenCalledOnce();
	});
});

function readRequest(requestId: string): DesktopClipboardRendererCommand {
	return DesktopClipboardRendererCommandSchema.parse({
		requestId,
		type: 'desktop-clipboard-read-text-requested',
	});
}

class FakeMessagePort {
	readonly #closeListeners = new Set<() => void>();
	readonly close = vi.fn<() => void>();
	readonly messages: unknown[] = [];
	readonly #messageListeners = new Set<(event: MessageEvent) => void>();

	asMessagePortMain(): MessagePortMain {
		return this as unknown as MessagePortMain;
	}

	emit(data: unknown, ports: readonly unknown[] = []): void {
		for (const listener of this.#messageListeners) {
			listener({ data, ports } as unknown as MessageEvent);
		}
	}

	off(event: string, listener: (...arguments_: never[]) => void): void {
		if (event === 'message') {
			this.#messageListeners.delete(listener as unknown as (event: MessageEvent) => void);
		} else if (event === 'close') {
			this.#closeListeners.delete(listener as unknown as () => void);
		}
	}

	on(event: string, listener: (...arguments_: never[]) => void): void {
		if (event === 'message') {
			this.#messageListeners.add(listener as unknown as (event: MessageEvent) => void);
		} else if (event === 'close') {
			this.#closeListeners.add(listener as unknown as () => void);
		}
	}

	postMessage(message: unknown): void {
		this.messages.push(message);
	}

	start(): void {}
}
