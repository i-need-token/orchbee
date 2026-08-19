import {
	DesktopTerminalLaunchProfileMainEventSchema,
	DesktopTerminalLaunchProfileIdSchema,
	DesktopTerminalLaunchProfileRendererCommandSchema,
	type DesktopTerminalLaunchProfileMainEvent,
} from '@orchbee/desktop-client-protocol';
import { describe, expect, it, vi } from 'vitest';

import { DesktopTerminalLaunchProfileClient } from '../DesktopTerminalLaunchProfileClient';

describe('DesktopTerminalLaunchProfileClient', () => {
	it('initializes from a correlated snapshot and applies committed mutations', async () => {
		const port = new FakeMessagePort();
		const client = new DesktopTerminalLaunchProfileClient(port.asMessagePort(), () => 'request');

		const initialization = client.initialize();
		const snapshotRequest = DesktopTerminalLaunchProfileRendererCommandSchema.parse(
			port.messages[0],
		);
		expect(snapshotRequest.type).toBe('desktop-terminal-launch-profile-snapshot-requested');
		port.emit(response(snapshotRequest.requestId));
		await expect(initialization).resolves.toEqual({ kind: 'ready' });
		expect(client.getSnapshot()).toMatchObject({
			catalog: { defaultProfileId: 'system-default' },
			failureCode: null,
			pending: false,
		});

		const update = client.selectDefault(
			DesktopTerminalLaunchProfileIdSchema.parse('system-default'),
		);
		expect(client.getSnapshot().pending).toBe(true);
		const updateRequest = DesktopTerminalLaunchProfileRendererCommandSchema.parse(port.messages[1]);
		port.emit(response(updateRequest.requestId));
		await expect(update).resolves.toMatchObject({ kind: 'completed' });
		expect(client.getSnapshot().pending).toBe(false);
	});

	it('shares one initialization request across concurrent callers', async () => {
		const port = new FakeMessagePort();
		const client = new DesktopTerminalLaunchProfileClient(port.asMessagePort(), () => 'request');

		const first = client.initialize();
		const second = client.initialize();
		expect(port.messages).toHaveLength(1);
		const snapshotRequest = DesktopTerminalLaunchProfileRendererCommandSchema.parse(
			port.messages[0],
		);
		port.emit(response(snapshotRequest.requestId));

		await expect(Promise.all([first, second])).resolves.toEqual([
			{ kind: 'ready' },
			{ kind: 'ready' },
		]);
		expect(port.messages).toHaveLength(1);
	});

	it('bounds renderer concurrency and retires on an uncorrelated event', async () => {
		const port = new FakeMessagePort();
		const client = new DesktopTerminalLaunchProfileClient(port.asMessagePort(), () => 'request');
		const initialization = client.initialize();
		const snapshotRequest = DesktopTerminalLaunchProfileRendererCommandSchema.parse(
			port.messages[0],
		);
		port.emit(response(snapshotRequest.requestId));
		await initialization;

		const first = client.reset();
		await expect(client.reset()).resolves.toMatchObject({ code: 'busy', kind: 'failed' });
		port.emit(response('unrelated:request'));
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
		const client = new DesktopTerminalLaunchProfileClient(port.asMessagePort(), () => 'request');

		await expect(client.initialize()).resolves.toEqual({ kind: 'unavailable' });
		expect(port.close).toHaveBeenCalledOnce();
	});
});

function response(requestId: string): DesktopTerminalLaunchProfileMainEvent {
	return DesktopTerminalLaunchProfileMainEventSchema.parse({
		requestId,
		result: {
			kind: 'completed',
			snapshot: {
				capabilities: { loginShell: true },
				catalog: {
					defaultProfileId: 'system-default',
					profiles: [
						{
							command: { kind: 'system-shell', login: false },
							id: 'system-default',
							name: null,
						},
					],
					version: 1,
				},
				persistenceIssue: null,
				source: 'built-in',
			},
		},
		type: 'desktop-terminal-launch-profile-responded',
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
