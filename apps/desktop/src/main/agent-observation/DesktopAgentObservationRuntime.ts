import {
	AgentEventIdSchema,
	AgentExecutionLocationIdSchema,
	AgentProviderIdSchema,
	AgentRuntime,
	AgentSessionIdSchema,
	transitionAgentRuntime,
	type AgentChangeSource,
	type AgentObservationEvent,
	type AgentPresentationAttachment,
	type AgentProviderId,
	type AgentRuntimeChangeReadInput,
	type AgentRuntimeChangeReadResult,
	type AgentRuntimeSnapshot,
	type AgentSemanticEventListener,
	type AgentSessionId,
	type AgentSessionSnapshot,
	type AgentTransition,
} from '@orchbee/agent';
import {
	DesktopAgentIntegrationIdSchema,
	type DesktopAgentIntegrationId,
} from '@orchbee/desktop-client-protocol';

const DEFAULT_DETACH_AFTER_MILLISECONDS = 120_000;
const DEFAULT_STALE_AFTER_MILLISECONDS = 30_000;
const MAX_PASSIVE_PRESENCE_BINDINGS = 256;
const SIGNATURE_ID_PATTERN = /^[a-z0-9](?:[a-z0-9.-]*[a-z0-9])?$/u;
const PASSIVE_PRESENCE_SOURCES = new Set<DesktopAgentPassivePresenceSource>([
	'process-ancestry',
	'screen-signature',
	'terminal-command',
	'terminal-title',
]);

type DesktopAgentPassivePresenceSource =
	| 'process-ancestry'
	| 'screen-signature'
	| 'terminal-command'
	| 'terminal-title';

export interface DesktopAgentPassivePresenceEvidence {
	readonly generation: string;
	readonly integrationId: DesktopAgentIntegrationId;
	readonly presentationId: string;
	readonly signatureId: string;
	readonly source: DesktopAgentPassivePresenceSource;
}

export interface DesktopAgentPassivePresenceDiagnostic extends DesktopAgentPassivePresenceEvidence {
	readonly freshness: 'disconnected' | 'fresh' | 'stale';
	readonly observedAt: number;
}

interface PassivePresenceBinding extends DesktopAgentPassivePresenceEvidence {
	detachTimer: ReturnType<typeof setTimeout> | null;
	epoch: number;
	freshness: DesktopAgentPassivePresenceDiagnostic['freshness'];
	observedAt: number;
	readonly sessionId: AgentSessionId;
	staleTimer: ReturnType<typeof setTimeout> | null;
}

export interface DesktopAgentObservationRuntimeOptions {
	readonly allocateEventId: () => string;
	readonly detachAfterMilliseconds?: number;
	readonly now?: () => number;
	readonly staleAfterMilliseconds?: number;
}

/**
 * Owns rich Agent state and a deliberately capability-free passive presence
 * projection. Passive evidence can create only heuristic/unknown sessions;
 * it cannot emit Agent semantic events or become notification authority.
 */
export class DesktopAgentObservationRuntime implements AgentChangeSource {
	readonly #allocateEventId: () => string;
	readonly #detachAfterMilliseconds: number;
	readonly #now: () => number;
	readonly #passivePresenceBindings = new Map<string, PassivePresenceBinding>();
	readonly #runtime: AgentRuntime;
	readonly #staleAfterMilliseconds: number;

	constructor(
		options: DesktopAgentObservationRuntimeOptions,
		runtime: AgentRuntime = new AgentRuntime(),
	) {
		this.#allocateEventId = options.allocateEventId;
		this.#detachAfterMilliseconds =
			options.detachAfterMilliseconds ?? DEFAULT_DETACH_AFTER_MILLISECONDS;
		this.#now = options.now ?? Date.now;
		this.#runtime = runtime;
		this.#staleAfterMilliseconds =
			options.staleAfterMilliseconds ?? DEFAULT_STALE_AFTER_MILLISECONDS;
		if (
			!Number.isSafeInteger(this.#staleAfterMilliseconds) ||
			this.#staleAfterMilliseconds < 1 ||
			!Number.isSafeInteger(this.#detachAfterMilliseconds) ||
			this.#detachAfterMilliseconds <= this.#staleAfterMilliseconds
		) {
			throw new Error('Passive Agent presence deadlines are invalid.');
		}
	}

	dispatch(event: AgentObservationEvent): AgentTransition {
		if (event.type === 'session-observed' || event.type === 'presentation-attached') {
			const preview = transitionAgentRuntime(this.#runtime.getSnapshot(), event);
			if (preview.kind === 'applied') {
				const richScope = richAttachmentScope(event, preview.snapshot);
				if (richScope !== null) {
					this.#retirePassivePresenceSupersededByRichObservation(
						richScope.providerId,
						richScope.attachments,
					);
				}
			}
		}
		return this.#runtime.dispatch(event);
	}

	dispose(): void {
		for (const binding of this.#passivePresenceBindings.values()) this.#clearTimers(binding);
		this.#passivePresenceBindings.clear();
		this.#runtime.dispose();
	}

	getPassivePresenceDiagnostics(): readonly DesktopAgentPassivePresenceDiagnostic[] {
		return [...this.#passivePresenceBindings.values()]
			.map(
				({
					freshness,
					generation,
					integrationId,
					observedAt,
					presentationId,
					signatureId,
					source,
				}) =>
					Object.freeze({
						freshness,
						generation,
						integrationId,
						observedAt,
						presentationId,
						signatureId,
						source,
					}),
			)
			.sort((left, right) => {
				const presentationOrder = left.presentationId.localeCompare(right.presentationId);
				return presentationOrder === 0
					? left.integrationId.localeCompare(right.integrationId)
					: presentationOrder;
			});
	}

	getSnapshot = (): AgentRuntimeSnapshot => this.#runtime.getSnapshot();

	observePassivePresence(evidence: DesktopAgentPassivePresenceEvidence): boolean {
		if (!this.#validEvidence(evidence)) return false;
		const key = passivePresenceKey(evidence);
		let binding = this.#passivePresenceBindings.get(key);
		if (this.#hasNavigableRichObservation(evidence)) {
			if (binding !== undefined) this.#retireBinding(binding);
			return true;
		}
		if (binding !== undefined && !this.#refreshBinding(binding)) {
			this.#clearTimers(binding);
			this.#passivePresenceBindings.delete(key);
			binding = undefined;
		}
		if (binding === undefined) {
			if (!this.#reserveBindingCapacity()) return false;
			const sessionId = AgentSessionIdSchema.parse(this.#allocateEventId());
			const transition = this.#runtime.dispatch({
				capabilities: [],
				eventId: this.#eventId(),
				executionLocation: {
					executionLocationId: AgentExecutionLocationIdSchema.parse(this.#allocateEventId()),
				},
				observation: {
					fidelity: 'heuristic',
					freshness: 'fresh',
					generation: 0,
					sequence: 1,
					source: 'heuristic',
				},
				parentSessionId: null,
				presentationAttachments: [presenceAttachment(evidence)],
				providerId: AgentProviderIdSchema.parse(evidence.integrationId),
				sessionId,
				type: 'session-observed',
			});
			if (transition.kind === 'rejected' || transition.semanticEvents.length > 0) return false;
			binding = {
				...evidence,
				detachTimer: null,
				epoch: 0,
				freshness: 'fresh',
				observedAt: this.#now(),
				sessionId,
				staleTimer: null,
			};
			this.#passivePresenceBindings.set(key, binding);
		}
		binding.freshness = 'fresh';
		binding.observedAt = this.#now();
		this.#scheduleExpiry(binding);
		return true;
	}

	readChanges = (input: AgentRuntimeChangeReadInput): AgentRuntimeChangeReadResult =>
		this.#runtime.readChanges(input);

	retirePassivePresencePresentation(presentationId: string): void {
		for (const binding of this.#passivePresenceBindings.values()) {
			if (binding.presentationId !== presentationId) continue;
			this.#retireBinding(binding);
		}
	}

	subscribe = (listener: () => void): (() => void) => this.#runtime.subscribe(listener);

	subscribeSemanticEvents = (listener: AgentSemanticEventListener): (() => void) =>
		this.#runtime.subscribeSemanticEvents(listener);

	#clearTimers(binding: PassivePresenceBinding): void {
		if (binding.staleTimer !== null) clearTimeout(binding.staleTimer);
		if (binding.detachTimer !== null) clearTimeout(binding.detachTimer);
		binding.staleTimer = null;
		binding.detachTimer = null;
	}

	#detachBinding(binding: PassivePresenceBinding): void {
		const session = this.#session(binding.sessionId);
		const attachment = session?.presentationAttachments.find(
			(candidate) =>
				candidate.presentationId === binding.presentationId &&
				candidate.generation === binding.generation &&
				candidate.navigable,
		);
		if (session === undefined || attachment === undefined) {
			binding.freshness = 'disconnected';
			return;
		}
		const transition = this.#runtime.dispatch({
			eventId: this.#eventId(),
			generation: session.observation.generation,
			presentationGeneration: binding.generation,
			presentationId: binding.presentationId,
			sequence: session.observation.sequence + 1,
			sessionId: binding.sessionId,
			type: 'presentation-detached',
		});
		if (transition.kind !== 'rejected' && transition.semanticEvents.length === 0) {
			binding.freshness = 'disconnected';
		}
	}

	#eventId(): ReturnType<typeof AgentEventIdSchema.parse> {
		return AgentEventIdSchema.parse(this.#allocateEventId());
	}

	#hasNavigableRichObservation(evidence: DesktopAgentPassivePresenceEvidence): boolean {
		return this.#runtime
			.getSnapshot()
			.sessions.some(
				(session) =>
					session.lifecycle === 'live' &&
					session.providerId === evidence.integrationId &&
					session.observation.fidelity === 'rich' &&
					session.presentationAttachments.some(
						(attachment) =>
							attachment.navigable &&
							attachment.presentationId === evidence.presentationId &&
							attachment.generation === evidence.generation,
					),
			);
	}

	#refreshBinding(binding: PassivePresenceBinding): boolean {
		let session = this.#session(binding.sessionId);
		if (session === undefined || session.lifecycle !== 'live') return false;
		if (session.observation.freshness !== 'fresh') {
			const transition = this.#runtime.dispatch({
				activityHint: null,
				capabilities: [],
				eventId: this.#eventId(),
				generation: session.observation.generation + 1,
				lastOutcome: null,
				pendingRequests: [],
				sequence: session.observation.sequence + 1,
				sessionId: binding.sessionId,
				turn: { kind: 'unknown' },
				type: 'observation-reconnected',
			});
			if (transition.kind === 'rejected' || transition.semanticEvents.length > 0) return false;
			session = this.#session(binding.sessionId);
			if (session === undefined || session.lifecycle !== 'live') return false;
		}
		const attachment = session.presentationAttachments.find(
			(candidate) =>
				candidate.presentationId === binding.presentationId &&
				candidate.generation === binding.generation,
		);
		if (attachment === undefined || !attachment.navigable) {
			const transition = this.#runtime.dispatch({
				attachment: presenceAttachment(binding),
				eventId: this.#eventId(),
				generation: session.observation.generation,
				sequence: session.observation.sequence + 1,
				sessionId: binding.sessionId,
				type: 'presentation-attached',
			});
			if (transition.kind === 'rejected' || transition.semanticEvents.length > 0) return false;
		}
		return true;
	}

	#reserveBindingCapacity(): boolean {
		if (this.#passivePresenceBindings.size < MAX_PASSIVE_PRESENCE_BINDINGS) return true;
		const removable = [...this.#passivePresenceBindings.entries()].find(
			([, binding]) => binding.freshness === 'disconnected',
		);
		if (removable === undefined) return false;
		this.#clearTimers(removable[1]);
		this.#passivePresenceBindings.delete(removable[0]);
		return true;
	}

	#retireBinding(binding: PassivePresenceBinding): void {
		this.#clearTimers(binding);
		binding.epoch += 1;
		this.#detachBinding(binding);
		const session = this.#session(binding.sessionId);
		if (session === undefined || session.lifecycle !== 'live') {
			binding.freshness = 'disconnected';
			return;
		}
		const transition = this.#runtime.dispatch({
			eventId: this.#eventId(),
			generation: session.observation.generation,
			sequence: session.observation.sequence + 1,
			sessionId: binding.sessionId,
			type: 'session-ended',
		});
		if (transition.kind !== 'rejected' && transition.semanticEvents.length === 0) {
			binding.freshness = 'disconnected';
		}
	}

	#retirePassivePresenceSupersededByRichObservation(
		providerId: AgentProviderId,
		attachments: readonly AgentPresentationAttachment[],
	): void {
		for (const binding of this.#passivePresenceBindings.values()) {
			if (binding.integrationId !== providerId || binding.freshness === 'disconnected') continue;
			if (
				!attachments.some(
					(attachment) =>
						attachment.navigable &&
						attachment.presentationId === binding.presentationId &&
						attachment.generation === binding.generation,
				)
			)
				continue;
			this.#retireBinding(binding);
		}
	}

	#scheduleExpiry(binding: PassivePresenceBinding): void {
		this.#clearTimers(binding);
		binding.epoch += 1;
		const epoch = binding.epoch;
		binding.staleTimer = setTimeout(() => {
			if (binding.epoch !== epoch || binding.freshness !== 'fresh') return;
			const session = this.#session(binding.sessionId);
			if (session === undefined || session.lifecycle !== 'live') return;
			const transition = this.#runtime.dispatch({
				eventId: this.#eventId(),
				generation: session.observation.generation,
				sequence: session.observation.sequence + 1,
				sessionId: binding.sessionId,
				type: 'observation-became-stale',
			});
			if (transition.kind !== 'rejected' && transition.semanticEvents.length === 0) {
				binding.freshness = 'stale';
			}
		}, this.#staleAfterMilliseconds);
		binding.staleTimer.unref?.();
		binding.detachTimer = setTimeout(() => {
			if (binding.epoch !== epoch) return;
			this.#retireBinding(binding);
		}, this.#detachAfterMilliseconds);
		binding.detachTimer.unref?.();
	}

	#session(sessionId: AgentSessionId): AgentSessionSnapshot | undefined {
		return this.#runtime
			.getSnapshot()
			.sessions.find((candidate) => candidate.sessionId === sessionId);
	}

	#validEvidence(evidence: DesktopAgentPassivePresenceEvidence): boolean {
		return (
			DesktopAgentIntegrationIdSchema.safeParse(evidence.integrationId).success &&
			PASSIVE_PRESENCE_SOURCES.has(evidence.source) &&
			evidence.generation.length > 0 &&
			evidence.generation.length <= 128 &&
			evidence.presentationId.length > 0 &&
			evidence.presentationId.length <= 256 &&
			evidence.signatureId.length <= 128 &&
			SIGNATURE_ID_PATTERN.test(evidence.signatureId)
		);
	}
}

function richAttachmentScope(
	event: AgentObservationEvent,
	snapshot: AgentRuntimeSnapshot,
): {
	readonly attachments: readonly AgentPresentationAttachment[];
	readonly providerId: AgentProviderId;
} | null {
	if (event.type !== 'session-observed' && event.type !== 'presentation-attached') return null;
	const session = snapshot.sessions.find((candidate) => candidate.sessionId === event.sessionId);
	if (
		session === undefined ||
		session.lifecycle !== 'live' ||
		session.observation.fidelity !== 'rich'
	)
		return null;
	return {
		attachments:
			event.type === 'session-observed' ? event.presentationAttachments : [event.attachment],
		providerId: session.providerId,
	};
}

function passivePresenceKey(evidence: DesktopAgentPassivePresenceEvidence): string {
	return `${evidence.integrationId}\0${evidence.presentationId}\0${evidence.generation}`;
}

function presenceAttachment(evidence: DesktopAgentPassivePresenceEvidence) {
	return {
		generation: evidence.generation,
		navigable: true,
		presentationId: evidence.presentationId,
		visibility: 'unknown' as const,
	};
}
