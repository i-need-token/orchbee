import { describe, expect, it } from 'vitest';

import { AgentRuntime } from '../AgentRuntime';
import { AgentRuntimeChangeReadResultSchema } from '../changes';
import type { AgentObservationEvent } from '../events';
import {
	AgentEventIdSchema,
	AgentExecutionLocationIdSchema,
	AgentProviderIdSchema,
	AgentSessionIdSchema,
	AgentTurnIdSchema,
} from '../model';

describe('Agent runtime changes', () => {
	it('retains one ordered content-free record for each applied transition', () => {
		const runtime = new AgentRuntime();
		expect(runtime.dispatch(observed())).toMatchObject({ kind: 'applied' });
		expect(runtime.dispatch(started())).toMatchObject({ kind: 'applied' });
		expect(runtime.dispatch(started())).toMatchObject({ kind: 'unchanged' });

		const first = runtime.readChanges({ afterRevision: 0, limit: 1 });
		expect(first.kind).toBe('changes');
		if (first.kind !== 'changes') throw new Error('Expected an Agent change page.');
		expect(Object.keys(first.changes[0]?.session ?? {}).sort()).toEqual([
			'lifecycle',
			'observation',
			'parentSessionId',
			'pendingRequestCount',
			'providerId',
			'sessionId',
			'status',
		]);
		expect(first).toMatchObject({
			changes: [
				{
					revision: 1,
					session: {
						lifecycle: 'live',
						pendingRequestCount: 0,
						providerId: 'codex',
						sessionId: 'session-changes',
						status: { activityHint: null, kind: 'unknown' },
					},
					transition: 'session-observed',
				},
			],
			hasMore: true,
			kind: 'changes',
			observedThroughRevision: 1,
			runtimeRevision: 2,
		});
		expect(AgentRuntimeChangeReadResultSchema.safeParse({ ...first, hasMore: false }).success).toBe(
			false,
		);

		expect(runtime.readChanges({ afterRevision: 1, limit: 100 })).toMatchObject({
			changes: [
				{
					revision: 2,
					session: { status: { activity: 'thinking', kind: 'working', phase: 'working' } },
					transition: 'turn-started',
				},
			],
			hasMore: false,
			kind: 'changes',
			observedThroughRevision: 2,
			runtimeRevision: 2,
		});
		expect(runtime.readChanges({ afterRevision: 2, limit: 100 })).toEqual({
			changes: [],
			hasMore: false,
			kind: 'current',
			observedThroughRevision: 2,
			runtimeRevision: 2,
		});
	});

	it('requires an explicit reset instead of silently crossing retained history', () => {
		const runtime = new AgentRuntime();
		runtime.dispatch({
			...terminalObserved(),
			capabilities: ['turnLifecycle'],
			observation: { ...terminalObserved().observation, fidelity: 'rich', source: 'hook' },
		});
		for (let sequence = 2; sequence <= 513; sequence += 1) {
			runtime.dispatch({
				eventId: AgentEventIdSchema.parse(`event-change-${sequence}`),
				generation: 0,
				sequence,
				sessionId: AgentSessionIdSchema.parse('session-retention'),
				type: 'session-idle-observed',
			});
		}

		expect(runtime.getSnapshot().revision).toBe(513);
		expect(runtime.readChanges({ afterRevision: 0, limit: 100 })).toEqual({
			changes: [],
			hasMore: false,
			kind: 'reset-required',
			observedThroughRevision: 513,
			runtimeRevision: 513,
		});
		expect(runtime.readChanges({ afterRevision: 1, limit: 1 })).toMatchObject({
			changes: [{ revision: 2 }],
			hasMore: true,
			kind: 'changes',
		});
		expect(runtime.readChanges({ afterRevision: 514, limit: 1 })).toEqual({
			changes: [],
			hasMore: false,
			kind: 'revision-ahead',
			observedThroughRevision: 514,
			runtimeRevision: 513,
		});
	});
});

function observed(): Extract<AgentObservationEvent, { readonly type: 'session-observed' }> {
	return {
		capabilities: ['sessionLifecycle', 'turnLifecycle'],
		eventId: AgentEventIdSchema.parse('event-observed'),
		executionLocation: {
			executionLocationId: AgentExecutionLocationIdSchema.parse('execution-changes'),
		},
		observation: {
			fidelity: 'rich',
			freshness: 'fresh',
			generation: 0,
			sequence: 1,
			source: 'hook',
		},
		parentSessionId: null,
		presentationAttachments: [],
		providerId: AgentProviderIdSchema.parse('codex'),
		sessionId: AgentSessionIdSchema.parse('session-changes'),
		type: 'session-observed',
	};
}

function started(): Extract<AgentObservationEvent, { readonly type: 'turn-started' }> {
	return {
		activity: 'thinking',
		eventId: AgentEventIdSchema.parse('event-started'),
		generation: 0,
		phase: 'working',
		sequence: 2,
		sessionId: AgentSessionIdSchema.parse('session-changes'),
		turnId: AgentTurnIdSchema.parse('turn-changes'),
		type: 'turn-started',
	};
}

function terminalObserved(): Extract<AgentObservationEvent, { readonly type: 'session-observed' }> {
	return {
		capabilities: ['activity'],
		eventId: AgentEventIdSchema.parse('event-retention-observed'),
		executionLocation: {
			executionLocationId: AgentExecutionLocationIdSchema.parse('execution-retention'),
		},
		observation: {
			fidelity: 'basic',
			freshness: 'fresh',
			generation: 0,
			sequence: 1,
			source: 'terminal',
		},
		parentSessionId: null,
		presentationAttachments: [],
		providerId: AgentProviderIdSchema.parse('terminal-signal'),
		sessionId: AgentSessionIdSchema.parse('session-retention'),
		type: 'session-observed',
	};
}
