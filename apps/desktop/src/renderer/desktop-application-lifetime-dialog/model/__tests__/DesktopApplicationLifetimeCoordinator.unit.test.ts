import type {
	WorkbenchTerminalSessionStopRequestResult,
	WorkbenchTerminalSessionStopSnapshot,
} from '@orchbee/workbench';
import { workbenchSharedDocumentRevision } from '@orchbee/workbench/document';
import { describe, expect, it, vi } from 'vitest';

import {
	DesktopApplicationExitRequestIdSchema,
	type DesktopApplicationLifetimeMainEvent,
	type DesktopApplicationLifetimeRendererCommand,
} from '../../../../shared';
import type { DesktopWorkbenchDocumentCoordinator } from '../../../workbench-document-control';
import { DesktopApplicationLifetimeCoordinator } from '../DesktopApplicationLifetimeCoordinator';

function createHarness(sessionCount = 2) {
	let lifetimeListener: ((event: DesktopApplicationLifetimeMainEvent) => void) | undefined;
	let documentListener: (() => void) | undefined;
	let snapshot: WorkbenchTerminalSessionStopSnapshot = {
		complete: false,
		failures: [],
		presentationCount: sessionCount,
		stoppedCount: 0,
		stoppingCount: sessionCount,
	};
	const commands: DesktopApplicationLifetimeRendererCommand[] = [];
	const runtime = {
		getTerminalSessionStopSnapshot: () => snapshot,
		requestAllTerminalSessionsStop: vi.fn<() => WorkbenchTerminalSessionStopRequestResult>(() => ({
			commandCount: sessionCount,
			kind: sessionCount === 0 ? ('unchanged' as const) : ('started' as const),
			snapshot,
		})),
		retryFailedTerminalSessionStops: vi.fn<() => WorkbenchTerminalSessionStopRequestResult>(() => ({
			commandCount: 1,
			kind: 'started' as const,
			snapshot,
		})),
		subscribeDocument: (listener: () => void) => {
			documentListener = listener;
			return () => {
				documentListener = undefined;
			};
		},
	};
	const document = {
		committedRevision: workbenchSharedDocumentRevision('5'),
		flush: vi.fn<DesktopWorkbenchDocumentCoordinator['flush']>().mockResolvedValue({
			kind: 'flushed',
			revision: workbenchSharedDocumentRevision('6'),
		}),
	};
	const coordinator = new DesktopApplicationLifetimeCoordinator(runtime, document, {
		post: (command) => {
			commands.push(command);
			return true;
		},
		subscribe: (listener) => {
			lifetimeListener = listener;
			return () => {
				lifetimeListener = undefined;
			};
		},
	});
	return {
		commands,
		acknowledgeMode(
			requestId: ReturnType<typeof DesktopApplicationExitRequestIdSchema.parse>,
			mode: 'preserve' | 'stop',
		) {
			lifetimeListener?.({
				mode,
				requestId,
				type: 'desktop-application-exit-mode-accepted',
			});
		},
		coordinator,
		emitLifetime(event: DesktopApplicationLifetimeMainEvent) {
			lifetimeListener?.(event);
		},
		emitDocument() {
			documentListener?.();
		},
		runtime,
		setComplete() {
			snapshot = { ...snapshot, complete: true, stoppedCount: sessionCount, stoppingCount: 0 };
		},
		document,
	};
}

describe('DesktopApplicationLifetimeCoordinator', () => {
	it('prompts for user quit and keeps preserve as the non-destructive path', async () => {
		const harness = createHarness();
		const requestId = DesktopApplicationExitRequestIdSchema.parse('exit-user');
		harness.emitLifetime({
			reason: 'user-quit',
			requestId,
			type: 'desktop-application-exit-requested',
		});
		expect(harness.coordinator.getSnapshot()).toEqual({
			kind: 'awaiting-choice',
			requestId,
			sessionCount: 2,
		});

		harness.coordinator.choose('preserve');
		expect(harness.document.flush).not.toHaveBeenCalled();
		harness.acknowledgeMode(requestId, 'preserve');
		await vi.waitFor(() =>
			expect(harness.commands).toContainEqual({
				mode: 'preserve',
				presentationCount: 2,
				requestId,
				documentRevision: '6',
				type: 'desktop-application-exit-renderer-prepared',
			}),
		);
		expect(harness.runtime.requestAllTerminalSessionsStop).not.toHaveBeenCalled();
	});

	it('stops all presentations before asking Main to drain unreferenced bindings', async () => {
		const harness = createHarness(3);
		const requestId = DesktopApplicationExitRequestIdSchema.parse('exit-stop');
		harness.emitLifetime({
			reason: 'user-quit',
			requestId,
			type: 'desktop-application-exit-requested',
		});
		harness.coordinator.choose('stop');
		expect(harness.runtime.requestAllTerminalSessionsStop).not.toHaveBeenCalled();
		harness.acknowledgeMode(requestId, 'stop');
		await vi.waitFor(() => expect(harness.document.flush).toHaveBeenCalledOnce());
		expect(
			harness.commands.some(
				(command) => command.type === 'desktop-application-exit-renderer-prepared',
			),
		).toBe(false);

		harness.setComplete();
		harness.emitDocument();
		await vi.waitFor(() =>
			expect(harness.commands).toContainEqual({
				mode: 'stop',
				presentationCount: 3,
				requestId,
				documentRevision: '6',
				type: 'desktop-application-exit-renderer-prepared',
			}),
		);
		expect(harness.runtime.requestAllTerminalSessionsStop).toHaveBeenCalledOnce();
		expect(harness.document.flush).toHaveBeenCalledTimes(2);
	});

	it('prepares update restart without prompting even when the flush is best-effort', async () => {
		const harness = createHarness();
		harness.document.flush.mockResolvedValueOnce({
			kind: 'unavailable',
			reason: 'write-failed',
		});
		const requestId = DesktopApplicationExitRequestIdSchema.parse('exit-update');
		harness.emitLifetime({
			reason: 'update-restart',
			requestId,
			type: 'desktop-application-exit-requested',
		});
		expect(harness.document.flush).not.toHaveBeenCalled();
		harness.acknowledgeMode(requestId, 'preserve');

		await vi.waitFor(() =>
			expect(harness.commands).toContainEqual(
				expect.objectContaining({
					mode: 'preserve',
					requestId,
					type: 'desktop-application-exit-renderer-prepared',
				}),
			),
		);
		expect(harness.coordinator.getSnapshot().kind).not.toBe('awaiting-choice');
	});

	it('quits without prompting when every retained presentation is already stopped', async () => {
		const harness = createHarness();
		harness.setComplete();
		const requestId = DesktopApplicationExitRequestIdSchema.parse('exit-stopped');
		harness.emitLifetime({
			reason: 'user-quit',
			requestId,
			type: 'desktop-application-exit-requested',
		});
		harness.acknowledgeMode(requestId, 'preserve');

		await vi.waitFor(() =>
			expect(harness.commands).toContainEqual(
				expect.objectContaining({
					mode: 'preserve',
					requestId,
					type: 'desktop-application-exit-renderer-prepared',
				}),
			),
		);
		expect(harness.coordinator.getSnapshot().kind).not.toBe('awaiting-choice');
	});

	it('cancels only while the initial user decision is pending', () => {
		const harness = createHarness();
		const requestId = DesktopApplicationExitRequestIdSchema.parse('exit-cancel');
		harness.emitLifetime({
			reason: 'user-quit',
			requestId,
			type: 'desktop-application-exit-requested',
		});
		harness.coordinator.choose('cancel');
		expect(harness.coordinator.getSnapshot()).toEqual({ kind: 'idle' });
		expect(harness.commands.at(-1)).toEqual({
			requestId,
			type: 'desktop-application-exit-cancelled',
		});
	});
});
