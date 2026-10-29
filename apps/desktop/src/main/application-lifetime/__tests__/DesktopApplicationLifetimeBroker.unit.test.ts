import { EventEmitter } from 'node:events';

import type { MessagePortMain } from 'electron';
import { describe, expect, it, vi } from 'vitest';

import { DesktopApplicationExitRequestIdSchema } from '@orchbee/desktop-client-protocol';
import {
	workbenchSharedDocumentRevision,
	terminalPresentationId,
} from '@orchbee/workbench-client-protocol';
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
		const prepareMode = vi
			.fn<NonNullable<DesktopApplicationLifetimeBrokerDependencies['prepareMode']>>()
			.mockReturnValue(true);
		const broker = new DesktopApplicationLifetimeBroker(port as unknown as MessagePortMain, {
			drainForStop,
			prepareMode,
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
			message({ choice: 'preserve', requestId, type: 'desktop-application-exit-choice' }),
		);
		expect(prepareMode).toHaveBeenCalledWith('preserve');
		expect(port.postMessage).toHaveBeenLastCalledWith({
			mode: 'preserve',
			requestId,
			type: 'desktop-application-exit-mode-accepted',
		});

		port.emit(
			'message',
			message({
				mode: 'preserve',
				presentationCount: 2,
				requestId,
				documentRevision: '3',
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
			prepareMode: () => true,
		});
		broker.start();
		const requestId = DesktopApplicationExitRequestIdSchema.parse('exit-stop');
		const result = broker.requestExit(requestId, 'user-quit');
		port.emit(
			'message',
			message({ choice: 'stop', requestId, type: 'desktop-application-exit-choice' }),
		);
		port.emit(
			'message',
			message({
				mode: 'stop',
				presentationCount: 3,
				requestId,
				documentRevision: '7',
				type: 'desktop-application-exit-renderer-prepared',
			}),
		);
		await vi.waitFor(() => expect(drainForStop).toHaveBeenCalledOnce());
		expect(drainForStop).toHaveBeenCalledWith(workbenchSharedDocumentRevision('7'));
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

	it('writes the ready plan only after a development Host drain and retries plan failure', async () => {
		const port = new TestMainPort();
		const restartPresentationIds = [
			terminalPresentationId('presentation-1'),
			terminalPresentationId('presentation-2'),
		] as const;
		const drainForStop = vi
			.fn<DesktopApplicationLifetimeBrokerDependencies['drainForStop']>()
			.mockResolvedValue({ closedUnreferencedCount: 2, kind: 'completed' });
		const prepareDevelopmentHostRestart = vi
			.fn<
				NonNullable<DesktopApplicationLifetimeBrokerDependencies['prepareDevelopmentHostRestart']>
			>()
			.mockResolvedValueOnce(false)
			.mockResolvedValueOnce(true);
		const prepareMode = vi
			.fn<NonNullable<DesktopApplicationLifetimeBrokerDependencies['prepareMode']>>()
			.mockReturnValue(true);
		const broker = new DesktopApplicationLifetimeBroker(port as unknown as MessagePortMain, {
			drainForStop,
			getDevelopmentHostRestartPresentationIds: () => restartPresentationIds,
			prepareDevelopmentHostRestart,
			prepareMode,
		});
		broker.start();
		const requestId = DesktopApplicationExitRequestIdSchema.parse('exit-development-host');
		const result = broker.requestExit(requestId, 'development-host-restart');
		expect(port.postMessage).toHaveBeenLastCalledWith({
			reason: 'development-host-restart',
			restartPresentationIds,
			requestId,
			type: 'desktop-application-exit-requested',
		});
		port.emit(
			'message',
			message({
				choice: 'development-host-restart',
				restartPresentationIds,
				requestId,
				type: 'desktop-application-exit-choice',
			}),
		);
		expect(prepareMode).toHaveBeenCalledWith('development-host-restart', restartPresentationIds);
		port.emit(
			'message',
			message({
				documentRevision: '9',
				mode: 'development-host-restart',
				presentationCount: 2,
				requestId,
				type: 'desktop-application-exit-renderer-prepared',
			}),
		);

		await vi.waitFor(() => expect(prepareDevelopmentHostRestart).toHaveBeenCalledOnce());
		expect(prepareDevelopmentHostRestart).toHaveBeenCalledWith(
			workbenchSharedDocumentRevision('9'),
		);
		expect(port.postMessage).toHaveBeenLastCalledWith(
			expect.objectContaining({ result: expect.objectContaining({ kind: 'failed' }) }),
		);
		port.emit(
			'message',
			message({ requestId, type: 'desktop-application-exit-main-retry-requested' }),
		);

		await expect(result).resolves.toEqual({
			forced: false,
			kind: 'ready',
			mode: 'development-host-restart',
		});
		expect(drainForStop).toHaveBeenCalledTimes(2);
		expect(prepareDevelopmentHostRestart).toHaveBeenCalledTimes(2);
	});
});
