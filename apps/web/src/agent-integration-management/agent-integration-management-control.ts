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
	/** Present when the report left the state unread because reading it starts the Agent. */
	notChecked: z.literal(true).optional(),
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

export type AgentIntegrationMarketplace = 'absent' | 'current' | 'update-required';

export type AgentIntegrationEntryState =
	| 'conflict'
	| 'current'
	| 'missing'
	| 'unknown'
	| 'update-required';

/** The provider-specific facts an explain report establishes, without paths. */
type AgentIntegrationInspectionDetail =
	| {
			readonly kind: 'cli';
			readonly marketplace: AgentIntegrationMarketplace | null;
			readonly pluginPresent: boolean | null;
	  }
	| {
			readonly kind: 'codex';
			readonly marketplace: AgentIntegrationMarketplace;
			readonly pluginPresent: boolean;
	  }
	| {
			readonly entry: AgentIntegrationEntryState;
			readonly kind: 'guarded';
			readonly sourceReady: boolean;
	  }
	| { readonly kind: 'plain' };

/** What one planned provider command does; mirrors `CommandPurpose` in the Rust contract. */
export type AgentIntegrationCommandPurpose =
	| 'add-plugin-source'
	| 'confirm-plugin'
	| 'enable-plugin'
	| 'install-plugin'
	| 'remove-plugin'
	| 'remove-plugin-source';

/**
 * One step turning the integration on would take; a command is named by its file name only, with
 * what it does.
 */
type AgentIntegrationPlanStep =
	| {
			readonly command: string;
			readonly kind: 'command';
			readonly purpose: AgentIntegrationCommandPurpose | null;
	  }
	| { readonly kind: 'publish-adapter' | 'remove-entry' | 'write-entry' };

/** What one explain job established about one Agent, without paths or command lines. */
export interface AgentIntegrationExplanation {
	readonly adapter: 'binding-failed' | 'not-ready' | 'ready' | 'source-absent';
	readonly detail: AgentIntegrationInspectionDetail;
	readonly item: AgentIntegrationSnapshotItem;
	readonly plan: readonly AgentIntegrationPlanStep[];
	readonly timedOut: boolean;
}

/** The latest diagnosis: waiting for its explain job, explained, or not answered. */
export type AgentIntegrationDiagnosis =
	| { readonly integrationId: AgentIntegrationId; readonly kind: 'pending' }
	| {
			readonly explanation: AgentIntegrationExplanation;
			readonly integrationId: AgentIntegrationId;
			readonly kind: 'explained';
	  }
	| {
			readonly code: AgentIntegrationManagementFailureCode;
			readonly integrationId: AgentIntegrationId;
			readonly kind: 'failed';
	  };

export interface AgentIntegrationManagementClientSnapshot extends AgentIntegrationManagementSnapshot {
	readonly diagnosis: AgentIntegrationDiagnosis | null;
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
	/**
	 * Runs the diagnosis for one Agent; it changes nothing. For an Agent read only through its own
	 * program it starts that program, so a caller asks first.
	 */
	explain(
		integrationId: AgentIntegrationId,
	): Promise<AgentIntegrationManagementClientOperationResult>;
	getSnapshot(): AgentIntegrationManagementClientSnapshot;
	refresh(): Promise<AgentIntegrationManagementClientOperationResult>;
	subscribe(listener: () => void): () => void;
}
