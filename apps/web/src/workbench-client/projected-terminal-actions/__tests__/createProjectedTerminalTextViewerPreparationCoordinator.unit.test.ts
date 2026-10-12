import { describe, expect, it, vi } from 'vitest';

import { createProjectedTerminalTextViewerPreparationCoordinator } from '../createProjectedTerminalTextViewerPreparationCoordinator';

describe('createProjectedTerminalTextViewerPreparationCoordinator', () => {
	it('cancels the previous source before preparing the latest source', async () => {
		const coordinator = createProjectedTerminalTextViewerPreparationCoordinator();
		let firstSignal: AbortSignal | undefined;
		const first = coordinator.run(new AbortController().signal, (signal) => {
			firstSignal = signal;
			return new Promise<string>((resolve) => {
				signal.addEventListener('abort', () => resolve('superseded'), { once: true });
			});
		});
		const second = coordinator.run(new AbortController().signal, async () => 'scrollback');

		await expect(first).resolves.toEqual({ kind: 'cancelled' });
		await expect(second).resolves.toEqual({ kind: 'prepared', value: 'scrollback' });
		expect(firstSignal?.aborted).toBe(true);
		coordinator.dispose();
	});

	it('maps caller cancellation and disposal without publishing preparation results', async () => {
		const coordinator = createProjectedTerminalTextViewerPreparationCoordinator();
		const caller = new AbortController();
		const cancelled = coordinator.run(caller.signal, async (signal) => {
			caller.abort();
			return signal.aborted ? 'cancelled' : 'unexpected';
		});
		await expect(cancelled).resolves.toEqual({ kind: 'cancelled' });

		const pending = coordinator.run(
			new AbortController().signal,
			(signal) =>
				new Promise<string>((resolve) => {
					signal.addEventListener('abort', () => resolve('disposed'), { once: true });
				}),
		);
		coordinator.dispose();
		await expect(pending).resolves.toEqual({ kind: 'cancelled' });
	});

	it('retains the latest source preparation started by previous cancellation', async () => {
		const coordinator = createProjectedTerminalTextViewerPreparationCoordinator();
		const caller = new AbortController().signal;
		let latest: ReturnType<typeof coordinator.run> | undefined;
		let latestSignal: AbortSignal | undefined;
		const first = coordinator.run(
			caller,
			(signal) =>
				new Promise<string>((resolve) => {
					signal.addEventListener('abort', () => {
						latest = coordinator.run(caller, async (currentSignal) => {
							latestSignal = currentSignal;
							return 'latest source';
						});
						resolve('superseded');
					});
				}),
		);
		const supersededPrepare = vi
			.fn<(signal: AbortSignal) => Promise<string>>()
			.mockResolvedValue('obsolete source');
		const superseded = coordinator.run(caller, supersededPrepare);
		coordinator.dispose();

		expect(supersededPrepare).not.toHaveBeenCalled();
		expect(latestSignal?.aborted).toBe(true);
		await expect(Promise.all([first, superseded, latest])).resolves.toEqual([
			{ kind: 'cancelled' },
			{ kind: 'cancelled' },
			{ kind: 'cancelled' },
		]);
	});

	it('does not prepare a source when previous cancellation disposes the owner', async () => {
		const coordinator = createProjectedTerminalTextViewerPreparationCoordinator();
		const caller = new AbortController().signal;
		const first = coordinator.run(
			caller,
			(signal) =>
				new Promise<string>((resolve) => {
					signal.addEventListener('abort', () => {
						coordinator.dispose();
						resolve('disposed');
					});
				}),
		);
		const prepare = vi
			.fn<(signal: AbortSignal) => Promise<string>>()
			.mockResolvedValue('unexpected source');
		const second = coordinator.run(caller, prepare);
		const outcomes = await Promise.all([first, second]);
		coordinator.dispose();

		expect(prepare).not.toHaveBeenCalled();
		expect(outcomes).toEqual([{ kind: 'cancelled' }, { kind: 'cancelled' }]);
	});

	it('rejects source preparation during disposal and after the registration lifetime', async () => {
		const coordinator = createProjectedTerminalTextViewerPreparationCoordinator();
		const caller = new AbortController().signal;
		const prepare = vi
			.fn<(signal: AbortSignal) => Promise<string>>()
			.mockResolvedValue('unexpected source');
		let duringDisposal: ReturnType<typeof coordinator.run> | undefined;
		const active = coordinator.run(
			caller,
			(signal) =>
				new Promise<string>((resolve) => {
					signal.addEventListener('abort', () => {
						duringDisposal = coordinator.run(caller, prepare);
						resolve('disposed');
					});
				}),
		);
		coordinator.dispose();
		const afterDisposal = coordinator.run(caller, prepare);
		const outcomes = await Promise.all([active, duringDisposal, afterDisposal]);
		coordinator.dispose();

		expect(prepare).not.toHaveBeenCalled();
		expect(outcomes).toEqual([{ kind: 'cancelled' }, { kind: 'cancelled' }, { kind: 'cancelled' }]);
	});

	it('keeps current source preparation when a caller is already cancelled at admission', async () => {
		const coordinator = createProjectedTerminalTextViewerPreparationCoordinator();
		let activeSignal: AbortSignal | undefined;
		const active = coordinator.run(new AbortController().signal, async (signal) => {
			activeSignal = signal;
			return 'current source';
		});
		const cancelledCaller = new AbortController();
		cancelledCaller.abort();
		const prepare = vi
			.fn<(signal: AbortSignal) => Promise<string>>()
			.mockResolvedValue('cancelled source');
		const cancelled = coordinator.run(cancelledCaller.signal, prepare);
		const wasActiveAborted = activeSignal?.aborted;
		const outcomes = await Promise.all([active, cancelled]);
		coordinator.dispose();

		expect(wasActiveAborted).toBe(false);
		expect(prepare).not.toHaveBeenCalled();
		expect(outcomes).toEqual([
			{ kind: 'prepared', value: 'current source' },
			{ kind: 'cancelled' },
		]);
	});
});
