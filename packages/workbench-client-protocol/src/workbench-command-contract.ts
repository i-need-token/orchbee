import { WORKBENCH_MAXIMUM_ENCODED_CLIENT_LAYOUT_BYTES } from './workbench-client-layout-contract';
import {
	WorkbenchClientViewRequestSchema,
	WorkbenchClientViewResultSchema,
	WORKBENCH_MAXIMUM_ENCODED_CLIENT_VIEW_RESULT_BYTES,
} from './workbench-client-view-contract';
import {
	WorkbenchWorkspaceGroupCloseRequestSchema,
	WorkbenchWorkspaceGroupCloseResultSchema,
	WORKBENCH_MAXIMUM_ENCODED_WORKSPACE_GROUP_CLOSE_RESULT_BYTES,
} from './workbench-workspace-group-close-contract';
import {
	WorkbenchWorktreeQuerySchema,
	WorkbenchWorktreeQueryResultSchema,
	WORKBENCH_MAXIMUM_ENCODED_WORKTREE_QUERY_RESULT_BYTES,
} from './workbench-worktree-query-contract';
import { z } from 'zod';
import {
	WorkbenchTerminalGeometrySchema,
	type WorkbenchTerminalUnavailableReasonSchema,
	type WorkbenchTerminalFailureReasonSchema,
	WorkbenchTerminalOutcomeSchema,
} from './workbench-terminal-launch-contract';
export {
	WorkbenchTerminalGeometrySchema,
	WorkbenchTerminalUnavailableReasonSchema,
	WorkbenchTerminalFailureReasonSchema,
	WorkbenchTerminalOutcomeSchema,
} from './workbench-terminal-launch-contract';
import {
	WorkbenchWorktreeRequestSchema,
	WorkbenchWorktreeResultSchema,
	WORKBENCH_MAXIMUM_ENCODED_WORKTREE_OPERATION_BYTES,
} from './workbench-worktree-contract';

import {
	WorkbenchTabCloseReadSchema,
	WorkbenchTabCloseReadResultSchema,
	WorkbenchTabCloseResumeSchema,
	WorkbenchTabCloseResumeResultSchema,
} from './workbench-tab-close-contract';
import {
	WORKBENCH_MAXIMUM_ENCODED_WORKSPACE_CLOSE_READ_RESULT_BYTES,
	WorkbenchWorkspaceCloseReadResultSchema,
	WorkbenchWorkspaceCloseReadSchema,
	WorkbenchWorkspaceCloseResumeResultSchema,
	WorkbenchWorkspaceCloseResumeSchema,
} from './workbench-workspace-close-contract';

import {
	SiteIdSchema,
	WORKBENCH_MAXIMUM_ENCODED_INTENT_BYTES,
	WORKBENCH_MAXIMUM_ENCODED_PROJECTION_BYTES,
	WORKBENCH_PROTOCOL_VERSION,
	WorkbenchDeltaSchema,
	WorkbenchDirectoryIdSchema,
	WorkbenchDirectoryPathSchema,
	WorkbenchLabelSchema,
	WorkbenchPanelLabelUpdateSchema,
	WorkbenchPanelIdSchema,
	WorkbenchPanelSlotIdSchema,
	WorkbenchProjectIdSchema,
	WorkbenchRequestIdSchema,
	WorkbenchRevisionSchema,
	WorkbenchSnapshotReadIntentSchema,
	WorkbenchSnapshotSchema,
	WorkbenchSplitIdSchema,
	WorkbenchTabIdSchema,
	WorkbenchTerminalContextIdSchema,
	WorkbenchWorkspaceIdSchema,
	TerminalPresentationIdSchema,
	WorkerIdSchema,
} from './workbench-semantic-contract';

export const WORKBENCH_MAXIMUM_ENCODED_SNAPSHOT_RESULT_BYTES =
	WORKBENCH_MAXIMUM_ENCODED_PROJECTION_BYTES + 1024;
export const WORKBENCH_MAXIMUM_RESULT_PUBLICATIONS = 64;

export const WorkbenchTabMoveDirectionSchema = z.enum(['previous', 'next', 'first', 'last']);
export const WorkbenchPanelSplitDirectionSchema = z.enum(['above', 'below', 'left', 'right']);

export const WorkbenchActionSchema = z.discriminatedUnion('kind', [
	z
		.strictObject({
			kind: z.literal('tab.close'),
			terminalContextId: WorkbenchTerminalContextIdSchema,
			tabId: WorkbenchTabIdSchema,
		})
		.readonly(),
	z.strictObject({ kind: z.literal('project.create'), label: WorkbenchLabelSchema }).readonly(),
	z
		.strictObject({
			kind: z.literal('project.rename'),
			projectId: WorkbenchProjectIdSchema,
			label: WorkbenchLabelSchema,
		})
		.readonly(),
	z
		.strictObject({
			kind: z.literal('workspace.create'),
			projectId: WorkbenchProjectIdSchema,
			label: WorkbenchLabelSchema,
		})
		.readonly(),
	z
		.strictObject({
			kind: z.literal('workspace.rename'),
			projectId: WorkbenchProjectIdSchema,
			workspaceId: WorkbenchWorkspaceIdSchema,
			label: WorkbenchLabelSchema,
		})
		.readonly(),
	z
		.strictObject({
			kind: z.literal('workspace.close'),
			projectId: WorkbenchProjectIdSchema,
			workspaceId: WorkbenchWorkspaceIdSchema,
		})
		.readonly(),
	z
		.strictObject({
			kind: z.literal('workspace.primary-directory.accept'),
			projectId: WorkbenchProjectIdSchema,
			workspaceId: WorkbenchWorkspaceIdSchema,
			path: WorkbenchDirectoryPathSchema,
		})
		.readonly(),
	z
		.strictObject({
			kind: z.literal('panel.rename'),
			terminalContextId: WorkbenchTerminalContextIdSchema,
			tabId: WorkbenchTabIdSchema,
			panelId: WorkbenchPanelIdSchema,
			label: WorkbenchPanelLabelUpdateSchema,
		})
		.readonly(),
	z
		.strictObject({
			kind: z.literal('tab.rename'),
			terminalContextId: WorkbenchTerminalContextIdSchema,
			tabId: WorkbenchTabIdSchema,
			label: WorkbenchLabelSchema,
		})
		.readonly(),
	z
		.strictObject({
			kind: z.literal('tab.create'),
			terminalContextId: WorkbenchTerminalContextIdSchema,
			label: WorkbenchLabelSchema,
			geometry: WorkbenchTerminalGeometrySchema,
		})
		.readonly(),
	z
		.strictObject({
			kind: z.literal('panel.split'),
			terminalContextId: WorkbenchTerminalContextIdSchema,
			tabId: WorkbenchTabIdSchema,
			panelId: WorkbenchPanelIdSchema,
			direction: WorkbenchPanelSplitDirectionSchema,
			label: WorkbenchLabelSchema,
			geometry: WorkbenchTerminalGeometrySchema,
		})
		.readonly(),
	z
		.strictObject({
			kind: z.literal('tab.move'),
			terminalContextId: WorkbenchTerminalContextIdSchema,
			tabId: WorkbenchTabIdSchema,
			direction: WorkbenchTabMoveDirectionSchema,
		})
		.readonly(),
	z
		.strictObject({
			kind: z.literal('panel.close'),
			terminalContextId: WorkbenchTerminalContextIdSchema,
			tabId: WorkbenchTabIdSchema,
			panelId: WorkbenchPanelIdSchema,
		})
		.readonly(),
	z
		.strictObject({
			kind: z.literal('panel.move'),
			terminalContextId: WorkbenchTerminalContextIdSchema,
			tabId: WorkbenchTabIdSchema,
			panelId: WorkbenchPanelIdSchema,
			targetPanelId: WorkbenchPanelIdSchema,
			direction: WorkbenchPanelSplitDirectionSchema,
		})
		.readonly(),
	z
		.strictObject({
			kind: z.literal('panel.relocate'),
			terminalContextId: WorkbenchTerminalContextIdSchema,
			tabId: WorkbenchTabIdSchema,
			panelId: WorkbenchPanelIdSchema,
			destination: z.discriminatedUnion('kind', [
				z
					.strictObject({
						kind: z.literal('tab'),
						tabId: WorkbenchTabIdSchema,
						direction: WorkbenchPanelSplitDirectionSchema,
					})
					.readonly(),
				z.strictObject({ kind: z.literal('new-tab'), label: WorkbenchLabelSchema }).readonly(),
			]),
		})
		.readonly(),
]);

const correlation = {
	protocolVersion: z.literal(WORKBENCH_PROTOCOL_VERSION),
	requestId: WorkbenchRequestIdSchema,
};

export const WorkbenchCommandSchema = z
	.strictObject({
		kind: z.literal('workbench.command'),
		...correlation,
		siteId: SiteIdSchema,
		expectedRevision: WorkbenchRevisionSchema,
		action: WorkbenchActionSchema,
	})
	.readonly();

export const WorkbenchPublicationReadIntentSchema = z
	.strictObject({
		kind: z.literal('workbench.publication.read'),
		...correlation,
		knownRevision: WorkbenchRevisionSchema,
	})
	.readonly();

export const WorkbenchRequestSchema = z
	.discriminatedUnion('kind', [
		WorkbenchSnapshotReadIntentSchema,
		WorkbenchCommandSchema,
		WorkbenchTabCloseReadSchema,
		WorkbenchTabCloseResumeSchema,
		WorkbenchPublicationReadIntentSchema,
		WorkbenchWorkspaceCloseReadSchema,
		WorkbenchWorkspaceCloseResumeSchema,
		WorkbenchWorktreeRequestSchema,
		WorkbenchWorktreeQuerySchema,
		WorkbenchWorkspaceGroupCloseRequestSchema,
		WorkbenchClientViewRequestSchema,
	])
	.refine((value) => {
		let maximum = WORKBENCH_MAXIMUM_ENCODED_INTENT_BYTES;
		if (value.kind === 'workbench.worktree.operation')
			maximum = WORKBENCH_MAXIMUM_ENCODED_WORKTREE_OPERATION_BYTES;
		if (
			value.kind === 'workbench.client-view' &&
			value.action.kind === 'synchronize' &&
			value.action.completion?.outcome.kind === 'layout-read'
		)
			maximum = WORKBENCH_MAXIMUM_ENCODED_CLIENT_LAYOUT_BYTES;
		return encodedBytes(value) <= maximum;
	}, 'Workbench request exceeds its encoded limit');

export const WorkbenchRejectionReasonSchema = z.enum([
	'site-mismatch',
	'revision-mismatch',
	'project-unavailable',
	'workspace-unavailable',
	'workspace-group-close-required',
	'workspace-directory-unavailable',
	'workspace-directory-conflict',
	'terminal-context-unavailable',
	'tab-unavailable',
	'panel-unavailable',
	'panel-not-leaf',
	'last-panel-in-tab',
	'same-tab',
	'resource-limit',
	'identity-exhausted',
	'revision-exhausted',
	'request-identity-conflict',
]);

export const WorkbenchCommandUnavailableReasonSchema = z.enum([
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

export const WorkbenchCommandOutcomeSchema = z.discriminatedUnion('kind', [
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
	z.strictObject({ kind: z.literal('unchanged'), revision: WorkbenchRevisionSchema }).readonly(),
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
			projectId: WorkbenchProjectIdSchema,
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
			kind: z.literal('panel-relocated'),
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

export const WorkbenchSnapshotOutcomeSchema = z.discriminatedUnion('kind', [
	z.strictObject({ kind: z.literal('available'), snapshot: WorkbenchSnapshotSchema }).readonly(),
	deniedSchema,
	z
		.strictObject({
			kind: z.literal('unavailable'),
			reason: WorkbenchSnapshotUnavailableReasonSchema,
		})
		.readonly(),
]);

export const WorkbenchCommandResultSchema = z
	.strictObject({
		kind: z.literal('workbench.command.result'),
		...correlation,
		outcome: WorkbenchCommandOutcomeSchema,
	})
	.readonly();
export const WorkbenchSnapshotResultSchema = z
	.strictObject({
		kind: z.literal('workbench.snapshot.result'),
		...correlation,
		outcome: WorkbenchSnapshotOutcomeSchema,
	})
	.readonly();

export const WorkbenchPublicationUnavailableReasonSchema = z.enum(['service-unavailable']);

export const WorkbenchPublicationDocumentSchema = z.union([
	WorkbenchSnapshotSchema,
	WorkbenchDeltaSchema,
]);

export const WorkbenchPublicationOutcomeSchema = z.discriminatedUnion('kind', [
	z.strictObject({ kind: z.literal('current'), revision: WorkbenchRevisionSchema }).readonly(),
	z
		.strictObject({
			kind: z.literal('published'),
			publications: z
				.array(WorkbenchPublicationDocumentSchema)
				.min(1)
				.max(WORKBENCH_MAXIMUM_RESULT_PUBLICATIONS)
				.readonly(),
		})
		.readonly(),
	deniedSchema,
	z
		.strictObject({
			kind: z.literal('unavailable'),
			reason: WorkbenchPublicationUnavailableReasonSchema,
		})
		.readonly(),
]);

export const WorkbenchPublicationResultSchema = z
	.strictObject({
		kind: z.literal('workbench.publication.result'),
		...correlation,
		outcome: WorkbenchPublicationOutcomeSchema,
	})
	.readonly();

export const WorkbenchResponseSchema = z
	.discriminatedUnion('kind', [
		WorkbenchCommandResultSchema,
		WorkbenchSnapshotResultSchema,
		WorkbenchTabCloseReadResultSchema,
		WorkbenchTabCloseResumeResultSchema,
		WorkbenchPublicationResultSchema,
		WorkbenchWorkspaceCloseReadResultSchema,
		WorkbenchWorkspaceCloseResumeResultSchema,
		WorkbenchWorktreeResultSchema,
		WorkbenchWorktreeQueryResultSchema,
		WorkbenchWorkspaceGroupCloseResultSchema,
		WorkbenchClientViewResultSchema,
	])
	.superRefine((value, context) => {
		let maximum = WORKBENCH_MAXIMUM_ENCODED_INTENT_BYTES;
		if (
			value.kind === 'workbench.snapshot.result' ||
			value.kind === 'workbench.publication.result'
		) {
			maximum = WORKBENCH_MAXIMUM_ENCODED_SNAPSHOT_RESULT_BYTES;
		} else if (value.kind === 'workbench.client-view.result') {
			maximum =
				value.outcome.kind === 'operation' && value.outcome.operation.status.kind === 'layout-read'
					? WORKBENCH_MAXIMUM_ENCODED_CLIENT_LAYOUT_BYTES
					: WORKBENCH_MAXIMUM_ENCODED_CLIENT_VIEW_RESULT_BYTES;
		} else if (value.kind === 'workbench.workspace-group-close.result') {
			maximum = WORKBENCH_MAXIMUM_ENCODED_WORKSPACE_GROUP_CLOSE_RESULT_BYTES;
		} else if (value.kind === 'workbench.worktree.query.result') {
			maximum = WORKBENCH_MAXIMUM_ENCODED_WORKTREE_QUERY_RESULT_BYTES;
		} else if (value.kind === 'workbench.worktree.operation.result') {
			maximum = WORKBENCH_MAXIMUM_ENCODED_WORKTREE_OPERATION_BYTES;
		} else if (value.kind === 'workbench.workspace-close.result') {
			maximum = WORKBENCH_MAXIMUM_ENCODED_WORKSPACE_CLOSE_READ_RESULT_BYTES;
		}

		if (encodedBytes(value) > maximum)
			context.addIssue({ code: 'custom', message: 'Workbench result exceeds its encoded limit' });
		if (
			value.kind === 'workbench.snapshot.result' &&
			value.outcome.kind === 'available' &&
			encodedBytes(value.outcome.snapshot) > WORKBENCH_MAXIMUM_ENCODED_PROJECTION_BYTES
		) {
			context.addIssue({
				code: 'custom',
				message: 'Nested Workbench snapshot exceeds its encoded limit',
			});
		}
		if (value.kind === 'workbench.publication.result' && value.outcome.kind === 'published') {
			for (const publication of value.outcome.publications) {
				if (encodedBytes(publication) > WORKBENCH_MAXIMUM_ENCODED_PROJECTION_BYTES) {
					context.addIssue({
						code: 'custom',
						message: 'Nested Workbench publication exceeds its encoded limit',
					});
					break;
				}
			}
		}
	});

export type WorkbenchTerminalGeometry = z.infer<typeof WorkbenchTerminalGeometrySchema>;
export type WorkbenchAction = z.infer<typeof WorkbenchActionSchema>;
export type WorkbenchTabMoveDirection = z.infer<typeof WorkbenchTabMoveDirectionSchema>;
export type WorkbenchPanelSplitDirection = z.infer<typeof WorkbenchPanelSplitDirectionSchema>;
export type WorkbenchCommand = z.infer<typeof WorkbenchCommandSchema>;
export type WorkbenchRequest = z.infer<typeof WorkbenchRequestSchema>;
export type WorkbenchCommandOutcome = z.infer<typeof WorkbenchCommandOutcomeSchema>;
export type WorkbenchCommandResult = z.infer<typeof WorkbenchCommandResultSchema>;
export type WorkbenchSnapshotOutcome = z.infer<typeof WorkbenchSnapshotOutcomeSchema>;
export type WorkbenchSnapshotResult = z.infer<typeof WorkbenchSnapshotResultSchema>;
export type WorkbenchPublicationReadIntent = z.infer<typeof WorkbenchPublicationReadIntentSchema>;
export type WorkbenchPublicationDocument = z.infer<typeof WorkbenchPublicationDocumentSchema>;
export type WorkbenchPublicationOutcome = z.infer<typeof WorkbenchPublicationOutcomeSchema>;
export type WorkbenchPublicationResult = z.infer<typeof WorkbenchPublicationResultSchema>;
export type WorkbenchPublicationUnavailableReason = z.infer<
	typeof WorkbenchPublicationUnavailableReasonSchema
>;
export type WorkbenchResponse = z.infer<typeof WorkbenchResponseSchema>;
export type WorkbenchRejectionReason = z.infer<typeof WorkbenchRejectionReasonSchema>;
export type WorkbenchCommandUnavailableReason = z.infer<
	typeof WorkbenchCommandUnavailableReasonSchema
>;
export type WorkbenchSnapshotUnavailableReason = z.infer<
	typeof WorkbenchSnapshotUnavailableReasonSchema
>;
export type WorkbenchTerminalUnavailableReason = z.infer<
	typeof WorkbenchTerminalUnavailableReasonSchema
>;
export type WorkbenchTerminalFailureReason = z.infer<typeof WorkbenchTerminalFailureReasonSchema>;
export type WorkbenchTerminalOutcome = z.infer<typeof WorkbenchTerminalOutcomeSchema>;

export function parseWorkbenchRequest(input: unknown): WorkbenchRequest {
	return WorkbenchRequestSchema.parse(input);
}

export function parseWorkbenchResponse(input: unknown): WorkbenchResponse {
	return WorkbenchResponseSchema.parse(input);
}

function encodedBytes(value: unknown): number {
	return new TextEncoder().encode(JSON.stringify(value)).length;
}
