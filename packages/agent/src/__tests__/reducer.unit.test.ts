import { describe, expect, it } from 'vitest';

import type { AgentObservationEvent } from '../events';
import {
	AgentEventIdSchema,
	AgentExecutionLocationIdSchema,
	AgentProviderIdSchema,
	AgentRequestIdSchema,
	AgentSessionIdSchema,
	AgentTurnIdSchema,
} from '../model';
import { createAgentRuntimeSnapshot, transitionAgentRuntime } from '../reducer';

describe('transitionAgentRuntime', () => {
	it('requires the exact capability for a classified attention request', () => {
		const sessionId = AgentSessionIdSchema.parse('session-reducer');
		const turnId = AgentTurnIdSchema.parse('turn-reducer');
		const observed = transitionAgentRuntime(createAgentRuntimeSnapshot(), {
			capabilities: ['approvals', 'turnLifecycle'],
			eventId: AgentEventIdSchema.parse('event-reducer-1'),
			executionLocation: {
				executionLocationId: AgentExecutionLocationIdSchema.parse('execution-reducer'),
			},
			observation: {
				fidelity: 'rich',
				freshness: 'fresh',
				generation: 0,
				sequence: 1,
				source: 'hook',
			},
			parentSessionId: null,
			presentationAttachments: [
				{
					generation: 'binding-reducer',
					navigable: true,
					presentationId: 'presentation-reducer',
					visibility: 'visible',
				},
			],
			providerId: AgentProviderIdSchema.parse('claude-code'),
			sessionId,
			type: 'session-observed',
		});
		if (observed.kind !== 'applied') throw new Error('Expected Session observation to apply.');
		const started = transitionAgentRuntime(observed.snapshot, {
			activity: 'thinking',
			eventId: AgentEventIdSchema.parse('event-reducer-2'),
			generation: 0,
			phase: 'working',
			sequence: 2,
			sessionId,
			turnId,
			type: 'turn-started',
		});
		if (started.kind !== 'applied') throw new Error('Expected Turn start to apply.');

		const transition = transitionAgentRuntime(started.snapshot, {
			eventId: AgentEventIdSchema.parse('event-reducer-3'),
			generation: 0,
			request: {
				actionPath: 'navigate-only',
				kind: 'question',
				requestId: AgentRequestIdSchema.parse('request-reducer'),
				turnId,
			},
			sequence: 3,
			sessionId,
			type: 'attention-requested',
		} satisfies AgentObservationEvent);

		expect(transition).toMatchObject({
			kind: 'rejected',
			reason: 'capability-not-declared',
			snapshot: started.snapshot,
		});
	});
});
