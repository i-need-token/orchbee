import { z } from 'zod';

export const DESKTOP_AGENT_INTEGRATION_IDS = [
	'claude-code',
	'codex',
	'grok',
	'opencode',
	'pi',
	'omp',
	'gemini',
	'github-copilot',
	'cline',
	'qwen-code',
] as const;

export const DesktopAgentIntegrationIdSchema = z.enum(DESKTOP_AGENT_INTEGRATION_IDS);

const DesktopAgentIntegrationAvailabilitySchema = z.enum(['available', 'not-found', 'unknown']);

const DesktopAgentIntegrationStateSchema = z.enum([
	'conflict',
	'disabled',
	'enabled',
	'not-installed',
	'update-required',
	'unknown',
]);

const DesktopAgentIntegrationManagementModeSchema = z.enum(['managed-local', 'manual-only']);

const DesktopAgentIntegrationIssueSchema = z.enum([
	'inspection-failed',
	'manual-action-required',
	'unsupported-platform',
	'unsupported-version',
]);

const DesktopAgentIntegrationSnapshotItemSchema = z.strictObject({
	availability: DesktopAgentIntegrationAvailabilitySchema,
	integrationId: DesktopAgentIntegrationIdSchema,
	issue: DesktopAgentIntegrationIssueSchema.nullable(),
	managementMode: DesktopAgentIntegrationManagementModeSchema,
	state: DesktopAgentIntegrationStateSchema,
});

export const DesktopAgentIntegrationManagementSnapshotSchema = z.strictObject({
	integrations: z
		.array(DesktopAgentIntegrationSnapshotItemSchema)
		.length(DESKTOP_AGENT_INTEGRATION_IDS.length)
		.refine(
			(integrations) =>
				DESKTOP_AGENT_INTEGRATION_IDS.every(
					(integrationId, index) => integrations[index]?.integrationId === integrationId,
				),
			'Desktop Agent integrations must use canonical order',
		),
	launchedIntegrationIds: z
		.array(DesktopAgentIntegrationIdSchema)
		.max(DESKTOP_AGENT_INTEGRATION_IDS.length)
		.refine(
			(integrationIds) => new Set(integrationIds).size === integrationIds.length,
			'Desktop Agent integration launch observations must be unique',
		),
	revision: z.number().int().nonnegative(),
});

export const DesktopAgentIntegrationManagementRequestIdSchema = z
	.string()
	.min(1)
	.max(128)
	.regex(/^[A-Za-z0-9:._-]+$/u)
	.brand<'DesktopAgentIntegrationManagementRequestId'>();

const requestIdentityShape = {
	requestId: DesktopAgentIntegrationManagementRequestIdSchema,
} as const;

export const DesktopAgentIntegrationManagementRendererCommandSchema = z.discriminatedUnion('type', [
	z.strictObject({
		...requestIdentityShape,
		type: z.literal('desktop-agent-integration-inventory-requested'),
	}),
	z.strictObject({
		...requestIdentityShape,
		type: z.literal('desktop-agent-integration-refresh-requested'),
	}),
	z.strictObject({
		...requestIdentityShape,
		integrationId: DesktopAgentIntegrationIdSchema,
		type: z.literal('desktop-agent-integration-enable-requested'),
	}),
	z.strictObject({
		...requestIdentityShape,
		integrationId: DesktopAgentIntegrationIdSchema,
		type: z.literal('desktop-agent-integration-disable-requested'),
	}),
]);

const DesktopAgentIntegrationManagementFailureCodeSchema = z.enum([
	'agent-unavailable',
	'busy',
	'conflict',
	'control-unavailable',
	'manual-action-required',
	'operation-failed',
	'verification-failed',
]);

const operationResultSchema = z.discriminatedUnion('kind', [
	z.strictObject({
		kind: z.literal('completed'),
		snapshot: DesktopAgentIntegrationManagementSnapshotSchema,
	}),
	z.strictObject({
		code: DesktopAgentIntegrationManagementFailureCodeSchema,
		integrationId: DesktopAgentIntegrationIdSchema.nullable(),
		kind: z.literal('failed'),
		snapshot: DesktopAgentIntegrationManagementSnapshotSchema,
	}),
]);

export const DesktopAgentIntegrationManagementMainEventSchema = z.discriminatedUnion('type', [
	z
		.strictObject({
			integration: DesktopAgentIntegrationSnapshotItemSchema,
			integrationId: DesktopAgentIntegrationIdSchema,
			revision: z.number().int().nonnegative(),
			type: z.literal('desktop-agent-integration-launch-observed'),
		})
		.refine(
			(event) => event.integration.integrationId === event.integrationId,
			'Desktop Agent launch event integration identities must match',
		),
	z.strictObject({
		requestId: DesktopAgentIntegrationManagementRequestIdSchema,
		result: operationResultSchema,
		type: z.literal('desktop-agent-integration-responded'),
	}),
]);

export type DesktopAgentIntegrationId = z.infer<typeof DesktopAgentIntegrationIdSchema>;

export type DesktopAgentIntegrationManagementFailureCode = z.infer<
	typeof DesktopAgentIntegrationManagementFailureCodeSchema
>;
export type DesktopAgentIntegrationManagementMainEvent = z.infer<
	typeof DesktopAgentIntegrationManagementMainEventSchema
>;
export type DesktopAgentIntegrationManagementMode = z.infer<
	typeof DesktopAgentIntegrationManagementModeSchema
>;
export type DesktopAgentIntegrationManagementRendererCommand = z.infer<
	typeof DesktopAgentIntegrationManagementRendererCommandSchema
>;
export type DesktopAgentIntegrationManagementRequestId = z.infer<
	typeof DesktopAgentIntegrationManagementRequestIdSchema
>;
export type DesktopAgentIntegrationManagementSnapshot = z.infer<
	typeof DesktopAgentIntegrationManagementSnapshotSchema
>;
export type DesktopAgentIntegrationSnapshotItem = z.infer<
	typeof DesktopAgentIntegrationSnapshotItemSchema
>;
export type DesktopAgentIntegrationState = z.infer<typeof DesktopAgentIntegrationStateSchema>;
