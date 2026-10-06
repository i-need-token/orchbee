import {
	PaneGraphicsRequestSchema,
	PaneGraphicsResultSchema,
} from './workbench-pane-graphics-contract';
import { GRAPHICS_REQUEST_BYTES, GRAPHICS_RESULT_BYTES } from './workbench-graphics-image-contract';
import {
	ConfiguredCommandRequestSchema,
	ConfiguredCommandResultSchema,
	WORKBENCH_MAXIMUM_CONFIGURED_COMMAND_BYTES,
} from './workbench-configured-command-contract';
import {
	SemanticEventsRequestSchema,
	SemanticEventsResultSchema,
	WORKBENCH_MAXIMUM_EVENTS_BYTES,
} from './workbench-events-contract';
import {
	WorkbenchCommandOutcomeSchema,
	type WorkbenchRejectionReasonSchema,
	type WorkbenchCommandUnavailableReasonSchema,
	WorkbenchSnapshotUnavailableReasonSchema,
	WorkbenchWorkspaceListSchema,
} from './workbench-command-outcome-contract';
export {
	WorkbenchCommandOutcomeSchema,
	WorkbenchRejectionReasonSchema,
	WorkbenchCommandUnavailableReasonSchema,
	WorkbenchSnapshotUnavailableReasonSchema,
} from './workbench-command-outcome-contract';
import {
	TerminalEditorRequestSchema,
	TerminalEditorResultSchema,
} from './workbench-terminal-editor-contract';
import {
	WorkspaceGitRequestSchema,
	WorkspaceGitResultSchema,
} from './workbench-workspace-git-contract';
import { PluginRequestSchema, PluginResultSchema } from './workbench-plugin-contract';
import {
	WindowTitleRequestSchema,
	WindowTitleResultSchema,
} from './workbench-window-title-contract';
import {
	NotificationRequestSchema,
	NotificationResultSchema,
} from './workbench-notification-contract';
import {
	WorkbenchLayoutTemplateSchema,
	WORKBENCH_MAXIMUM_ENCODED_LAYOUT_COMMAND_BYTES,
} from './workbench-layout-template-contract';
import {
	AgentManifestsRequestSchema,
	AgentManifestsResultSchema,
	WORKBENCH_MAXIMUM_ENCODED_AGENT_MANIFESTS_BYTES,
} from './workbench-agent-manifests-contract';
import {
	PanelAgentRequestSchema,
	PanelAgentResultSchema,
	WORKBENCH_MAXIMUM_ENCODED_PANEL_AGENT_RESULT_BYTES,
} from './workbench-panel-agent-contract';
import {
	PanelMetadataRequestSchema,
	PanelMetadataResultSchema,
	WORKBENCH_MAXIMUM_ENCODED_PANEL_METADATA_RESULT_BYTES,
} from './workbench-panel-metadata-contract';
import {
	WorkspaceMetadataRequestSchema,
	WorkspaceMetadataResultSchema,
	WORKBENCH_MAXIMUM_ENCODED_WORKSPACE_METADATA_RESULT_BYTES,
} from './workbench-workspace-metadata-contract';
import { WORKBENCH_MAXIMUM_ENCODED_CLIENT_LAYOUT_BYTES } from './workbench-client-layout-contract';
import {
	WORKBENCH_MAXIMUM_ENCODED_CLIENT_ATTENTION_BYTES,
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
	ConfigurationRequestSchema,
	ConfigurationResultSchema,
	WORKBENCH_MAXIMUM_ENCODED_CONFIGURATION_BYTES,
} from './workbench-configuration-contract';
import {
	NativeSettingsRequestSchema,
	NativeSettingsResultSchema,
	WORKBENCH_MAXIMUM_NATIVE_SETTINGS_REQUEST_BYTES,
} from './workbench-settings-contract';
import {
	WorkbenchTerminalLaunchFieldsSchema,
	normalizeWorkbenchTerminalLaunch,
	WORKBENCH_MAXIMUM_ENCODED_LAUNCH_COMMAND_BYTES,
	WorkbenchTerminalGeometrySchema,
	type WorkbenchTerminalUnavailableReasonSchema,
	type WorkbenchTerminalFailureReasonSchema,
	type WorkbenchTerminalOutcomeSchema,
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
	WorkbenchSplitIdSchema,
	WorkbenchSplitRatioSchema,
	WorkbenchProjectIdSchema,
	WorkbenchRequestIdSchema,
	WorkbenchRevisionSchema,
	WorkbenchSnapshotReadIntentSchema,
	WorkbenchSnapshotSchema,
	WorkbenchTabIdSchema,
	WorkbenchTerminalContextIdSchema,
	WorkbenchWorkspaceIdSchema,
	TerminalPresentationIdSchema,
} from './workbench-semantic-contract';

const WORKBENCH_MAXIMUM_ENCODED_WORKSPACE_ORDER_BYTES = 48 * 1024;
export const WORKBENCH_MAXIMUM_ENCODED_SNAPSHOT_RESULT_BYTES =
	WORKBENCH_MAXIMUM_ENCODED_PROJECTION_BYTES + 1024;
export const WORKBENCH_MAXIMUM_RESULT_PUBLICATIONS = 64;

export const WorkbenchTabMoveDirectionSchema = z.enum(['previous', 'next', 'first', 'last']);
export const WorkbenchPanelSplitDirectionSchema = z.enum(['above', 'below', 'left', 'right']);

export const WorkbenchPanelTerminalTargetSchema = z.discriminatedUnion('kind', [
	z.strictObject({ kind: z.literal('child'), label: WorkbenchLabelSchema }).readonly(),
	z
		.strictObject({ kind: z.literal('unbound'), presentationId: TerminalPresentationIdSchema })
		.readonly(),
]);

export const WorkbenchActionSchema = z.discriminatedUnion('kind', [
	z
		.strictObject({
			kind: z.literal('tab.create-layout'),
			terminalContextId: WorkbenchTerminalContextIdSchema,
			label: WorkbenchLabelSchema,
			automaticLabel: z.boolean(),
			layout: WorkbenchLayoutTemplateSchema,
		})
		.readonly(),
	WorkbenchTerminalLaunchFieldsSchema.safeExtend({
		kind: z.literal('panel.create-terminal'),
		terminalContextId: WorkbenchTerminalContextIdSchema,
		tabId: WorkbenchTabIdSchema,
		panelId: WorkbenchPanelIdSchema,
		target: WorkbenchPanelTerminalTargetSchema,
		geometry: WorkbenchTerminalGeometrySchema,
	})
		.refine(
			(value) => !value.options?.pluginPane && !value.options?.pluginContext,
			'Plugin Pane creation requires tab.create or panel.split',
		)
		.transform(normalizeWorkbenchTerminalLaunch)
		.readonly(),
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
			kind: z.literal('workspace.move'),
			workspaceId: WorkbenchWorkspaceIdSchema,
			insertIndex: z.number().int().min(0).max(1023),
		})
		.readonly(),
	z
		.strictObject({
			kind: z.literal('workspace.move-block'),
			workspaceIds: WorkbenchWorkspaceListSchema,
			beforeWorkspaceId: WorkbenchWorkspaceIdSchema.nullish().transform(
				(value) => value ?? undefined,
			),
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
			kind: z.literal('panel.input.set'),
			terminalContextId: WorkbenchTerminalContextIdSchema,
			tabId: WorkbenchTabIdSchema,
			panelId: WorkbenchPanelIdSchema,
			rightClick: z.enum(['orchbee', 'panel']),
		})
		.readonly(),
	z
		.strictObject({
			kind: z.literal('layout.set-split-ratio'),
			terminalContextId: WorkbenchTerminalContextIdSchema,
			tabId: WorkbenchTabIdSchema,
			split: z
				.strictObject({
					splitId: WorkbenchSplitIdSchema,
					firstSlotId: WorkbenchPanelSlotIdSchema,
					secondSlotId: WorkbenchPanelSlotIdSchema,
				})
				.refine((split) => split.firstSlotId !== split.secondSlotId)
				.readonly(),
			previousRatio: WorkbenchSplitRatioSchema,
			ratio: WorkbenchSplitRatioSchema,
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
	WorkbenchTerminalLaunchFieldsSchema.safeExtend({
		kind: z.literal('tab.create'),
		automaticLabel: z.boolean().optional(),
		terminalContextId: WorkbenchTerminalContextIdSchema,
		label: WorkbenchLabelSchema,
		geometry: WorkbenchTerminalGeometrySchema,
	})
		.transform(({ automaticLabel, ...action }) => ({
			...normalizeWorkbenchTerminalLaunch(action),
			...(automaticLabel ? { automaticLabel } : {}),
		}))
		.readonly(),
	WorkbenchTerminalLaunchFieldsSchema.safeExtend({
		kind: z.literal('panel.split'),
		rightClick: z.enum(['orchbee', 'panel']).optional(),
		terminalContextId: WorkbenchTerminalContextIdSchema,
		tabId: WorkbenchTabIdSchema,
		panelId: WorkbenchPanelIdSchema,
		direction: WorkbenchPanelSplitDirectionSchema,
		label: WorkbenchLabelSchema,
		geometry: WorkbenchTerminalGeometrySchema,
	})
		.transform(({ rightClick, ...action }) => ({
			...normalizeWorkbenchTerminalLaunch(action),
			...(rightClick === 'panel' ? { rightClick } : {}),
		}))
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
			kind: z.literal('tab.move-index'),
			terminalContextId: WorkbenchTerminalContextIdSchema,
			tabId: WorkbenchTabIdSchema,
			insertIndex: z.number().int().min(0).max(256),
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
			kind: z.literal('panel.swap'),
			terminalContextId: WorkbenchTerminalContextIdSchema,
			tabId: WorkbenchTabIdSchema,
			panelId: WorkbenchPanelIdSchema,
			targetPanelId: WorkbenchPanelIdSchema,
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
						terminalContextId: WorkbenchTerminalContextIdSchema.optional(),
						tabId: WorkbenchTabIdSchema,
						targetPanelId: WorkbenchPanelIdSchema.optional(),
						direction: WorkbenchPanelSplitDirectionSchema,
					})
					.readonly(),
				z
					.strictObject({
						kind: z.literal('new-tab'),
						terminalContextId: WorkbenchTerminalContextIdSchema.optional(),
						label: WorkbenchLabelSchema.optional(),
					})
					.readonly(),
				z
					.strictObject({
						kind: z.literal('new-workspace'),
						projectId: WorkbenchProjectIdSchema,
						primaryDirectoryId: WorkbenchDirectoryIdSchema.optional(),
						primaryDirectoryPath: WorkbenchDirectoryPathSchema.optional(),
						label: WorkbenchLabelSchema,
						tabLabel: WorkbenchLabelSchema.optional(),
					})
					.refine(
						(value) =>
							value.primaryDirectoryId === undefined || value.primaryDirectoryPath === undefined,
					)
					.readonly(),
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
		WorkspaceMetadataRequestSchema,
		WorkspaceGitRequestSchema,
		AgentManifestsRequestSchema,
		ConfigurationRequestSchema,
		NativeSettingsRequestSchema,
		TerminalEditorRequestSchema,
		ConfiguredCommandRequestSchema,
		SemanticEventsRequestSchema,
		PaneGraphicsRequestSchema,
		PluginRequestSchema,
		WindowTitleRequestSchema,
		NotificationRequestSchema,
		PanelAgentRequestSchema,
		PanelMetadataRequestSchema,
		WorkbenchWorkspaceGroupCloseRequestSchema,
		WorkbenchClientViewRequestSchema,
	])
	.refine((value) => {
		let maximum = WORKBENCH_MAXIMUM_ENCODED_INTENT_BYTES;
		if (value.kind === 'workbench.configuration')
			maximum = WORKBENCH_MAXIMUM_ENCODED_CONFIGURATION_BYTES;
		if (value.kind === 'workbench.settings')
			maximum = WORKBENCH_MAXIMUM_NATIVE_SETTINGS_REQUEST_BYTES;
		if (value.kind === 'workbench.configured-command')
			maximum = WORKBENCH_MAXIMUM_CONFIGURED_COMMAND_BYTES;
		if (value.kind === 'workbench.events') maximum = WORKBENCH_MAXIMUM_EVENTS_BYTES;
		if (value.kind === 'workbench.pane-graphics') maximum = GRAPHICS_REQUEST_BYTES;
		if (value.kind === 'workbench.command' && value.action.kind === 'tab.create-layout')
			maximum = WORKBENCH_MAXIMUM_ENCODED_LAYOUT_COMMAND_BYTES;
		if (
			value.kind === 'workbench.command' &&
			(value.action.kind === 'panel.create-terminal' ||
				value.action.kind === 'tab.create' ||
				value.action.kind === 'panel.split')
		)
			maximum = WORKBENCH_MAXIMUM_ENCODED_LAUNCH_COMMAND_BYTES;
		if (
			value.kind === 'workbench.client-view' &&
			((value.action.kind === 'submit' && value.action.command.intent.kind === 'agent-attention') ||
				(value.action.kind === 'synchronize' &&
					value.action.completion?.outcome.kind === 'attention-read'))
		)
			maximum = WORKBENCH_MAXIMUM_ENCODED_CLIENT_ATTENTION_BYTES;
		if (value.kind === 'workbench.worktree.operation')
			maximum = WORKBENCH_MAXIMUM_ENCODED_WORKTREE_OPERATION_BYTES;
		if (
			value.kind === 'workbench.client-view' &&
			value.action.kind === 'synchronize' &&
			value.action.completion?.outcome.kind === 'layout-read'
		)
			maximum = WORKBENCH_MAXIMUM_ENCODED_CLIENT_LAYOUT_BYTES;
		if (value.kind === 'workbench.command' && value.action.kind === 'workspace.move-block')
			maximum = WORKBENCH_MAXIMUM_ENCODED_WORKSPACE_ORDER_BYTES;
		return encodedBytes(value) <= maximum;
	}, 'Workbench request exceeds its encoded limit');

const deniedSchema = z.strictObject({ kind: z.literal('denied') }).readonly();

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
		WorkspaceMetadataResultSchema,
		WorkspaceGitResultSchema,
		AgentManifestsResultSchema,
		ConfigurationResultSchema,
		NativeSettingsResultSchema,
		TerminalEditorResultSchema,
		ConfiguredCommandResultSchema,
		SemanticEventsResultSchema,
		PaneGraphicsResultSchema,
		PluginResultSchema,
		WindowTitleResultSchema,
		NotificationResultSchema,
		PanelAgentResultSchema,
		PanelMetadataResultSchema,
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
		} else if (value.kind === 'workbench.configured-command.result') {
			maximum = WORKBENCH_MAXIMUM_CONFIGURED_COMMAND_BYTES;
		} else if (value.kind === 'workbench.pane-graphics.result') {
			maximum = GRAPHICS_RESULT_BYTES;
		} else if (value.kind === 'workbench.events.result') {
			maximum = WORKBENCH_MAXIMUM_EVENTS_BYTES;
		} else if (value.kind === 'workbench.client-view.result') {
			maximum =
				value.outcome.kind === 'operation' && value.outcome.operation.status.kind === 'layout-read'
					? WORKBENCH_MAXIMUM_ENCODED_CLIENT_LAYOUT_BYTES
					: WORKBENCH_MAXIMUM_ENCODED_CLIENT_VIEW_RESULT_BYTES;
		} else if (value.kind === 'workbench.workspace-group-close.result') {
			maximum = WORKBENCH_MAXIMUM_ENCODED_WORKSPACE_GROUP_CLOSE_RESULT_BYTES;
		} else if (value.kind === 'workbench.agent-manifests.result') {
			maximum = WORKBENCH_MAXIMUM_ENCODED_AGENT_MANIFESTS_BYTES;
		} else if (
			value.kind === 'workbench.configuration.result' ||
			value.kind === 'workbench.settings.result'
		) {
			maximum = WORKBENCH_MAXIMUM_ENCODED_CONFIGURATION_BYTES;
		} else if (value.kind === 'workbench.panel-agent.result') {
			maximum = WORKBENCH_MAXIMUM_ENCODED_PANEL_AGENT_RESULT_BYTES;
		} else if (value.kind === 'workbench.panel-metadata.result') {
			maximum = WORKBENCH_MAXIMUM_ENCODED_PANEL_METADATA_RESULT_BYTES;
		} else if (value.kind === 'workbench.workspace-metadata.result') {
			maximum = WORKBENCH_MAXIMUM_ENCODED_WORKSPACE_METADATA_RESULT_BYTES;
		} else if (value.kind === 'workbench.worktree.query.result') {
			maximum = WORKBENCH_MAXIMUM_ENCODED_WORKTREE_QUERY_RESULT_BYTES;
		} else if (value.kind === 'workbench.worktree.operation.result') {
			maximum = WORKBENCH_MAXIMUM_ENCODED_WORKTREE_OPERATION_BYTES;
		} else if (value.kind === 'workbench.workspace-close.result') {
			maximum = WORKBENCH_MAXIMUM_ENCODED_WORKSPACE_CLOSE_READ_RESULT_BYTES;
		}

		if (value.kind === 'workbench.command.result' && value.outcome.kind === 'workspaces-reordered')
			maximum = WORKBENCH_MAXIMUM_ENCODED_WORKSPACE_ORDER_BYTES;
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
export type WorkbenchPanelTerminalTarget = z.infer<typeof WorkbenchPanelTerminalTargetSchema>;
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
