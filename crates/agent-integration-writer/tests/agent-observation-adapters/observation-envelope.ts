import { z } from 'zod';

// The Server's hook ingress admits what the Capsule sends through `parse_envelope` in
// apps/orchbee-server/src/agent/hook_contract.rs, a port of the retired Desktop schema restored
// here. The adapter tests check that both envelope shapes stay admissible; the ingress's own
// rejections are the Server's tests.

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
	'antigravity-cli',
	'mastracode',
	'droid',
	'kimi',
	'letta',
	'devin',
	'hermes',
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

const AgentActivitySchema = z.enum([
	'thinking',
	'tool',
	'editing',
	'command',
	'subagent',
	'compacting',
	'retrying',
	'other',
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
		// Optional so envelopes from integrations installed before the run token stay admissible.
		runToken: z
			.string()
			.regex(/^(?!0{32}$)[0-9a-f]{32}$/u)
			.optional(),
		version: z.literal(1),
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
