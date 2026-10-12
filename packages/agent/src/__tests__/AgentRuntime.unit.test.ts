import { describe, expect, it, vi } from 'vitest';

import { AgentRuntime } from '../AgentRuntime';
import type { AgentObservationEvent } from '../events';
import {
	AgentEventIdSchema,
	AgentExecutionLocationIdSchema,
	AgentProviderIdSchema,
	AgentRequestIdSchema,
	AgentSessionIdSchema,
	AgentTurnIdSchema,
} from '../model';

const sessionId = AgentSessionIdSchema.parse('session-runtime');
const turnId = AgentTurnIdSchema.parse('turn-runtime');

describe('AgentRuntime', () => {
	it('publishes applied snapshots and semantic events from stable listener snapshots', () => {
		const runtime = new AgentRuntime();
		const snapshotListener = vi.fn<() => void>();
		const lateListener = vi.fn<() => void>();
		const semanticListener = vi.fn<(event: unknown) => void>();
		runtime.subscribe(() => {
			snapshotListener();
			runtime.subscribe(lateListener);
		});
		runtime.subscribeSemanticEvents(semanticListener);

		runtime.dispatch(observed());
		expect(snapshotListener).toHaveBeenCalledOnce();
		expect(lateListener).not.toHaveBeenCalled();

		runtime.dispatch(started());
		runtime.dispatch(attentionRequested());

		expect(runtime.getSnapshot().revision).toBe(3);
		expect(lateListener).toHaveBeenCalledTimes(2);
		expect(semanticListener).toHaveBeenCalledWith(
			expect.objectContaining({ type: 'attention-requested' }),
		);
	});

	it('preserves committed results and remaining publications when observers throw', () => {
		const runtime = new AgentRuntime();
		runtime.dispatch(observed());
		runtime.dispatch(started());
		const snapshotListener = vi.fn<() => void>();
		const semanticListener = vi.fn<(event: unknown) => void>();
		runtime.subscribe(() => {
			throw new Error('snapshot observer failed');
		});
		runtime.subscribe(snapshotListener);
		runtime.subscribeSemanticEvents(() => {
			throw new Error('semantic observer failed');
		});
		runtime.subscribeSemanticEvents(semanticListener);

		const transition = runtime.dispatch(attentionRequested());

		expect(transition).toMatchObject({ kind: 'applied', snapshot: { revision: 3 } });
		expect(runtime.getSnapshot()).toBe(transition.snapshot);
		expect(snapshotListener).toHaveBeenCalledOnce();
		expect(semanticListener).toHaveBeenCalledOnce();
		expect(semanticListener).toHaveBeenCalledWith(
			expect.objectContaining({ type: 'attention-requested', sessionId }),
		);
		runtime.dispose();
	});

	it('validates injected state and rejects dispatch after idempotent disposal', () => {
		expect(() => new AgentRuntime({ revision: -1, sessions: [] })).toThrow(/revision/u);
		const runtime = new AgentRuntime();

		runtime.dispose();
		runtime.dispose();

		expect(runtime.dispatch(observed())).toMatchObject({
			kind: 'rejected',
			reason: 'runtime-disposed',
		});
	});
});

function observed(): Extract<AgentObservationEvent, { readonly type: 'session-observed' }> {
	return {
		capabilities: ['approvals', 'sessionLifecycle', 'turnLifecycle'],
		eventId: AgentEventIdSchema.parse('event-runtime-1'),
		executionLocation: {
			executionLocationId: AgentExecutionLocationIdSchema.parse('execution-runtime'),
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
				generation: 'binding-runtime',
				navigable: true,
				presentationId: 'presentation-runtime',
				visibility: 'visible',
			},
		],
		providerId: AgentProviderIdSchema.parse('claude-code'),
		sessionId,
		type: 'session-observed',
	};
}

function started(): Extract<AgentObservationEvent, { readonly type: 'turn-started' }> {
	return {
		activity: 'thinking',
		eventId: AgentEventIdSchema.parse('event-runtime-2'),
		generation: 0,
		phase: 'working',
		sequence: 2,
		sessionId,
		turnId,
		type: 'turn-started',
	};
}

function attentionRequested(): Extract<
	AgentObservationEvent,
	{ readonly type: 'attention-requested' }
> {
	return {
		eventId: AgentEventIdSchema.parse('event-runtime-3'),
		generation: 0,
		request: {
			actionPath: 'navigate-only',
			kind: 'approval',
			requestId: AgentRequestIdSchema.parse('request-runtime'),
			turnId,
		},
		sequence: 3,
		sessionId,
		type: 'attention-requested',
	};
}
