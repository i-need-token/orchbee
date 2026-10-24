import { AgentRuntimeSnapshotSchema, AgentSemanticEventSchema } from '@orchbee/agent';
import { z } from 'zod';

export const DESKTOP_AGENT_OBSERVATION_PROTOCOL_VERSION = 1;

export const DesktopAgentObservationRequestIdSchema = z
	.string()
	.min(1)
	.max(128)
	.regex(/^[A-Za-z0-9:._-]+$/u)
	.brand<'DesktopAgentObservationRequestId'>();

export const DesktopAgentObservationRendererCommandSchema = z.strictObject({
	requestId: DesktopAgentObservationRequestIdSchema,
	type: z.literal('desktop-agent-observation-snapshot-requested'),
	version: z.literal(DESKTOP_AGENT_OBSERVATION_PROTOCOL_VERSION),
});

export const DesktopAgentObservationMainEventSchema = z.discriminatedUnion('type', [
	z.strictObject({
		requestId: DesktopAgentObservationRequestIdSchema.nullable(),
		snapshot: AgentRuntimeSnapshotSchema,
		type: z.literal('desktop-agent-observation-snapshot-published'),
		version: z.literal(DESKTOP_AGENT_OBSERVATION_PROTOCOL_VERSION),
	}),
	z.strictObject({
		event: AgentSemanticEventSchema,
		type: z.literal('desktop-agent-observation-semantic-event-published'),
		version: z.literal(DESKTOP_AGENT_OBSERVATION_PROTOCOL_VERSION),
	}),
]);

export type DesktopAgentObservationRequestId = z.infer<
	typeof DesktopAgentObservationRequestIdSchema
>;
