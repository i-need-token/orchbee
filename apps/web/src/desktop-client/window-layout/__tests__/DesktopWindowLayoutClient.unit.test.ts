import {
	DesktopWindowLayoutMainEventSchema,
	DesktopWindowLayoutRendererCommandSchema,
	type DesktopWindowLayoutMainEvent,
} from '@orchbee/desktop-client-protocol';
import { describe, expect, it, vi } from 'vitest';

import { DesktopWindowLayoutClient } from '../DesktopWindowLayoutClient';

describe('DesktopWindowLayoutClient', () => {
	it('initializes from the first pushed window projection', async () => {
		const port = new FakeMessagePort();
		const client = new DesktopWindowLayoutClient(port.asMessagePort(), () => 'layout-client');

		const initialization = client.initialize();
		port.emit(snapshotEvent('1'));

		await expect(initialization).resolves.toEqual({ kind: 'ready' });
		expect(client.getSnapshot()).toMatchObject({
			currentWindowId: 'desktop-window-1',
			terminalContexts: [{ tabIds: ['tab-1'] }],
		});
	});

	it('correlates serialized Tab moves and reports local concurrency', async () => {
		const port = new FakeMessagePort();
		const client = new DesktopWindowLayoutClient(port.asMessagePort(), () => 'layout-client');
		port.emit(snapshotEvent('1'));

		const first = client.moveTabs('context-1', ['tab-1'], { kind: 'new' });
		const command = DesktopWindowLayoutRendererCommandSchema.parse(port.messages[0]);
		if (command.type !== 'desktop-window-layout-move-tabs-requested') {
			throw new Error('Expected a Tab-move request.');
		}
		await expect(client.moveGroup('context-1', 'group-1', { kind: 'new' })).resolves.toEqual({
			code: 'busy',
			kind: 'failed',
		});
		port.emit(
			DesktopWindowLayoutMainEventSchema.parse({
				requestId: command.requestId,
				result: { destinationWindowId: 'desktop-window-2', kind: 'completed' },
				type: 'desktop-window-layout-operation-responded',
			}),
		);

		await expect(first).resolves.toEqual({
			destinationWindowId: 'desktop-window-2',
			kind: 'completed',
		});
	});

	it('tracks one cross-window drag target through settlement', async () => {
		const port = new FakeMessagePort();
		const client = new DesktopWindowLayoutClient(port.asMessagePort(), () => 'layout-client');
		port.emit(snapshotEvent('1'));

		const sessionId = client.startDrag('context-1', ['tab-1']);
		if (sessionId === null) throw new Error('Expected the drag to start.');
		port.emit(
			DesktopWindowLayoutMainEventSchema.parse({
				sessionId,
				target: { kind: 'existing', windowId: 'desktop-window-2' },
				type: 'desktop-window-layout-drag-target-changed',
			}),
		);
		expect(client.getDragTarget(sessionId)).toEqual({
			kind: 'existing',
			windowId: 'desktop-window-2',
		});

		client.updateDrag(sessionId, { x: 100, y: 200 });
		const settlement = client.finishDrag(sessionId, {
			point: { x: 120, y: 210 },
			reason: 'complete',
		});
		port.emit(
			DesktopWindowLayoutMainEventSchema.parse({
				result: { destinationWindowId: 'desktop-window-2', kind: 'completed' },
				sessionId,
				type: 'desktop-window-layout-drag-settled',
			}),
		);

		await expect(settlement).resolves.toMatchObject({ kind: 'completed' });
		expect(client.getDragTarget(sessionId)).toBeNull();
		expect(
			port.messages.map((message) => DesktopWindowLayoutRendererCommandSchema.parse(message).type),
		).toEqual([
			'desktop-window-layout-drag-started',
			'desktop-window-layout-drag-updated',
			'desktop-window-layout-drag-completed',
		]);
	});

	it('fails closed on malformed or transferred events', async () => {
		const port = new FakeMessagePort();
		const client = new DesktopWindowLayoutClient(port.asMessagePort());
		const initialization = client.initialize();

		port.emit({ type: 'unknown' });

		await expect(initialization).resolves.toEqual({ kind: 'unavailable' });
		expect(port.close).toHaveBeenCalledOnce();
	});
});

function snapshotEvent(revision: string): DesktopWindowLayoutMainEvent {
	return DesktopWindowLayoutMainEventSchema.parse({
		snapshot: {
			currentWindowId: 'desktop-window-1',
			revision,
			terminalContexts: [{ tabIds: ['tab-1'], terminalContextId: 'context-1' }],
			windows: [
				{ ordinal: 1, windowId: 'desktop-window-1' },
				{ ordinal: 2, windowId: 'desktop-window-2' },
			],
		},
		type: 'desktop-window-layout-snapshot-published',
	});
}

class FakeMessagePort {
	public readonly close = vi.fn<() => void>();
	public readonly messages: unknown[] = [];
	readonly #messageListeners = new Set<(event: MessageEvent<unknown>) => void>();
	public readonly postMessage = vi.fn<(message: unknown) => void>((message) => {
		this.messages.push(message);
	});

	public addEventListener(type: string, listener: EventListenerOrEventListenerObject): void {
		if (type === 'message') {
			this.#messageListeners.add(listener as (event: MessageEvent<unknown>) => void);
		}
	}

	public asMessagePort(): MessagePort {
		return this as unknown as MessagePort;
	}

	public emit(data: unknown): void {
		for (const listener of this.#messageListeners) {
			listener({ data, ports: [] } as unknown as MessageEvent<unknown>);
		}
	}

	public removeEventListener(type: string, listener: EventListenerOrEventListenerObject): void {
		if (type === 'message') {
			this.#messageListeners.delete(listener as (event: MessageEvent<unknown>) => void);
		}
	}

	public start(): void {}
}
