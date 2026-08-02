import { EventEmitter } from 'node:events';

import { workbenchTopologyRevision } from '@orchbee/workbench/topology';
import type { MessagePortMain } from 'electron';
import { describe, expect, it, vi } from 'vitest';

import { DesktopApplicationExitRequestIdSchema } from '../../../shared';
import {
	DesktopApplicationLifetimeBroker,
	type DesktopApplicationLifetimeBrokerDependencies,
} from '../DesktopApplicationLifetimeBroker';

class TestMainPort extends EventEmitter {
	readonly close = vi.fn<() => void>();
	readonly postMessage = vi.fn<(value: unknown) => void>();
	readonly start = vi.fn<() => void>();
}

function message(data: unknown) {
	return { data, ports: [] };
}

describe('DesktopApplicationLifetimeBroker', () => {
	it('coalesces duplicate requests and prepares preserve without draining', async () => {
		const port = new TestMainPort();
		const drainForStop = vi.fn<DesktopApplicationLifetimeBrokerDependencies['drainForStop']>();
		const broker = new DesktopApplicationLifetimeBroker(port as unknown as MessagePortMain, {
			drainForStop,
		});
		broker.start();
		const requestId = DesktopApplicationExitRequestIdSchema.parse('exit-1');
		const first = broker.requestExit(requestId, 'user-quit');
		const duplicate = broker.requestExit(
			DesktopApplicationExitRequestIdSchema.parse('exit-duplicate'),
			'user-quit',
		);
		expect(first).toBe(duplicate);
		expect(port.postMessage).toHaveBeenCalledTimes(1);

		port.emit(
			'message',
			message({
				mode: 'preserve',
				presentationCount: 2,
				requestId,
				topologyRevision: '3',
				type: 'desktop-application-exit-renderer-prepared',
			}),
		);

		await expect(first).resolves.toEqual({ forced: false, kind: 'ready', mode: 'preserve' });
		expect(drainForStop).not.toHaveBeenCalled();
	});

	it('keeps a failed stop workflow active and retries Main drain exactly once', async () => {
		const port = new TestMainPort();
		const drainForStop = vi
			.fn<DesktopApplicationLifetimeBrokerDependencies['drainForStop']>()
			.mockResolvedValueOnce({
				failures: [{ code: 'unreferenced-binding-close-failed', count: 1, retryable: true }],
				kind: 'failed',
			})
			.mockResolvedValueOnce({ closedUnreferencedCount: 1, kind: 'completed' });
		const broker = new DesktopApplicationLifetimeBroker(port as unknown as MessagePortMain, {
			drainForStop,
		});
		broker.start();
		const requestId = DesktopApplicationExitRequestIdSchema.parse('exit-stop');
		const result = broker.requestExit(requestId, 'user-quit');
		port.emit(
			'message',
			message({
				mode: 'stop',
				presentationCount: 3,
				requestId,
				topologyRevision: '7',
				type: 'desktop-application-exit-renderer-prepared',
			}),
		);
		await vi.waitFor(() => expect(drainForStop).toHaveBeenCalledOnce());
		expect(drainForStop).toHaveBeenCalledWith(workbenchTopologyRevision('7'));
		expect(port.postMessage).toHaveBeenLastCalledWith(
			expect.objectContaining({ result: expect.objectContaining({ kind: 'failed' }) }),
		);

		port.emit(
			'message',
			message({ requestId, type: 'desktop-application-exit-main-retry-requested' }),
		);
		await expect(result).resolves.toEqual({ forced: false, kind: 'ready', mode: 'stop' });
		expect(drainForStop).toHaveBeenCalledTimes(2);
	});

	it('allows cancellation only before effects and exposes the explicit failure escape', async () => {
		const port = new TestMainPort();
		const broker = new DesktopApplicationLifetimeBroker(port as unknown as MessagePortMain, {
			drainForStop: vi.fn<DesktopApplicationLifetimeBrokerDependencies['drainForStop']>(),
		});
		broker.start();
		const cancelId = DesktopApplicationExitRequestIdSchema.parse('exit-cancel');
		const cancelled = broker.requestExit(cancelId, 'user-quit');
		port.emit(
			'message',
			message({ requestId: cancelId, type: 'desktop-application-exit-cancelled' }),
		);
		await expect(cancelled).resolves.toEqual({ kind: 'cancelled' });

		const forceId = DesktopApplicationExitRequestIdSchema.parse('exit-force');
		const forced = broker.requestExit(forceId, 'user-quit');
		port.emit(
			'message',
			message({
				failures: [{ code: 'host-orphan-detected', count: 1, retryable: false }],
				progress: {
					completedCount: 0,
					failedCount: 1,
					phase: 'failed',
					totalCount: 1,
				},
				requestId: forceId,
				type: 'desktop-application-exit-progress',
			}),
		);
		port.emit(
			'message',
			message({ requestId: forceId, type: 'desktop-application-exit-quit-anyway-requested' }),
		);
		await expect(forced).resolves.toEqual({ forced: true, kind: 'ready', mode: 'stop' });
	});
});
