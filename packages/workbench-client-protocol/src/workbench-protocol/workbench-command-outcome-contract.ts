import { z } from 'zod';
import { WorkbenchTerminalOutcomeSchema } from './workbench-terminal-launch-contract';
import {
	TerminalPresentationIdSchema,
	WorkbenchDirectoryIdSchema,
	WorkbenchPanelIdSchema,
	WorkbenchPanelSlotIdSchema,
	WorkbenchProjectIdSchema,
	WorkbenchRequestIdSchema,
	WorkbenchRevisionSchema,
	WorkbenchSplitIdSchema,
	WorkbenchTabIdSchema,
	WorkbenchTabGroupIdSchema,
	WorkbenchTerminalContextIdSchema,
	WorkbenchWorkspaceIdSchema,
	WorkerIdSchema,
} from './workbench-semantic-contract';
export const WorkbenchWorkspaceListSchema = z
	.array(WorkbenchWorkspaceIdSchema)
	.min(1)
	.max(1024)
	.refine((ids) => new Set(ids).size === ids.length, 'Workspace IDs must be unique')
	.readonly();

const workspaceLaunchReceipt = {
	revision: WorkbenchRevisionSchema,
	projectId: WorkbenchProjectIdSchema.nullable(),
	workspaceId: WorkbenchWorkspaceIdSchema,
	directoryId: WorkbenchDirectoryIdSchema,
	terminalContextId: WorkbenchTerminalContextIdSchema,
	tabId: WorkbenchTabIdSchema,
	panelId: WorkbenchPanelIdSchema,
	presentationId: TerminalPresentationIdSchema,
};

const WorkbenchRejectionReasonSchema = z.enum([
	'session-mismatch',
	'revision-mismatch',
	'project-unavailable',
	'workspace-unavailable',
	'workspace-group-close-required',
	'workspace-directory-unavailable',
	'workspace-directory-conflict',
	'terminal-context-unavailable',
	'tab-unavailable',
	'tab-group-unavailable',
	'tab-move-index-out-of-bounds',
	'tab-move-order-conflict',
	'workspace-move-index-out-of-bounds',
	'workspace-move-target-in-block',
	'invalid-navigation-configuration',
	'panel-unavailable',
	'split-unavailable',
	'same-tab',
	'resource-limit',
	'identity-exhausted',
	'revision-exhausted',
	'request-identity-conflict',
	'confirmation-required',
	'launch-request-too-large',
]);

const WorkbenchCommandUnavailableReasonSchema = z.enum([
	'request-ledger-full',
	'no-compatible-worker',
	'terminal-busy',
	'terminal-runtime-unavailable',
	'server-busy',
	'service-unavailable',
	'directory-unavailable',
]);
export const WorkbenchSnapshotUnavailableReasonSchema = z.enum(['service-unavailable']);
const deniedSchema = z.strictObject({ kind: z.literal('denied') }).readonly();
const commandUnavailableSchema = z
	.strictObject({ kind: z.literal('unavailable'), reason: WorkbenchCommandUnavailableReasonSchema })
	.readonly();

const WorkbenchPanelTransferSchema = z
	.strictObject({
		targetTerminalContextId: WorkbenchTerminalContextIdSchema,
		closedSourceTab: z.boolean(),
		closedSourceWorkspaceId: WorkbenchWorkspaceIdSchema.optional(),
		createdWorkspaceId: WorkbenchWorkspaceIdSchema.optional(),
		createdDirectoryId: WorkbenchDirectoryIdSchema.optional(),
	})
	.readonly();

export const WorkbenchCommandOutcomeSchema = z.discriminatedUnion('kind', [
	z
		.strictObject({ kind: z.literal('navigation-configured'), revision: WorkbenchRevisionSchema })
		.readonly(),
	z
		.strictObject({
			kind: z.literal('tabs-configured'),
			revision: WorkbenchRevisionSchema,
			groupId: WorkbenchTabGroupIdSchema.nullable(),
		})
		.readonly(),
	z
		.strictObject({
			kind: z.literal('tab-layout-created'),
			revision: WorkbenchRevisionSchema,
			terminalContextId: WorkbenchTerminalContextIdSchema,
			tabId: WorkbenchTabIdSchema,
		})
		.readonly(),
	z.strictObject({ kind: z.literal('launch-profile-unavailable') }).readonly(),
	z
		.strictObject({
			kind: z.literal('launch-profile-ambiguous'),
			candidateIds: z
				.array(z.string().regex(/^[A-Za-z0-9][A-Za-z0-9_.:-]{0,127}$/u))
				.max(64)
				.readonly(),
		})
		.readonly(),
	z
		.strictObject({
			kind: z.literal('panel-terminal-pending'),
			revision: WorkbenchRevisionSchema,
			terminalContextId: WorkbenchTerminalContextIdSchema,
			tabId: WorkbenchTabIdSchema,
			panelId: WorkbenchPanelIdSchema,
			presentationId: TerminalPresentationIdSchema,
		})
		.readonly(),
	z
		.strictObject({
			kind: z.literal('panel-terminal-created'),
			revision: WorkbenchRevisionSchema,
			terminalContextId: WorkbenchTerminalContextIdSchema,
			tabId: WorkbenchTabIdSchema,
			panelId: WorkbenchPanelIdSchema,
			presentationId: TerminalPresentationIdSchema,
			terminalOutcome: WorkbenchTerminalOutcomeSchema,
		})
		.readonly(),
	z
		.strictObject({
			kind: z.literal('tab-close-accepted'),
			closeRequestId: WorkbenchRequestIdSchema,
			terminalContextId: WorkbenchTerminalContextIdSchema,
			tabId: WorkbenchTabIdSchema,
			acceptedRevision: WorkbenchRevisionSchema,
		})
		.readonly(),
	z
		.strictObject({
			kind: z.literal('project-created'),
			revision: WorkbenchRevisionSchema,
			projectId: WorkbenchProjectIdSchema,
		})
		.readonly(),
	z
		.strictObject({
			kind: z.literal('project-dissolved'),
			revision: WorkbenchRevisionSchema,
			projectId: WorkbenchProjectIdSchema,
		})
		.readonly(),
	z
		.strictObject({
			kind: z.literal('workspace-project-changed'),
			revision: WorkbenchRevisionSchema,
			workspaceId: WorkbenchWorkspaceIdSchema,
			projectId: WorkbenchProjectIdSchema.nullable(),
		})
		.readonly(),
	z
		.strictObject({ kind: z.literal('workspace-launch-pending'), ...workspaceLaunchReceipt })
		.readonly(),
	z
		.strictObject({
			kind: z.literal('workspace-launched'),
			...workspaceLaunchReceipt,
			terminalOutcome: WorkbenchTerminalOutcomeSchema,
		})
		.readonly(),
	z
		.strictObject({
			kind: z.literal('workspace-created'),
			revision: WorkbenchRevisionSchema,
			workspaceId: WorkbenchWorkspaceIdSchema,
			terminalContextId: WorkbenchTerminalContextIdSchema,
		})
		.readonly(),
	z
		.strictObject({
			kind: z.literal('workspace-primary-directory-accepted'),
			revision: WorkbenchRevisionSchema,
			directoryId: WorkbenchDirectoryIdSchema,
			workerId: WorkerIdSchema,
		})
		.readonly(),
	z.strictObject({ kind: z.literal('renamed'), revision: WorkbenchRevisionSchema }).readonly(),
	z
		.strictObject({ kind: z.literal('panel-input-configured'), revision: WorkbenchRevisionSchema })
		.readonly(),
	z
		.strictObject({ kind: z.literal('split-ratio-configured'), revision: WorkbenchRevisionSchema })
		.readonly(),
	z.strictObject({ kind: z.literal('unchanged'), revision: WorkbenchRevisionSchema }).readonly(),
	z
		.strictObject({
			kind: z.literal('workspaces-reordered'),
			revision: WorkbenchRevisionSchema,
			workspaceOrder: WorkbenchWorkspaceListSchema,
		})
		.readonly(),
	z
		.strictObject({
			kind: z.literal('tab-moved'),
			revision: WorkbenchRevisionSchema,
			terminalContextId: WorkbenchTerminalContextIdSchema,
			tabId: WorkbenchTabIdSchema,
		})
		.readonly(),
	z
		.strictObject({
			kind: z.literal('tab-create-pending'),
			revision: WorkbenchRevisionSchema,
			terminalContextId: WorkbenchTerminalContextIdSchema,
			tabId: WorkbenchTabIdSchema,
			panelId: WorkbenchPanelIdSchema,
			presentationId: TerminalPresentationIdSchema,
		})
		.readonly(),
	z
		.strictObject({
			kind: z.literal('tab-created'),
			revision: WorkbenchRevisionSchema,
			terminalContextId: WorkbenchTerminalContextIdSchema,
			tabId: WorkbenchTabIdSchema,
			panelId: WorkbenchPanelIdSchema,
			presentationId: TerminalPresentationIdSchema,
			terminalOutcome: WorkbenchTerminalOutcomeSchema,
		})
		.readonly(),
	z
		.strictObject({
			kind: z.literal('panel-split-pending'),
			revision: WorkbenchRevisionSchema,
			terminalContextId: WorkbenchTerminalContextIdSchema,
			tabId: WorkbenchTabIdSchema,
			panelId: WorkbenchPanelIdSchema,
			newPanelId: WorkbenchPanelIdSchema,
			splitId: WorkbenchSplitIdSchema,
			firstSlotId: WorkbenchPanelSlotIdSchema,
			secondSlotId: WorkbenchPanelSlotIdSchema,
			presentationId: TerminalPresentationIdSchema,
		})
		.readonly(),
	z
		.strictObject({
			kind: z.literal('panel-split-created'),
			revision: WorkbenchRevisionSchema,
			terminalContextId: WorkbenchTerminalContextIdSchema,
			tabId: WorkbenchTabIdSchema,
			panelId: WorkbenchPanelIdSchema,
			newPanelId: WorkbenchPanelIdSchema,
			splitId: WorkbenchSplitIdSchema,
			firstSlotId: WorkbenchPanelSlotIdSchema,
			secondSlotId: WorkbenchPanelSlotIdSchema,
			presentationId: TerminalPresentationIdSchema,
			terminalOutcome: WorkbenchTerminalOutcomeSchema,
		})
		.readonly(),
	z
		.strictObject({
			kind: z.literal('panel-close-accepted'),
			closeRequestId: WorkbenchRequestIdSchema,
			terminalContextId: WorkbenchTerminalContextIdSchema,
			tabId: WorkbenchTabIdSchema,
			panelId: WorkbenchPanelIdSchema,
			acceptedRevision: WorkbenchRevisionSchema,
		})
		.readonly(),
	z
		.strictObject({
			kind: z.literal('workspace-close-accepted'),
			closeRequestId: WorkbenchRequestIdSchema,
			workspaceId: WorkbenchWorkspaceIdSchema,
			terminalContextId: WorkbenchTerminalContextIdSchema,
			acceptedRevision: WorkbenchRevisionSchema,
		})
		.readonly(),
	z
		.strictObject({
			kind: z.literal('panel-moved'),
			revision: WorkbenchRevisionSchema,
			terminalContextId: WorkbenchTerminalContextIdSchema,
			tabId: WorkbenchTabIdSchema,
			panelId: WorkbenchPanelIdSchema,
			targetPanelId: WorkbenchPanelIdSchema,
		})
		.readonly(),
	z
		.strictObject({
			kind: z.literal('panel-swapped'),
			revision: WorkbenchRevisionSchema,
			terminalContextId: WorkbenchTerminalContextIdSchema,
			tabId: WorkbenchTabIdSchema,
			panelId: WorkbenchPanelIdSchema,
			targetPanelId: WorkbenchPanelIdSchema,
		})
		.readonly(),
	z
		.strictObject({
			kind: z.literal('panel-relocated'),
			transfer: WorkbenchPanelTransferSchema.optional(),
			revision: WorkbenchRevisionSchema,
			terminalContextId: WorkbenchTerminalContextIdSchema,
			tabId: WorkbenchTabIdSchema,
			panelId: WorkbenchPanelIdSchema,
			targetTabId: WorkbenchTabIdSchema,
			splitId: WorkbenchSplitIdSchema,
			firstSlotId: WorkbenchPanelSlotIdSchema,
			secondSlotId: WorkbenchPanelSlotIdSchema,
		})
		.readonly(),
	z
		.strictObject({
			kind: z.literal('panel-relocated-to-new-tab'),
			transfer: WorkbenchPanelTransferSchema.optional(),
			revision: WorkbenchRevisionSchema,
			terminalContextId: WorkbenchTerminalContextIdSchema,
			tabId: WorkbenchTabIdSchema,
			panelId: WorkbenchPanelIdSchema,
			newTabId: WorkbenchTabIdSchema,
		})
		.readonly(),
	z
		.strictObject({
			kind: z.literal('rejected'),
			revision: WorkbenchRevisionSchema,
			reason: WorkbenchRejectionReasonSchema,
		})
		.readonly(),
	deniedSchema,
	commandUnavailableSchema,
]);
