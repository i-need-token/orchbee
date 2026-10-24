import type { AgentObservationEvent, AgentSemanticEvent } from './events';
import {
	AgentRuntimeSnapshotSchema,
	AgentSessionSnapshotSchema,
	compareAgentIdentity,
	type AgentCapability,
	type AgentPresentationAttachment,
	type AgentRuntimeSnapshot,
	type AgentSessionSnapshot,
} from './model';

type AgentTransitionRejection =
	| 'capability-not-declared'
	| 'capacity-exceeded'
	| 'generation-mismatch'
	| 'identity-conflict'
	| 'invalid-event'
	| 'invalid-parent'
	| 'presentation-generation-mismatch'
	| 'presentation-not-found'
	| 'request-conflict'
	| 'request-not-found'
	| 'runtime-disposed'
	| 'session-ended'
	| 'session-not-found'
	| 'source-capability-exceeded'
	| 'turn-conflict'
	| 'turn-not-found';

export type AgentTransition =
	| {
			readonly kind: 'applied';
			readonly semanticEvents: readonly AgentSemanticEvent[];
			readonly snapshot: AgentRuntimeSnapshot;
	  }
	| {
			readonly kind: 'unchanged';
			readonly semanticEvents: readonly [];
			readonly snapshot: AgentRuntimeSnapshot;
	  }
	| {
			readonly kind: 'rejected';
			readonly reason: AgentTransitionRejection;
			readonly semanticEvents: readonly [];
			readonly snapshot: AgentRuntimeSnapshot;
	  };

const EMPTY_SEMANTIC_EVENTS: readonly [] = Object.freeze([]);
const MAX_AGENT_PENDING_REQUESTS = 128;
const MAX_AGENT_PRESENTATION_ATTACHMENTS = 32;
const MAX_AGENT_SESSIONS = 512;

export function createAgentRuntimeSnapshot(): AgentRuntimeSnapshot {
	return AgentRuntimeSnapshotSchema.parse({ revision: 0, sessions: [] });
}

export function transitionAgentRuntime(
	snapshot: AgentRuntimeSnapshot,
	event: AgentObservationEvent,
): AgentTransition {
	if (event.type === 'session-observed') return observeSession(snapshot, event);
	if (!validOrderedIdentity(event.generation) || !validOrderedIdentity(event.sequence)) {
		return rejected(snapshot, 'invalid-event');
	}
	const index = snapshot.sessions.findIndex((session) => session.sessionId === event.sessionId);
	if (index < 0) return rejected(snapshot, 'session-not-found');
	const session = snapshot.sessions[index];
	if (session === undefined) return rejected(snapshot, 'session-not-found');
	if (event.generation !== session.observation.generation) {
		if (
			event.type === 'observation-reconnected' &&
			event.generation > session.observation.generation
		) {
			return reconnectObservation(snapshot, index, session, event);
		}
		return rejected(snapshot, 'generation-mismatch');
	}
	if (event.sequence <= session.observation.sequence) return unchanged(snapshot);

	switch (event.type) {
		case 'turn-started':
			return startTurn(snapshot, index, session, event);
		case 'turn-activity-changed':
			return changeTurnActivity(snapshot, index, session, event);
		case 'attention-requested':
			return requestAttention(snapshot, index, session, event);
		case 'attention-resolved':
			return resolveAttention(snapshot, index, session, event);
		case 'turn-finished':
			return finishTurn(snapshot, index, session, event);
		case 'session-idle-observed':
			return observeIdle(snapshot, index, session, event);
		case 'observation-became-stale':
			return becomeStale(snapshot, index, session, event);
		case 'presentation-attached':
			return attachPresentation(snapshot, index, session, event);
		case 'presentation-visibility-changed':
			return changePresentationVisibility(snapshot, index, session, event);
		case 'presentation-detached':
			return detachPresentation(snapshot, index, session, event);
		case 'session-ended':
			return endSession(snapshot, index, session, event);
		case 'observation-reconnected':
			return rejected(snapshot, 'generation-mismatch');
	}
}

function observeSession(
	snapshot: AgentRuntimeSnapshot,
	event: Extract<AgentObservationEvent, { readonly type: 'session-observed' }>,
): AgentTransition {
	if (
		!validOrderedIdentity(event.observation.generation) ||
		!validOrderedIdentity(event.observation.sequence) ||
		!observationSourceIsValid(event.observation.source, event.observation.fidelity) ||
		!capabilitiesFitSource(event.capabilities, event.observation.source)
	) {
		return rejected(snapshot, 'source-capability-exceeded');
	}
	const existing = snapshot.sessions.find((session) => session.sessionId === event.sessionId);
	if (existing !== undefined) {
		if (
			existing.providerId === event.providerId &&
			existing.executionLocation.executionLocationId ===
				event.executionLocation.executionLocationId &&
			attachmentsEqual(existing.presentationAttachments, event.presentationAttachments)
		) {
			return unchanged(snapshot);
		}
		return rejected(snapshot, 'identity-conflict');
	}
	if (snapshot.sessions.length >= MAX_AGENT_SESSIONS) {
		return rejected(snapshot, 'capacity-exceeded');
	}
	if (event.presentationAttachments.length > MAX_AGENT_PRESENTATION_ATTACHMENTS) {
		return rejected(snapshot, 'capacity-exceeded');
	}
	if (
		event.presentationAttachments.some((attachment) =>
			presentationGenerationConflicts(snapshot, attachment),
		)
	) {
		return rejected(snapshot, 'presentation-generation-mismatch');
	}
	if (
		event.parentSessionId !== null &&
		!snapshot.sessions.some((session) => session.sessionId === event.parentSessionId)
	) {
		return rejected(snapshot, 'invalid-parent');
	}
	if (event.parentSessionId !== null && !event.capabilities.includes('subagents')) {
		return rejected(snapshot, 'capability-not-declared');
	}
	const session: AgentSessionSnapshot = {
		activityHint: null,
		capabilities: uniqueCapabilities(event.capabilities),
		executionLocation: { ...event.executionLocation },
		lastOutcome: null,
		lifecycle: 'live',
		observation: { ...event.observation },
		parentSessionId: event.parentSessionId,
		pendingRequests: [],
		presentationAttachments: sortAttachments(event.presentationAttachments),
		providerId: event.providerId,
		sessionId: event.sessionId,
		turn: { kind: 'unknown' },
	};
	if (!AgentSessionSnapshotSchema.safeParse(session).success) {
		return rejected(snapshot, 'invalid-event');
	}
	return applied(snapshot, [...snapshot.sessions, session], []);
}

function startTurn(
	snapshot: AgentRuntimeSnapshot,
	index: number,
	session: AgentSessionSnapshot,
	event: Extract<AgentObservationEvent, { readonly type: 'turn-started' }>,
): AgentTransition {
	const unavailable = requireLiveCapability(snapshot, session, 'turnLifecycle');
	if (unavailable !== null) return unavailable;
	if (session.turn.kind === 'active') {
		return session.turn.turnId === event.turnId
			? unchanged(snapshot)
			: rejected(snapshot, 'turn-conflict');
	}
	return replaceSession(snapshot, index, {
		...session,
		activityHint: null,
		lastOutcome: null,
		observation: advanceObservation(session, event.sequence),
		turn: {
			activity: event.activity,
			kind: 'active',
			phase: event.phase,
			turnId: event.turnId,
		},
	});
}

function changeTurnActivity(
	snapshot: AgentRuntimeSnapshot,
	index: number,
	session: AgentSessionSnapshot,
	event: Extract<AgentObservationEvent, { readonly type: 'turn-activity-changed' }>,
): AgentTransition {
	const unavailable = requireLiveCapability(snapshot, session, 'activity');
	if (unavailable !== null) return unavailable;
	if (session.turn.kind !== 'active' || session.turn.turnId !== event.turnId) {
		return rejected(snapshot, 'turn-not-found');
	}
	return replaceSession(snapshot, index, {
		...session,
		observation: advanceObservation(session, event.sequence),
		turn: { ...session.turn, activity: event.activity, phase: event.phase },
	});
}

function requestAttention(
	snapshot: AgentRuntimeSnapshot,
	index: number,
	session: AgentSessionSnapshot,
	event: Extract<AgentObservationEvent, { readonly type: 'attention-requested' }>,
): AgentTransition {
	if (session.lifecycle !== 'live') return rejected(snapshot, 'session-ended');
	const requiredCapability = attentionCapability(event.request.kind);
	if (
		requiredCapability === null
			? !hasAnyAttentionCapability(session.capabilities)
			: !session.capabilities.includes(requiredCapability)
	) {
		return rejected(snapshot, 'capability-not-declared');
	}
	if (
		event.request.turnId !== null &&
		(session.turn.kind !== 'active' || session.turn.turnId !== event.request.turnId)
	) {
		return rejected(snapshot, 'turn-not-found');
	}
	const existing = session.pendingRequests.find(
		(request) => request.requestId === event.request.requestId,
	);
	if (existing !== undefined) {
		return requestsEqual(existing, event.request)
			? unchanged(snapshot)
			: rejected(snapshot, 'request-conflict');
	}
	if (session.pendingRequests.length >= MAX_AGENT_PENDING_REQUESTS) {
		return rejected(snapshot, 'capacity-exceeded');
	}
	const nextSession: AgentSessionSnapshot = {
		...session,
		observation: advanceObservation(session, event.sequence),
		pendingRequests: [...session.pendingRequests, { ...event.request }].sort((left, right) =>
			compareAgentIdentity(left.requestId, right.requestId),
		),
	};
	return replaceSession(snapshot, index, nextSession, [
		{
			eventId: event.eventId,
			request: event.request,
			sessionId: session.sessionId,
			type: 'attention-requested',
		},
	]);
}

function resolveAttention(
	snapshot: AgentRuntimeSnapshot,
	index: number,
	session: AgentSessionSnapshot,
	event: Extract<AgentObservationEvent, { readonly type: 'attention-resolved' }>,
): AgentTransition {
	if (session.lifecycle !== 'live') return rejected(snapshot, 'session-ended');
	if (!session.pendingRequests.some((request) => request.requestId === event.requestId)) {
		return rejected(snapshot, 'request-not-found');
	}
	return replaceSession(
		snapshot,
		index,
		{
			...session,
			observation: advanceObservation(session, event.sequence),
			pendingRequests: session.pendingRequests.filter(
				(request) => request.requestId !== event.requestId,
			),
		},
		[
			{
				eventId: event.eventId,
				requestId: event.requestId,
				sessionId: session.sessionId,
				type: 'attention-resolved',
			},
		],
	);
}

function finishTurn(
	snapshot: AgentRuntimeSnapshot,
	index: number,
	session: AgentSessionSnapshot,
	event: Extract<AgentObservationEvent, { readonly type: 'turn-finished' }>,
): AgentTransition {
	const unavailable = requireLiveCapability(snapshot, session, 'turnLifecycle');
	if (unavailable !== null) return unavailable;
	if (session.turn.kind !== 'active' || session.turn.turnId !== event.turnId) {
		return rejected(snapshot, 'turn-not-found');
	}
	return replaceSession(
		snapshot,
		index,
		{
			...session,
			lastOutcome: { eventId: event.eventId, kind: event.outcome, turnId: event.turnId },
			observation: advanceObservation(session, event.sequence),
			pendingRequests: session.pendingRequests.filter((request) => request.turnId !== event.turnId),
			turn: { kind: 'idle' },
		},
		[
			{
				eventId: event.eventId,
				outcome: event.outcome,
				sessionId: session.sessionId,
				turnId: event.turnId,
				type: 'turn-outcome',
			},
		],
	);
}

function observeIdle(
	snapshot: AgentRuntimeSnapshot,
	index: number,
	session: AgentSessionSnapshot,
	event: Extract<AgentObservationEvent, { readonly type: 'session-idle-observed' }>,
): AgentTransition {
	const unavailable = requireLiveCapability(snapshot, session, 'turnLifecycle');
	if (unavailable !== null) return unavailable;
	const activeTurnId = session.turn.kind === 'active' ? session.turn.turnId : null;
	return replaceSession(snapshot, index, {
		...session,
		observation: advanceObservation(session, event.sequence),
		pendingRequests:
			activeTurnId === null
				? session.pendingRequests
				: session.pendingRequests.filter((request) => request.turnId !== activeTurnId),
		turn: { kind: 'idle' },
	});
}

function becomeStale(
	snapshot: AgentRuntimeSnapshot,
	index: number,
	session: AgentSessionSnapshot,
	event: Extract<AgentObservationEvent, { readonly type: 'observation-became-stale' }>,
): AgentTransition {
	if (session.observation.freshness === 'disconnected') return unchanged(snapshot);
	return replaceSession(snapshot, index, {
		...session,
		observation: {
			...advanceObservation(session, event.sequence),
			freshness: 'stale',
		},
	});
}

function reconnectObservation(
	snapshot: AgentRuntimeSnapshot,
	index: number,
	session: AgentSessionSnapshot,
	event: Extract<AgentObservationEvent, { readonly type: 'observation-reconnected' }>,
): AgentTransition {
	if (event.sequence <= session.observation.sequence || session.lifecycle !== 'live') {
		return event.sequence <= session.observation.sequence
			? unchanged(snapshot)
			: rejected(snapshot, 'session-ended');
	}
	if (!capabilitiesFitSource(event.capabilities, session.observation.source)) {
		return rejected(snapshot, 'source-capability-exceeded');
	}
	const candidate: AgentSessionSnapshot = {
		...session,
		activityHint: event.activityHint,
		capabilities: uniqueCapabilities(event.capabilities),
		lastOutcome: event.lastOutcome,
		observation: {
			...session.observation,
			freshness: 'fresh',
			generation: event.generation,
			sequence: event.sequence,
		},
		pendingRequests: [...event.pendingRequests],
		turn: event.turn,
	};
	if (!sessionClaimsFitCapabilities(candidate)) {
		return rejected(snapshot, 'capability-not-declared');
	}
	const sessions = [...snapshot.sessions];
	sessions[index] = candidate;
	if (!AgentRuntimeSnapshotSchema.safeParse({ ...snapshot, sessions }).success) {
		return rejected(snapshot, 'invalid-event');
	}
	return replaceSession(snapshot, index, candidate);
}

function attachPresentation(
	snapshot: AgentRuntimeSnapshot,
	index: number,
	session: AgentSessionSnapshot,
	event: Extract<AgentObservationEvent, { readonly type: 'presentation-attached' }>,
): AgentTransition {
	if (session.lifecycle !== 'live') return rejected(snapshot, 'session-ended');
	if (!event.attachment.navigable) return rejected(snapshot, 'invalid-event');
	if (presentationGenerationConflicts(snapshot, event.attachment)) {
		return rejected(snapshot, 'presentation-generation-mismatch');
	}
	const attachmentIndex = findAttachmentIndex(
		session.presentationAttachments,
		event.attachment.presentationId,
		event.attachment.generation,
	);
	if (attachmentIndex >= 0) {
		const existing = session.presentationAttachments[attachmentIndex];
		if (existing === undefined) return rejected(snapshot, 'presentation-not-found');
		if (presentationAttachmentsEqual(existing, event.attachment)) return unchanged(snapshot);
		const presentationAttachments = [...session.presentationAttachments];
		presentationAttachments[attachmentIndex] = { ...event.attachment };
		return replaceSession(snapshot, index, {
			...session,
			observation: advanceObservation(session, event.sequence),
			presentationAttachments: sortAttachments(presentationAttachments),
		});
	}
	if (session.presentationAttachments.length >= MAX_AGENT_PRESENTATION_ATTACHMENTS) {
		return rejected(snapshot, 'capacity-exceeded');
	}
	return replaceSession(snapshot, index, {
		...session,
		observation: advanceObservation(session, event.sequence),
		presentationAttachments: sortAttachments([
			...session.presentationAttachments,
			{ ...event.attachment },
		]),
	});
}

function changePresentationVisibility(
	snapshot: AgentRuntimeSnapshot,
	index: number,
	session: AgentSessionSnapshot,
	event: Extract<AgentObservationEvent, { readonly type: 'presentation-visibility-changed' }>,
): AgentTransition {
	const attachmentIndex = findAttachmentIndex(
		session.presentationAttachments,
		event.presentationId,
		event.presentationGeneration,
	);
	if (attachmentIndex < 0) return rejected(snapshot, 'presentation-not-found');
	const attachment = session.presentationAttachments[attachmentIndex];
	if (attachment === undefined || !attachment.navigable) {
		return rejected(snapshot, 'presentation-not-found');
	}
	if (attachment.visibility === event.visibility) return unchanged(snapshot);
	const presentationAttachments = [...session.presentationAttachments];
	presentationAttachments[attachmentIndex] = {
		...attachment,
		visibility: event.visibility,
	};
	return replaceSession(snapshot, index, {
		...session,
		observation: advanceObservation(session, event.sequence),
		presentationAttachments,
	});
}

function detachPresentation(
	snapshot: AgentRuntimeSnapshot,
	index: number,
	session: AgentSessionSnapshot,
	event: Extract<AgentObservationEvent, { readonly type: 'presentation-detached' }>,
): AgentTransition {
	const attachmentIndex = findAttachmentIndex(
		session.presentationAttachments,
		event.presentationId,
		event.presentationGeneration,
	);
	if (attachmentIndex < 0) return rejected(snapshot, 'presentation-not-found');
	const attachment = session.presentationAttachments[attachmentIndex];
	if (attachment === undefined) return rejected(snapshot, 'presentation-not-found');
	if (!attachment.navigable) return unchanged(snapshot);
	const presentationAttachments = [...session.presentationAttachments];
	presentationAttachments[attachmentIndex] = {
		...attachment,
		navigable: false,
		visibility: 'unknown',
	};
	const hasNavigableAttachment = presentationAttachments.some((candidate) => candidate.navigable);
	return replaceSession(snapshot, index, {
		...session,
		observation: {
			...advanceObservation(session, event.sequence),
			freshness: hasNavigableAttachment ? session.observation.freshness : 'disconnected',
		},
		presentationAttachments,
	});
}

function endSession(
	snapshot: AgentRuntimeSnapshot,
	index: number,
	session: AgentSessionSnapshot,
	event: Extract<AgentObservationEvent, { readonly type: 'session-ended' }>,
): AgentTransition {
	const retiresHeuristicPresence =
		session.observation.source === 'heuristic' &&
		session.observation.fidelity === 'heuristic' &&
		session.capabilities.length === 0;
	if (session.lifecycle !== 'live') return rejected(snapshot, 'session-ended');
	const unavailable = retiresHeuristicPresence
		? null
		: requireLiveCapability(snapshot, session, 'sessionLifecycle');
	if (unavailable !== null) return unavailable;
	const interrupted =
		session.turn.kind === 'active'
			? {
					eventId: event.eventId,
					kind: 'interrupted' as const,
					turnId: session.turn.turnId,
				}
			: session.lastOutcome;
	const semanticEvents: AgentSemanticEvent[] = [];
	if (!retiresHeuristicPresence && session.turn.kind === 'active') {
		semanticEvents.push({
			eventId: event.eventId,
			outcome: 'interrupted',
			sessionId: session.sessionId,
			turnId: session.turn.turnId,
			type: 'turn-outcome',
		});
	}
	if (!retiresHeuristicPresence) {
		semanticEvents.push({
			eventId: event.eventId,
			sessionId: session.sessionId,
			type: 'session-ended',
		});
	}
	return replaceSession(
		snapshot,
		index,
		{
			...session,
			activityHint: null,
			lastOutcome: interrupted,
			lifecycle: 'ended',
			observation: retiresHeuristicPresence
				? {
						...advanceObservation(session, event.sequence),
						freshness: 'disconnected',
					}
				: advanceObservation(session, event.sequence),
			pendingRequests: [],
			turn: { kind: 'unknown' },
		},
		semanticEvents,
	);
}

function requireLiveCapability(
	snapshot: AgentRuntimeSnapshot,
	session: AgentSessionSnapshot,
	capability: AgentCapability,
): AgentTransition | null {
	if (session.lifecycle !== 'live') return rejected(snapshot, 'session-ended');
	return session.capabilities.includes(capability)
		? null
		: rejected(snapshot, 'capability-not-declared');
}

function replaceSession(
	snapshot: AgentRuntimeSnapshot,
	index: number,
	session: AgentSessionSnapshot,
	semanticEvents: readonly AgentSemanticEvent[] = [],
): AgentTransition {
	const sessions = [...snapshot.sessions];
	sessions[index] = session;
	return applied(snapshot, sessions, semanticEvents);
}

function applied(
	previous: AgentRuntimeSnapshot,
	sessions: readonly AgentSessionSnapshot[],
	semanticEvents: readonly AgentSemanticEvent[],
): AgentTransition {
	if (previous.revision >= Number.MAX_SAFE_INTEGER) {
		return rejected(previous, 'capacity-exceeded');
	}
	const snapshot = AgentRuntimeSnapshotSchema.parse({
		revision: previous.revision + 1,
		sessions: [...sessions].sort((left, right) =>
			compareAgentIdentity(left.sessionId, right.sessionId),
		),
	});
	return { kind: 'applied', semanticEvents: [...semanticEvents], snapshot };
}

function unchanged(snapshot: AgentRuntimeSnapshot): AgentTransition {
	return { kind: 'unchanged', semanticEvents: EMPTY_SEMANTIC_EVENTS, snapshot };
}

function rejected(
	snapshot: AgentRuntimeSnapshot,
	reason: AgentTransitionRejection,
): AgentTransition {
	return { kind: 'rejected', reason, semanticEvents: EMPTY_SEMANTIC_EVENTS, snapshot };
}

function advanceObservation(session: AgentSessionSnapshot, sequence: number) {
	return { ...session.observation, freshness: 'fresh' as const, sequence };
}

function validOrderedIdentity(value: number): boolean {
	return Number.isSafeInteger(value) && value >= 0;
}

function uniqueCapabilities(capabilities: readonly AgentCapability[]): AgentCapability[] {
	return [...new Set(capabilities)].sort();
}

function observationSourceIsValid(
	source: AgentSessionSnapshot['observation']['source'],
	fidelity: AgentSessionSnapshot['observation']['fidelity'],
): boolean {
	if (source === 'heuristic') return fidelity === 'heuristic';
	if (source === 'terminal') return fidelity !== 'rich';
	return fidelity !== 'heuristic';
}

function capabilitiesFitSource(
	capabilities: readonly AgentCapability[],
	source: AgentSessionSnapshot['observation']['source'],
): boolean {
	if (source === 'heuristic') return capabilities.length === 0;
	if (source === 'terminal') return capabilities.every((capability) => capability === 'activity');
	return true;
}

function attentionCapability(
	kind: AgentSessionSnapshot['pendingRequests'][number]['kind'],
): AgentCapability | null {
	switch (kind) {
		case 'question':
			return 'questions';
		case 'approval':
			return 'approvals';
		case 'authentication':
			return 'authentication';
		case 'unclassified':
			return null;
	}
}

function hasAnyAttentionCapability(capabilities: readonly AgentCapability[]): boolean {
	return capabilities.some(
		(capability) =>
			capability === 'questions' || capability === 'approvals' || capability === 'authentication',
	);
}

function sessionClaimsFitCapabilities(session: AgentSessionSnapshot): boolean {
	if (
		session.activityHint !== null &&
		(!session.capabilities.includes('activity') || session.observation.source !== 'terminal')
	) {
		return false;
	}
	if (
		(session.turn.kind !== 'unknown' || session.lastOutcome !== null) &&
		!session.capabilities.includes('turnLifecycle')
	) {
		return false;
	}
	return session.pendingRequests.every((request) => {
		const requiredCapability = attentionCapability(request.kind);
		return requiredCapability === null
			? hasAnyAttentionCapability(session.capabilities)
			: session.capabilities.includes(requiredCapability);
	});
}

function requestsEqual(
	left: AgentSessionSnapshot['pendingRequests'][number],
	right: AgentSessionSnapshot['pendingRequests'][number],
): boolean {
	return (
		left.actionPath === right.actionPath &&
		left.kind === right.kind &&
		left.requestId === right.requestId &&
		left.turnId === right.turnId
	);
}

function findAttachmentIndex(
	attachments: readonly AgentPresentationAttachment[],
	presentationId: string,
	generation: string,
): number {
	return attachments.findIndex(
		(attachment) =>
			attachment.presentationId === presentationId && attachment.generation === generation,
	);
}

function sortAttachments(
	attachments: readonly AgentPresentationAttachment[],
): AgentPresentationAttachment[] {
	return attachments
		.map((attachment) => ({ ...attachment }))
		.sort((left, right) => {
			const presentationOrder = compareAgentIdentity(left.presentationId, right.presentationId);
			return presentationOrder === 0
				? compareAgentIdentity(left.generation, right.generation)
				: presentationOrder;
		});
}

function presentationAttachmentsEqual(
	left: AgentPresentationAttachment,
	right: AgentPresentationAttachment,
): boolean {
	return (
		left.generation === right.generation &&
		left.navigable === right.navigable &&
		left.presentationId === right.presentationId &&
		left.visibility === right.visibility
	);
}

function attachmentsEqual(
	left: readonly AgentPresentationAttachment[],
	right: readonly AgentPresentationAttachment[],
): boolean {
	if (left.length !== right.length) return false;
	const sortedLeft = sortAttachments(left);
	const sortedRight = sortAttachments(right);
	return sortedLeft.every((attachment, index) => {
		const candidate = sortedRight[index];
		return candidate !== undefined && presentationAttachmentsEqual(attachment, candidate);
	});
}

function presentationGenerationConflicts(
	snapshot: AgentRuntimeSnapshot,
	attachment: AgentPresentationAttachment,
): boolean {
	if (!attachment.navigable) return false;
	return snapshot.sessions.some((session) =>
		session.presentationAttachments.some(
			(candidate) =>
				candidate.navigable &&
				candidate.presentationId === attachment.presentationId &&
				candidate.generation !== attachment.generation,
		),
	);
}
