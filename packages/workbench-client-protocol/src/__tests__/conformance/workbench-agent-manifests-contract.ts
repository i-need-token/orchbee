import { z } from 'zod';
import {
	AgentManifestsFailureSchema as failure,
	AgentManifestMaintenanceSchema,
	AgentManifestsOperationSchema,
} from './workbench-agent-manifest-update-contract';

import { AgentDetectionManifestSchema } from './workbench-panel-agent-contract';
import {
	SessionIdSchema,
	WorkbenchRequestIdSchema,
	WorkbenchRevisionSchema,
	WORKBENCH_PROTOCOL_VERSION,
} from './workbench-semantic-contract';
export const WORKBENCH_MAXIMUM_ENCODED_AGENT_MANIFESTS_BYTES = 512 * 1024;
const revision = WorkbenchRevisionSchema.refine((v) => v !== '0');
const version = z
	.string()
	.max(128)
	.refine(
		(s) =>
			/^[0-9]+(?:\.[0-9]+)*$/u.test(s) &&
			s.split('.').every((p) => BigInt(p) <= 18446744073709551615n),
	)
	.nullable();
const summary = z
	.strictObject({
		agent: z
			.string()
			.min(1)
			.max(64)
			.regex(/^[a-z0-9-]+$/u),
		version,
		manifest: AgentDetectionManifestSchema,
	})
	.refine(
		(m) => m.manifest.source !== 'remote-cache' || m.version === m.manifest.cachedRemoteVersion,
	)
	.readonly();
const AgentManifestsSnapshotSchema = z
	.strictObject({
		revision,
		engineVersion: z.literal(3),
		maintenance: AgentManifestMaintenanceSchema.optional(),
		configurationRoot: z
			.string()
			.refine(
				(s) =>
					s.length > 0 && new TextEncoder().encode(s).byteLength <= 32768 && !/\p{Cc}/u.test(s),
			)
			.nullable(),
		manifests: z
			.array(summary)
			.max(24)
			.refine((rows) => new Set(rows.map((m) => m.agent)).size === rows.length)
			.readonly(),
	})
	.readonly();
const AgentManifestsActionSchema = z.discriminatedUnion('kind', [
	z.strictObject({ kind: z.literal('current') }),
	z.strictObject({ kind: z.literal('reload'), operationId: WorkbenchRequestIdSchema }),
	z.strictObject({ kind: z.literal('update'), operationId: WorkbenchRequestIdSchema }),
	z.strictObject({ kind: z.literal('operation'), operationId: WorkbenchRequestIdSchema }),
]);
export const AgentManifestsRequestSchema = z
	.strictObject({
		kind: z.literal('workbench.agent-manifests'),
		protocolVersion: z.literal(WORKBENCH_PROTOCOL_VERSION),
		requestId: WorkbenchRequestIdSchema,
		sessionId: SessionIdSchema,
		action: AgentManifestsActionSchema,
	})
	.readonly();
export const AgentManifestsResultSchema = z
	.strictObject({
		kind: z.literal('workbench.agent-manifests.result'),
		protocolVersion: z.literal(WORKBENCH_PROTOCOL_VERSION),
		requestId: WorkbenchRequestIdSchema,
		sessionId: SessionIdSchema,
		outcome: z.discriminatedUnion('kind', [
			z.strictObject({ kind: z.literal('current'), snapshot: AgentManifestsSnapshotSchema }),
			z.strictObject({
				kind: z.literal('operation'),
				operationId: WorkbenchRequestIdSchema,
				status: AgentManifestsOperationSchema,
			}),
			z.strictObject({ kind: z.literal('failed'), reason: failure }),
		]),
	})
	.readonly();
