import { describe, expect, it } from 'vitest';

import { AgentSessionSnapshotSchema, type AgentSessionSnapshot } from '../../model';
import {
	AgentSessionListInputSchema,
	AgentSessionWaitInputSchema,
	AgentSessionWaitOutputSchema,
	DEFAULT_AGENT_WAIT_CONDITIONS,
} from '../agent-semantic-command-contracts';
import {
	explainAgentSession,
	matchingAgentWaitConditions,
	projectAgentSessionDetail,
	projectAgentSessionSummary,
} from '../agent-semantic-read-model';

describe('Agent semantic command contracts', () => {
	it('canonicalizes bounded list and wait defaults', () => {
		expect(AgentSessionListInputSchema.parse({})).toEqual({
			includeEnded: false,
			limit: 50,
		});
		expect(AgentSessionWaitInputSchema.parse({ sessionId: 'session-semantic' })).toEqual({
			sessionId: 'session-semantic',
			timeoutMilliseconds: 600_000,
			until: DEFAULT_AGENT_WAIT_CONDITIONS,
		});
	});

	it('rejects duplicate wait conditions and out-of-bound timeouts', () => {
		expect(
			AgentSessionWaitInputSchema.safeParse({
				sessionId: 'session-semantic',
				timeoutMilliseconds: 3_600_001,
				until: ['idle'],
			}).success,
		).toBe(false);
		expect(
			AgentSessionWaitInputSchema.safeParse({
				sessionId: 'session-semantic',
				until: ['idle', 'idle'],
			}).success,
		).toBe(false);
		expect(
			AgentSessionWaitOutputSchema.safeParse({
				matchedConditions: ['fresh', 'fresh'],
				revision: 1,
				session: projectAgentSessionDetail(sessionSnapshot()),
			}).success,
		).toBe(false);
	});
});

describe('Agent semantic read model', () => {
	it('projects bounded summaries and exact detail from the canonical Session', () => {
		const session = sessionSnapshot();

		expect(projectAgentSessionSummary(session)).toMatchObject({
			pendingRequestCount: 1,
			providerId: 'codex',
			sessionId: 'session-semantic',
			status: { count: 1, kind: 'needs-attention' },
		});
		expect(projectAgentSessionDetail(session)).toEqual({
			snapshot: session,
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
