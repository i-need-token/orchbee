import fc from 'fast-check';
import { describe, expect, it } from 'vitest';

import type { WorkbenchSearchSection } from '../workbench-search-result';
import {
	createWorkbenchSearchProviderState,
	transitionWorkbenchSearchProviderState,
	type WorkbenchSearchProviderEvent,
	type WorkbenchSearchProviderRequest,
	type WorkbenchSearchProviderState,
} from '../workbench-search-provider-state';

const request: WorkbenchSearchProviderRequest = {
	filter: 'all',
	query: 'term',
	queryRevision: 3,
	sessionId: 2,
};

function section(id: string): WorkbenchSearchSection {
	return { id, label: id, order: 10, results: [] };
}

function start(
	providerIds: readonly string[] = ['agents', 'remote-actions'],
): WorkbenchSearchProviderState {
	return transitionWorkbenchSearchProviderState(createWorkbenchSearchProviderState(), {
		providerIds,
		request,
		type: 'request-changed',
	});
}

describe('workbench search provider state', () => {
	it('starts one current loading run per unique provider', () => {
		expect(start(['agents', 'agents', 'remote-actions'])).toEqual({
			request,
			runs: [
				{
					attempt: 0,
					phase: 'loading',
					providerId: 'agents',
					queryRevision: 3,
					sessionId: 2,
				},
				{
					attempt: 0,
					phase: 'loading',
					providerId: 'remote-actions',
					queryRevision: 3,
					sessionId: 2,
				},
			],
		});
	});

	it('accepts only the current loading run completion', () => {
		const state = start();
		const staleSession = transitionWorkbenchSearchProviderState(state, {
			attempt: 0,
			providerId: 'agents',
			queryRevision: 3,
			sections: [section('stale-session')],
			sessionId: 1,
			type: 'provider-completed',
		});
		const staleRevision = transitionWorkbenchSearchProviderState(state, {
			attempt: 0,
			providerId: 'agents',
			queryRevision: 2,
			sections: [section('stale-revision')],
			sessionId: 2,
			type: 'provider-completed',
		});
		const current = transitionWorkbenchSearchProviderState(state, {
			attempt: 0,
			providerId: 'agents',
			queryRevision: 3,
			sections: [section('current')],
			sessionId: 2,
			type: 'provider-completed',
		});
		const duplicate = transitionWorkbenchSearchProviderState(current, {
			attempt: 0,
			providerId: 'agents',
			queryRevision: 3,
			sections: [section('duplicate')],
			sessionId: 2,
			type: 'provider-completed',
		});

		expect(staleSession).toBe(state);
		expect(staleRevision).toBe(state);
		expect(current.runs[0]).toMatchObject({
			phase: 'ready',
			sections: [section('current')],
		});
		expect(duplicate).toBe(current);
	});

	it('isolates failure and retries only the failed provider', () => {
		const loading = start();
		const failed = transitionWorkbenchSearchProviderState(loading, {
			attempt: 0,
			failureCode: 'provider-failed',
			providerId: 'agents',
			queryRevision: 3,
			sessionId: 2,
			type: 'provider-failed',
		});
		const readyPeer = transitionWorkbenchSearchProviderState(failed, {
			attempt: 0,
			providerId: 'remote-actions',
			queryRevision: 3,
			sections: [section('remote')],
			sessionId: 2,
			type: 'provider-completed',
		});
		const retried = transitionWorkbenchSearchProviderState(readyPeer, {
			providerId: 'agents',
			type: 'retry-requested',
		});

		expect(retried.runs).toEqual([
			{
				attempt: 1,
				phase: 'loading',
				providerId: 'agents',
				queryRevision: 3,
				sessionId: 2,
			},
			readyPeer.runs[1],
		]);
		expect(readyPeer.runs[1]).toMatchObject({
			phase: 'ready',
			sections: [section('remote')],
		});
	});

	it('ignores an old attempt after retry and restarts a changed provider model', () => {
		const failed = transitionWorkbenchSearchProviderState(start(), {
			attempt: 0,
			failureCode: 'provider-failed',
			providerId: 'agents',
			queryRevision: 3,
			sessionId: 2,
			type: 'provider-failed',
		});
		const retried = transitionWorkbenchSearchProviderState(failed, {
			providerId: 'agents',
			type: 'retry-requested',
		});
		const stale = transitionWorkbenchSearchProviderState(retried, {
			attempt: 0,
			providerId: 'agents',
			queryRevision: 3,
			sections: [section('stale')],
			sessionId: 2,
			type: 'provider-completed',
		});
		const changed = transitionWorkbenchSearchProviderState(retried, {
			providerId: 'agents',
			type: 'provider-model-changed',
		});

		expect(stale).toBe(retried);
		expect(changed.runs[0]).toMatchObject({ attempt: 2, phase: 'loading' });
	});

	it('produces the same ordered provider projection for every completion order', () => {
		const initial = start();
		const agentsCompleted: WorkbenchSearchProviderEvent = {
			attempt: 0,
			providerId: 'agents',
			queryRevision: 3,
			sections: [section('agents')],
			sessionId: 2,
			type: 'provider-completed',
		};
		const actionsCompleted: WorkbenchSearchProviderEvent = {
			attempt: 0,
			providerId: 'remote-actions',
			queryRevision: 3,
			sections: [section('remote-actions')],
			sessionId: 2,
			type: 'provider-completed',
		};
		const leftThenRight = transitionWorkbenchSearchProviderState(
			transitionWorkbenchSearchProviderState(initial, agentsCompleted),
			actionsCompleted,
		);
		const rightThenLeft = transitionWorkbenchSearchProviderState(
			transitionWorkbenchSearchProviderState(initial, actionsCompleted),
			agentsCompleted,
		);

		expect(leftThenRight).toEqual(rightThenLeft);
	});

	it('clears every provider projection on close', () => {
		const state = start();
		const closed = transitionWorkbenchSearchProviderState(state, { type: 'closed' });

		expect(closed).toEqual(createWorkbenchSearchProviderState());
		expect(
			transitionWorkbenchSearchProviderState(closed, {
				attempt: 0,
				providerId: 'agents',
				queryRevision: 3,
				sections: [section('late')],
				sessionId: 2,
				type: 'provider-completed',
			}),
		).toBe(closed);
	});

	it('preserves provider identity and current-request invariants across generated event sequences', () => {
		const eventArbitrary = fc.oneof(
			fc
				.record({
					attempt: fc.nat({ max: 4 }),
					providerId: fc.constantFrom('agents', 'remote-actions', 'unknown'),
					queryRevision: fc.nat({ max: 5 }),
					sessionId: fc.nat({ max: 4 }),
				})
				.map(
					(identity): WorkbenchSearchProviderEvent => ({
						...identity,
						sections: [],
						type: 'provider-completed',
					}),
				),
			fc
				.record({
					attempt: fc.nat({ max: 4 }),
					providerId: fc.constantFrom('agents', 'remote-actions', 'unknown'),
					queryRevision: fc.nat({ max: 5 }),
					sessionId: fc.nat({ max: 4 }),
				})
				.map(
					(identity): WorkbenchSearchProviderEvent => ({
						...identity,
						failureCode: 'provider-failed',
						type: 'provider-failed',
					}),
				),
			fc
				.record({
					providerId: fc.constantFrom('agents', 'remote-actions', 'unknown'),
					type: fc.constantFrom('retry-requested', 'provider-model-changed'),
				})
				.map((event): WorkbenchSearchProviderEvent => event),
			fc.constant<WorkbenchSearchProviderEvent>({ type: 'closed' }),
		);

		fc.assert(
			fc.property(fc.array(eventArbitrary, { maxLength: 80 }), (events) => {
				let state = start();
				for (const event of events) {
					state = transitionWorkbenchSearchProviderState(state, event);
					const ids = state.runs.map((run) => run.providerId);
					expect(new Set(ids).size).toBe(ids.length);
					expect(state.request === null ? state.runs : []).toEqual([]);
					if (state.request === null) {
						continue;
					}
					for (const run of state.runs) {
						expect(run.sessionId).toBe(state.request.sessionId);
						expect(run.queryRevision).toBe(state.request.queryRevision);
						expect(run.attempt).toBeGreaterThanOrEqual(0);
					}
				}
			}),
			{ numRuns: 200, seed: 2_026_072_9 },
		);
	});
});
