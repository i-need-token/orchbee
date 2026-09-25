import { AgentActivitySchema } from '@orchbee/agent';
import { z } from 'zod';

const MAX_CORRELATIONS_PER_EVENT = 32;
const hookIdentitySchema = z
	.string()
	.min(1)
	.max(128)
	.regex(/^[A-Za-z0-9._:-]+$/u);
const transportIdentitySchema = z
	.string()
	.min(16)
	.max(64)
	.regex(/^[A-Za-z0-9_-]+$/u);

const AgentHookAdapterIdSchema = z.enum([
	'claude-code',
	'codex',
	'grok',
	'opencode',
	'kilo',
	'cursor',
	'qodercli',
	'pi',
	'omp',
	'gemini',
	'github-copilot',
	'cline',
	'qwen-code',
]);

const AgentHookObservationEventSchema = z.enum([
	'session-identified',
	'session-started',
	'session-idle',
	'turn-started',
	'tool-started',
	'tool-finished',
	'tool-failed',
	'question-requested',
	'approval-requested',
	'attention-resolved',
	'compaction-started',
	'compaction-finished',
	'subagent-started',
	'subagent-stopped',
	'turn-stopped',
	'turn-failed',
	'turn-canceled',
	'turn-interrupted',
	'session-ended',
]);

const AgentHookSanitizedObservationSchema = z.strictObject({
	activity: AgentActivitySchema.nullable(),
	agentKey: hookIdentitySchema.nullable(),
	event: AgentHookObservationEventSchema,
	operationKey: hookIdentitySchema.nullable(),
	sessionKey: hookIdentitySchema,
	turnKey: hookIdentitySchema.nullable(),
	version: z.literal(1),
});

export const AgentHookRichObservationEnvelopeSchema = z
	.strictObject({
		adapterId: AgentHookAdapterIdSchema,
		correlations: z
			.array(
				z.strictObject({
					correlationId: transportIdentitySchema,
					visibility: z.enum(['visible', 'hidden', 'unknown']),
				}),
			)
			.min(1)
			.max(MAX_CORRELATIONS_PER_EVENT),
		eventId: transportIdentitySchema,
		observation: AgentHookSanitizedObservationSchema,
		version: z.literal(3),
	})
	.superRefine((envelope, context) => {
		const correlationIds = envelope.correlations.map(({ correlationId }) => correlationId);
		if (new Set(correlationIds).size !== correlationIds.length) {
			context.addIssue({
				code: 'custom',
				message: 'Agent hook correlation identities must be unique.',
				path: ['correlations'],
			});
		}
	});

export const AgentHookPresentationCorrelationHintSchema = z.strictObject({
	correlationId: transportIdentitySchema,
	eventId: transportIdentitySchema,
	kind: z.literal('correlation'),
	version: z.literal(3),
});

export type AgentHookAdapterId = z.infer<typeof AgentHookAdapterIdSchema>;
export type AgentHookObservationEvent = z.infer<typeof AgentHookObservationEventSchema>;
export type AgentHookRichObservationEnvelope = z.infer<
	typeof AgentHookRichObservationEnvelopeSchema
>;
export type AgentHookSanitizedObservation = z.infer<typeof AgentHookSanitizedObservationSchema>;
