import {
	AgentActivitySchema,
	AgentEventIdSchema,
	AgentExecutionLocationIdSchema,
	AgentProviderIdSchema,
	AgentRuntime,
	AgentSessionIdSchema,
	type AgentChangeSource,
	type AgentObservationEvent,
	type AgentRuntimeChangeReadInput,
	type AgentRuntimeChangeReadResult,
	type AgentRuntimeSnapshot,
	type AgentSessionId,
	type AgentSemanticEventListener,
	type AgentTransition,
} from '@orchbee/agent';
import type { TerminalApplicationSignal } from '@orchbee/terminal-protocol';
import { z } from 'zod';

const TERMINAL_SIGNAL_PROVIDER_ID = AgentProviderIdSchema.parse('terminal-signal');
const TerminalAgentActivityHintPayloadSchema = z.strictObject({
	activity: AgentActivitySchema.nullable(),
	version: z.literal(1),
});

interface TerminalSignalSession {
	readonly generation: string;
	readonly sessionId: AgentSessionId;
	readonly presentationId: string;
	lastTransportSequence: bigint;
}

export interface DesktopAgentObservationRuntimeOptions {
	readonly allocateEventId: () => string;
}

export class DesktopAgentObservationRuntime implements AgentChangeSource {
	readonly #allocateEventId: () => string;
	readonly #runtime: AgentRuntime;
	readonly #terminalSignalSessions = new Map<string, TerminalSignalSession>();

	constructor(
		options: DesktopAgentObservationRuntimeOptions,
		runtime: AgentRuntime = new AgentRuntime(),
	) {
		this.#allocateEventId = options.allocateEventId;
		this.#runtime = runtime;
	}

	dispatch(event: AgentObservationEvent): AgentTransition {
		return this.#runtime.dispatch(event);
	}

	dispose(): void {
		this.#terminalSignalSessions.clear();
		this.#runtime.dispose();
	}

	getSnapshot = (): AgentRuntimeSnapshot => this.#runtime.getSnapshot();

	readChanges = (input: AgentRuntimeChangeReadInput): AgentRuntimeChangeReadResult =>
		this.#runtime.readChanges(input);

	retireTerminalSignalPresentation(presentationId: string): void {
		for (const [key, binding] of this.#terminalSignalSessions) {
			if (binding.presentationId !== presentationId) continue;
			this.#terminalSignalSessions.delete(key);
			const session = this.#runtime
				.getSnapshot()
				.sessions.find((candidate) => candidate.sessionId === binding.sessionId);
			if (session === undefined) continue;
			const attachment = session.presentationAttachments.find(
				(candidate) =>
					candidate.presentationId === presentationId &&
					candidate.generation === binding.generation &&
					candidate.navigable,
			);
			if (attachment === undefined) continue;
			this.#runtime.dispatch({
				eventId: AgentEventIdSchema.parse(this.#allocateEventId()),
				generation: session.observation.generation,
				presentationGeneration: binding.generation,
				presentationId,
				sequence: session.observation.sequence + 1,
				sessionId: session.sessionId,
				type: 'presentation-detached',
			});
		}
	}

	observeTerminalApplicationSignal(event: TerminalApplicationSignal, presentationId: string): void {
		const payload = TerminalAgentActivityHintPayloadSchema.safeParse(parseJson(event.payload));
		if (!payload.success) return;

		const transportSequence = BigInt(event.sequence);
		const key = this.#terminalSignalKey(presentationId, event.identity.creationId);
		let binding = this.#terminalSignalSessions.get(key);
		if (binding !== undefined && transportSequence <= binding.lastTransportSequence) return;

		if (binding === undefined) {
			const sessionId = AgentSessionIdSchema.parse(this.#allocateEventId());
			const observed = this.#runtime.dispatch({
				capabilities: ['activity'],
				eventId: this.#eventId(),
				executionLocation: {
					executionLocationId: AgentExecutionLocationIdSchema.parse(this.#allocateEventId()),
				},
				observation: {
					fidelity: 'basic',
					freshness: 'fresh',
					generation: 0,
					sequence: 1,
					source: 'terminal',
				},
				parentSessionId: null,
				presentationAttachments: [
					{
						generation: event.identity.creationId,
						navigable: true,
						presentationId,
						visibility: 'unknown',
					},
				],
				providerId: TERMINAL_SIGNAL_PROVIDER_ID,
				sessionId,
				type: 'session-observed',
			});
			if (observed.kind === 'rejected') return;
			binding = {
				generation: event.identity.creationId,
				lastTransportSequence: 0n,
				presentationId,
				sessionId,
			};
			this.#terminalSignalSessions.set(key, binding);
		}

		const session = this.#runtime
			.getSnapshot()
			.sessions.find((candidate) => candidate.sessionId === binding.sessionId);
		if (
			session === undefined ||
			!session.presentationAttachments.some(
				(attachment) =>
					attachment.presentationId === presentationId &&
					attachment.generation === event.identity.creationId &&
					attachment.navigable,
			)
		)
			return;
		const transition = this.#runtime.dispatch({
			activity: payload.data.activity,
			eventId: this.#eventId(),
			generation: session.observation.generation,
			sequence: session.observation.sequence + 1,
			sessionId: binding.sessionId,
			type: 'limited-activity-observed',
		});
		if (transition.kind !== 'rejected') binding.lastTransportSequence = transportSequence;
	}

	#eventId(): ReturnType<typeof AgentEventIdSchema.parse> {
		return AgentEventIdSchema.parse(this.#allocateEventId());
	}

	#terminalSignalKey(presentationId: string, generation: string): string {
		return `${presentationId}\0${generation}`;
	}

	subscribe = (listener: () => void): (() => void) => this.#runtime.subscribe(listener);

	subscribeSemanticEvents = (listener: AgentSemanticEventListener): (() => void) =>
		this.#runtime.subscribeSemanticEvents(listener);
}

function parseJson(value: string): unknown {
	try {
		return JSON.parse(value);
	} catch {
		return undefined;
	}
}
