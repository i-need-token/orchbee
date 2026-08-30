import { describe, expect, it } from 'vitest';

import {
	AgentEventIdSchema,
	AgentProviderIdSchema,
	AgentRequestIdSchema,
	AgentRuntimeSnapshotSchema,
	AgentSessionIdSchema,
	AgentTurnIdSchema,
} from '../model';
import {
	projectAgentLocation,
	projectAgentPresentationGroup,
	projectAgentSessionGroup,
} from '../projection';

describe('projectAgentLocation', () => {
	it('selects attention before active work while retaining degraded freshness', () => {
		const turnId = AgentTurnIdSchema.parse('turn-projection');
		const snapshot = AgentRuntimeSnapshotSchema.parse({
			revision: 2,
			sessions: [
				{
					activityHint: null,
					capabilities: ['questions', 'turnLifecycle'],
					executionLocation: { executionLocationId: 'execution-projection' },
					lastOutcome: null,
					lifecycle: 'live',
					observation: {
						fidelity: 'rich',
						freshness: 'stale',
						generation: 0,
						sequence: 3,
						source: 'hook',
					},
					parentSessionId: null,
					pendingRequests: [
						{
							actionPath: 'navigate-only',
							kind: 'question',
							requestId: AgentRequestIdSchema.parse('request-projection'),
							turnId,
						},
					],
					presentationAttachments: [
						{
							generation: 'binding-projection',
							navigable: true,
							presentationId: 'presentation-projection',
							visibility: 'visible',
						},
					],
					providerId: AgentProviderIdSchema.parse('claude-code'),
					sessionId: AgentSessionIdSchema.parse('session-projection'),
					turn: { activity: 'thinking', kind: 'active', phase: 'working', turnId },
				},
			],
		});

		expect(projectAgentLocation(snapshot, 'presentation-projection')).toMatchObject({
			attentionCount: 1,
			freshness: 'stale',
			status: { count: 1, kind: 'needs-attention' },
		});
	});

	it('prefers a navigable replacement generation over a retired outcome', () => {
		const turnId = AgentTurnIdSchema.parse('turn-replacement');
		const snapshot = AgentRuntimeSnapshotSchema.parse({
			revision: 2,
			sessions: [
				{
					activityHint: null,
					capabilities: ['turnLifecycle'],
					executionLocation: { executionLocationId: 'execution-retired' },
					lastOutcome: {
						eventId: AgentEventIdSchema.parse('event-retired'),
						kind: 'failed',
						turnId,
					},
					lifecycle: 'live',
					observation: {
						fidelity: 'rich',
						freshness: 'disconnected',
						generation: 0,
						sequence: 4,
						source: 'native',
					},
					parentSessionId: null,
					pendingRequests: [],
					presentationAttachments: [
						{
							generation: 'binding-1',
							navigable: false,
							presentationId: 'presentation-reused',
							visibility: 'unknown',
						},
					],
					providerId: AgentProviderIdSchema.parse('codex'),
					sessionId: AgentSessionIdSchema.parse('session-retired'),
					turn: { kind: 'idle' },
				},
				{
					activityHint: null,
					capabilities: [],
					executionLocation: { executionLocationId: 'execution-replacement' },
					lastOutcome: null,
					lifecycle: 'live',
					observation: {
						fidelity: 'heuristic',
						freshness: 'fresh',
						generation: 0,
						sequence: 1,
						source: 'heuristic',
					},
					parentSessionId: null,
					pendingRequests: [],
					presentationAttachments: [
						{
							generation: 'binding-2',
							navigable: true,
							presentationId: 'presentation-reused',
							visibility: 'visible',
						},
					],
					providerId: AgentProviderIdSchema.parse('unknown-agent'),
					sessionId: AgentSessionIdSchema.parse('session-replacement'),
					turn: { kind: 'unknown' },
				},
			],
		});

		expect(projectAgentLocation(snapshot, 'presentation-reused')).toMatchObject({
			location: { generation: 'binding-2', navigable: true },
			status: { kind: 'unknown' },
		});
	});

	it('derives freshness from the resolved status instead of lower-priority Sessions', () => {
		const workingTurnId = AgentTurnIdSchema.parse('turn-working');
		const snapshot = AgentRuntimeSnapshotSchema.parse({
			revision: 2,
			sessions: [
				{
					activityHint: null,
					capabilities: ['turnLifecycle'],
					executionLocation: { executionLocationId: 'execution-working' },
					lastOutcome: null,
					lifecycle: 'live',
					observation: {
						fidelity: 'rich',
						freshness: 'fresh',
						generation: 0,
						sequence: 2,
						source: 'native',
					},
					parentSessionId: null,
					pendingRequests: [],
					presentationAttachments: [
						{
							generation: 'binding-shared',
							navigable: true,
							presentationId: 'presentation-shared',
							visibility: 'visible',
						},
					],
					providerId: AgentProviderIdSchema.parse('codex'),
					sessionId: AgentSessionIdSchema.parse('session-working'),
					turn: {
						activity: 'thinking',
						kind: 'active',
						phase: 'working',
						turnId: workingTurnId,
					},
				},
				{
					activityHint: null,
					capabilities: ['turnLifecycle'],
					executionLocation: { executionLocationId: 'execution-idle' },
					lastOutcome: null,
					lifecycle: 'live',
					observation: {
						fidelity: 'rich',
						freshness: 'disconnected',
						generation: 0,
						sequence: 3,
						source: 'native',
					},
					parentSessionId: null,
					pendingRequests: [],
					presentationAttachments: [
						{
							generation: 'binding-shared',
							navigable: true,
							presentationId: 'presentation-shared',
							visibility: 'visible',
						},
					],
					providerId: AgentProviderIdSchema.parse('claude-code'),
					sessionId: AgentSessionIdSchema.parse('session-idle'),
					turn: { kind: 'idle' },
				},
			],
		});

		expect(projectAgentLocation(snapshot, 'presentation-shared')).toMatchObject({
			freshness: 'fresh',
			status: { kind: 'working' },
		});
	});

	it('excludes retained ended Sessions when the same location still has a live Session', () => {
		const outcomeTurnId = AgentTurnIdSchema.parse('turn-ended');
		const snapshot = AgentRuntimeSnapshotSchema.parse({
			revision: 2,
			sessions: [
				{
					activityHint: null,
					capabilities: ['turnLifecycle'],
					executionLocation: { executionLocationId: 'execution-ended' },
					lastOutcome: {
						eventId: AgentEventIdSchema.parse('event-ended'),
						kind: 'failed',
						turnId: outcomeTurnId,
					},
					lifecycle: 'ended',
					observation: {
						fidelity: 'rich',
						freshness: 'disconnected',
						generation: 0,
						sequence: 4,
						source: 'native',
					},
					parentSessionId: null,
					pendingRequests: [],
					presentationAttachments: [
						{
							generation: 'binding-shared-live',
							navigable: true,
							presentationId: 'presentation-shared-live',
							visibility: 'visible',
						},
					],
					providerId: AgentProviderIdSchema.parse('claude-code'),
					sessionId: AgentSessionIdSchema.parse('session-ended'),
					turn: { kind: 'unknown' },
				},
				{
					activityHint: null,
					capabilities: [],
					executionLocation: { executionLocationId: 'execution-live' },
					lastOutcome: null,
					lifecycle: 'live',
					observation: {
						fidelity: 'heuristic',
						freshness: 'fresh',
						generation: 0,
						sequence: 1,
						source: 'heuristic',
					},
					parentSessionId: null,
					pendingRequests: [],
					presentationAttachments: [
						{
							generation: 'binding-shared-live',
							navigable: true,
							presentationId: 'presentation-shared-live',
							visibility: 'visible',
						},
					],
					providerId: AgentProviderIdSchema.parse('generic-agent'),
					sessionId: AgentSessionIdSchema.parse('session-live'),
					turn: { kind: 'unknown' },
				},
			],
		});

		expect(projectAgentLocation(snapshot, 'presentation-shared-live')).toMatchObject({
			freshness: 'fresh',
			providerIds: [AgentProviderIdSchema.parse('generic-agent')],
			sessionCount: 1,
			status: { kind: 'unknown' },
		});
	});
});

describe('projectAgentPresentationGroup', () => {
	it('deduplicates a Session attached to multiple presentations in the same group', () => {
		const turnId = AgentTurnIdSchema.parse('turn-group');
		const snapshot = AgentRuntimeSnapshotSchema.parse({
			revision: 3,
			sessions: [
				{
					activityHint: null,
					capabilities: ['turnLifecycle'],
					executionLocation: { executionLocationId: 'execution-group-working' },
					lastOutcome: null,
					lifecycle: 'live',
					observation: {
						fidelity: 'rich',
						freshness: 'fresh',
						generation: 0,
						sequence: 2,
						source: 'hook',
					},
					parentSessionId: null,
					pendingRequests: [],
					presentationAttachments: [
						{
							generation: 'binding-group-a',
							navigable: true,
							presentationId: 'presentation-group-a',
							visibility: 'hidden',
						},
						{
							generation: 'binding-group-b',
							navigable: true,
							presentationId: 'presentation-group-b',
							visibility: 'visible',
						},
					],
					providerId: AgentProviderIdSchema.parse('codex'),
					sessionId: AgentSessionIdSchema.parse('session-group-working'),
					turn: { activity: 'thinking', kind: 'active', phase: 'working', turnId },
				},
				{
					activityHint: null,
					capabilities: ['questions'],
					executionLocation: { executionLocationId: 'execution-group-attention' },
					lastOutcome: null,
					lifecycle: 'live',
					observation: {
						fidelity: 'rich',
						freshness: 'fresh',
						generation: 0,
						sequence: 4,
						source: 'hook',
					},
					parentSessionId: null,
					pendingRequests: [
						{
							actionPath: 'navigate-only',
							kind: 'question',
							requestId: AgentRequestIdSchema.parse('request-group'),
							turnId: null,
						},
					],
					presentationAttachments: [
						{
							generation: 'binding-group-b',
							navigable: true,
							presentationId: 'presentation-group-b',
							visibility: 'hidden',
						},
					],
					providerId: AgentProviderIdSchema.parse('claude-code'),
					sessionId: AgentSessionIdSchema.parse('session-group-attention'),
					turn: { kind: 'idle' },
				},
			],
		});

		expect(
			projectAgentPresentationGroup(snapshot, ['presentation-group-a', 'presentation-group-b']),
		).toMatchObject({
			activeCount: 1,
			attentionCount: 1,
			providerIds: ['claude-code', 'codex'],
			sessionCount: 2,
			status: { count: 1, kind: 'needs-attention' },
		});
	});
});

describe('projectAgentSessionGroup', () => {
	it('aggregates an explicit deduplicated Session set without presentation selection', () => {
		const snapshot = AgentRuntimeSnapshotSchema.parse({
			revision: 1,
			sessions: [
				{
					activityHint: null,
					capabilities: [],
					executionLocation: { executionLocationId: 'explicit-execution-a' },
					lastOutcome: null,
					lifecycle: 'live',
					observation: {
						fidelity: 'heuristic',
						freshness: 'fresh',
						generation: 0,
						sequence: 1,
						source: 'heuristic',
					},
					parentSessionId: null,
					pendingRequests: [],
					presentationAttachments: [],
					providerId: AgentProviderIdSchema.parse('codex'),
					sessionId: AgentSessionIdSchema.parse('explicit-session-a'),
					turn: { kind: 'idle' },
				},
				{
					activityHint: null,
					capabilities: [],
					executionLocation: { executionLocationId: 'explicit-execution-b' },
					lastOutcome: null,
					lifecycle: 'live',
					observation: {
						fidelity: 'heuristic',
						freshness: 'fresh',
						generation: 0,
						sequence: 1,
						source: 'heuristic',
					},
					parentSessionId: null,
					pendingRequests: [],
					presentationAttachments: [],
					providerId: AgentProviderIdSchema.parse('claude-code'),
					sessionId: AgentSessionIdSchema.parse('explicit-session-b'),
					turn: { kind: 'unknown' },
				},
			],
		});

		expect(projectAgentSessionGroup(snapshot.sessions)).toMatchObject({
			providerIds: [
				AgentProviderIdSchema.parse('claude-code'),
				AgentProviderIdSchema.parse('codex'),
			],
			sessionCount: 2,
			status: { kind: 'idle' },
		});
		expect(projectAgentSessionGroup([])).toBeNull();
	});
});
