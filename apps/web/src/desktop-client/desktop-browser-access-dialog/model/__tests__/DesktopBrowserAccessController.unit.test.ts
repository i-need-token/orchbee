import type { DesktopBrowserAccessState } from '@orchbee/desktop-client-protocol';
import { describe, expect, it, vi } from 'vitest';

import type { DesktopBrowserAccessClientPort } from '../DesktopBrowserAccessClient';
import { DesktopBrowserAccessController } from '../DesktopBrowserAccessController';

describe('DesktopBrowserAccessController', () => {
	it('refreshes on open and starts loopback before opening the local Browser', async () => {
		const client = createClient();
		const controller = new DesktopBrowserAccessController(client);

		controller.open();
		await vi.waitFor(() => expect(controller.getSnapshot().pendingAction).toBeNull());
		controller.openLocalBrowser();
		await vi.waitFor(() => expect(controller.getSnapshot().pendingAction).toBeNull());

		expect(client.getState).toHaveBeenCalledOnce();
		expect(client.start).toHaveBeenCalledWith('loopback');
		expect(client.start.mock.invocationCallOrder[0]).toBeLessThan(
			client.openLocalBrowser.mock.invocationCallOrder[0] ?? Number.POSITIVE_INFINITY,
		);
		expect(controller.getSnapshot().runtime).toMatchObject({ mode: 'loopback', phase: 'ready' });
		controller.dispose();
	});

	it('keeps a bounded failure and current state available for localized rendering', async () => {
		const client = createClient();
		client.start.mockRejectedValueOnce(
			Object.assign(new Error('network address unavailable'), {
				code: 'network-address-unavailable' as const,
				state: { phase: 'stopped' } as const,
			}),
		);
		const controller = new DesktopBrowserAccessController(client);

		controller.start('local-network');
		await vi.waitFor(() => expect(controller.getSnapshot().pendingAction).toBeNull());

		expect(controller.getSnapshot()).toMatchObject({
			error: 'network-address-unavailable',
			runtime: { phase: 'stopped' },
		});
		controller.dispose();
	});

	it('records only the copied origin after Main performs the clipboard effect', async () => {
		const client = createClient(readyState('local-network'));
		const controller = new DesktopBrowserAccessController(client);

		controller.copyAccessLink('http://192.168.50.12:43110');
		await vi.waitFor(() => expect(controller.getSnapshot().pendingAction).toBeNull());

		expect(controller.getSnapshot().copiedOrigin).toBe('http://192.168.50.12:43110');
		expect(JSON.stringify(controller.getSnapshot())).not.toContain('#');
		controller.dispose();
	});
});

function createClient(initial: DesktopBrowserAccessState = { phase: 'stopped' }) {
	let state = initial;
	const client = {
		copyAccessLink: vi.fn<DesktopBrowserAccessClientPort['copyAccessLink']>(async () => state),
		dispose: vi.fn<() => void>(),
		getState: vi.fn<DesktopBrowserAccessClientPort['getState']>(async () => state),
		openLocalBrowser: vi.fn<DesktopBrowserAccessClientPort['openLocalBrowser']>(async () => state),
		start: vi.fn<DesktopBrowserAccessClientPort['start']>(async (mode) => {
			state = readyState(mode);
			return state;
		}),
		stop: vi.fn<DesktopBrowserAccessClientPort['stop']>(async () => {
			state = { phase: 'stopped' };
			return state;
		}),
	} satisfies DesktopBrowserAccessClientPort;
	return client;
}

function readyState(mode: 'local-network' | 'loopback'): DesktopBrowserAccessState {
	return {
		localOrigin: 'http://127.0.0.1:43110',
		mode,
		networkOrigins: mode === 'local-network' ? ['http://192.168.50.12:43110'] : [],
		phase: 'ready',
	};
}
