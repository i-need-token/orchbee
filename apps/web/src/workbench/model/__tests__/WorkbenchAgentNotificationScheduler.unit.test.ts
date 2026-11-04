import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import type {
	WorkbenchAgentObservation,
	WorkbenchAgentSnapshot,
	WorkbenchAgentState,
} from '../../types/workbench-agent-observation';
import {
	WorkbenchAgentNotificationScheduler,
	type WorkbenchAgentNotificationCandidate,
} from '../WorkbenchAgentNotificationScheduler';

beforeEach(() => {
	vi.useFakeTimers();
});

afterEach(() => {
	vi.useRealTimers();
});

function agent(
	state: WorkbenchAgentState,
	sequence: bigint,
	{ binding = 'terminal:1', label = 'claude' }: { binding?: string; label?: string } = {},
): WorkbenchAgentObservation {
	return { agent: label, binding, sequence, state };
}

function snapshot(
	entries: Readonly<Record<string, WorkbenchAgentObservation>>,
): WorkbenchAgentSnapshot {
	return { complete: true, presentations: new Map(Object.entries(entries)) };
}

function createScheduler() {
	const delivered: WorkbenchAgentNotificationCandidate[] = [];
	const scheduler = new WorkbenchAgentNotificationScheduler((candidate) => {
		delivered.push(candidate);
	});
	return { delivered, scheduler };
}

describe('Workbench Agent notification scheduling', () => {
	it('notifies a finished turn and a new request one second later', () => {
		const { delivered, scheduler } = createScheduler();
		scheduler.observe(
			snapshot({ a: agent('working', 1n), b: agent('idle', 1n), c: agent('working', 1n) }),
		);
		scheduler.observe(
			snapshot({ a: agent('done', 2n), b: agent('blocked', 2n), c: agent('idle', 2n) }),
		);
		vi.advanceTimersByTime(999);
		expect(delivered).toEqual([]);
		vi.advanceTimersByTime(1);
		expect(delivered).toEqual([
			{ kind: 'done', observation: agent('done', 2n), presentationId: 'a' },
			{ kind: 'request', observation: agent('blocked', 2n), presentationId: 'b' },
			{ kind: 'done', observation: agent('idle', 2n), presentationId: 'c' },
		]);
	});

	it('stays silent for baselines and every other transition', () => {
		const { delivered, scheduler } = createScheduler();
		// A first observation is a baseline, whatever its state.
		scheduler.observe(snapshot({ a: agent('done', 1n) }));
		scheduler.observe(snapshot({ a: agent('working', 2n), b: agent('blocked', 1n) }));
		// A replacement terminal is a baseline, and leaving a request is no notification.
		scheduler.observe(
			snapshot({ a: agent('done', 1n, { binding: 'terminal:2' }), b: agent('working', 2n) }),
		);
		scheduler.observe(
			snapshot({ a: agent('working', 2n, { binding: 'terminal:2' }), b: agent('unknown', 3n) }),
		);
		// A changed Agent label is a baseline, and limited status proves nothing.
		scheduler.observe(
			snapshot({
				a: agent('done', 3n, { binding: 'terminal:2', label: 'codex' }),
				b: agent('done', 4n),
			}),
		);
		vi.runAllTimers();
		expect(delivered).toEqual([]);
	});

	it('withdraws a candidate that changed, vanished or was painted before delivery', () => {
		const { delivered, scheduler } = createScheduler();
		const working = agent('working', 1n);
		scheduler.observe(snapshot({ a: working, b: working, c: working, d: working }));
		const done = agent('done', 2n);
		const finished = snapshot({ a: done, b: done, c: done, d: done });
		scheduler.observe(finished);
		scheduler.painted(finished, new Set(['c']));
		// Only the exact pending state counts as seen.
		scheduler.painted(snapshot({ d: working }), new Set(['d']));
		scheduler.observe(snapshot({ a: agent('working', 3n), c: done, d: done }));
		vi.runAllTimers();
		expect(delivered.map((candidate) => candidate.presentationId)).toEqual(['d']);
	});

	it('drops everything pending on reset', () => {
		const { delivered, scheduler } = createScheduler();
		scheduler.observe(snapshot({ a: agent('working', 1n) }));
		scheduler.observe(snapshot({ a: agent('done', 2n) }));
		scheduler.reset();
		vi.runAllTimers();
		expect(delivered).toEqual([]);
	});
});
