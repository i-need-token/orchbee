import { describe, expect, it } from 'vitest';

import { AgentSessionSnapshotSchema, type AgentSessionSnapshot } from '../../model';

import {
	explainAgentSession,
	matchingAgentWaitConditions,
	projectAgentSessionSummary,
} from '../agent-semantic-read-model';

describe('Agent semantic read model', () => {
	it('projects bounded summaries from the canonical Session', () => {
		const session = sessionSnapshot();

		expect(projectAgentSessionSummary(session)).toMatchObject({
			pendingRequestCount: 1,
			providerId: 'codex',
			sessionId: 'session-semantic',
			status: { count: 1, kind: 'needs-attention' },
		});
	});

	it('explains canonical provenance without provider or terminal content', () => {
		const explanation = explainAgentSession(sessionSnapshot());

		expect(explanation).toEqual({
			actionPath: 'navigate-only',
			capabilityCeiling: ['approvals', 'turnLifecycle'],
			contentAvailable: false,
			fidelity: 'rich',
			freshness: 'fresh',
			freshnessMeaning: 'current-observation',
			generation: 3,
			sequence: 7,
			source: 'hook',
			status: { count: 1, kind: 'needs-attention' },
			statusBasis: 'attention-request',
		});
		expect(JSON.stringify(explanation)).not.toContain('secret prompt');
	});

	it('matches semantic conditions only while fresh and health conditions explicitly', () => {
		const fresh = sessionSnapshot();
		const stale = AgentSessionSnapshotSchema.parse({
			...fresh,
			observation: { ...fresh.observation, freshness: 'stale' },
		});

		expect(matchingAgentWaitConditions(fresh, ['needs-attention', 'fresh', 'stale'])).toEqual([
			'needs-attention',
			'fresh',
		]);
		expect(matchingAgentWaitConditions(stale, ['needs-attention', 'fresh', 'stale'])).toEqual([
			'stale',
		]);
	});
});

function sessionSnapshot(): AgentSessionSnapshot {
	return AgentSessionSnapshotSchema.parse({
		activityHint: null,
		capabilities: ['approvals', 'turnLifecycle'],
		executionLocation: { executionLocationId: 'execution-semantic' },
		lastOutcome: null,
		lifecycle: 'live',
		observation: {
			fidelity: 'rich',
			freshness: 'fresh',
			generation: 3,
			sequence: 7,
			source: 'hook',
		},
		parentSessionId: null,
		pendingRequests: [
			{
				actionPath: 'navigate-only',
				kind: 'approval',
				requestId: 'request-semantic',
				turnId: 'turn-semantic',
			},
		],
		presentationAttachments: [
			{
				generation: 'attachment-semantic',
				navigable: true,
				presentationId: 'presentation-semantic',
				visibility: 'hidden',
			},
		],
		providerId: 'codex',
		sessionId: 'session-semantic',
		turn: {
			activity: 'tool',
			kind: 'active',
			phase: 'waiting-external',
			turnId: 'turn-semantic',
		},
	});
}
