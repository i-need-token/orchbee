import {
	DesktopBrowserAccessMainEventSchema,
	type DesktopBrowserAccessState,
} from '@orchbee/desktop-client-protocol';
import type { MessageEvent, MessagePortMain } from 'electron';
import { describe, expect, it, vi } from 'vitest';

import {
	DesktopBrowserAccessBroker,
	type DesktopBrowserAccessBrokerRuntime,
} from '../DesktopBrowserAccessBroker';

describe('DesktopBrowserAccessBroker', () => {
	it('routes bounded requests and returns only the non-secret state projection', async () => {
		const port = new FakeMessagePort();
		let state: DesktopBrowserAccessState = { phase: 'stopped' };
		const runtime = createRuntime(() => state);
		runtime.start.mockImplementation(async (mode) => {
			state = {
				localOrigin: 'http://127.0.0.1:43110',
				mode,
				networkOrigins: [],
				phase: 'ready',
			};
			return state;
		});
		const broker = new DesktopBrowserAccessBroker(port.asMessagePortMain(), runtime);
		broker.start();

		port.emit({
			mode: 'loopback',
			requestId: 'browser-access:request:1',
			type: 'desktop-browser-access-start-requested',
		});
		await vi.waitFor(() => expect(port.messages).toHaveLength(1));

		const response = DesktopBrowserAccessMainEventSchema.parse(port.messages[0]);
		expect(response).toMatchObject({
			requestId: 'browser-access:request:1',
			state: { mode: 'loopback', phase: 'ready' },
			type: 'desktop-browser-access-completed',
		});
		expect(JSON.stringify(response)).not.toContain('capability');
	});

	it('maps privileged failures to bounded codes and preserves current state', async () => {
		const port = new FakeMessagePort();
		const state = { phase: 'stopped' } as const;
		const runtime = createRuntime(() => state);
		runtime.start.mockRejectedValueOnce(
			Object.assign(new Error('raw network interface detail'), {
				code: 'network-address-unavailable' as const,
			}),
		);
		const broker = new DesktopBrowserAccessBroker(port.asMessagePortMain(), runtime);
		broker.start();

		port.emit({
			mode: 'local-network',
			requestId: 'browser-access:request:2',
			type: 'desktop-browser-access-start-requested',
		});
		await vi.waitFor(() => expect(port.messages).toHaveLength(1));

		expect(DesktopBrowserAccessMainEventSchema.parse(port.messages[0])).toEqual({
			code: 'network-address-unavailable',
			requestId: 'browser-access:request:2',
			state,
			type: 'desktop-browser-access-failed',
		});
		expect(JSON.stringify(port.messages[0])).not.toContain('raw network');
	});

	it('retires the renderer capability on malformed input without stopping shared access', () => {
		const port = new FakeMessagePort();
		const runtime = createRuntime(() => ({ phase: 'stopped' }));
		const broker = new DesktopBrowserAccessBroker(port.asMessagePortMain(), runtime);
		broker.start();

		port.emit({ type: 'unknown-browser-effect' });

		expect(port.close).toHaveBeenCalledOnce();
		expect(runtime.stop).not.toHaveBeenCalled();
	});
});

function createRuntime(
	getSnapshot: () => DesktopBrowserAccessState,
): DesktopBrowserAccessBrokerRuntime & {
	readonly copyAccessLink: ReturnType<typeof vi.fn>;
	readonly openLocalBrowser: ReturnType<typeof vi.fn>;
	readonly start: ReturnType<typeof vi.fn>;
	readonly stop: ReturnType<typeof vi.fn>;
} {
	return {
		copyAccessLink: vi.fn<DesktopBrowserAccessBrokerRuntime['copyAccessLink']>(async () =>
			getSnapshot(),
		),
		getSnapshot,
		openLocalBrowser: vi.fn<DesktopBrowserAccessBrokerRuntime['openLocalBrowser']>(async () =>
			getSnapshot(),
		),
		start: vi.fn<DesktopBrowserAccessBrokerRuntime['start']>(async () => getSnapshot()),
		stop: vi.fn<DesktopBrowserAccessBrokerRuntime['stop']>(async () => getSnapshot()),
	};
}

class FakeMessagePort {
	readonly #closeListeners = new Set<() => void>();
	readonly close = vi.fn<() => void>();
	readonly messages: unknown[] = [];
	readonly #messageListeners = new Set<(event: MessageEvent) => void>();

	asMessagePortMain(): MessagePortMain {
		return this as unknown as MessagePortMain;
	}

	emit(data: unknown): void {
		for (const listener of this.#messageListeners) {
			listener({ data, ports: [] } as unknown as MessageEvent);
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
