import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import type {
	WorkbenchUpdateCheckAnswer,
	WorkbenchUpdateOffer,
} from '../../types/workbench-client-update';
import { WorkbenchUpdateCheckModel } from '../WorkbenchUpdateCheckModel';

/** A Session Server that answers each exchange from `answers` in turn, and a host offer. */
function harness(
	answers: readonly WorkbenchUpdateCheckAnswer[],
	unavailable: 'development' | 'installation' | 'session' | null = null,
) {
	const asked: ('read' | 'start')[] = [];
	const remaining = [...answers];
	let offer: WorkbenchUpdateOffer | null = null;
	const offerListeners = new Set<() => void>();
	const model = new WorkbenchUpdateCheckModel({
		ask: (action) => {
			asked.push(action);
			return Promise.resolve(remaining.shift() ?? 'not-answered');
		},
		offer: {
			getSnapshot: () => offer,
			subscribe: (listener) => {
				offerListeners.add(listener);
				return () => offerListeners.delete(listener);
			},
		},
		unavailable,
	});
	return {
		asked,
		model,
		offer: (version: string) => {
			offer = { kind: 'client', version };
			for (const listener of offerListeners) listener();
		},
		status: () => model.getSnapshot().kind,
	};
}

describe('WorkbenchUpdateCheckModel', () => {
	beforeEach(() => vi.useFakeTimers());
	afterEach(() => vi.useRealTimers());

	it('reads a run this window started about once a second while shown, and shows its result', async () => {
		const check = harness(['idle', 'checking', 'checking', 'up-to-date']);
		check.model.show();
		await vi.advanceTimersByTimeAsync(0);
		check.model.start();
		expect(check.status()).toBe('checking');
		await vi.advanceTimersByTimeAsync(2000);
		expect(check.asked).toEqual(['read', 'start', 'read', 'read']);
		expect(check.status()).toBe('up-to-date');
	});

	it('keeps Checking after an install until the offer arrives, and never claims Up to date', async () => {
		const offered = harness(['checking', 'installed']);
		offered.model.show();
		offered.model.start();
		await vi.advanceTimersByTimeAsync(1000);
		expect(offered.status()).toBe('checking');
		offered.offer('1.3.0');
		expect(offered.status()).toBe('idle');

		const unoffered = harness(['checking', 'installed']);
		unoffered.model.show();
		unoffered.model.start();
		await vi.advanceTimersByTimeAsync(14_000);
		expect(unoffered.status()).toBe('checking');
		await vi.advanceTimersByTimeAsync(2000);
		expect(unoffered.status()).toBe('idle');
	});

	it('fails a started check that ran nothing or lost its answer, but shows nothing for a background run', async () => {
		for (const answer of ['idle', 'not-answered', 'run-failed'] as const) {
			const started = harness([answer]);
			started.model.start();
			await vi.advanceTimersByTimeAsync(0);
			expect(started.status()).toBe('failed');
		}
		const background = harness(['checking', 'run-failed']);
		background.model.show();
		await vi.advanceTimersByTimeAsync(0);
		expect(background.status()).toBe('checking');
		await vi.advanceTimersByTimeAsync(1000);
		expect(background.status()).toBe('idle');
	});

	it('clears a finished result when shown again, and treats an unavailable job as the installation', async () => {
		const check = harness(['up-to-date', 'idle', 'unavailable']);
		check.model.start();
		await vi.advanceTimersByTimeAsync(0);
		expect(check.status()).toBe('up-to-date');
		check.model.show()();
		expect(check.status()).toBe('idle');
		await vi.advanceTimersByTimeAsync(0);
		check.model.show();
		await vi.advanceTimersByTimeAsync(0);
		expect(check.model.getSnapshot()).toEqual({ kind: 'unavailable', reason: 'installation' });
	});

	it('never asks when the check cannot run in this window', async () => {
		const check = harness(['checking'], 'session');
		check.model.show();
		check.model.start();
		await vi.advanceTimersByTimeAsync(5000);
		expect(check.asked).toEqual([]);
		expect(check.model.getSnapshot()).toEqual({ kind: 'unavailable', reason: 'session' });
	});
});
