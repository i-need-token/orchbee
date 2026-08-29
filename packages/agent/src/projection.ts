import type {
	AgentActivity,
	AgentExecutionLocation,
	AgentObservation,
	AgentPresentationAttachment,
	AgentProviderId,
	AgentRuntimeSnapshot,
	AgentSessionId,
	AgentSessionSnapshot,
} from './model';

export type AgentSemanticStatus =
	| { readonly count: number; readonly kind: 'needs-attention' }
	| { readonly kind: 'failed' }
	| { readonly kind: 'succeeded' }
	| { readonly kind: 'canceled' | 'interrupted' }
	| {
			readonly activity: AgentActivity | null;
			readonly kind: 'working';
			readonly phase: 'queued' | 'working' | 'waiting-external';
	  }
	| { readonly kind: 'idle' }
	| { readonly activityHint: AgentActivity | null; readonly kind: 'unknown' }
	| { readonly kind: 'ended' };

export interface AgentSessionPresentation {
	readonly executionLocation: AgentExecutionLocation;
	readonly observation: AgentObservation;
	readonly presentationAttachments: readonly AgentPresentationAttachment[];
	readonly providerId: AgentProviderId;
	readonly sessionId: AgentSessionId;
	readonly status: AgentSemanticStatus;
}

export interface AgentAggregatePresentation {
	readonly activeCount: number;
	readonly attentionCount: number;
	readonly freshness: AgentObservation['freshness'];
	readonly providerIds: readonly AgentProviderId[];
	readonly sessionCount: number;
	readonly status: AgentSemanticStatus;
}

export interface AgentLocationPresentation extends AgentAggregatePresentation {
	readonly location: AgentPresentationAttachment;
}

export function projectAgentSession(session: AgentSessionSnapshot): AgentSessionPresentation {
	return {
		executionLocation: session.executionLocation,
		observation: session.observation,
		presentationAttachments: session.presentationAttachments,
		providerId: session.providerId,
		sessionId: session.sessionId,
		status: resolveStatus(session),
	};
}

export function projectAgentLocations(
	snapshot: AgentRuntimeSnapshot,
): readonly AgentLocationPresentation[] {
	const byLocation = new Map<
		string,
		{
			readonly location: AgentPresentationAttachment;
			readonly sessions: AgentSessionSnapshot[];
		}
	>();
	for (const session of snapshot.sessions) {
		for (const location of session.presentationAttachments) {
			const key = locationKey(location);
			const group = byLocation.get(key);
			if (group === undefined) byLocation.set(key, { location, sessions: [session] });
			else group.sessions.push(session);
		}
	}
	return [...byLocation.values()].map(projectLocation).sort((left, right) => {
		const presentationOrder = left.location.presentationId.localeCompare(
			right.location.presentationId,
		);
		return presentationOrder === 0
			? left.location.generation.localeCompare(right.location.generation)
			: presentationOrder;
	});
}

export function projectAgentLocation(
	snapshot: AgentRuntimeSnapshot,
	presentationId: string,
): AgentLocationPresentation | null {
	const candidates = projectAgentLocations(snapshot).filter(
		(presentation) => presentation.location.presentationId === presentationId,
	);
	return candidates.sort(compareLocationPreference)[0] ?? null;
}

export function projectAgentPresentationGroup(
	snapshot: AgentRuntimeSnapshot,
	presentationIds: readonly string[],
): AgentAggregatePresentation | null {
	const selectedLocations = new Map<string, AgentPresentationAttachment>();
	for (const presentationId of new Set(presentationIds)) {
		const presentation = projectAgentLocation(snapshot, presentationId);
		if (presentation !== null) {
			selectedLocations.set(presentationId, presentation.location);
		}
	}
	if (selectedLocations.size === 0) return null;

	const sessions = snapshot.sessions.filter((session) =>
		session.presentationAttachments.some((attachment) => {
			const selected = selectedLocations.get(attachment.presentationId);
			return selected?.generation === attachment.generation;
		}),
	);
	return sessions.length === 0 ? null : projectAggregate(sessions);
}

export function aggregateAgentSessions(
	sessions: readonly AgentSessionSnapshot[],
): AgentSemanticStatus {
	return resolveAggregate(sessions).status;
}

interface AgentAggregateResolution {
	readonly freshness: AgentObservation['freshness'];
	readonly status: AgentSemanticStatus;
}

function resolveAggregate(sessions: readonly AgentSessionSnapshot[]): AgentAggregateResolution {
	const presentations = sessions.map(projectAgentSession).sort(compareSessionPresentation);
	const highest = presentations[0];
	if (highest === undefined) {
		return { freshness: 'disconnected', status: { activityHint: null, kind: 'unknown' } };
	}
	if (highest.status.kind !== 'needs-attention') {
		return { freshness: highest.observation.freshness, status: highest.status };
	}
	const attentionPresentations = presentations.filter(
		(presentation) => presentation.status.kind === 'needs-attention',
	);
	return {
		freshness: aggregateFreshness(
			attentionPresentations.map((presentation) => presentation.observation),
		),
		status: {
			count: attentionPresentations.reduce(
				(total, presentation) =>
					total + (presentation.status.kind === 'needs-attention' ? presentation.status.count : 0),
				0,
			),
			kind: 'needs-attention',
		},
	};
}

function projectLocation(group: {
	readonly location: AgentPresentationAttachment;
	readonly sessions: readonly AgentSessionSnapshot[];
}): AgentLocationPresentation {
	const { location, sessions } = group;
	return { ...projectAggregate(sessions), location };
}

function projectAggregate(sessions: readonly AgentSessionSnapshot[]): AgentAggregatePresentation {
	const live = sessions.filter((session) => session.lifecycle === 'live');
	const retained = live.length > 0 ? live : sessions;
	const sorted = [...retained].sort((left, right) => left.sessionId.localeCompare(right.sessionId));
	const first = sorted[0];
	if (first === undefined) throw new Error('An Agent aggregate projection requires a Session.');
	const resolution = resolveAggregate(sorted);
	return {
		activeCount: sorted.filter((session) => session.turn.kind === 'active').length,
		attentionCount: sorted.reduce((total, session) => total + session.pendingRequests.length, 0),
		freshness: resolution.freshness,
		providerIds: [...new Set(sorted.map((session) => session.providerId))].sort(),
		sessionCount: sorted.length,
		status: resolution.status,
	};
}

function resolveStatus(session: AgentSessionSnapshot): AgentSemanticStatus {
	if (session.lifecycle === 'ended') return { kind: 'ended' };
	if (session.pendingRequests.length > 0) {
		return { count: session.pendingRequests.length, kind: 'needs-attention' };
	}
	if (session.lastOutcome !== null) return { kind: session.lastOutcome.kind };
	if (session.turn.kind === 'active') {
		return {
			activity: session.turn.activity,
			kind: 'working',
			phase: session.turn.phase,
		};
	}
	return session.turn.kind === 'idle'
		? { kind: 'idle' }
		: { activityHint: session.activityHint, kind: 'unknown' };
}

function compareSessionPresentation(
	left: AgentSessionPresentation,
	right: AgentSessionPresentation,
): number {
	const priority = statusPriority(right.status) - statusPriority(left.status);
	if (priority !== 0) return priority;
	const freshnessPriority =
		freshnessRank(right.observation.freshness) - freshnessRank(left.observation.freshness);
	if (freshnessPriority !== 0) return freshnessPriority;
	const activityHintPriority =
		Number(right.status.kind === 'unknown' && right.status.activityHint !== null) -
		Number(left.status.kind === 'unknown' && left.status.activityHint !== null);
	return activityHintPriority === 0
		? left.sessionId.localeCompare(right.sessionId)
		: activityHintPriority;
}

function statusPriority(status: AgentSemanticStatus): number {
	switch (status.kind) {
		case 'needs-attention':
			return 6;
		case 'failed':
		case 'interrupted':
		case 'canceled':
			return 5;
		case 'succeeded':
			return 4;
		case 'working':
			return 3;
		case 'idle':
			return 2;
		case 'unknown':
			return 1;
		case 'ended':
			return 0;
	}
}

function compareLocationPreference(
	left: AgentLocationPresentation,
	right: AgentLocationPresentation,
): number {
	const navigationPriority = Number(right.location.navigable) - Number(left.location.navigable);
	if (navigationPriority !== 0) return navigationPriority;
	const freshnessPriority = freshnessRank(right.freshness) - freshnessRank(left.freshness);
	if (freshnessPriority !== 0) return freshnessPriority;
	const semanticPriority = statusPriority(right.status) - statusPriority(left.status);
	if (semanticPriority !== 0) return semanticPriority;
	return right.location.generation.localeCompare(left.location.generation);
}

function freshnessRank(freshness: AgentObservation['freshness']): number {
	switch (freshness) {
		case 'fresh':
			return 2;
		case 'stale':
			return 1;
		case 'disconnected':
			return 0;
	}
}

function aggregateFreshness(
	observations: readonly AgentObservation[],
): AgentObservation['freshness'] {
	if (observations.some((observation) => observation.freshness === 'disconnected')) {
		return 'disconnected';
	}
	return observations.some((observation) => observation.freshness === 'stale') ? 'stale' : 'fresh';
}

function locationKey(location: AgentPresentationAttachment): string {
	return `${location.presentationId}\0${location.generation}`;
}
