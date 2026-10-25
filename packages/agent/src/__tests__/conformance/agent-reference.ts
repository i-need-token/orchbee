import type { AgentSessionSnapshot } from '../../model';
import { projectAgentSession } from '../../projection';
import type {
	AgentFreshnessMeaning,
	AgentSessionExplanation,
	AgentSessionSummary,
	AgentStatusBasis,
	AgentWaitCondition,
} from './agent-reference-contracts';

export function projectAgentSessionSummary(session: AgentSessionSnapshot): AgentSessionSummary {
	return {
		executionLocation: session.executionLocation,
		lifecycle: session.lifecycle,
		observation: session.observation,
		parentSessionId: session.parentSessionId,
		pendingRequestCount: session.pendingRequests.length,
		presentationAttachments: session.presentationAttachments,
		providerId: session.providerId,
		sessionId: session.sessionId,
		status: projectAgentSession(session).status,
	};
}

export function explainAgentSession(session: AgentSessionSnapshot): AgentSessionExplanation {
	return {
		actionPath: 'navigate-only',
		capabilityCeiling: session.capabilities,
		contentAvailable: false,
		fidelity: session.observation.fidelity,
		freshness: session.observation.freshness,
		freshnessMeaning: freshnessMeaning(session),
		generation: session.observation.generation,
		sequence: session.observation.sequence,
		source: session.observation.source,
		status: projectAgentSession(session).status,
		statusBasis: statusBasis(session),
	};
}

export function matchingAgentWaitConditions(
	session: AgentSessionSnapshot,
	conditions: readonly AgentWaitCondition[],
): readonly AgentWaitCondition[] {
	const status = projectAgentSession(session).status.kind;
	return conditions.filter((condition) => {
		if (condition === 'fresh' || condition === 'stale' || condition === 'disconnected') {
			return session.observation.freshness === condition;
		}
		return session.observation.freshness === 'fresh' && status === condition;
	});
}

function statusBasis(session: AgentSessionSnapshot): AgentStatusBasis {
	if (session.lifecycle === 'ended') return 'session-ended';
	if (session.pendingRequests.length > 0) return 'attention-request';
	if (session.lastOutcome !== null) return 'turn-outcome';
	if (session.turn.kind === 'active') return 'active-turn';
	if (session.turn.kind === 'idle') return 'affirmative-idle';
	return session.activityHint === null ? 'unobserved-turn-state' : 'limited-activity-hint';
}

function freshnessMeaning(session: AgentSessionSnapshot): AgentFreshnessMeaning {
	switch (session.observation.freshness) {
		case 'fresh':
			return 'current-observation';
		case 'stale':
			return 'last-observation-may-be-outdated';
		case 'disconnected':
			return 'no-trusted-current-route';
	}
}
