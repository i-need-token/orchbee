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
	type AgentHookSanitizedObservation,
} from './agent-hook-observation-contract';
import {
	AgentObservationJoinStateMachine,
	type AgentObservationJoinCommand,
} from './AgentObservationJoinStateMachine';
import {
	resolveAgentHookAdapter,
	type AgentHookAdapterDefinition,
} from './resolveAgentHookAdapter';

const MAX_ACTIVE_CONNECTIONS = 64;
const MAX_CHILD_SESSIONS_PER_SESSION = 128;
const MAX_OBSERVATION_BODY_BYTES = 16 * 1024;
const MAX_PENDING_REQUESTS_PER_SESSION = 128;
const MAX_SESSIONS = 128;
const CONNECTION_TIMEOUT_MILLISECONDS = 500;
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

/**
 * Owns the trusted Desktop ingress for locally sanitized Agent hook
 * observations. A rich Unix-socket half has no presentation authority and an
 * OSC half has no semantic authority; only a bounded exact join admits both.
 */
export class AgentHookObservationIngress {
	readonly #activeConnections = new Set<Socket>();
	readonly #allocateIdentifier: () => string;
	readonly #now: () => number;
	readonly #observationJoin: AgentObservationJoinStateMachine;
	readonly #runtime: AgentHookObservationRuntimeSink;
	readonly #server: Server;
	readonly #sessions = new Map<string, SessionBinding>();
	readonly #settlementTimers = new Map<
		string,
		{ readonly eventExpiresAt: number; readonly handle: ReturnType<typeof setTimeout> }
	>();
	readonly #socketPath: string | null;
	#started = false;

	constructor(
		runtime: AgentHookObservationRuntimeSink,
		options: AgentHookObservationIngressOptions,
	) {
		this.#runtime = runtime;
		this.#allocateIdentifier = options.allocateIdentifier;
		this.#now = options.now ?? Date.now;
		this.#observationJoin = new AgentObservationJoinStateMachine(
			options.joinSettleMilliseconds === undefined
				? {}
				: { joinSettleMilliseconds: options.joinSettleMilliseconds },
		);
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
		this.#handleJoinCommands(
			this.#observationJoin.registerPresentationGeneration(presentationId, creationId),
		);
		return [{ name: 'ORCHBEE_AGENT_OBSERVATION_SOCKET', value: this.#socketPath }];
	}

	observeTerminalApplicationSignal(
		event: TerminalApplicationSignal,
		presentationId: TerminalPresentationId,
	): void {
		const parsed = AgentHookPresentationCorrelationHintSchema.safeParse(parseJson(event.payload));
		if (!parsed.success) return;
		this.#handleJoinCommands(
			this.#observationJoin.receiveHint(
				{
					correlationId: parsed.data.correlationId,
					eventId: parsed.data.eventId,
					generation: event.identity.creationId,
					presentationId,
					visibility: parsed.data.visibility,
				},
				this.#now(),
			),
		);
	}

	retirePresentation(presentationId: TerminalPresentationId): void {
		this.#handleJoinCommands(this.#observationJoin.retirePresentation(presentationId));
	}

	async dispose(): Promise<void> {
		for (const timer of this.#settlementTimers.values()) clearTimeout(timer.handle);
		this.#settlementTimers.clear();
		this.#observationJoin.reset();
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
		this.#handleJoinCommands(this.#observationJoin.receiveEvent(adapter, parsed.data, this.#now()));
	}

	#handleJoinCommands(commands: readonly AgentObservationJoinCommand[]): void {
		for (const command of commands) {
			switch (command.type) {
				case 'schedule-settlement':
					this.#scheduleSettlement(command);
					break;
				case 'cancel-settlement':
					this.#cancelSettlement(command.eventKey, command.eventExpiresAt);
					break;
				case 'apply-observation':
					this.#observe(
						command.adapter,
						command.envelope.observation,
						command.envelope.eventId,
						command.attachments,
					);
					break;
				case 'attach-presentations': {
					const binding = this.#sessions.get(sessionKeyFor(command.adapterId, command.sessionKey));
					if (binding !== undefined) this.#attachPresentations(binding, command.attachments);
					break;
				}
				case 'retire-presentation-generation':
					this.#detachPresentationGeneration(command.presentationId, command.generation);
					break;
			}
		}
	}

	#scheduleSettlement(
		command: Extract<AgentObservationJoinCommand, { readonly type: 'schedule-settlement' }>,
	): void {
		const existing = this.#settlementTimers.get(command.eventKey);
		if (existing?.eventExpiresAt === command.eventExpiresAt) return;
		if (existing !== undefined) clearTimeout(existing.handle);
		const handle = setTimeout(() => {
			const current = this.#settlementTimers.get(command.eventKey);
			if (current?.eventExpiresAt !== command.eventExpiresAt) return;
			this.#settlementTimers.delete(command.eventKey);
			this.#handleJoinCommands(
				this.#observationJoin.settle(command.eventKey, command.eventExpiresAt, this.#now()),
			);
		}, command.milliseconds);
		this.#settlementTimers.set(command.eventKey, {
			eventExpiresAt: command.eventExpiresAt,
			handle,
		});
	}

	#cancelSettlement(eventKey: string, eventExpiresAt: number): void {
		const existing = this.#settlementTimers.get(eventKey);
		if (existing?.eventExpiresAt !== eventExpiresAt) return;
		clearTimeout(existing.handle);
		this.#settlementTimers.delete(eventKey);
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

	#detachPresentationGeneration(
		presentationId: TerminalPresentationId,
		generation: CreationId,
	): void {
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

function sessionKeyFor(adapterId: AgentHookAdapterId, externalSessionKey: string): string {
	return `${adapterId}\0${externalSessionKey}`;
}
