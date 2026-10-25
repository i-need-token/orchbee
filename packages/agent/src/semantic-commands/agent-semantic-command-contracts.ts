import { z } from 'zod';

import {
	AgentCapabilitySchema,
	AgentExecutionLocationSchema,
	AgentObservationSchema,
	AgentPresentationAttachmentSchema,
	AgentProviderIdSchema,
	AgentSessionIdSchema,
} from '../model';

import { AgentSemanticStatusSchema } from '../projection';

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

export type AgentFreshnessMeaning = z.infer<typeof AgentFreshnessMeaningSchema>;

export type AgentSessionExplanation = z.infer<typeof AgentSessionExplanationSchema>;

export type AgentSessionSummary = z.infer<typeof AgentSessionSummarySchema>;

export type AgentStatusBasis = z.infer<typeof AgentStatusBasisSchema>;
export type AgentWaitCondition = z.infer<typeof AgentWaitConditionSchema>;
