import { describe, expect, it } from 'vitest';

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
});
