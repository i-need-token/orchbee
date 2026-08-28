import {
	DesktopWindowLayoutDragSessionIdSchema,
	DesktopWindowLayoutMainEventSchema,
	DesktopWindowLayoutSnapshotSchema,
} from '@orchbee/desktop-client-protocol';
import type { MessageEvent, MessagePortMain } from 'electron';
import { describe, expect, it, vi } from 'vitest';

import {
	DesktopWindowLayoutBroker,
	type DesktopWindowLayoutBrokerController,
} from '../DesktopWindowLayoutBroker';

const snapshot = DesktopWindowLayoutSnapshotSchema.parse({
	currentWindowId: 'desktop-window-1',
	revision: '1',
	terminalContexts: [{ tabIds: ['tab-1'], terminalContextId: 'context-1' }],
	windows: [
		{ ordinal: 1, windowId: 'desktop-window-1' },
		{ ordinal: 2, windowId: 'desktop-window-2' },
	],
});

describe('DesktopWindowLayoutBroker', () => {
	it('publishes the window scope and routes correlated movement', async () => {
		const port = new FakeMessagePort();
		const controller = createController();
		const broker = new DesktopWindowLayoutBroker(port.asMessagePortMain(), controller);
		broker.start();

		expect(DesktopWindowLayoutMainEventSchema.parse(port.messages[0])).toEqual({
			snapshot,
			type: 'desktop-window-layout-snapshot-published',
		});
		port.emit({
			destination: { kind: 'existing', windowId: 'desktop-window-2' },
			requestId: 'window-layout:request:1',
			tabIds: ['tab-1'],
			terminalContextId: 'context-1',
			type: 'desktop-window-layout-move-tabs-requested',
		});

		await vi.waitFor(() => expect(port.messages).toHaveLength(2));
		expect(controller.moveTabs).toHaveBeenCalledWith('context-1', ['tab-1'], {
			kind: 'existing',
			windowId: 'desktop-window-2',
		});
		expect(DesktopWindowLayoutMainEventSchema.parse(port.messages[1])).toMatchObject({
			requestId: 'window-layout:request:1',
			result: { destinationWindowId: 'desktop-window-2', kind: 'completed' },
		});
	});

	it('routes one drag session and returns hit-test changes', async () => {
		const port = new FakeMessagePort();
		const controller = createController();
		const broker = new DesktopWindowLayoutBroker(port.asMessagePortMain(), controller);
		const sessionId = DesktopWindowLayoutDragSessionIdSchema.parse('window-layout:drag:1');
		broker.start();
		port.emit({
			sessionId,
			tabIds: ['tab-1'],
			terminalContextId: 'context-1',
			type: 'desktop-window-layout-drag-started',
		});
		port.emit({
			point: { x: 100, y: 200 },
			sessionId,
			type: 'desktop-window-layout-drag-updated',
		});

		expect(DesktopWindowLayoutMainEventSchema.parse(port.messages[1])).toMatchObject({
			target: { kind: 'existing', windowId: 'desktop-window-2' },
			type: 'desktop-window-layout-drag-target-changed',
		});
		port.emit({
			point: { x: 110, y: 210 },
			sessionId,
			type: 'desktop-window-layout-drag-completed',
		});
		await vi.waitFor(() => expect(port.messages).toHaveLength(3));
		expect(DesktopWindowLayoutMainEventSchema.parse(port.messages[2])).toMatchObject({
			result: { destinationWindowId: 'desktop-window-2', kind: 'completed' },
			type: 'desktop-window-layout-drag-settled',
		});
	});

	it('retires the renderer capability on malformed or stale-session commands', () => {
		const port = new FakeMessagePort();
		const broker = new DesktopWindowLayoutBroker(port.asMessagePortMain(), createController());
		broker.start();

		port.emit({ type: 'unknown-window-layout-command' });

		expect(port.close).toHaveBeenCalledOnce();
	});
});

function createController(): DesktopWindowLayoutBrokerController & {
	readonly moveTabs: ReturnType<typeof vi.fn>;
} {
	return {
		cancelDrag: vi.fn<DesktopWindowLayoutBrokerController['cancelDrag']>(async () => ({
			kind: 'unchanged',
		})),
		completeDrag: vi.fn<DesktopWindowLayoutBrokerController['completeDrag']>(async () => ({
			destinationWindowId: 'desktop-window-2' as never,
			kind: 'completed',
		})),
		getSnapshot: () => snapshot,
		moveGroup: vi.fn<DesktopWindowLayoutBrokerController['moveGroup']>(async () => ({
			destinationWindowId: 'desktop-window-2' as never,
			kind: 'completed',
		})),
		moveTabs: vi.fn<DesktopWindowLayoutBrokerController['moveTabs']>(async () => ({
			destinationWindowId: 'desktop-window-2' as never,
			kind: 'completed',
		})),
		setActiveTerminalContext:
			vi.fn<DesktopWindowLayoutBrokerController['setActiveTerminalContext']>(),
		startDrag: vi.fn<DesktopWindowLayoutBrokerController['startDrag']>(),
		subscribe: () => () => undefined,
		updateDrag: vi.fn<DesktopWindowLayoutBrokerController['updateDrag']>(() => ({
			kind: 'existing',
			windowId: 'desktop-window-2' as never,
		})),
	};
}

class FakeMessagePort {
	readonly #closeListeners = new Set<() => void>();
	public readonly close = vi.fn<() => void>();
	public readonly messages: unknown[] = [];
	readonly #messageListeners = new Set<(event: MessageEvent) => void>();

	public asMessagePortMain(): MessagePortMain {
		return this as unknown as MessagePortMain;
	}

	public emit(data: unknown, ports: readonly unknown[] = []): void {
		for (const listener of this.#messageListeners) {
			listener({ data, ports } as unknown as MessageEvent);
		}
	}

	public off(event: string, listener: (...arguments_: never[]) => void): void {
		if (event === 'message') {
			this.#messageListeners.delete(listener as unknown as (event: MessageEvent) => void);
		} else if (event === 'close') {
			this.#closeListeners.delete(listener as unknown as () => void);
		}
	}

	public on(event: string, listener: (...arguments_: never[]) => void): void {
		if (event === 'message') {
			this.#messageListeners.add(listener as unknown as (event: MessageEvent) => void);
		} else if (event === 'close') {
			this.#closeListeners.add(listener as unknown as () => void);
		}
	}

	public postMessage(message: unknown): void {
		this.messages.push(message);
	}

	public start(): void {}
}
