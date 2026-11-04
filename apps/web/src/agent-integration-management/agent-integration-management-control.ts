import { AGENT_INTEGRATION_PROVIDERS } from '@orchbee/workbench-client-protocol';
import { z } from 'zod';

/** The Agents Settings lists every wire provider, in registry order. */
export const AGENT_INTEGRATION_IDS = AGENT_INTEGRATION_PROVIDERS;

const AgentIntegrationIdSchema = z.enum(AGENT_INTEGRATION_IDS);

const AgentIntegrationAvailabilitySchema = z.enum(['available', 'not-found', 'unknown']);

const AgentIntegrationStateSchema = z.enum([
	'conflict',
	'disabled',
	'enabled',
	'not-installed',
	'update-required',
	'unknown',
]);

const AgentIntegrationManagementModeSchema = z.enum(['managed-local', 'manual-only']);

const AgentIntegrationIssueSchema = z.enum([
	'inspection-failed',
	'manual-action-required',
	'unsupported-platform',
	'unsupported-version',
]);

const AgentIntegrationSnapshotItemSchema = z.strictObject({
	availability: AgentIntegrationAvailabilitySchema,
	integrationId: AgentIntegrationIdSchema,
	issue: AgentIntegrationIssueSchema.nullable(),
	managementMode: AgentIntegrationManagementModeSchema,
	state: AgentIntegrationStateSchema,
});

const AgentIntegrationManagementSnapshotSchema = z.strictObject({
	integrations: z
		.array(AgentIntegrationSnapshotItemSchema)
		.length(AGENT_INTEGRATION_IDS.length)
		.refine(
			(integrations) =>
				AGENT_INTEGRATION_IDS.every(
					(integrationId, index) => integrations[index]?.integrationId === integrationId,
				),
			'Agent integrations must use canonical order',
		),
	revision: z.number().int().nonnegative(),
});

const AgentIntegrationManagementFailureCodeSchema = z.enum([
	'agent-unavailable',
	'busy',
	'conflict',
	'control-unavailable',
	'manual-action-required',
	'operation-failed',
	'verification-failed',
]);

const AgentIntegrationManagementResultSchema = z.discriminatedUnion('kind', [
	z.strictObject({
		kind: z.literal('completed'),
		snapshot: AgentIntegrationManagementSnapshotSchema,
	}),
	z.strictObject({
		code: AgentIntegrationManagementFailureCodeSchema,
		integrationId: AgentIntegrationIdSchema.nullable(),
		kind: z.literal('failed'),
		snapshot: AgentIntegrationManagementSnapshotSchema,
	}),
]);

export type AgentIntegrationId = z.infer<typeof AgentIntegrationIdSchema>;
export type AgentIntegrationManagementFailureCode = z.infer<
	typeof AgentIntegrationManagementFailureCodeSchema
>;
export type AgentIntegrationManagementSnapshot = z.infer<
	typeof AgentIntegrationManagementSnapshotSchema
>;
export type AgentIntegrationSnapshotItem = z.infer<typeof AgentIntegrationSnapshotItemSchema>;

export interface AgentIntegrationManagementClientSnapshot extends AgentIntegrationManagementSnapshot {
	readonly failureCode: AgentIntegrationManagementFailureCode | null;
	readonly failureIntegrationId: AgentIntegrationId | null;
	readonly pending: boolean;
	readonly pendingIntegrationId: AgentIntegrationId | null;
}

export type AgentIntegrationManagementClientOperationResult =
	| z.infer<typeof AgentIntegrationManagementResultSchema>
	| { readonly kind: 'unavailable' };

export interface AgentIntegrationManagementControl {
	disable(
		integrationId: AgentIntegrationId,
	): Promise<AgentIntegrationManagementClientOperationResult>;
	enable(
		integrationId: AgentIntegrationId,
	): Promise<AgentIntegrationManagementClientOperationResult>;
	getSnapshot(): AgentIntegrationManagementClientSnapshot;
	refresh(): Promise<AgentIntegrationManagementClientOperationResult>;
	subscribe(listener: () => void): () => void;
}
