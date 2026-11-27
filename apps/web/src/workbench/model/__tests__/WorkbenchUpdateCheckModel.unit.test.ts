import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import type {
	WorkbenchUpdateCheckAnswer,
	WorkbenchUpdateOffer,
} from '../../types/workbench-client-update';
import { WorkbenchUpdateCheckModel } from '../WorkbenchUpdateCheckModel';

type AnswerKind = Exclude<WorkbenchUpdateCheckAnswer['kind'], 'downloading' | 'run-failed'>;

/**
 * A Session Server that answers each exchange from `answers` in turn, a bare kind standing for its
 * answer, and a host offer.
 */
function harness(
	answers: readonly (AnswerKind | WorkbenchUpdateCheckAnswer)[],
	unavailable: 'development' | 'installation' | 'session' | null = null,
) {
	const asked: ('read' | 'start')[] = [];
	const remaining = answers.map((answer) =>
		typeof answer === 'string' ? { kind: answer } : answer,
	);
	let offer: WorkbenchUpdateOffer | null = null;
	const offerListeners = new Set<() => void>();
	const model = new WorkbenchUpdateCheckModel({
		ask: (action) => {
			asked.push(action);
			return Promise.resolve(remaining.shift() ?? { kind: 'not-answered' });
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

	it('reads a run this window started about once a second while shown, through every stage, and shows its result', async () => {
		const check = harness([
			'idle',
			'checking',
			{ kind: 'downloading', receivedBytes: 1, totalBytes: 4, version: '1.3.0' },
			'installing',
			'up-to-date',
		]);
		check.model.show();
		await vi.advanceTimersByTimeAsync(0);
		check.model.start();
		expect(check.status()).toBe('checking');
		await vi.advanceTimersByTimeAsync(1000);
		expect(check.model.getSnapshot()).toEqual({
			kind: 'downloading',
			percent: 25,
			version: '1.3.0',
		});
		// A press while a run is under way joins it.
		check.model.start();
		await vi.advanceTimersByTimeAsync(2000);
		expect(check.asked).toEqual(['read', 'start', 'read', 'read', 'read']);
		expect(check.status()).toBe('up-to-date');
	});

	it('stays under way after an install until the offer arrives, and never claims Up to date', async () => {
		const offered = harness(['checking', 'installed']);
		offered.model.show();
		offered.model.start();
		await vi.advanceTimersByTimeAsync(1000);
		expect(['idle', 'up-to-date', 'failed']).not.toContain(offered.status());
		offered.offer('1.3.0');
		expect(offered.status()).toBe('idle');

		const unoffered = harness(['checking', 'installed']);
		unoffered.model.show();
		unoffered.model.start();
		await vi.advanceTimersByTimeAsync(14_000);
		expect(['idle', 'up-to-date', 'failed']).not.toContain(unoffered.status());
		await vi.advanceTimersByTimeAsync(2000);
		expect(unoffered.status()).toBe('idle');
		// The wait for the offer reads nothing more.
		expect(unoffered.asked).toEqual(['read', 'start']);
	});

	it('fails a started check in the phase its run ended in, or its check when it ran nothing or lost its answer, but shows nothing for a background run', async () => {
		for (const [answer, phase] of [
			['idle', 'check'],
			['not-answered', 'check'],
			[{ kind: 'run-failed', phase: 'download' }, 'download'],
			[{ kind: 'run-failed', phase: 'install' }, 'install'],
		] as const) {
			const started = harness([answer]);
			started.model.start();
			await vi.advanceTimersByTimeAsync(0);
			expect(started.model.getSnapshot()).toEqual({ kind: 'failed', phase });
		}
		const background = harness(['checking', { kind: 'run-failed', phase: 'check' }]);
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
