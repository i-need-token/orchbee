import { describe, expect, it } from 'vitest';

import { AgentRuntime } from '../AgentRuntime';
import type { AgentObservationEvent } from '../events';
import {
	AgentEventIdSchema,
	AgentExecutionLocationIdSchema,
	AgentPresentationAttachmentSchema,
	AgentProviderIdSchema,
	AgentRequestIdSchema,
	AgentRuntimeSnapshotSchema,
	AgentSessionIdSchema,
	AgentSessionSnapshotSchema,
	type AgentPresentationAttachment,
	type AgentRuntimeSnapshot,
	type AgentSessionSnapshot,
} from '../model';
import { projectAgentPresentationGroup, projectAgentLocation } from '../projection';
import { createAgentRuntimeSnapshot, transitionAgentRuntime } from '../reducer';

const providerId = AgentProviderIdSchema.parse('claude-code');
const executionLocation = {
	executionLocationId: AgentExecutionLocationIdSchema.parse('execution-canonical'),
};

let eventCounter = 0;
function eventId() {
	eventCounter += 1;
	return AgentEventIdSchema.parse(`event-canonical-${eventCounter}`);
}

function attachment(presentationId: string, generation: string): AgentPresentationAttachment {
	return AgentPresentationAttachmentSchema.parse({
		generation,
		navigable: false,
		presentationId,
		visibility: 'visible',
	});
}

function observeSession(
	sessionIdText: string,
	presentationAttachments: readonly AgentPresentationAttachment[],
): Extract<AgentObservationEvent, { readonly type: 'session-observed' }> {
	return {
		capabilities: ['questions'],
		eventId: eventId(),
		executionLocation,
		observation: {
			fidelity: 'rich',
			freshness: 'fresh',
			generation: 0,
			sequence: 1,
			source: 'hook',
		},
		parentSessionId: null,
		presentationAttachments: [...presentationAttachments],
		providerId,
		sessionId: AgentSessionIdSchema.parse(sessionIdText),
		type: 'session-observed',
	};
}

function apply(snapshot: AgentRuntimeSnapshot, event: AgentObservationEvent): AgentRuntimeSnapshot {
	const transition = transitionAgentRuntime(snapshot, event);
	if (transition.kind !== 'applied') {
		throw new Error(`Expected applied transition, received ${transition.kind}`);
	}
	return transition.snapshot;
}

function sessionOrder(insertionOrder: readonly string[]): string[] {
	let snapshot = createAgentRuntimeSnapshot();
	for (const sessionId of insertionOrder) {
		snapshot = apply(snapshot, observeSession(sessionId, []));
	}
	return snapshot.sessions.map((session) => session.sessionId);
}

function attachmentOrder(presentationIds: readonly string[]): string[] {
	const snapshot = apply(
		createAgentRuntimeSnapshot(),
		observeSession(
			'session-canonical',
			presentationIds.map((presentationId, index) => attachment(presentationId, `gen-${index}`)),
		),
	);
	return (snapshot.sessions[0]?.presentationAttachments ?? []).map((entry) => entry.presentationId);
}

describe('canonical UTF-16 code-unit ordering', () => {
	it('orders Sessions by code-unit sessionId order independent of insertion order', () => {
		const ids = ['s:z', 's:_', 's:Z', 's:9', 's:.', 's::', 's:a', 's:-', 's:1', 's:A'];
		const expected = ['s:-', 's:.', 's:1', 's:9', 's::', 's:A', 's:Z', 's:_', 's:a', 's:z'];
		expect(sessionOrder(ids)).toEqual(expected);
		expect(sessionOrder([...ids].reverse())).toEqual(expected);
		expect(
			sessionOrder(['s:.', 's:-', 's:z', 's:1', 's:Z', 's:_', 's:9', 's::', 's:A', 's:a']),
		).toEqual(expected);
	});

	it.each([
		['B', 'a'],
		['A', 'a'],
		['I', 'i'],
		['9', 'A'],
		[':', 'A'],
		['-', '.'],
		['.', '0'],
		['10', '2'],
		['_', 'a'],
		['e\u0301', '\u00e9'],
		['ab', 'a\u00adb'],
		['ab', 'a\u200bb'],
		['a\u0000b', 'ab'],
		['中', '文'],
		['\u{10000}', '\ue000'],
		['\u{1f600}', '\uffff'],
		['z', '\u{1f600}'],
	] as const)('orders attachment presentationId %j before %j by code units', (lower, higher) => {
		expect(attachmentOrder([higher, lower])).toEqual([lower, higher]);
		expect(attachmentOrder([lower, higher])).toEqual([lower, higher]);
	});

	it('orders pending attention requests by code-unit requestId', () => {
		let snapshot = apply(createAgentRuntimeSnapshot(), observeSession('session-canonical', []));
		let sequence = 1;
		for (const requestIdText of ['r:a', 'r:1', 'r:A', 'r:-']) {
			sequence += 1;
			snapshot = apply(snapshot, {
				eventId: eventId(),
				generation: 0,
				request: {
					actionPath: 'navigate-only',
					kind: 'question',
					requestId: AgentRequestIdSchema.parse(requestIdText),
					turnId: null,
				},
				sequence,
				sessionId: AgentSessionIdSchema.parse('session-canonical'),
				type: 'attention-requested',
			});
		}
		expect(snapshot.sessions[0]?.pendingRequests.map((request) => request.requestId)).toEqual([
			'r:-',
			'r:1',
			'r:A',
			'r:a',
		]);
	});

	it('projects each attached presentation and counts the shared Session once', () => {
		const snapshot = apply(
			createAgentRuntimeSnapshot(),
			observeSession('session-canonical', [
				attachment('p:b', 'g:a'),
				attachment('p:A', 'g:a'),
				attachment('p:x', 'g:b'),
				attachment('p:1', 'g:a'),
				attachment('p:x', 'g:a'),
			]),
		);
		expect(
			['p:b', 'p:A', 'p:x', 'p:1'].map((id) => projectAgentLocation(snapshot, id)?.location),
		).toEqual([
			{ presentationId: 'p:b', generation: 'g:a', navigable: false, visibility: 'visible' },
			{ presentationId: 'p:A', generation: 'g:a', navigable: false, visibility: 'visible' },
			{ presentationId: 'p:x', generation: 'g:b', navigable: false, visibility: 'visible' },
			{ presentationId: 'p:1', generation: 'g:a', navigable: false, visibility: 'visible' },
		]);
		expect(
			projectAgentPresentationGroup(snapshot, ['p:b', 'p:A', 'p:x', 'p:1'])?.sessionCount,
		).toBe(1);
	});

	it('prefers the higher presentation generation on an exact tie (descending)', () => {
		const snapshot = apply(
			createAgentRuntimeSnapshot(),
			observeSession('session-canonical', [attachment('p:x', 'g:a'), attachment('p:x', 'g:b')]),
		);
		expect(projectAgentLocation(snapshot, 'p:x')?.location.generation).toBe('g:b');
	});

	it('treats a reordered identical attachment set as unchanged, not an identity conflict', () => {
		const snapshot = apply(
			createAgentRuntimeSnapshot(),
			observeSession('session-canonical', [
				attachment('\u00e9', 'g1'),
				attachment('e\u0301', 'g1'),
			]),
		);
		const reordered = transitionAgentRuntime(
			snapshot,
			observeSession('session-canonical', [
				attachment('e\u0301', 'g1'),
				attachment('\u00e9', 'g1'),
			]),
		);
		expect(reordered.kind).toBe('unchanged');
		expect(reordered.snapshot).toBe(snapshot);
	});
});

describe('well-formed presentation identity', () => {
	it.each([
		['a'.repeat(255), true],
		['a'.repeat(256), true],
		['a'.repeat(257), false],
		['\u{1f600}'.repeat(128), true],
		['\u{1f600}'.repeat(129), false],
		['a\ud800', false],
		['\udc00', false],
		['x\ud83dy', false],
		['\u{1f600}', true],
		['a\u{1f600}b', true],
		['a\u0000b', true],
		['\u00ad', true],
		['\u200b', true],
		['\u00e9', true],
		['e\u0301', true],
		['  a', true],
		['a  ', true],
		['', false],
	] as const)('presentationId of %j accepted=%s', (presentationId, expected) => {
		expect(
			AgentPresentationAttachmentSchema.safeParse({
				generation: 'gen-canonical',
				navigable: false,
				presentationId,
				visibility: 'visible',
			}).success,
		).toBe(expected);
	});

	it('rejects a lone-surrogate presentationId as a typed invalid-event on session-observed', () => {
		const event = {
			...observeSession('session-canonical', []),
			presentationAttachments: [
				{
					generation: 'g1',
					navigable: false,
					presentationId: 'a\ud800',
					visibility: 'visible',
				},
			],
		} as AgentObservationEvent;
		const transition = transitionAgentRuntime(createAgentRuntimeSnapshot(), event);
		expect(transition).toMatchObject({ kind: 'rejected', reason: 'invalid-event' });
	});
});

describe('maximum safe revision', () => {
	function sessionSnapshot(): AgentSessionSnapshot {
		return AgentSessionSnapshotSchema.parse({
			activityHint: null,
			capabilities: [],
			executionLocation,
			lastOutcome: null,
			lifecycle: 'live',
			observation: {
				fidelity: 'rich',
				freshness: 'fresh',
				generation: 0,
				sequence: 7,
				source: 'hook',
			},
			parentSessionId: null,
			pendingRequests: [],
			presentationAttachments: [],
			providerId,
			sessionId: 'session-canonical',
			turn: { kind: 'unknown' },
		});
	}

	function snapshotAtRevision(revision: number): AgentRuntimeSnapshot {
		return AgentRuntimeSnapshotSchema.parse({ revision, sessions: [sessionSnapshot()] });
	}

	function staleEvent(
		sequence: number,
	): Extract<AgentObservationEvent, { readonly type: 'observation-became-stale' }> {
		return {
			eventId: eventId(),
			generation: 0,
			sequence,
			sessionId: AgentSessionIdSchema.parse('session-canonical'),
			type: 'observation-became-stale',
		};
	}

	it('rejects a would-be-applied transition at MAX_SAFE_INTEGER as capacity-exceeded', () => {
		const snapshot = snapshotAtRevision(Number.MAX_SAFE_INTEGER);
		const transition = transitionAgentRuntime(snapshot, staleEvent(8));
		expect(transition).toMatchObject({ kind: 'rejected', reason: 'capacity-exceeded' });
		expect(transition.snapshot).toBe(snapshot);
		expect(transition.semanticEvents).toEqual([]);
	});

	it('publishes no listener or journal record for the exhausted-revision rejection', () => {
		const runtime = new AgentRuntime(snapshotAtRevision(Number.MAX_SAFE_INTEGER));
		let notified = 0;
		runtime.subscribe(() => {
			notified += 1;
		});
		runtime.subscribeSemanticEvents(() => {
			notified += 1;
		});
		const transition = runtime.dispatch(staleEvent(8));
		expect(transition.kind).toBe('rejected');
		expect(notified).toBe(0);
		expect(
			runtime.readChanges({ afterRevision: Number.MAX_SAFE_INTEGER, limit: 100 }),
		).toMatchObject({
			changes: [],
			kind: 'current',
		});
		expect(runtime.getSnapshot().revision).toBe(Number.MAX_SAFE_INTEGER);
	});

	it('preserves earlier unchanged and rejected outcomes at the maximum revision', () => {
		const snapshot = snapshotAtRevision(Number.MAX_SAFE_INTEGER);
		expect(transitionAgentRuntime(snapshot, staleEvent(7))).toMatchObject({
			kind: 'unchanged',
			snapshot,
		});
		expect(transitionAgentRuntime(snapshot, { ...staleEvent(8), generation: 1 })).toMatchObject({
			kind: 'rejected',
			reason: 'generation-mismatch',
		});
		expect(
			transitionAgentRuntime(snapshot, {
				...staleEvent(8),
				sessionId: AgentSessionIdSchema.parse('session-missing'),
			}),
		).toMatchObject({ kind: 'rejected', reason: 'session-not-found' });
		expect(transitionAgentRuntime(snapshot, observeSession('session-new', []))).toMatchObject({
			kind: 'rejected',
			reason: 'capacity-exceeded',
		});
	});

	it('still applies one revision below the maximum', () => {
		const snapshot = snapshotAtRevision(Number.MAX_SAFE_INTEGER - 1);
		expect(transitionAgentRuntime(snapshot, staleEvent(8)).kind).toBe('applied');
	});
});
