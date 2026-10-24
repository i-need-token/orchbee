import { z } from 'zod';

import type { AgentObservationEvent } from './events';
import {
	AgentObservationSchema,
	AgentProviderIdSchema,
	AgentSessionIdSchema,
	type AgentRuntimeSnapshot,
	type AgentSessionSnapshot,
} from './model';
import { AgentSemanticStatusSchema, projectAgentSession } from './projection';

const AGENT_RUNTIME_CHANGE_RETENTION = 512;
export const MAX_AGENT_RUNTIME_CHANGE_PAGE_SIZE = 100;

const AgentObservationTransitionKindSchema = z.enum(
	completeAgentObservationTransitionKinds([
		'session-observed',
		'turn-started',
		'turn-activity-changed',
		'attention-requested',
		'attention-resolved',
		'turn-finished',
		'session-idle-observed',
		'observation-became-stale',
		'observation-reconnected',
		'presentation-attached',
		'presentation-visibility-changed',
		'presentation-detached',
		'session-ended',
	]),
);

const AgentRuntimeChangeSessionSchema = z.strictObject({
	lifecycle: z.enum(['live', 'ended']),
	observation: AgentObservationSchema,
	parentSessionId: AgentSessionIdSchema.nullable(),
	pendingRequestCount: z.number().int().nonnegative().max(128),
	providerId: AgentProviderIdSchema,
	sessionId: AgentSessionIdSchema,
	status: AgentSemanticStatusSchema,
});

export const AgentRuntimeChangeSchema = z.strictObject({
	revision: z.number().int().positive().safe(),
	session: AgentRuntimeChangeSessionSchema,
	transition: AgentObservationTransitionKindSchema,
});

const AgentRuntimeChangeReadInputSchema = z.strictObject({
	afterRevision: z.number().int().nonnegative().safe(),
	limit: z.number().int().min(1).max(MAX_AGENT_RUNTIME_CHANGE_PAGE_SIZE),
});

const commonReadResultShape = {
	hasMore: z.boolean(),
	observedThroughRevision: z.number().int().nonnegative().safe(),
	runtimeRevision: z.number().int().nonnegative().safe(),
};

export const AgentRuntimeChangeReadResultSchema = z
	.discriminatedUnion('kind', [
		z.strictObject({
			...commonReadResultShape,
			changes: z.array(AgentRuntimeChangeSchema).min(1).max(MAX_AGENT_RUNTIME_CHANGE_PAGE_SIZE),
			kind: z.literal('changes'),
		}),
		z.strictObject({
			...commonReadResultShape,
			changes: z.tuple([]),
			hasMore: z.literal(false),
			kind: z.literal('current'),
		}),
		z.strictObject({
			...commonReadResultShape,
			changes: z.tuple([]),
			hasMore: z.literal(false),
			kind: z.literal('reset-required'),
		}),
		z.strictObject({
			...commonReadResultShape,
			changes: z.tuple([]),
			hasMore: z.literal(false),
			kind: z.literal('revision-ahead'),
		}),
	])
	.superRefine((result, context) => {
		if (result.kind === 'revision-ahead') {
			if (result.observedThroughRevision <= result.runtimeRevision) {
				context.addIssue({
					code: 'custom',
					message: 'An ahead Agent change revision must exceed the runtime revision',
					path: ['observedThroughRevision'],
				});
			}
			return;
		}
		if (result.kind !== 'changes') {
			if (result.observedThroughRevision !== result.runtimeRevision) {
				context.addIssue({
					code: 'custom',
					message: 'An empty Agent change result must advance through the runtime revision',
					path: ['observedThroughRevision'],
				});
			}
			return;
		}
		validateAgentChangePage(
			result.changes,
			result.hasMore,
			result.observedThroughRevision,
			result.runtimeRevision,
			context,
		);
	});

export type AgentRuntimeChange = z.infer<typeof AgentRuntimeChangeSchema>;
export type AgentRuntimeChangeReadInput = z.infer<typeof AgentRuntimeChangeReadInputSchema>;
export type AgentRuntimeChangeReadResult = z.infer<typeof AgentRuntimeChangeReadResultSchema>;
type AgentRuntimeChangeSession = z.infer<typeof AgentRuntimeChangeSessionSchema>;

export function createAgentRuntimeChange(
	snapshot: AgentRuntimeSnapshot,
	event: AgentObservationEvent,
): AgentRuntimeChange {
	const session = snapshot.sessions.find((candidate) => candidate.sessionId === event.sessionId);
	if (session === undefined) {
		throw new Error('An applied Agent transition must retain its target Session.');
	}
	return AgentRuntimeChangeSchema.parse({
		revision: snapshot.revision,
		session: projectChangeSession(session),
		transition: event.type,
	});
}

export function appendAgentRuntimeChange(
	changes: readonly AgentRuntimeChange[],
	change: AgentRuntimeChange,
): readonly AgentRuntimeChange[] {
	const parsed = AgentRuntimeChangeSchema.parse(change);
	const previous = changes.at(-1);
	if (previous !== undefined && parsed.revision !== previous.revision + 1) {
		throw new Error('Agent runtime change revisions must remain contiguous.');
	}
	return [...changes, parsed].slice(-AGENT_RUNTIME_CHANGE_RETENTION);
}

export function readAgentRuntimeChanges(
	runtimeRevision: number,
	changes: readonly AgentRuntimeChange[],
	input: AgentRuntimeChangeReadInput,
): AgentRuntimeChangeReadResult {
	const parsedInput = AgentRuntimeChangeReadInputSchema.parse(input);
	if (parsedInput.afterRevision > runtimeRevision) {
		return parseReadResult({
			changes: [],
			hasMore: false,
			kind: 'revision-ahead',
			observedThroughRevision: parsedInput.afterRevision,
			runtimeRevision,
		});
	}

	const first = changes[0];
	if (
		parsedInput.afterRevision < runtimeRevision &&
		(first === undefined || parsedInput.afterRevision < first.revision - 1)
	) {
		return parseReadResult({
			changes: [],
			hasMore: false,
			kind: 'reset-required',
			observedThroughRevision: runtimeRevision,
			runtimeRevision,
		});
	}

	const available = changes.filter((change) => change.revision > parsedInput.afterRevision);
	if (available.length === 0) {
		return parseReadResult({
			changes: [],
			hasMore: false,
			kind: 'current',
			observedThroughRevision: parsedInput.afterRevision,
			runtimeRevision,
		});
	}

	const page = available.slice(0, parsedInput.limit);
	const last = page.at(-1);
	if (last === undefined) throw new Error('An Agent change page must contain one change.');
	return parseReadResult({
		changes: page,
		hasMore: available.length > page.length,
		kind: 'changes',
		observedThroughRevision: last.revision,
		runtimeRevision,
	});
}

function projectChangeSession(session: AgentSessionSnapshot): AgentRuntimeChangeSession {
	return {
		lifecycle: session.lifecycle,
		observation: session.observation,
		parentSessionId: session.parentSessionId,
		pendingRequestCount: session.pendingRequests.length,
		providerId: session.providerId,
		sessionId: session.sessionId,
		status: projectAgentSession(session).status,
	};
}

function parseReadResult(result: unknown): AgentRuntimeChangeReadResult {
	return AgentRuntimeChangeReadResultSchema.parse(result);
}

function validateAgentChangePage(
	changes: readonly AgentRuntimeChange[],
	hasMore: boolean,
	observedThroughRevision: number,
	runtimeRevision: number,
	context: z.RefinementCtx,
): void {
	for (let index = 1; index < changes.length; index += 1) {
		if (changes[index]?.revision !== (changes[index - 1]?.revision ?? 0) + 1) {
			context.addIssue({
				code: 'custom',
				message: 'Agent change page revisions must remain contiguous',
				path: ['changes', index, 'revision'],
			});
		}
	}
	if (changes.at(-1)?.revision !== observedThroughRevision) {
		context.addIssue({
			code: 'custom',
			message: 'An Agent change page cursor must match its final revision',
			path: ['observedThroughRevision'],
		});
	}
	if (observedThroughRevision > runtimeRevision) {
		context.addIssue({
			code: 'custom',
			message: 'An Agent change page cannot lead the runtime revision',
			path: ['observedThroughRevision'],
		});
	}
	const expectedHasMore = observedThroughRevision < runtimeRevision;
	if (hasMore !== expectedHasMore) {
		context.addIssue({
			code: 'custom',
			message: 'Agent change page continuation must match the runtime revision',
			path: ['hasMore'],
		});
	}
}

function completeAgentObservationTransitionKinds<
	const Kinds extends readonly [AgentObservationEvent['type'], ...AgentObservationEvent['type'][]],
>(
	kinds: Exclude<AgentObservationEvent['type'], Kinds[number]> extends never ? Kinds : never,
): Kinds {
	return kinds;
}
