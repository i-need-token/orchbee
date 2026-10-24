import { z } from 'zod';

import {
	AgentCapabilitySchema,
	AgentExecutionLocationSchema,
	AgentObservationSchema,
	AgentPresentationAttachmentSchema,
	AgentProviderIdSchema,
	AgentSessionIdSchema,
	AgentSessionSnapshotSchema,
} from '../model';
import {
	AgentRuntimeChangeReadResultSchema,
	AgentRuntimeChangeSchema,
	MAX_AGENT_RUNTIME_CHANGE_PAGE_SIZE,
} from '../changes';
import { AgentSemanticStatusSchema } from '../projection';

const optionalPageShape = {
	cursor: z.string().min(1).max(4096).optional(),
	limit: z.number().int().min(1).max(MAX_AGENT_RUNTIME_CHANGE_PAGE_SIZE).default(50),
};

export const AgentSessionListInputSchema = z.strictObject({
	includeEnded: z.boolean().default(false),
	...optionalPageShape,
});

export const AgentSessionChangesInputSchema = z.strictObject({
	...optionalPageShape,
	timeoutMilliseconds: z.number().int().min(1).max(600_000).default(30_000),
});

const AgentWaitConditionSchema = z.enum([
	'needs-attention',
	'failed',
	'succeeded',
	'canceled',
	'interrupted',
	'working',
	'idle',
	'unknown',
	'ended',
	'fresh',
	'stale',
	'disconnected',
]);

export const DEFAULT_AGENT_WAIT_CONDITIONS = Object.freeze([
	'needs-attention',
	'failed',
	'succeeded',
	'canceled',
	'interrupted',
	'idle',
	'ended',
] as const);

const agentWaitConditionListSchema = z
	.array(AgentWaitConditionSchema)
	.min(1)
	.max(AgentWaitConditionSchema.options.length)
	.superRefine((conditions, context) => {
		if (new Set(conditions).size !== conditions.length) {
			context.addIssue({ code: 'custom', message: 'Agent wait conditions must be unique' });
		}
	});

export const AgentSessionWaitInputSchema = z.strictObject({
	afterRevision: z.number().int().nonnegative().safe().optional(),
	sessionId: AgentSessionIdSchema,
	timeoutMilliseconds: z.number().int().min(1).max(3_600_000).default(600_000),
	until: agentWaitConditionListSchema.default([...DEFAULT_AGENT_WAIT_CONDITIONS]),
});

const AgentSessionSummarySchema = z.strictObject({
	executionLocation: AgentExecutionLocationSchema,
	lifecycle: z.enum(['live', 'ended']),
	observation: AgentObservationSchema,
	parentSessionId: AgentSessionIdSchema.nullable(),
	pendingRequestCount: z.number().int().nonnegative().max(128),
	presentationAttachments: z.array(AgentPresentationAttachmentSchema).max(32),
	providerId: AgentProviderIdSchema,
	sessionId: AgentSessionIdSchema,
	status: AgentSemanticStatusSchema,
});

const AgentSessionDetailSchema = z.strictObject({
	snapshot: AgentSessionSnapshotSchema,
	status: AgentSemanticStatusSchema,
});

const AgentStatusBasisSchema = z.enum([
	'attention-request',
	'turn-outcome',
	'active-turn',
	'affirmative-idle',
	'session-ended',
	'limited-activity-hint',
	'unobserved-turn-state',
]);

const AgentFreshnessMeaningSchema = z.enum([
	'current-observation',
	'last-observation-may-be-outdated',
	'no-trusted-current-route',
]);

const AgentSessionExplanationSchema = z.strictObject({
	actionPath: z.literal('navigate-only'),
	capabilityCeiling: z.array(AgentCapabilitySchema).max(10),
	contentAvailable: z.literal(false),
	fidelity: AgentObservationSchema.shape.fidelity,
	freshness: AgentObservationSchema.shape.freshness,
	freshnessMeaning: AgentFreshnessMeaningSchema,
	generation: AgentObservationSchema.shape.generation,
	sequence: AgentObservationSchema.shape.sequence,
	source: AgentObservationSchema.shape.source,
	status: AgentSemanticStatusSchema,
	statusBasis: AgentStatusBasisSchema,
});

export const AgentSessionWaitOutputSchema = z.strictObject({
	matchedConditions: agentWaitConditionListSchema,
	revision: z.number().int().nonnegative().safe(),
	session: AgentSessionDetailSchema,
});

const agentSessionChangesPageShape = {
	nextCursor: z.string().min(1).max(4096),
	observedThroughRevision: z.number().int().nonnegative().safe(),
	runtimeRevision: z.number().int().nonnegative().safe(),
};

export const AgentSessionChangesPageSchema = z
	.discriminatedUnion('kind', [
		z.strictObject({
			...agentSessionChangesPageShape,
			hasMore: z.literal(false),
			items: z.tuple([]),
			kind: z.literal('baseline'),
		}),
		z.strictObject({
			...agentSessionChangesPageShape,
			hasMore: z.boolean(),
			items: z.array(AgentRuntimeChangeSchema).min(1).max(MAX_AGENT_RUNTIME_CHANGE_PAGE_SIZE),
			kind: z.literal('changes'),
		}),
		z.strictObject({
			...agentSessionChangesPageShape,
			hasMore: z.literal(false),
			items: z.tuple([]),
			kind: z.literal('current'),
		}),
		z.strictObject({
			...agentSessionChangesPageShape,
			hasMore: z.literal(false),
			items: z.tuple([]),
			kind: z.literal('reset-required'),
		}),
	])
	.superRefine((page, context) => {
		if (
			!AgentRuntimeChangeReadResultSchema.safeParse({
				changes: page.items,
				hasMore: page.hasMore,
				kind: page.kind === 'baseline' ? 'current' : page.kind,
				observedThroughRevision: page.observedThroughRevision,
				runtimeRevision: page.runtimeRevision,
			}).success
		) {
			context.addIssue({
				code: 'custom',
				message: 'Agent change page revisions and continuation must be consistent',
			});
		}
	});

export type AgentFreshnessMeaning = z.infer<typeof AgentFreshnessMeaningSchema>;
export type AgentSessionDetail = z.infer<typeof AgentSessionDetailSchema>;

export type AgentSessionExplanation = z.infer<typeof AgentSessionExplanationSchema>;

export type AgentSessionSummary = z.infer<typeof AgentSessionSummarySchema>;

export type AgentStatusBasis = z.infer<typeof AgentStatusBasisSchema>;
export type AgentWaitCondition = z.infer<typeof AgentWaitConditionSchema>;
