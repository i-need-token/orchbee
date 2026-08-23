import {
	DesktopTerminalDefaultSettingsMainEventSchema,
	DesktopTerminalDefaultSettingsRendererCommandSchema,
	type DesktopTerminalDefaultSettingsMainEvent,
} from '@orchbee/desktop-client-protocol';
import { describe, expect, it, vi } from 'vitest';

import { DesktopTerminalDefaultSettingsClient } from '../DesktopTerminalDefaultSettingsClient';

describe('DesktopTerminalDefaultSettingsClient', () => {
	it('initializes from a correlated snapshot and applies committed updates', async () => {
		const port = new FakeMessagePort();
		const client = new DesktopTerminalDefaultSettingsClient(port.asMessagePort(), () => 'request');

		const initialization = client.initialize();
		const snapshotRequest = DesktopTerminalDefaultSettingsRendererCommandSchema.parse(
			port.messages[0],
		);
		port.emit(response(snapshotRequest.requestId, 10_000_000));
		await expect(initialization).resolves.toEqual({ kind: 'ready' });
		expect(client.getSnapshot()).toMatchObject({
			configuration: { scrollbackBytes: 10_000_000 },
			failureCode: null,
			pending: false,
		});

		const update = client.update({ scrollbackBytes: 25_000_000 });
		expect(client.getSnapshot().pending).toBe(true);
		const updateRequest = DesktopTerminalDefaultSettingsRendererCommandSchema.parse(
			port.messages[1],
		);
		expect(updateRequest).toMatchObject({ configuration: { scrollbackBytes: 25_000_000 } });
		port.emit(response(updateRequest.requestId, 25_000_000));
		await expect(update).resolves.toMatchObject({ kind: 'completed' });
		expect(client.getSnapshot()).toMatchObject({
			configuration: { scrollbackBytes: 25_000_000 },
			pending: false,
		});
	});

	it('bounds concurrency and retires on an uncorrelated event', async () => {
		const port = new FakeMessagePort();
		const client = new DesktopTerminalDefaultSettingsClient(port.asMessagePort(), () => 'request');
		const initialization = client.initialize();
		const request = DesktopTerminalDefaultSettingsRendererCommandSchema.parse(port.messages[0]);
		port.emit(response(request.requestId, 10_000_000));
		await initialization;

		const first = client.update({ scrollbackBytes: 20_000_000 });
		await expect(client.update({ scrollbackBytes: 30_000_000 })).resolves.toMatchObject({
			code: 'busy',
			kind: 'failed',
		});
		port.emit(response('unrelated:request', 20_000_000));
		await expect(first).resolves.toEqual({ kind: 'unavailable' });
		expect(client.getSnapshot()).toMatchObject({
			failureCode: 'control-unavailable',
			pending: false,
		});
		expect(port.close).toHaveBeenCalledOnce();
	});

	it('fails initialization closed when posting is unavailable', async () => {
		const port = new FakeMessagePort();
		port.postMessage.mockImplementationOnce(() => {
			throw new Error('detached');
		});
		const client = new DesktopTerminalDefaultSettingsClient(port.asMessagePort(), () => 'request');

		await expect(client.initialize()).resolves.toEqual({ kind: 'unavailable' });
		expect(port.close).toHaveBeenCalledOnce();
	});
});

function response(
	requestId: string,
	scrollbackBytes: number,
): DesktopTerminalDefaultSettingsMainEvent {
	return DesktopTerminalDefaultSettingsMainEventSchema.parse({
		requestId,
		result: {
			kind: 'completed',
			snapshot: {
				configuration: { scrollbackBytes },
				persistenceIssue: null,
				source: 'stored',
			},
		},
		type: 'desktop-terminal-default-settings-responded',
	});
}

class FakeMessagePort {
	readonly close = vi.fn<() => void>();
	readonly messages: unknown[] = [];
	readonly #messageListeners = new Set<(event: MessageEvent<unknown>) => void>();
	readonly postMessage = vi.fn<(message: unknown) => void>((message) => {
		this.messages.push(message);
	});

	addEventListener(type: string, listener: EventListenerOrEventListenerObject): void {
		if (type === 'message') {
			this.#messageListeners.add(listener as (event: MessageEvent<unknown>) => void);
		}
	}

	asMessagePort(): MessagePort {
		return this as unknown as MessagePort;
	}

	emit(data: unknown): void {
		for (const listener of this.#messageListeners) {
			listener({ data, ports: [] } as unknown as MessageEvent<unknown>);
		}
	}

	removeEventListener(type: string, listener: EventListenerOrEventListenerObject): void {
		if (type === 'message') {
			this.#messageListeners.delete(listener as (event: MessageEvent<unknown>) => void);
		}
	}

	start(): void {}
}
