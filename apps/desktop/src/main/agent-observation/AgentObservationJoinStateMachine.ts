import type { AgentPresentationAttachment } from '@orchbee/agent';
import type { CreationId } from '@orchbee/terminal-protocol';
import type { TerminalPresentationId } from '@orchbee/workbench/terminal-presentation';

import type { AgentHookRichObservationEnvelope } from './agent-hook-observation-contract';
import type { AgentHookAdapterDefinition } from './resolveAgentHookAdapter';

const DEFAULT_JOIN_SETTLE_MILLISECONDS = 25;
const DEFAULT_JOIN_TTL_MILLISECONDS = 10_000;
const DEFAULT_MAX_PENDING_EVENTS = 512;
const DEFAULT_MAX_PENDING_HINTS = 1024;
const DEFAULT_RECENT_EVENT_TTL_MILLISECONDS = 60_000;

export interface AgentObservationJoinStateMachineOptions {
	readonly joinSettleMilliseconds?: number;
	readonly joinTtlMilliseconds?: number;
	readonly maxPendingEvents?: number;
	readonly maxPendingHints?: number;
	readonly recentEventTtlMilliseconds?: number;
}

export interface AgentObservationJoinHint {
	readonly correlationId: string;
	readonly eventId: string;
	readonly generation: CreationId;
	readonly presentationId: TerminalPresentationId;
}

export type AgentObservationJoinCommand =
	| {
			readonly eventExpiresAt: number;
			readonly eventKey: string;
			readonly type: 'cancel-settlement';
	  }
	| {
			readonly adapter: AgentHookAdapterDefinition;
			readonly attachments: readonly AgentPresentationAttachment[];
			readonly envelope: AgentHookRichObservationEnvelope;
			readonly type: 'apply-observation';
	  }
	| {
			readonly adapterId: AgentHookRichObservationEnvelope['adapterId'];
			readonly attachments: readonly AgentPresentationAttachment[];
			readonly sessionKey: string;
			readonly type: 'attach-presentations';
	  }
	| {
			readonly eventExpiresAt: number;
			readonly eventKey: string;
			readonly milliseconds: number;
			readonly type: 'schedule-settlement';
	  }
	| {
			readonly generation: CreationId;
			readonly presentationId: TerminalPresentationId;
			readonly type: 'retire-presentation-generation';
	  };

export interface AgentObservationJoinEventAdmission {
	readonly accepted: boolean;
	readonly commands: readonly AgentObservationJoinCommand[];
}

interface PendingHint extends AgentObservationJoinHint {
	readonly expiresAt: number;
}

interface PendingEvent {
	readonly adapter: AgentHookAdapterDefinition;
	applied: boolean;
	readonly envelope: AgentHookRichObservationEnvelope;
	readonly expiresAt: number;
	readonly matchedAttachments: Map<string, AgentPresentationAttachment>;
	readonly matchedCorrelationIds: Set<string>;
	readonly matchedPresentationGenerations: Map<TerminalPresentationId, CreationId>;
	settlementScheduled: boolean;
}

interface RequiredAgentObservationJoinStateMachineOptions {
	readonly joinSettleMilliseconds: number;
	readonly joinTtlMilliseconds: number;
	readonly maxPendingEvents: number;
	readonly maxPendingHints: number;
	readonly recentEventTtlMilliseconds: number;
}

/**
 * Deterministically owns the bounded two-half Agent observation join. External
 * time, timers, sockets, and Agent runtime effects remain in the ingress shell.
 */
export class AgentObservationJoinStateMachine {
	readonly #correlationEventIds = new Map<string, string>();
	readonly #currentGenerations = new Map<TerminalPresentationId, CreationId>();
	readonly #options: RequiredAgentObservationJoinStateMachineOptions;
	readonly #pendingEvents = new Map<string, PendingEvent>();
	readonly #pendingHints = new Map<string, PendingHint>();
	readonly #recentEventExpirations = new Map<string, number>();

	constructor(options: AgentObservationJoinStateMachineOptions = {}) {
		this.#options = {
			joinSettleMilliseconds: options.joinSettleMilliseconds ?? DEFAULT_JOIN_SETTLE_MILLISECONDS,
			joinTtlMilliseconds: options.joinTtlMilliseconds ?? DEFAULT_JOIN_TTL_MILLISECONDS,
			maxPendingEvents: options.maxPendingEvents ?? DEFAULT_MAX_PENDING_EVENTS,
			maxPendingHints: options.maxPendingHints ?? DEFAULT_MAX_PENDING_HINTS,
			recentEventTtlMilliseconds:
				options.recentEventTtlMilliseconds ?? DEFAULT_RECENT_EVENT_TTL_MILLISECONDS,
		};
	}

	registerPresentationGeneration(
		presentationId: TerminalPresentationId,
		generation: CreationId,
	): readonly AgentObservationJoinCommand[] {
		const previousGeneration = this.#currentGenerations.get(presentationId);
		const commands: AgentObservationJoinCommand[] = [];
		if (previousGeneration !== undefined && previousGeneration !== generation) {
			this.#retireGeneration(presentationId, previousGeneration);
			commands.push({
				generation: previousGeneration,
				presentationId,
				type: 'retire-presentation-generation',
			});
		}
		this.#currentGenerations.set(presentationId, generation);
		return commands;
	}

	retirePresentation(
		presentationId: TerminalPresentationId,
	): readonly AgentObservationJoinCommand[] {
		const generation = this.#currentGenerations.get(presentationId);
		this.#currentGenerations.delete(presentationId);
		if (generation === undefined) return [];
		this.#retireGeneration(presentationId, generation);
		return [{ generation, presentationId, type: 'retire-presentation-generation' }];
	}

	receiveHint(hint: AgentObservationJoinHint, now: number): readonly AgentObservationJoinCommand[] {
		const commands = this.#cleanupExpired(now);
		if (this.#currentGenerations.get(hint.presentationId) !== hint.generation) return commands;

		const pendingHint: PendingHint = {
			...hint,
			expiresAt: now + this.#options.joinTtlMilliseconds,
		};
		const existing = this.#pendingHints.get(hint.correlationId);
		if (existing !== undefined) {
			if (!pendingHintsEqual(existing, pendingHint)) return commands;
		} else {
			if (this.#pendingHints.size >= this.#options.maxPendingHints) return commands;
			this.#pendingHints.set(hint.correlationId, pendingHint);
		}
		commands.push(...this.#joinCorrelation(hint.correlationId));
		return commands;
	}

	receiveEvent(
		adapter: AgentHookAdapterDefinition,
		envelope: AgentHookRichObservationEnvelope,
		now: number,
	): readonly AgentObservationJoinCommand[] {
		return this.admitEvent(adapter, envelope, now).commands;
	}

	admitEvent(
		adapter: AgentHookAdapterDefinition,
		envelope: AgentHookRichObservationEnvelope,
		now: number,
	): AgentObservationJoinEventAdmission {
		const commands = this.#cleanupExpired(now);
		const transportEventKey = eventKey(envelope.adapterId, envelope.eventId);
		if (this.#recentEventExpirations.has(transportEventKey)) {
			return { accepted: false, commands };
		}

		const existing = this.#pendingEvents.get(transportEventKey);
		if (existing !== undefined) {
			if (!envelopesEqual(existing.envelope, envelope)) {
				return { accepted: false, commands };
			}
			for (const { correlationId } of envelope.correlations) {
				commands.push(...this.#joinCorrelation(correlationId));
			}
			return { accepted: true, commands };
		}
		if (this.#pendingEvents.size >= this.#options.maxPendingEvents) {
			return { accepted: false, commands };
		}
		if (
			envelope.correlations.some(({ correlationId }) => {
				const claimedEventKey = this.#correlationEventIds.get(correlationId);
				return claimedEventKey !== undefined && claimedEventKey !== transportEventKey;
			})
		) {
			return { accepted: false, commands };
		}

		const pendingEvent: PendingEvent = {
			adapter,
			applied: false,
			envelope,
			expiresAt: now + this.#options.joinTtlMilliseconds,
			matchedAttachments: new Map(),
			matchedCorrelationIds: new Set(),
			matchedPresentationGenerations: new Map(),
			settlementScheduled: false,
		};
		this.#pendingEvents.set(transportEventKey, pendingEvent);
		for (const { correlationId } of envelope.correlations) {
			this.#correlationEventIds.set(correlationId, transportEventKey);
			commands.push(...this.#joinCorrelation(correlationId));
		}
		return { accepted: true, commands };
	}

	settle(
		transportEventKey: string,
		eventExpiresAt: number,
		now: number,
	): readonly AgentObservationJoinCommand[] {
		const commands = this.#cleanupExpired(now);
		const pendingEvent = this.#pendingEvents.get(transportEventKey);
		if (
			pendingEvent === undefined ||
			pendingEvent.expiresAt !== eventExpiresAt ||
			!pendingEvent.settlementScheduled
		) {
			return commands;
		}
		pendingEvent.settlementScheduled = false;
		if (pendingEvent.applied || pendingEvent.matchedAttachments.size === 0) return commands;
		pendingEvent.applied = true;
		commands.push(applyObservationCommand(pendingEvent));
		return commands;
	}

	reset(): void {
		this.#pendingEvents.clear();
		this.#pendingHints.clear();
		this.#correlationEventIds.clear();
		this.#recentEventExpirations.clear();
		this.#currentGenerations.clear();
	}

	#joinCorrelation(correlationId: string): readonly AgentObservationJoinCommand[] {
		const transportEventKey = this.#correlationEventIds.get(correlationId);
		const hint = this.#pendingHints.get(correlationId);
		if (transportEventKey === undefined || hint === undefined) return [];
		const pendingEvent = this.#pendingEvents.get(transportEventKey);
		if (pendingEvent === undefined || hint.eventId !== pendingEvent.envelope.eventId) return [];
		const correlation = pendingEvent.envelope.correlations.find(
			(candidate) => candidate.correlationId === correlationId,
		);
		if (correlation === undefined) return [];
		if (pendingEvent.matchedCorrelationIds.has(correlationId)) {
			this.#pendingHints.delete(correlationId);
			return [];
		}

		const matchedGeneration = pendingEvent.matchedPresentationGenerations.get(hint.presentationId);
		if (matchedGeneration !== undefined && matchedGeneration !== hint.generation) {
			this.#pendingHints.delete(correlationId);
			return [];
		}

		pendingEvent.matchedCorrelationIds.add(correlationId);
		pendingEvent.matchedPresentationGenerations.set(hint.presentationId, hint.generation);
		const key = attachmentKey(hint.presentationId, hint.generation);
		const matchedAttachment = pendingEvent.matchedAttachments.get(key);
		const nextAttachment = attachmentFor(hint, correlation.visibility);
		const attachment =
			matchedAttachment === undefined
				? nextAttachment
				: {
						...nextAttachment,
						visibility: combinePresentationVisibility(
							matchedAttachment.visibility,
							nextAttachment.visibility,
						),
					};
		pendingEvent.matchedAttachments.set(key, attachment);
		this.#pendingHints.delete(correlationId);

		if (pendingEvent.applied) {
			return [
				{
					adapterId: pendingEvent.envelope.adapterId,
					attachments: [attachment],
					sessionKey: pendingEvent.envelope.observation.sessionKey,
					type: 'attach-presentations',
				},
			];
		}
		if (pendingEvent.matchedCorrelationIds.size === pendingEvent.envelope.correlations.length) {
			return this.#applyPendingEvent(transportEventKey, pendingEvent);
		}
		if (pendingEvent.settlementScheduled) return [];
		pendingEvent.settlementScheduled = true;
		return [
			{
				eventExpiresAt: pendingEvent.expiresAt,
				eventKey: transportEventKey,
				milliseconds: this.#options.joinSettleMilliseconds,
				type: 'schedule-settlement',
			},
		];
	}

	#applyPendingEvent(
		transportEventKey: string,
		pendingEvent: PendingEvent,
	): readonly AgentObservationJoinCommand[] {
		if (pendingEvent.applied) return [];
		const commands: AgentObservationJoinCommand[] = [];
		if (pendingEvent.settlementScheduled) {
			pendingEvent.settlementScheduled = false;
			commands.push({
				eventExpiresAt: pendingEvent.expiresAt,
				eventKey: transportEventKey,
				type: 'cancel-settlement',
			});
		}
		if (pendingEvent.matchedAttachments.size === 0) return commands;
		pendingEvent.applied = true;
		commands.push(applyObservationCommand(pendingEvent));
		return commands;
	}

	#retireGeneration(presentationId: TerminalPresentationId, generation: CreationId): void {
		for (const [correlationId, hint] of this.#pendingHints) {
			if (hint.presentationId === presentationId && hint.generation === generation) {
				this.#pendingHints.delete(correlationId);
			}
		}
		for (const pendingEvent of this.#pendingEvents.values()) {
			pendingEvent.matchedAttachments.delete(attachmentKey(presentationId, generation));
		}
	}

	#cleanupExpired(now: number): AgentObservationJoinCommand[] {
		const commands: AgentObservationJoinCommand[] = [];
		for (const [correlationId, hint] of this.#pendingHints) {
			if (hint.expiresAt <= now) this.#pendingHints.delete(correlationId);
		}
		for (const [transportEventKey, pendingEvent] of this.#pendingEvents) {
			if (pendingEvent.expiresAt > now) continue;
			if (pendingEvent.settlementScheduled) {
				commands.push({
					eventExpiresAt: pendingEvent.expiresAt,
					eventKey: transportEventKey,
					type: 'cancel-settlement',
				});
			}
			this.#pendingEvents.delete(transportEventKey);
			for (const { correlationId } of pendingEvent.envelope.correlations) {
				if (this.#correlationEventIds.get(correlationId) === transportEventKey) {
					this.#correlationEventIds.delete(correlationId);
				}
			}
			if (pendingEvent.applied) {
				this.#recentEventExpirations.set(
					transportEventKey,
					now + this.#options.recentEventTtlMilliseconds,
				);
			}
		}
		for (const [transportEventKey, expiresAt] of this.#recentEventExpirations) {
			if (expiresAt <= now) this.#recentEventExpirations.delete(transportEventKey);
		}
		return commands;
	}
}

function applyObservationCommand(pendingEvent: PendingEvent): AgentObservationJoinCommand {
	return {
		adapter: pendingEvent.adapter,
		attachments: [...pendingEvent.matchedAttachments.values()],
		envelope: pendingEvent.envelope,
		type: 'apply-observation',
	};
}

function attachmentFor(
	hint: AgentObservationJoinHint,
	visibility: AgentPresentationAttachment['visibility'],
): AgentPresentationAttachment {
	return {
		generation: hint.generation,
		navigable: true,
		presentationId: hint.presentationId,
		visibility,
	};
}

function eventKey(
	adapterId: AgentHookRichObservationEnvelope['adapterId'],
	externalEventId: string,
): string {
	return `${adapterId}\0${externalEventId}`;
}

function attachmentKey(presentationId: TerminalPresentationId, generation: CreationId): string {
	return `${presentationId}\0${generation}`;
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
		left.presentationId === right.presentationId &&
		left.generation === right.generation
	);
}

function envelopesEqual(
	left: AgentHookRichObservationEnvelope,
	right: AgentHookRichObservationEnvelope,
): boolean {
	return JSON.stringify(left) === JSON.stringify(right);
}
