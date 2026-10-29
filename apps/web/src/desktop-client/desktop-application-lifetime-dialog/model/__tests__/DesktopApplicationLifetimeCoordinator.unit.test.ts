import type {
	WorkbenchProjectWorkspaceRuntime,
	WorkbenchTerminalSessionStopSnapshot,
} from '@orchbee/workbench';
import { describe, expect, it, vi } from 'vitest';

import {
	DesktopApplicationExitRequestIdSchema,
	type DesktopApplicationLifetimeMainEvent,
	type DesktopApplicationLifetimeRendererCommand,
} from '@orchbee/desktop-client-protocol';
import {
	workbenchSharedDocumentRevision,
	terminalPresentationId,
} from '@orchbee/workbench-client-protocol';
import type { WorkbenchDocumentCoordinator } from '../../../../workbench-client';
import { DesktopApplicationLifetimeCoordinator } from '../DesktopApplicationLifetimeCoordinator';

function createHarness(sessionCount = 2, initialCloseConfirmationCount = sessionCount) {
	let lifetimeListener: ((event: DesktopApplicationLifetimeMainEvent) => void) | undefined;
	let documentListener: (() => void) | undefined;
	let closeConfirmationCount = initialCloseConfirmationCount;
	let snapshot: WorkbenchTerminalSessionStopSnapshot = {
		complete: false,
		failures: [],
		presentationCount: sessionCount,
		stoppedCount: 0,
		stoppingCount: sessionCount,
	};
	const commands: DesktopApplicationLifetimeRendererCommand[] = [];
	const restartPresentationIds = Array.from({ length: sessionCount }, (_, index) =>
		terminalPresentationId(`presentation-${index + 1}`),
	);
	const runtime = {
		getTerminalSessionCloseConfirmationCount: vi.fn<() => number>(() => closeConfirmationCount),
		getTerminalSessionStopSnapshot: () => snapshot,
		getRestartableTerminalPresentationIds: vi.fn<() => typeof restartPresentationIds>(
			() => restartPresentationIds,
		),
		requestAllTerminalSessionsStop: vi.fn<
			WorkbenchProjectWorkspaceRuntime['requestAllTerminalSessionsStop']
		>(() => ({
			commandCount: sessionCount,
			kind: sessionCount === 0 ? ('unchanged' as const) : ('started' as const),
			snapshot,
		})),
		retryFailedTerminalSessionStops: vi.fn<
			WorkbenchProjectWorkspaceRuntime['retryFailedTerminalSessionStops']
		>(() => ({
			commandCount: 1,
			kind: 'started' as const,
			snapshot,
		})),
		stageStoppedTerminalSessionsForRestart: vi.fn<
			WorkbenchProjectWorkspaceRuntime['stageStoppedTerminalSessionsForRestart']
		>(() => ({ kind: 'staged', presentationCount: restartPresentationIds.length })),
		subscribeDocument: (listener: () => void) => {
			documentListener = listener;
			return () => {
				documentListener = undefined;
			};
		},
	};
	const document = {
		committedRevision: workbenchSharedDocumentRevision('5'),
		flush: vi.fn<WorkbenchDocumentCoordinator['flush']>().mockResolvedValue({
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
			mode: 'development-host-restart' | 'preserve' | 'stop',
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
			closeConfirmationCount = 0;
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

	it('stops, durably stages fresh provisions, and prepares a development Host restart', async () => {
		const harness = createHarness(2);
		const requestId = DesktopApplicationExitRequestIdSchema.parse('exit-development-host');
		harness.emitLifetime({
			reason: 'development-host-restart',
			requestId,
			type: 'desktop-application-exit-requested',
		});
		expect(harness.coordinator.getSnapshot().kind).toBe('working');
		expect(harness.commands).toContainEqual({
			choice: 'development-host-restart',
			restartPresentationIds: ['presentation-1', 'presentation-2'],
			requestId,
			type: 'desktop-application-exit-choice',
		});
		expect(harness.runtime.getRestartableTerminalPresentationIds).toHaveBeenCalledOnce();

		harness.acknowledgeMode(requestId, 'development-host-restart');
		await vi.waitFor(() => expect(harness.document.flush).toHaveBeenCalledOnce());
		harness.setComplete();
		harness.emitDocument();

		await vi.waitFor(() =>
			expect(harness.commands).toContainEqual({
				documentRevision: '6',
				mode: 'development-host-restart',
				presentationCount: 2,
				requestId,
				type: 'desktop-application-exit-renderer-prepared',
			}),
		);
		expect(harness.runtime.requestAllTerminalSessionsStop).toHaveBeenCalledOnce();
		expect(harness.runtime.stageStoppedTerminalSessionsForRestart).toHaveBeenCalledWith([
			'presentation-1',
			'presentation-2',
		]);
		expect(harness.document.flush).toHaveBeenCalledTimes(3);
	});

	it('reuses the Main-owned restart targets after the renderer reloads with stopped sessions', async () => {
		const harness = createHarness(2);
		harness.runtime.getRestartableTerminalPresentationIds.mockReturnValue([]);
		const requestId = DesktopApplicationExitRequestIdSchema.parse('exit-development-reloaded');
		harness.setComplete();
		harness.emitLifetime({
			reason: 'development-host-restart',
			restartPresentationIds: [
				terminalPresentationId('presentation-1'),
				terminalPresentationId('presentation-2'),
			],
			requestId,
			type: 'desktop-application-exit-requested',
		});

		expect(harness.runtime.getRestartableTerminalPresentationIds).not.toHaveBeenCalled();
		expect(harness.commands).toContainEqual({
			choice: 'development-host-restart',
			restartPresentationIds: ['presentation-1', 'presentation-2'],
			requestId,
			type: 'desktop-application-exit-choice',
		});
		harness.acknowledgeMode(requestId, 'development-host-restart');

		await vi.waitFor(() =>
			expect(harness.runtime.stageStoppedTerminalSessionsForRestart).toHaveBeenCalledWith([
				'presentation-1',
				'presentation-2',
			]),
		);
		expect(harness.commands).toContainEqual({
			documentRevision: '6',
			mode: 'development-host-restart',
			presentationCount: 2,
			requestId,
			type: 'desktop-application-exit-renderer-prepared',
		});
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

	it('preserves without prompting when retained terminals are running but idle', async () => {
		const harness = createHarness(2, 0);
		const requestId = DesktopApplicationExitRequestIdSchema.parse('exit-idle');
		harness.emitLifetime({
			reason: 'user-quit',
			requestId,
			type: 'desktop-application-exit-requested',
		});

		expect(harness.coordinator.getSnapshot().kind).toBe('working');
		expect(harness.commands).toContainEqual({
			choice: 'preserve',
			requestId,
			type: 'desktop-application-exit-choice',
		});
		expect(harness.runtime.getTerminalSessionCloseConfirmationCount).toHaveBeenCalledOnce();
		expect(harness.runtime.requestAllTerminalSessionsStop).not.toHaveBeenCalled();

		harness.acknowledgeMode(requestId, 'preserve');
		await vi.waitFor(() =>
			expect(harness.commands).toContainEqual(
				expect.objectContaining({
					mode: 'preserve',
					presentationCount: 2,
					requestId,
					type: 'desktop-application-exit-renderer-prepared',
				}),
			),
		);
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
