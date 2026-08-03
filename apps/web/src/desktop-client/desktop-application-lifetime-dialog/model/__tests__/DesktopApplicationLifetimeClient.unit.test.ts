import { describe, expect, it, vi } from 'vitest';

import { DesktopApplicationExitRequestIdSchema } from '@orchbee/desktop-client-protocol';
import { DesktopApplicationLifetimeClient } from '../DesktopApplicationLifetimeClient';

class TestMessagePort extends EventTarget {
	readonly close = vi.fn<() => void>();
	readonly postMessage = vi.fn<(message: unknown) => void>();
	readonly start = vi.fn<() => void>();
}

describe('DesktopApplicationLifetimeClient', () => {
	it('publishes validated commands and typed Main events on the scoped port', () => {
		const port = new TestMessagePort();
		const client = new DesktopApplicationLifetimeClient(port as unknown as MessagePort);
		const listener = vi.fn<(event: unknown) => void>();
		client.subscribe(listener);
		const requestId = DesktopApplicationExitRequestIdSchema.parse('exit-client');

		expect(
			client.post({ choice: 'preserve', requestId, type: 'desktop-application-exit-choice' }),
		).toBe(true);
		expect(port.postMessage).toHaveBeenCalledWith({
			choice: 'preserve',
			requestId,
			type: 'desktop-application-exit-choice',
		});
		port.dispatchEvent(
			new MessageEvent('message', {
				data: { reason: 'user-quit', requestId, type: 'desktop-application-exit-requested' },
			}),
		);
		expect(listener).toHaveBeenCalledWith({
			reason: 'user-quit',
			requestId,
			type: 'desktop-application-exit-requested',
		});
		expect(port.start).toHaveBeenCalledOnce();
	});

	it('retires the capability on malformed or transferred Main messages', () => {
		const port = new TestMessagePort();
		const client = new DesktopApplicationLifetimeClient(port as unknown as MessagePort);
		const listener = vi.fn<(event: unknown) => void>();
		const transferred = new MessageChannel();
		client.subscribe(listener);

		port.dispatchEvent(
			new MessageEvent('message', {
				data: { channel: 'generic-ipc' },
				ports: [transferred.port1],
			}),
		);

		expect(port.close).toHaveBeenCalledOnce();
		expect(listener).not.toHaveBeenCalled();
		expect(
			client.post({
				requestId: DesktopApplicationExitRequestIdSchema.parse('exit-retired'),
				type: 'desktop-application-exit-cancelled',
			}),
		).toBe(false);
		transferred.port1.close();
		transferred.port2.close();
	});
});
