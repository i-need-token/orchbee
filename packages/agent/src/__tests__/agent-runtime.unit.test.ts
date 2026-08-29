import fc from 'fast-check';
import { describe, expect, it } from 'vitest';

import type { AgentObservationEvent } from '../events';
import {
	AgentAttentionRequestSchema,
	AgentEventIdSchema,
	AgentExecutionLocationIdSchema,
	AgentProviderIdSchema,
	AgentRequestIdSchema,
	AgentRuntimeSnapshotSchema,
	AgentSessionIdSchema,
	AgentTurnIdSchema,
	type AgentRuntimeSnapshot,
} from '../model';
import { projectAgentLocation, projectAgentSession } from '../projection';
import { createAgentRuntimeSnapshot, transitionAgentRuntime } from '../reducer';
import { AgentRuntime } from '../AgentRuntime';

const sessionId = AgentSessionIdSchema.parse('session-1');
const providerId = AgentProviderIdSchema.parse('claude-code');
const turnId = AgentTurnIdSchema.parse('turn-1');
const requestId = AgentRequestIdSchema.parse('request-1');
const executionLocationId = AgentExecutionLocationIdSchema.parse('execution-1');

describe('Agent runtime transition', () => {
	it('creates one orthogonal unknown Session from a declared observation', () => {
		const transition = transitionAgentRuntime(createAgentRuntimeSnapshot(), observed());

		expect(transition.kind).toBe('applied');
		expect(transition.snapshot.sessions).toEqual([
			{
				activityHint: null,
				capabilities: ['activity', 'approvals', 'questions', 'sessionLifecycle', 'turnLifecycle'],
				executionLocation: { executionLocationId },
				lastOutcome: null,
				lifecycle: 'live',
				observation: {
					fidelity: 'rich',
					freshness: 'fresh',
					generation: 0,
					sequence: 1,
					source: 'hook',
				},
				parentSessionId: null,
				pendingRequests: [],
				presentationAttachments: [
					{
						generation: 'binding-1',
						navigable: true,
						presentationId: 'presentation-1',
						visibility: 'visible',
					},
				],
				providerId,
				sessionId,
				turn: { kind: 'unknown' },
			},
		]);
	});

	it('keeps attention durable and cancels only Turn-scoped requests at an exact outcome', () => {
		let snapshot = apply(createAgentRuntimeSnapshot(), observed());
		snapshot = apply(snapshot, started(2));
		snapshot = apply(snapshot, {
			eventId: eventId(3),
			generation: 0,
			request: {
				actionPath: 'navigate-only',
				kind: 'approval',
				requestId,
				turnId,
			},
			sequence: 3,
			sessionId,
			type: 'attention-requested',
		});
		const sessionRequestId = AgentRequestIdSchema.parse('request-session');
		snapshot = apply(snapshot, {
			eventId: eventId(4),
			generation: 0,
			request: {
				actionPath: 'navigate-only',
				kind: 'question',
				requestId: sessionRequestId,
				turnId: null,
			},
			sequence: 4,
			sessionId,
			type: 'attention-requested',
		});

		const transition = transitionAgentRuntime(snapshot, {
			eventId: eventId(5),
			generation: 0,
			outcome: 'succeeded',
			sequence: 5,
			sessionId,
			turnId,
			type: 'turn-finished',
		});

		expect(transition.kind).toBe('applied');
		expect(transition.snapshot.sessions[0]).toMatchObject({
			lastOutcome: { eventId: eventId(5), kind: 'succeeded', turnId },
			pendingRequests: [{ requestId: sessionRequestId, turnId: null }],
			turn: { kind: 'idle' },
		});
		expect(transition.semanticEvents).toEqual([
			{
				eventId: eventId(5),
				outcome: 'succeeded',
				sessionId,
				turnId,
				type: 'turn-outcome',
			},
		]);
	});

	it('observes an active Turn settling without fabricating an outcome', () => {
		let snapshot = apply(createAgentRuntimeSnapshot(), observed());
		snapshot = apply(snapshot, started(2));
		snapshot = apply(snapshot, {
			eventId: eventId(3),
			generation: 0,
			request: {
				actionPath: 'navigate-only',
				kind: 'approval',
				requestId,
				turnId,
			},
			sequence: 3,
			sessionId,
			type: 'attention-requested',
		});
		const sessionRequestId = AgentRequestIdSchema.parse('request-session');
		snapshot = apply(snapshot, {
			eventId: eventId(4),
			generation: 0,
			request: {
				actionPath: 'navigate-only',
				kind: 'question',
				requestId: sessionRequestId,
				turnId: null,
			},
			sequence: 4,
			sessionId,
			type: 'attention-requested',
		});

		const transition = transitionAgentRuntime(snapshot, {
			eventId: eventId(5),
			generation: 0,
			sequence: 5,
			sessionId,
			type: 'session-idle-observed',
		});

		expect(transition.kind).toBe('applied');
		expect(transition.snapshot.sessions[0]).toMatchObject({
			lastOutcome: null,
			pendingRequests: [{ requestId: sessionRequestId, turnId: null }],
			turn: { kind: 'idle' },
		});
		expect(transition.semanticEvents).toEqual([]);
	});

	it('rejects replacement Turns, stale generations, and reordered arrivals without mutation', () => {
		let snapshot = apply(createAgentRuntimeSnapshot(), observed());
		snapshot = apply(snapshot, started(2));
		const otherTurnId = AgentTurnIdSchema.parse('turn-2');

		const conflict = transitionAgentRuntime(snapshot, {
			...started(3),
			turnId: otherTurnId,
		});
		const staleGeneration = transitionAgentRuntime(snapshot, {
			...started(4),
			generation: 1,
		});
		const reordered = transitionAgentRuntime(snapshot, {
			...started(2),
			activity: 'command',
		});

		expect(conflict).toMatchObject({ kind: 'rejected', reason: 'turn-conflict', snapshot });
		expect(staleGeneration).toMatchObject({
			kind: 'rejected',
			reason: 'generation-mismatch',
			snapshot,
		});
		expect(reordered).toMatchObject({ kind: 'unchanged', snapshot });
	});

	it('overlays stale health without fabricating idle or an outcome', () => {
		let snapshot = apply(createAgentRuntimeSnapshot(), observed());
		snapshot = apply(snapshot, started(2));
		const transition = transitionAgentRuntime(snapshot, {
			eventId: eventId(3),
			generation: 0,
			sequence: 3,
			sessionId,
			type: 'observation-became-stale',
		});

		expect(transition.snapshot.sessions[0]).toMatchObject({
			lastOutcome: null,
			observation: { freshness: 'stale' },
			turn: { kind: 'active', turnId },
		});
		const [session] = transition.snapshot.sessions;
		if (session === undefined) throw new Error('Expected the observed Agent Session.');
		expect(projectAgentSession(session).status).toMatchObject({
			kind: 'working',
		});
	});

	it('interrupts exact active work and clears requests when a Session ends', () => {
		let snapshot = apply(createAgentRuntimeSnapshot(), observed());
		snapshot = apply(snapshot, started(2));
		snapshot = apply(snapshot, {
			eventId: eventId(3),
			generation: 0,
			request: {
				actionPath: 'navigate-only',
				kind: 'approval',
				requestId,
				turnId,
			},
			sequence: 3,
			sessionId,
			type: 'attention-requested',
		});

		const transition = transitionAgentRuntime(snapshot, {
			eventId: eventId(4),
			generation: 0,
			sequence: 4,
			sessionId,
			type: 'session-ended',
		});

		expect(transition.snapshot.sessions[0]).toMatchObject({
			lastOutcome: { eventId: eventId(4), kind: 'interrupted', turnId },
			lifecycle: 'ended',
			pendingRequests: [],
			turn: { kind: 'unknown' },
		});
		expect(transition.semanticEvents.map((event) => event.type)).toEqual([
			'turn-outcome',
			'session-ended',
		]);
	});

	it('rejects semantic authority declared by heuristic observation', () => {
		const transition = transitionAgentRuntime(createAgentRuntimeSnapshot(), {
			...observed(),
			capabilities: ['turnLifecycle'],
			observation: {
				fidelity: 'heuristic',
				freshness: 'fresh',
				generation: 0,
				sequence: 1,
				source: 'heuristic',
			},
		});

		expect(transition).toMatchObject({
			kind: 'rejected',
			reason: 'source-capability-exceeded',
		});
	});

	it('retains terminal-scoped activity as a limited hint without inventing a Turn', () => {
		const limitedSessionId = AgentSessionIdSchema.parse('session-limited');
		let snapshot = apply(createAgentRuntimeSnapshot(), {
			...observed(),
			capabilities: ['activity'],
			observation: {
				fidelity: 'basic',
				freshness: 'fresh',
				generation: 0,
				sequence: 1,
				source: 'terminal',
			},
			sessionId: limitedSessionId,
		});

		const transition = transitionAgentRuntime(snapshot, {
			activity: 'command',
			eventId: eventId(2),
			generation: 0,
			sequence: 2,
			sessionId: limitedSessionId,
			type: 'limited-activity-observed',
		});

		expect(transition.kind).toBe('applied');
		snapshot = transition.snapshot;
		expect(snapshot.sessions[0]).toMatchObject({
			activityHint: 'command',
			turn: { kind: 'unknown' },
		});
		expect(projectAgentLocation(snapshot, 'presentation-1')).toMatchObject({
			freshness: 'fresh',
			status: { activityHint: 'command', kind: 'unknown' },
		});
		expect(transition.semanticEvents).toEqual([]);
	});

	it('rejects limited activity from a source without terminal-scoped authority', () => {
		const snapshot = apply(createAgentRuntimeSnapshot(), observed());

		expect(
			transitionAgentRuntime(snapshot, {
				activity: 'thinking',
				eventId: eventId(2),
				generation: 0,
				sequence: 2,
				sessionId,
				type: 'limited-activity-observed',
			}),
		).toMatchObject({
			kind: 'rejected',
			reason: 'source-capability-exceeded',
			snapshot,
		});
	});

	it('requires the exact declared capability for a classified attention request', () => {
		let snapshot = apply(createAgentRuntimeSnapshot(), {
			...observed(),
			capabilities: ['approvals', 'sessionLifecycle', 'turnLifecycle'],
		});
		snapshot = apply(snapshot, started(2));

		const transition = transitionAgentRuntime(snapshot, {
			eventId: eventId(3),
			generation: 0,
			request: {
				actionPath: 'navigate-only',
				kind: 'question',
				requestId,
				turnId,
			},
			sequence: 3,
			sessionId,
			type: 'attention-requested',
		});

		expect(transition).toMatchObject({
			kind: 'rejected',
			reason: 'capability-not-declared',
			snapshot,
		});
	});

	it('cannot expand a heuristic source through a reconnect snapshot', () => {
		const snapshot = apply(createAgentRuntimeSnapshot(), {
			...observed(),
			capabilities: [],
			observation: {
				fidelity: 'heuristic',
				freshness: 'fresh',
				generation: 0,
				sequence: 1,
				source: 'heuristic',
			},
		});

		const transition = transitionAgentRuntime(snapshot, {
			activityHint: null,
			capabilities: ['turnLifecycle'],
			eventId: eventId(2),
			generation: 1,
			lastOutcome: null,
			pendingRequests: [],
			sequence: 2,
			sessionId,
			turn: { kind: 'idle' },
			type: 'observation-reconnected',
		});

		expect(transition).toMatchObject({
			kind: 'rejected',
			reason: 'source-capability-exceeded',
			snapshot,
		});
	});

	it('rejects native action paths and dangling parent Sessions at schema boundaries', () => {
		expect(
			AgentAttentionRequestSchema.safeParse({
				actionPath: 'native-action',
				kind: 'approval',
				requestId,
				turnId,
			}).success,
		).toBe(false);
		const snapshot = apply(createAgentRuntimeSnapshot(), observed());

		expect(
			AgentRuntimeSnapshotSchema.safeParse({
				...snapshot,
				sessions: [
					{
						...snapshot.sessions[0],
						parentSessionId: AgentSessionIdSchema.parse('missing-parent'),
					},
				],
			}).success,
		).toBe(false);
	});

	it('adopts only an explicit higher-generation reconnect snapshot', () => {
		let snapshot = apply(createAgentRuntimeSnapshot(), observed());
		snapshot = apply(snapshot, started(2));

		const transition = transitionAgentRuntime(snapshot, {
			activityHint: null,
			capabilities: ['sessionLifecycle', 'turnLifecycle'],
			eventId: eventId(3),
			generation: 1,
			lastOutcome: null,
			pendingRequests: [],
			sequence: 3,
			sessionId,
			turn: { kind: 'idle' },
			type: 'observation-reconnected',
		});

		expect(transition.snapshot.sessions[0]).toMatchObject({
			capabilities: ['sessionLifecycle', 'turnLifecycle'],
			observation: { freshness: 'fresh', generation: 1, sequence: 3 },
			turn: { kind: 'idle' },
		});
	});

	it('projects attention before outcomes and active work while retaining freshness', () => {
		let snapshot = apply(createAgentRuntimeSnapshot(), observed());
		snapshot = apply(snapshot, started(2));
		snapshot = apply(snapshot, {
			eventId: eventId(3),
			generation: 0,
			request: {
				actionPath: 'navigate-only',
				kind: 'question',
				requestId,
				turnId,
			},
			sequence: 3,
			sessionId,
			type: 'attention-requested',
		});

		expect(projectAgentLocation(snapshot, 'presentation-1')).toMatchObject({
			activeCount: 1,
			attentionCount: 1,
			freshness: 'fresh',
			providerIds: [providerId],
			status: { count: 1, kind: 'needs-attention' },
		});
	});

	it('fences presentation reuse until the exact prior binding generation retires', () => {
		let snapshot = apply(createAgentRuntimeSnapshot(), observed());
		const nextSessionId = AgentSessionIdSchema.parse('session-2');
		const replacement: Extract<AgentObservationEvent, { readonly type: 'session-observed' }> = {
			...observed(),
			eventId: eventId(3),
			executionLocation: {
				executionLocationId: AgentExecutionLocationIdSchema.parse('execution-2'),
			},
			observation: { ...observed().observation, sequence: 3 },
			presentationAttachments: [
				{
					generation: 'binding-2',
					navigable: true,
					presentationId: 'presentation-1',
					visibility: 'visible',
				},
			],
			sessionId: nextSessionId,
		};

		expect(transitionAgentRuntime(snapshot, replacement)).toMatchObject({
			kind: 'rejected',
			reason: 'presentation-generation-mismatch',
		});
		snapshot = apply(snapshot, {
			eventId: eventId(2),
			generation: 0,
			presentationGeneration: 'binding-1',
			presentationId: 'presentation-1',
			sequence: 2,
			sessionId,
			type: 'presentation-detached',
		});
		snapshot = apply(snapshot, replacement);

		expect(projectAgentLocation(snapshot, 'presentation-1')).toMatchObject({
			location: { generation: 'binding-2', navigable: true },
		});
	});

	it('fans one execution out to several presentation attachments and retires them independently', () => {
		let snapshot = apply(createAgentRuntimeSnapshot(), {
			...observed(),
			presentationAttachments: [
				{
					generation: 'binding-1',
					navigable: true,
					presentationId: 'presentation-1',
					visibility: 'hidden',
				},
			],
		});
		snapshot = apply(snapshot, {
			attachment: {
				generation: 'binding-remote-2',
				navigable: true,
				presentationId: 'presentation-remote-2',
				visibility: 'visible',
			},
			eventId: eventId(2),
			generation: 0,
			sequence: 2,
			sessionId,
			type: 'presentation-attached',
		});
		snapshot = apply(snapshot, started(3));

		expect(projectAgentLocation(snapshot, 'presentation-1')).toMatchObject({
			location: { visibility: 'hidden' },
			status: { kind: 'working' },
		});
		expect(projectAgentLocation(snapshot, 'presentation-remote-2')).toMatchObject({
			location: { visibility: 'visible' },
			status: { kind: 'working' },
		});

		snapshot = apply(snapshot, {
			eventId: eventId(4),
			generation: 0,
			presentationGeneration: 'binding-1',
			presentationId: 'presentation-1',
			sequence: 4,
			sessionId,
			type: 'presentation-detached',
		});
		expect(snapshot.sessions[0]).toMatchObject({ observation: { freshness: 'fresh' } });

		snapshot = apply(snapshot, {
			eventId: eventId(5),
			generation: 0,
			presentationGeneration: 'binding-remote-2',
			presentationId: 'presentation-remote-2',
			sequence: 5,
			sessionId,
			type: 'presentation-detached',
		});
		expect(snapshot.sessions[0]).toMatchObject({
			observation: { freshness: 'disconnected' },
		});
	});

	it('validates injected snapshots and rejects dispatch after runtime disposal', () => {
		expect(() => new AgentRuntime({ revision: -1, sessions: [] } as AgentRuntimeSnapshot)).toThrow(
			/revision/u,
		);
		const runtime = new AgentRuntime();
		runtime.dispose();

		expect(runtime.dispatch(observed())).toMatchObject({
			kind: 'rejected',
			reason: 'runtime-disposed',
		});
	});

	it('preserves all schema invariants across generated accepted and rejected sequences', () => {
		fc.assert(
			fc.property(
				fc.array(
					fc.record({
						activity: fc.constantFrom('tool' as const, 'editing' as const, 'command' as const),
						kind: fc.constantFrom(
							'start' as const,
							'activity' as const,
							'finish' as const,
							'stale' as const,
							'end' as const,
						),
						sequenceDelta: fc.integer({ max: 2, min: 0 }),
					}),
					{ maxLength: 100 },
				),
				(steps) => {
					let snapshot = apply(createAgentRuntimeSnapshot(), observed());
					let sequence = 1;
					for (const step of steps) {
						sequence += step.sequenceDelta;
						const event = generatedEvent(step.kind, step.activity, sequence);
						const transition = transitionAgentRuntime(snapshot, event);
						if (transition.kind === 'applied') snapshot = transition.snapshot;
						expect(AgentRuntimeSnapshotSchema.safeParse(snapshot).success).toBe(true);
					}
				},
			),
			{ numRuns: 200, seed: 0x0a6e17 },
		);
	});
});

function observed(): Extract<AgentObservationEvent, { readonly type: 'session-observed' }> {
	return {
		capabilities: ['sessionLifecycle', 'turnLifecycle', 'questions', 'approvals', 'activity'],
		eventId: eventId(1),
		executionLocation: { executionLocationId },
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
				generation: 'binding-1',
				navigable: true,
				presentationId: 'presentation-1',
				visibility: 'visible',
			},
		],
		providerId,
		sessionId,
		type: 'session-observed',
	};
}

function started(
	sequence: number,
): Extract<AgentObservationEvent, { readonly type: 'turn-started' }> {
	return {
		activity: 'thinking',
		eventId: eventId(sequence),
		generation: 0,
		phase: 'working',
		sequence,
		sessionId,
		turnId,
		type: 'turn-started',
	};
}

function generatedEvent(
	kind: 'activity' | 'end' | 'finish' | 'stale' | 'start',
	activity: 'command' | 'editing' | 'tool',
	sequence: number,
): AgentObservationEvent {
	switch (kind) {
		case 'start':
			return { ...started(sequence), activity };
		case 'activity':
			return {
				activity,
				eventId: eventId(sequence),
				generation: 0,
				phase: 'working',
				sequence,
				sessionId,
				turnId,
				type: 'turn-activity-changed',
			};
		case 'finish':
			return {
				eventId: eventId(sequence),
				generation: 0,
				outcome: 'succeeded',
				sequence,
				sessionId,
				turnId,
				type: 'turn-finished',
			};
		case 'stale':
			return {
				eventId: eventId(sequence),
				generation: 0,
				sequence,
				sessionId,
				type: 'observation-became-stale',
			};
		case 'end':
			return {
				eventId: eventId(sequence),
				generation: 0,
				sequence,
				sessionId,
				type: 'session-ended',
			};
	}
}

function apply(snapshot: AgentRuntimeSnapshot, event: AgentObservationEvent): AgentRuntimeSnapshot {
	const transition = transitionAgentRuntime(snapshot, event);
	if (transition.kind !== 'applied') {
		throw new Error(`Expected applied Agent transition, received ${transition.kind}.`);
	}
	return transition.snapshot;
}

function eventId(sequence: number) {
	return AgentEventIdSchema.parse(`event-${sequence}`);
}
