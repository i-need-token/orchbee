import { chmod, lstat, unlink } from 'node:fs/promises';
import { createServer, type Server, type Socket } from 'node:net';

import {
	AgentEventIdSchema,
	AgentExecutionLocationIdSchema,
	AgentRequestIdSchema,
	AgentSessionIdSchema,
	AgentTurnIdSchema,
	type AgentActivity,
	type AgentEventId,
	type AgentObservationEvent,
	type AgentPresentationAttachment,
	type AgentRuntimeSnapshot,
	type AgentSessionId,
	type AgentTransition,
	type AgentTurnId,
} from '@orchbee/agent';
import type {
	CreationId,
	TerminalApplicationSignal,
	TerminalEnvironmentOverlay,
} from '@orchbee/terminal-protocol';
import type { TerminalPresentationId } from '@orchbee/workbench/terminal-presentation';

import {
	AgentHookPresentationCorrelationHintSchema,
	AgentHookRichObservationEnvelopeSchema,
	type AgentHookAdapterId,
	type AgentHookRichObservationEnvelope,
	type AgentHookSanitizedObservation,
} from './agent-hook-observation-contract';
import {
	resolveAgentHookAdapter,
	type AgentHookAdapterDefinition,
} from './resolveAgentHookAdapter';

const MAX_ACTIVE_CONNECTIONS = 64;
const MAX_CHILD_SESSIONS_PER_SESSION = 128;
const MAX_OBSERVATION_BODY_BYTES = 16 * 1024;
const MAX_PENDING_EVENTS = 512;
const MAX_PENDING_HINTS = 1024;
const MAX_PENDING_REQUESTS_PER_SESSION = 128;
const MAX_SESSIONS = 128;
const CONNECTION_TIMEOUT_MILLISECONDS = 500;
const JOIN_SETTLE_MILLISECONDS = 25;
const JOIN_TTL_MILLISECONDS = 10_000;
const RECENT_EVENT_TTL_MILLISECONDS = 60_000;
type AgentSessionObservationEvent = Exclude<
	AgentObservationEvent,
	{ readonly type: 'session-observed' }
>;
type AgentSessionObservationInput<
	Event extends AgentSessionObservationEvent = AgentSessionObservationEvent,
> = Event extends AgentSessionObservationEvent
	? Omit<Event, 'eventId' | 'generation' | 'sequence' | 'sessionId'>
	: never;

interface AgentHookObservationRuntimeSink {
	dispatch(event: AgentObservationEvent): AgentTransition;
	getSnapshot(): AgentRuntimeSnapshot;
}

export interface AgentHookObservationIngressOptions {
	readonly allocateIdentifier: () => string;
	readonly joinSettleMilliseconds?: number;
	readonly now?: () => number;
	readonly socketPath: string | null;
}

interface SessionBinding {
	activeTurnKey: string | null;
	activeTurnId: AgentTurnId | null;
	readonly adapter: AgentHookAdapterDefinition;
	readonly childSessions: Map<string, AgentSessionId>;
	readonly executionLocationId: ReturnType<typeof AgentExecutionLocationIdSchema.parse>;
	readonly requests: Map<string, ReturnType<typeof AgentRequestIdSchema.parse>>;
	readonly sessionId: AgentSessionId;
}

interface PendingHint {
	readonly attachment: AgentPresentationAttachment;
	readonly eventId: string;
	readonly expiresAt: number;
}

interface PendingEvent {
	readonly adapter: AgentHookAdapterDefinition;
	applied: boolean;
	readonly envelope: AgentHookRichObservationEnvelope;
	readonly expiresAt: number;
	readonly matchedAttachments: Map<string, AgentPresentationAttachment>;
	readonly matchedCorrelationIds: Set<string>;
	settleTimer: ReturnType<typeof setTimeout> | null;
}

/**
 * Owns the trusted Desktop ingress for locally sanitized Agent hook
 * observations. A rich Unix-socket half has no presentation authority and an
 * OSC half has no semantic authority; only a bounded exact join admits both.
 */
export class AgentHookObservationIngress {
	readonly #activeConnections = new Set<Socket>();
	readonly #allocateIdentifier: () => string;
	readonly #correlationEventIds = new Map<string, string>();
	readonly #currentGenerations = new Map<string, CreationId>();
	readonly #joinSettleMilliseconds: number;
	readonly #now: () => number;
	readonly #pendingEvents = new Map<string, PendingEvent>();
	readonly #pendingHints = new Map<string, PendingHint>();
	readonly #recentEventExpirations = new Map<string, number>();
	readonly #runtime: AgentHookObservationRuntimeSink;
	readonly #server: Server;
	readonly #sessions = new Map<string, SessionBinding>();
	readonly #socketPath: string | null;
	#started = false;

	constructor(
		runtime: AgentHookObservationRuntimeSink,
		options: AgentHookObservationIngressOptions,
	) {
		this.#runtime = runtime;
		this.#allocateIdentifier = options.allocateIdentifier;
		this.#joinSettleMilliseconds = options.joinSettleMilliseconds ?? JOIN_SETTLE_MILLISECONDS;
		this.#now = options.now ?? Date.now;
		this.#socketPath = options.socketPath;
		this.#server = createServer(this.#acceptConnection);
		this.#server.maxConnections = MAX_ACTIVE_CONNECTIONS;
	}

	async start(): Promise<void> {
		if (this.#started || this.#socketPath === null) return;
		await removeStaleSocket(this.#socketPath);
		await new Promise<void>((resolve, reject) => {
			const handleError = (error: Error): void => {
				this.#server.off('listening', handleListening);
				reject(error);
			};
			const handleListening = (): void => {
				this.#server.off('error', handleError);
				resolve();
			};
			this.#server.once('error', handleError);
			this.#server.once('listening', handleListening);
			this.#server.listen({ exclusive: true, path: this.#socketPath ?? undefined });
		});
		try {
			await chmod(this.#socketPath, 0o600);
			this.#started = true;
		} catch (error) {
			await this.dispose();
			throw error;
		}
	}

	environmentOverlayFor(
		presentationId: TerminalPresentationId,
		creationId: CreationId,
	): TerminalEnvironmentOverlay {
		if (!this.#started || this.#socketPath === null) return [];
		const previousGeneration = this.#currentGenerations.get(presentationId);
		if (previousGeneration !== undefined && previousGeneration !== creationId) {
			this.#retirePresentationGeneration(presentationId, previousGeneration);
		}
		this.#currentGenerations.set(presentationId, creationId);
		return [{ name: 'ORCHBEE_AGENT_OBSERVATION_SOCKET', value: this.#socketPath }];
	}

	observeTerminalApplicationSignal(
		event: TerminalApplicationSignal,
		presentationId: TerminalPresentationId,
	): void {
		const parsed = AgentHookPresentationCorrelationHintSchema.safeParse(parseJson(event.payload));
		if (!parsed.success) return;
		const currentGeneration = this.#currentGenerations.get(presentationId);
		if (currentGeneration === undefined || currentGeneration !== event.identity.creationId) return;

		const now = this.#now();
		this.#cleanupExpired(now);
		const hint: PendingHint = {
			attachment: {
				generation: event.identity.creationId,
				navigable: true,
				presentationId,
				visibility: parsed.data.visibility,
			},
			eventId: parsed.data.eventId,
			expiresAt: now + JOIN_TTL_MILLISECONDS,
		};
		const existing = this.#pendingHints.get(parsed.data.correlationId);
		if (existing !== undefined) {
			if (!pendingHintsEqual(existing, hint)) return;
		} else {
			if (this.#pendingHints.size >= MAX_PENDING_HINTS) return;
			this.#pendingHints.set(parsed.data.correlationId, hint);
		}
		this.#joinCorrelation(parsed.data.correlationId);
	}

	retirePresentation(presentationId: TerminalPresentationId): void {
		const generation = this.#currentGenerations.get(presentationId);
		this.#currentGenerations.delete(presentationId);
		if (generation !== undefined) this.#retirePresentationGeneration(presentationId, generation);
	}

	async dispose(): Promise<void> {
		for (const pendingEvent of this.#pendingEvents.values()) {
			if (pendingEvent.settleTimer !== null) clearTimeout(pendingEvent.settleTimer);
		}
		this.#pendingEvents.clear();
		this.#pendingHints.clear();
		this.#correlationEventIds.clear();
		this.#recentEventExpirations.clear();
		this.#currentGenerations.clear();
		this.#sessions.clear();
		for (const connection of this.#activeConnections) connection.destroy();
		this.#activeConnections.clear();
		this.#started = false;
		if (this.#server.listening) {
			await new Promise<void>((resolve) => this.#server.close(() => resolve()));
		}
		if (this.#socketPath !== null) await unlinkSocketIfPresent(this.#socketPath);
	}

	readonly #acceptConnection = (socket: Socket): void => {
		if (this.#activeConnections.size >= MAX_ACTIVE_CONNECTIONS) {
			socket.destroy();
			return;
		}
		this.#activeConnections.add(socket);
		socket.setTimeout(CONNECTION_TIMEOUT_MILLISECONDS, () => socket.destroy());
		const chunks: Buffer[] = [];
		let byteLength = 0;
		let rejected = false;
		socket.on('data', (chunk: Buffer) => {
			if (rejected) return;
			byteLength += chunk.byteLength;
			if (byteLength > MAX_OBSERVATION_BODY_BYTES) {
				rejected = true;
				socket.destroy();
				return;
			}
			chunks.push(chunk);
		});
		socket.once('end', () => {
			if (!rejected) this.#acceptEnvelope(Buffer.concat(chunks, byteLength));
			socket.destroy();
		});
		socket.once('error', () => undefined);
		socket.once('close', () => this.#activeConnections.delete(socket));
	};

	#acceptEnvelope(body: Buffer): void {
		let decoded: unknown;
		try {
			decoded = JSON.parse(body.toString('utf8'));
		} catch {
			return;
		}
		const parsed = AgentHookRichObservationEnvelopeSchema.safeParse(decoded);
		if (!parsed.success) return;
		const adapter = resolveAgentHookAdapter(parsed.data.adapterId, parsed.data.observation.event);
		if (adapter === null) return;
		const now = this.#now();
		this.#cleanupExpired(now);
		const transportEventKey = eventKey(parsed.data.adapterId, parsed.data.eventId);
		if (this.#recentEventExpirations.has(transportEventKey)) return;

		const existing = this.#pendingEvents.get(transportEventKey);
		if (existing !== undefined) {
			if (!envelopesEqual(existing.envelope, parsed.data)) return;
			for (const correlationId of parsed.data.correlationIds) {
				this.#joinCorrelation(correlationId);
			}
			return;
		}
		if (this.#pendingEvents.size >= MAX_PENDING_EVENTS) return;
		if (
			parsed.data.correlationIds.some((correlationId) => {
				const claimedEventKey = this.#correlationEventIds.get(correlationId);
				return claimedEventKey !== undefined && claimedEventKey !== transportEventKey;
			})
		) {
			return;
		}

		const pendingEvent: PendingEvent = {
			adapter,
			applied: false,
			envelope: parsed.data,
			expiresAt: now + JOIN_TTL_MILLISECONDS,
			matchedAttachments: new Map(),
			matchedCorrelationIds: new Set(),
			settleTimer: null,
		};
		this.#pendingEvents.set(transportEventKey, pendingEvent);
		for (const correlationId of parsed.data.correlationIds) {
			this.#correlationEventIds.set(correlationId, transportEventKey);
			this.#joinCorrelation(correlationId);
		}
	}

	#joinCorrelation(correlationId: string): void {
		const transportEventKey = this.#correlationEventIds.get(correlationId);
		const hint = this.#pendingHints.get(correlationId);
		if (transportEventKey === undefined || hint === undefined) return;
		const pendingEvent = this.#pendingEvents.get(transportEventKey);
		if (pendingEvent === undefined || hint.eventId !== pendingEvent.envelope.eventId) return;
		if (pendingEvent.matchedCorrelationIds.has(correlationId)) return;
		pendingEvent.matchedCorrelationIds.add(correlationId);
		const key = attachmentKey(hint.attachment);
		const matchedAttachment = pendingEvent.matchedAttachments.get(key);
		const attachment =
			matchedAttachment === undefined
				? hint.attachment
				: {
						...hint.attachment,
						visibility: combinePresentationVisibility(
							matchedAttachment.visibility,
							hint.attachment.visibility,
						),
					};
		pendingEvent.matchedAttachments.set(key, attachment);
		this.#pendingHints.delete(correlationId);

		if (pendingEvent.applied) {
			const binding = this.#sessions.get(
				sessionKeyFor(
					pendingEvent.envelope.adapterId,
					pendingEvent.envelope.observation.sessionKey,
				),
			);
			if (binding !== undefined) this.#attachPresentations(binding, [attachment]);
			return;
		}
		if (pendingEvent.matchedCorrelationIds.size === pendingEvent.envelope.correlationIds.length) {
			this.#applyPendingEvent(transportEventKey);
			return;
		}
		if (pendingEvent.settleTimer !== null) return;
		pendingEvent.settleTimer = setTimeout(
			() => this.#applyPendingEvent(transportEventKey),
			this.#joinSettleMilliseconds,
		);
	}

	#applyPendingEvent(transportEventKey: string): void {
		const pendingEvent = this.#pendingEvents.get(transportEventKey);
		if (pendingEvent === undefined || pendingEvent.applied) return;
		if (pendingEvent.settleTimer !== null) clearTimeout(pendingEvent.settleTimer);
		pendingEvent.settleTimer = null;
		if (pendingEvent.matchedAttachments.size === 0) return;
		pendingEvent.applied = true;
		this.#observe(
			pendingEvent.adapter,
			pendingEvent.envelope.observation,
			pendingEvent.envelope.eventId,
			[...pendingEvent.matchedAttachments.values()],
		);
	}

	#observe(
		adapter: AgentHookAdapterDefinition,
		observation: AgentHookSanitizedObservation,
		externalEventId: string,
		attachments: readonly AgentPresentationAttachment[],
	): void {
		const binding = this.#ensureSession(
			adapter,
			observation.sessionKey,
			attachments,
			externalEventId,
		);
		if (binding === null) return;
		this.#attachPresentations(binding, attachments);
		const eventId = this.#externalEventId(adapter.adapterId, externalEventId);
		switch (observation.event) {
			case 'session-started':
				this.#observeIdle(binding);
				break;
			case 'session-idle':
				if (!this.#acceptTurnScopedObservation(binding, observation.turnKey)) break;
				this.#observeIdle(binding);
				break;
			case 'turn-started':
				this.#startTurn(binding, observation.activity ?? 'thinking', observation.turnKey, eventId);
				break;
			case 'tool-started':
				if (!this.#acceptTurnScopedObservation(binding, observation.turnKey)) break;
				this.#ensureActiveTurn(binding, observation.turnKey);
				this.#resolveRequest(binding, observation.operationKey, eventId);
				this.#changeActivity(binding, observation.activity ?? 'tool');
				break;
			case 'tool-finished':
			case 'tool-failed':
				if (!this.#acceptTurnScopedObservation(binding, observation.turnKey)) break;
				this.#resolveRequest(binding, observation.operationKey, eventId);
				if (binding.activeTurnId !== null) this.#changeActivity(binding, 'thinking');
				break;
			case 'question-requested':
				if (!this.#acceptTurnScopedObservation(binding, observation.turnKey)) break;
				this.#requestAttention(
					binding,
					observation.operationKey,
					'question',
					observation.turnKey,
					eventId,
				);
				break;
			case 'approval-requested':
				if (!this.#acceptTurnScopedObservation(binding, observation.turnKey)) break;
				this.#requestAttention(
					binding,
					observation.operationKey,
					'approval',
					observation.turnKey,
					eventId,
				);
				break;
			case 'attention-resolved':
				if (!this.#acceptTurnScopedObservation(binding, observation.turnKey)) break;
				this.#resolveRequest(binding, observation.operationKey, eventId);
				if (binding.activeTurnId !== null) this.#changeActivity(binding, 'thinking');
				break;
			case 'compaction-started':
				if (!this.#acceptTurnScopedObservation(binding, observation.turnKey)) break;
				this.#ensureActiveTurn(binding, observation.turnKey);
				this.#changeActivity(binding, 'compacting');
				break;
			case 'compaction-finished':
				if (!this.#acceptTurnScopedObservation(binding, observation.turnKey)) break;
				if (binding.activeTurnId !== null) this.#changeActivity(binding, 'thinking');
				break;
			case 'subagent-started':
				if (!this.#acceptTurnScopedObservation(binding, observation.turnKey)) break;
				this.#observeSubagent(binding, observation.agentKey, eventId);
				break;
			case 'subagent-stopped':
				if (!this.#acceptTurnScopedObservation(binding, observation.turnKey)) break;
				this.#endSubagent(binding, observation.agentKey, eventId);
				break;
			case 'turn-stopped':
				this.#finishTurn(binding, 'succeeded', observation.turnKey, eventId);
				break;
			case 'turn-failed':
				this.#finishTurn(binding, 'failed', observation.turnKey, eventId);
				break;
			case 'turn-canceled':
				this.#finishTurn(binding, 'canceled', observation.turnKey, eventId);
				break;
			case 'turn-interrupted':
				this.#finishTurn(binding, 'interrupted', observation.turnKey, eventId);
				break;
			case 'session-ended':
				if (
					this.#dispatchFor(binding.sessionId, { type: 'session-ended' }, eventId).kind ===
					'applied'
				) {
					binding.activeTurnId = null;
					binding.activeTurnKey = null;
					binding.requests.clear();
				}
				break;
		}
	}

	#ensureSession(
		adapter: AgentHookAdapterDefinition,
		sessionKey: string,
		attachments: readonly AgentPresentationAttachment[],
		externalEventId: string,
	): SessionBinding | null {
		const bindingKey = sessionKeyFor(adapter.adapterId, sessionKey);
		const existing = this.#sessions.get(bindingKey);
		if (existing !== undefined) return existing;
		if (this.#sessions.size >= MAX_SESSIONS) return null;
		const binding: SessionBinding = {
			activeTurnKey: null,
			activeTurnId: null,
			adapter,
			childSessions: new Map(),
			executionLocationId: AgentExecutionLocationIdSchema.parse(
				`desktop:agent:execution:${this.#allocateIdentifier()}`,
			),
			requests: new Map(),
			sessionId: AgentSessionIdSchema.parse(`desktop:agent:session:${this.#allocateIdentifier()}`),
		};
		const transition = this.#runtime.dispatch({
			capabilities: adapter.capabilities,
			eventId: this.#externalEventId(adapter.adapterId, externalEventId),
			executionLocation: { executionLocationId: binding.executionLocationId },
			observation: {
				fidelity: 'rich',
				freshness: 'fresh',
				generation: 0,
				sequence: 1,
				source: 'hook',
			},
			parentSessionId: null,
			presentationAttachments: attachments,
			providerId: adapter.providerId,
			sessionId: binding.sessionId,
			type: 'session-observed',
		});
		if (transition.kind !== 'applied') return null;
		this.#sessions.set(bindingKey, binding);
		return binding;
	}

	#attachPresentations(
		binding: SessionBinding,
		attachments: readonly AgentPresentationAttachment[],
	): void {
		const sessionIds = [binding.sessionId, ...binding.childSessions.values()];
		for (const sessionId of sessionIds) {
			for (const attachment of attachments) {
				const session = this.#session(sessionId);
				if (session === undefined || session.lifecycle !== 'live') continue;
				const existing = session.presentationAttachments.find(
					(candidate) =>
						candidate.presentationId === attachment.presentationId &&
						candidate.generation === attachment.generation,
				);
				if (existing === undefined || !existing.navigable) {
					this.#dispatchFor(sessionId, { attachment, type: 'presentation-attached' });
					continue;
				}
				if (existing.visibility !== attachment.visibility) {
					this.#dispatchFor(sessionId, {
						presentationGeneration: attachment.generation,
						presentationId: attachment.presentationId,
						type: 'presentation-visibility-changed',
						visibility: attachment.visibility,
					});
				}
			}
		}
	}

	#observeIdle(binding: SessionBinding): void {
		const transition = this.#dispatchFor(binding.sessionId, { type: 'session-idle-observed' });
		if (transition.kind !== 'applied') return;
		binding.activeTurnId = null;
		binding.activeTurnKey = null;
		binding.requests.clear();
	}

	#startTurn(
		binding: SessionBinding,
		activity: AgentActivity,
		turnKey: string | null,
		eventId?: AgentEventId,
	): void {
		if (binding.activeTurnId !== null) {
			if (turnKey !== null && binding.activeTurnKey === turnKey) {
				this.#changeActivity(binding, activity);
				return;
			}
			if (turnKey === null) {
				this.#changeActivity(binding, activity);
				return;
			}
			this.#finishTurn(binding, 'interrupted', binding.activeTurnKey, eventId);
			if (binding.activeTurnId !== null) return;
		}
		const turnId = AgentTurnIdSchema.parse(`desktop:agent:turn:${this.#allocateIdentifier()}`);
		const transition = this.#dispatchFor(
			binding.sessionId,
			{ activity, phase: 'working', turnId, type: 'turn-started' },
			eventId,
		);
		if (transition.kind !== 'applied') return;
		binding.activeTurnKey = turnKey;
		binding.activeTurnId = turnId;
	}

	#ensureActiveTurn(binding: SessionBinding, turnKey: string | null = null): void {
		if (binding.activeTurnId === null) this.#startTurn(binding, 'thinking', turnKey);
	}

	#acceptTurnScopedObservation(binding: SessionBinding, turnKey: string | null): boolean {
		if (turnKey === null || binding.activeTurnId === null) return true;
		if (binding.activeTurnKey === null) {
			binding.activeTurnKey = turnKey;
			return true;
		}
		return binding.activeTurnKey === turnKey;
	}

	#changeActivity(binding: SessionBinding, activity: AgentActivity): void {
		const turnId = binding.activeTurnId;
		if (turnId === null) return;
		this.#dispatchFor(binding.sessionId, {
			activity,
			phase: 'working',
			turnId,
			type: 'turn-activity-changed',
		});
	}

	#requestAttention(
		binding: SessionBinding,
		operationKey: string | null,
		kind: 'approval' | 'question',
		turnKey: string | null,
		eventId: AgentEventId,
	): void {
		if (operationKey === null) return;
		this.#ensureActiveTurn(binding, turnKey);
		const turnId = binding.activeTurnId;
		if (
			turnId === null ||
			binding.requests.has(operationKey) ||
			binding.requests.size >= MAX_PENDING_REQUESTS_PER_SESSION
		)
			return;
		const requestId = AgentRequestIdSchema.parse(
			`desktop:agent:request:${this.#allocateIdentifier()}`,
		);
		const transition = this.#dispatchFor(
			binding.sessionId,
			{
				request: { actionPath: 'navigate-only', kind, requestId, turnId },
				type: 'attention-requested',
			},
			eventId,
		);
		if (transition.kind === 'applied') binding.requests.set(operationKey, requestId);
	}

	#resolveRequest(
		binding: SessionBinding,
		operationKey: string | null,
		eventId: AgentEventId,
	): void {
		if (operationKey === null) return;
		const requestId = binding.requests.get(operationKey);
		if (requestId === undefined) return;
		const transition = this.#dispatchFor(
			binding.sessionId,
			{ requestId, type: 'attention-resolved' },
			eventId,
		);
		if (transition.kind === 'applied') binding.requests.delete(operationKey);
	}

	#finishTurn(
		binding: SessionBinding,
		outcome: 'canceled' | 'failed' | 'interrupted' | 'succeeded',
		turnKey: string | null,
		eventId?: AgentEventId,
	): void {
		if (!this.#acceptTurnScopedObservation(binding, turnKey)) return;
		if (binding.activeTurnId === null) this.#startTurn(binding, 'thinking', turnKey);
		const turnId = binding.activeTurnId;
		if (turnId === null) return;
		const transition = this.#dispatchFor(
			binding.sessionId,
			{ outcome, turnId, type: 'turn-finished' },
			eventId,
		);
		if (transition.kind !== 'applied') return;
		binding.activeTurnId = null;
		binding.activeTurnKey = null;
		binding.requests.clear();
	}

	#observeSubagent(parent: SessionBinding, agentKey: string | null, eventId: AgentEventId): void {
		if (
			agentKey === null ||
			parent.childSessions.has(agentKey) ||
			parent.childSessions.size >= MAX_CHILD_SESSIONS_PER_SESSION
		)
			return;
		this.#ensureActiveTurn(parent);
		this.#changeActivity(parent, 'subagent');
		const parentSession = this.#session(parent.sessionId);
		if (parentSession === undefined) return;
		const sessionId = AgentSessionIdSchema.parse(
			`desktop:agent:session:${this.#allocateIdentifier()}`,
		);
		const transition = this.#runtime.dispatch({
			capabilities: parent.adapter.capabilities,
			eventId,
			executionLocation: { executionLocationId: parent.executionLocationId },
			observation: {
				fidelity: 'rich',
				freshness: 'fresh',
				generation: 0,
				sequence: 1,
				source: 'hook',
			},
			parentSessionId: parent.sessionId,
			presentationAttachments: parentSession.presentationAttachments.filter(
				(attachment) => attachment.navigable,
			),
			providerId: parent.adapter.providerId,
			sessionId,
			type: 'session-observed',
		});
		if (transition.kind !== 'applied') return;
		parent.childSessions.set(agentKey, sessionId);
		const turnId = AgentTurnIdSchema.parse(`desktop:agent:turn:${this.#allocateIdentifier()}`);
		this.#dispatchFor(sessionId, {
			activity: 'subagent',
			phase: 'working',
			turnId,
			type: 'turn-started',
		});
	}

	#endSubagent(parent: SessionBinding, agentKey: string | null, eventId: AgentEventId): void {
		if (agentKey === null) return;
		const sessionId = parent.childSessions.get(agentKey);
		if (sessionId === undefined) return;
		const transition = this.#dispatchFor(sessionId, { type: 'session-ended' }, eventId);
		if (transition.kind !== 'applied') return;
		parent.childSessions.delete(agentKey);
		if (parent.activeTurnId !== null) this.#changeActivity(parent, 'thinking');
	}

	#retirePresentationGeneration(presentationId: string, generation: CreationId): void {
		for (const [correlationId, hint] of this.#pendingHints) {
			if (
				hint.attachment.presentationId === presentationId &&
				hint.attachment.generation === generation
			) {
				this.#pendingHints.delete(correlationId);
			}
		}
		for (const pendingEvent of this.#pendingEvents.values()) {
			for (const [key, attachment] of pendingEvent.matchedAttachments) {
				if (attachment.presentationId === presentationId && attachment.generation === generation) {
					pendingEvent.matchedAttachments.delete(key);
				}
			}
		}
		for (const session of this.#runtime.getSnapshot().sessions) {
			const attachment = session.presentationAttachments.find(
				(candidate) =>
					candidate.presentationId === presentationId &&
					candidate.generation === generation &&
					candidate.navigable,
			);
			if (attachment === undefined) continue;
			this.#dispatchFor(session.sessionId, {
				presentationGeneration: generation,
				presentationId,
				type: 'presentation-detached',
			});
		}
	}

	#dispatchFor(
		sessionId: AgentSessionId,
		event: AgentSessionObservationInput,
		eventId: AgentEventId = this.#eventId(),
	): AgentTransition {
		const session = this.#session(sessionId);
		if (session === undefined) {
			return {
				kind: 'rejected',
				reason: 'session-not-found',
				semanticEvents: [],
				snapshot: this.#runtime.getSnapshot(),
			};
		}
		const observationEvent: AgentSessionObservationEvent = {
			...event,
			eventId,
			generation: session.observation.generation,
			sequence: session.observation.sequence + 1,
			sessionId,
		};
		return this.#runtime.dispatch(observationEvent);
	}

	#session(sessionId: AgentSessionId) {
		return this.#runtime
			.getSnapshot()
			.sessions.find((candidate) => candidate.sessionId === sessionId);
	}

	#cleanupExpired(now: number): void {
		for (const [correlationId, hint] of this.#pendingHints) {
			if (hint.expiresAt <= now) this.#pendingHints.delete(correlationId);
		}
		for (const [transportEventKey, pendingEvent] of this.#pendingEvents) {
			if (pendingEvent.expiresAt > now) continue;
			if (pendingEvent.settleTimer !== null) clearTimeout(pendingEvent.settleTimer);
			this.#pendingEvents.delete(transportEventKey);
			for (const correlationId of pendingEvent.envelope.correlationIds) {
				if (this.#correlationEventIds.get(correlationId) === transportEventKey) {
					this.#correlationEventIds.delete(correlationId);
				}
			}
			if (pendingEvent.applied) {
				this.#recentEventExpirations.set(transportEventKey, now + RECENT_EVENT_TTL_MILLISECONDS);
			}
		}
		for (const [eventId, expiresAt] of this.#recentEventExpirations) {
			if (expiresAt <= now) this.#recentEventExpirations.delete(eventId);
		}
	}

	#externalEventId(adapterId: AgentHookAdapterId, externalEventId: string): AgentEventId {
		return AgentEventIdSchema.parse(`desktop:agent:hook:${adapterId}:${externalEventId}`);
	}

	#eventId(): AgentEventId {
		return AgentEventIdSchema.parse(`desktop:agent:event:${this.#allocateIdentifier()}`);
	}
}

async function removeStaleSocket(socketPath: string): Promise<void> {
	let metadata: Awaited<ReturnType<typeof lstat>>;
	try {
		metadata = await lstat(socketPath);
	} catch (error) {
		if (hasErrorCode(error, 'ENOENT')) return;
		throw error;
	}
	if (!metadata.isSocket()) {
		throw new Error('Agent hook observation path exists and is not a Unix socket.');
	}
	const currentUserId = process.getuid?.();
	if (currentUserId !== undefined && metadata.uid !== currentUserId) {
		throw new Error('Agent hook observation socket is not owned by the current user.');
	}
	await unlink(socketPath);
}

async function unlinkSocketIfPresent(socketPath: string): Promise<void> {
	try {
		const metadata = await lstat(socketPath);
		if (metadata.isSocket()) await unlink(socketPath);
	} catch {
		// Shutdown cleanup is best effort and never changes terminal lifetime.
	}
}

function hasErrorCode(error: unknown, code: string): boolean {
	return typeof error === 'object' && error !== null && 'code' in error && error.code === code;
}

function parseJson(value: string): unknown {
	try {
		return JSON.parse(value);
	} catch {
		return undefined;
	}
}

function eventKey(adapterId: AgentHookAdapterId, externalEventId: string): string {
	return `${adapterId}\0${externalEventId}`;
}

function sessionKeyFor(adapterId: AgentHookAdapterId, externalSessionKey: string): string {
	return `${adapterId}\0${externalSessionKey}`;
}

function attachmentKey(attachment: AgentPresentationAttachment): string {
	return `${attachment.presentationId}\0${attachment.generation}`;
}

function combinePresentationVisibility(
	left: AgentPresentationAttachment['visibility'],
	right: AgentPresentationAttachment['visibility'],
): AgentPresentationAttachment['visibility'] {
	if (left === 'hidden' || right === 'hidden') return 'hidden';
	return left === 'visible' && right === 'visible' ? 'visible' : 'unknown';
}

function pendingHintsEqual(left: PendingHint, right: PendingHint): boolean {
	return (
		left.eventId === right.eventId &&
		left.attachment.presentationId === right.attachment.presentationId &&
		left.attachment.generation === right.attachment.generation &&
		left.attachment.visibility === right.attachment.visibility
	);
}

function envelopesEqual(
	left: AgentHookRichObservationEnvelope,
	right: AgentHookRichObservationEnvelope,
): boolean {
	return JSON.stringify(left) === JSON.stringify(right);
}
