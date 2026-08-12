import { describe, expect, it, vi } from 'vitest';

import { OrderedInputDispatcher } from '../OrderedInputDispatcher';

describe('OrderedInputDispatcher', () => {
	it('holds synchronous input behind an active asynchronous decision', async () => {
		const order: string[] = [];
		let release: (() => void) | undefined;
		const gate = new Promise<void>((resolve) => {
			release = resolve;
		});
		const dispatcher = new OrderedInputDispatcher();
		const first = dispatcher.dispatch(
			async () => {
				order.push('paste-start');
				await gate;
				order.push('paste-end');
			},
			() => undefined,
		);
		dispatcher.dispatchSync(() => order.push('typing'));

		await Promise.resolve();
		expect(order).toEqual(['paste-start']);
		release?.();
		await first;
		await dispatcher.dispatch(
			() => order.push('barrier'),
			() => undefined,
		);
		expect(order).toEqual(['paste-start', 'paste-end', 'typing', 'barrier']);
	});

	it('reports a deferred synchronous failure and continues the queue', async () => {
		const error = new Error('listener failed');
		const onError = vi.fn<(value: unknown) => void>();
		const dispatcher = new OrderedInputDispatcher(onError);
		let release: (() => void) | undefined;
		const gate = new Promise<void>((resolve) => {
			release = resolve;
		});
		const first = dispatcher.dispatch(
			() => gate,
			() => undefined,
		);
		dispatcher.dispatchSync(() => {
			throw error;
		});
		release?.();
		await first;
		await dispatcher.dispatch(
			() => undefined,
			() => undefined,
		);

		expect(onError).toHaveBeenCalledWith(error);
		expect(dispatcher.isPending).toBe(false);
	});

	it('invalidates a stalled generation without allowing its queued work to run', async () => {
		const order: string[] = [];
		const dispatcher = new OrderedInputDispatcher();
		const stalled = dispatcher.dispatch(
			async () => {
				order.push('review-start');
				await new Promise<void>(() => undefined);
				return 'completed';
			},
			() => 'cancelled',
		);
		dispatcher.dispatchSync(() => order.push('stale-key'));
		await Promise.resolve();

		dispatcher.invalidate();
		dispatcher.dispatchSync(() => order.push('repair'));

		await expect(stalled).resolves.toBe('cancelled');
		await Promise.resolve();
		expect(order).toEqual(['review-start', 'repair']);
		expect(dispatcher.isPending).toBe(false);
	});

	it('observes a late rejection after cancellation and keeps the new generation usable', async () => {
		let rejectOperation: ((error: Error) => void) | undefined;
		const dispatcher = new OrderedInputDispatcher();
		const cancelled = dispatcher.dispatch<void | 'cancelled'>(
			() =>
				new Promise<void>((_resolve, reject) => {
					rejectOperation = reject;
				}),
			() => 'cancelled',
		);
		await Promise.resolve();
		dispatcher.invalidate();

		await expect(cancelled).resolves.toBe('cancelled');
		rejectOperation?.(new Error('late rejection'));
		await Promise.resolve();
		await expect(
			dispatcher.dispatch(
				() => 'current',
				() => 'cancelled',
			),
		).resolves.toBe('current');
	});
});
