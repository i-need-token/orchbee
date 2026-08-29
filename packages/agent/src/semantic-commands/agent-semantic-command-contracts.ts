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

export const AgentSessionTargetInputSchema = z.strictObject({
	sessionId: AgentSessionIdSchema,
});

export const AgentSessionChangesInputSchema = z.strictObject({
	...optionalPageShape,
	timeoutMilliseconds: z.number().int().min(1).max(600_000).default(30_000),
});

export const AgentWaitConditionSchema = z.enum([
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

export const AgentSessionSummarySchema = z.strictObject({
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

export const AgentSessionDetailSchema = z.strictObject({
	snapshot: AgentSessionSnapshotSchema,
	status: AgentSemanticStatusSchema,
});

export const AgentSessionListPageSchema = z.strictObject({
	items: z.array(AgentSessionSummarySchema).max(100),
	nextCursor: z.string().min(1).max(4096).nullable(),
	revision: z.number().int().nonnegative().safe(),
});

export const AgentSessionGetOutputSchema = z.strictObject({
	revision: z.number().int().nonnegative().safe(),
	session: AgentSessionDetailSchema,
});

export const AgentStatusBasisSchema = z.enum([
	'attention-request',
	'turn-outcome',
	'active-turn',
	'affirmative-idle',
	'session-ended',
	'limited-activity-hint',
	'unobserved-turn-state',
]);

export const AgentFreshnessMeaningSchema = z.enum([
	'current-observation',
	'last-observation-may-be-outdated',
	'no-trusted-current-route',
]);

export const AgentSessionExplanationSchema = z.strictObject({
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

export const AgentSessionExplainOutputSchema = z.strictObject({
	explanation: AgentSessionExplanationSchema,
	revision: z.number().int().nonnegative().safe(),
	session: AgentSessionDetailSchema,
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

export const AGENT_SESSION_LIST_OPERATION = Object.freeze({
	confirmation: 'noninteractive-allowed',
	id: 'agent.session.list',
	input: { name: 'agent.session-list-input', schemaVersion: 1 },
	kind: 'query',
	output: { name: 'agent.session-list-page', schemaVersion: 1 },
	retry: 'same-request-while-retained',
	schemaVersion: 1,
} as const);

export const AGENT_SESSION_GET_OPERATION = Object.freeze({
	confirmation: 'noninteractive-allowed',
	id: 'agent.session.get',
	input: { name: 'agent.session-target-input', schemaVersion: 1 },
	kind: 'query',
	output: { name: 'agent.session-get-output', schemaVersion: 1 },
	retry: 'same-request-while-retained',
	schemaVersion: 1,
} as const);

export const AGENT_SESSION_EXPLAIN_OPERATION = Object.freeze({
	confirmation: 'noninteractive-allowed',
	id: 'agent.session.explain',
	input: { name: 'agent.session-target-input', schemaVersion: 1 },
	kind: 'query',
	output: { name: 'agent.session-explain-output', schemaVersion: 1 },
	retry: 'same-request-while-retained',
	schemaVersion: 1,
} as const);

export const AGENT_SESSION_WAIT_OPERATION = Object.freeze({
	confirmation: 'noninteractive-allowed',
	id: 'agent.session.wait',
	input: { name: 'agent.session-wait-input', schemaVersion: 1 },
	kind: 'query',
	output: { name: 'agent.session-wait-output', schemaVersion: 1 },
	retry: 'same-request-while-retained',
	schemaVersion: 1,
} as const);

export const AGENT_SESSION_CHANGES_OPERATION = Object.freeze({
	confirmation: 'noninteractive-allowed',
	id: 'agent.session.changes',
	input: { name: 'agent.session-changes-input', schemaVersion: 1 },
	kind: 'query',
	output: { name: 'agent.session-changes-page', schemaVersion: 1 },
	retry: 'same-request-while-retained',
	schemaVersion: 1,
} as const);

export type AgentFreshnessMeaning = z.infer<typeof AgentFreshnessMeaningSchema>;
export type AgentSessionDetail = z.infer<typeof AgentSessionDetailSchema>;
export type AgentSessionChangesInput = z.infer<typeof AgentSessionChangesInputSchema>;
export type AgentSessionChangesPage = z.infer<typeof AgentSessionChangesPageSchema>;
export type AgentSessionExplainOutput = z.infer<typeof AgentSessionExplainOutputSchema>;
export type AgentSessionExplanation = z.infer<typeof AgentSessionExplanationSchema>;
export type AgentSessionGetOutput = z.infer<typeof AgentSessionGetOutputSchema>;
export type AgentSessionListInput = z.infer<typeof AgentSessionListInputSchema>;
export type AgentSessionListPage = z.infer<typeof AgentSessionListPageSchema>;
export type AgentSessionSummary = z.infer<typeof AgentSessionSummarySchema>;
export type AgentSessionTargetInput = z.infer<typeof AgentSessionTargetInputSchema>;
export type AgentSessionWaitInput = z.infer<typeof AgentSessionWaitInputSchema>;
export type AgentSessionWaitOutput = z.infer<typeof AgentSessionWaitOutputSchema>;
export type AgentStatusBasis = z.infer<typeof AgentStatusBasisSchema>;
export type AgentWaitCondition = z.infer<typeof AgentWaitConditionSchema>;
