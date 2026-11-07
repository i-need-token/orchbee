import {
	CloseOperationReadSchema,
	CloseOperationResultSchema,
} from './workbench-close-operation-contract';
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
	WorkbenchSnapshotUnavailableReasonSchema,
	WorkbenchWorkspaceListSchema,
} from './workbench-command-outcome-contract';

import {
	TerminalEditorRequestSchema,
	TerminalEditorResultSchema,
} from './workbench-terminal-editor-contract';
import {
	WorkspaceGitRequestSchema,
	WorkspaceGitResultSchema,
} from './workbench-workspace-git-contract';
import {
	CurrentDirectoryRequestSchema,
	CurrentDirectoryResultSchema,
	WORKBENCH_MAXIMUM_CURRENT_DIRECTORY_BYTES,
} from './workbench-current-directory-contract';
import {
	AgentIntegrationRequestSchema,
	AgentIntegrationResultSchema,
	WORKBENCH_MAXIMUM_ENCODED_AGENT_INTEGRATION_REQUEST_BYTES,
	WORKBENCH_MAXIMUM_ENCODED_AGENT_INTEGRATION_RESULT_BYTES,
} from './workbench-agent-integration-contract';
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
	WorkbenchTabDuplicateRequestSchema,
	WorkbenchTabDuplicateResultSchema,
	WORKBENCH_MAXIMUM_ENCODED_TAB_DUPLICATE_RESULT_BYTES,
} from './workbench-tab-duplicate-contract';
import {
	WorkbenchLayoutHistoryRequestSchema,
	WorkbenchLayoutHistoryResultSchema,
} from './workbench-layout-history-contract';
import {
	WorkbenchTabSetCloseRequestSchema,
	WorkbenchTabSetCloseResultSchema,
	WORKBENCH_MAXIMUM_ENCODED_TAB_SET_CLOSE_RESULT_BYTES,
} from './workbench-tab-set-close-contract';
import {
	WorkbenchWorktreeQuerySchema,
	WorkbenchWorktreeQueryResultSchema,
	WORKBENCH_MAXIMUM_ENCODED_WORKTREE_QUERY_RESULT_BYTES,
} from './workbench-worktree-query-contract';
import { z } from 'zod';
import {
	WorkbenchWorktreeListReadSchema,
	WorkbenchWorktreeListResultSchema,
	WORKBENCH_MAXIMUM_ENCODED_WORKTREE_LIST_RESULT_BYTES,
} from './workbench-worktree-list-contract';
import {
	ConfigurationRequestSchema,
	ConfigurationResultSchema,
	WORKBENCH_MAXIMUM_ENCODED_CONFIGURATION_BYTES,
} from './workbench-configuration-contract';
import {
	SettingsRequestSchema,
	SettingsResultSchema,
	WORKBENCH_MAXIMUM_NATIVE_SETTINGS_REQUEST_BYTES,
} from './workbench-settings-contract';
import {
	WorkbenchTerminalLaunchFieldsSchema,
	WorkbenchWorkspaceLaunchFieldsSchema,
	normalizeWorkbenchTerminalLaunch,
	WORKBENCH_MAXIMUM_ENCODED_LAUNCH_COMMAND_BYTES,
	WorkbenchTerminalGeometrySchema,
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
	SessionIdSchema,
	WORKBENCH_MAXIMUM_ENCODED_INTENT_BYTES,
	WORKBENCH_MAXIMUM_TABS_PER_CONTEXT,
	WORKBENCH_MAXIMUM_ENCODED_PROJECTION_BYTES,
	WORKBENCH_PROTOCOL_VERSION,
	WorkbenchDeltaSchema,
	WorkbenchDirectoryIdSchema,
	WorkbenchDirectoryPathSchema,
	WorkbenchLabelSchema,
	WorkbenchTabGroupLabelSchema,
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
	WorkbenchTabGroupIdSchema,
	WorkbenchTabGroupSchema,
	WorkbenchTerminalContextIdSchema,
	WorkbenchWorkspaceIdSchema,
	WorkbenchWorkspacePlacementSchema,
	TerminalPresentationIdSchema,
} from './workbench-semantic-contract';

const WORKBENCH_MAXIMUM_ENCODED_WORKSPACE_ORDER_BYTES = 48 * 1024;

// Match Rust's Unicode whitespace check while preserving exact accepted text.
const WorkspaceLabelSchema = WorkbenchLabelSchema.refine((value) => /\P{White_Space}/u.test(value));
const WORKBENCH_MAXIMUM_ENCODED_SNAPSHOT_RESULT_BYTES =
	WORKBENCH_MAXIMUM_ENCODED_PROJECTION_BYTES + 1024;
const WORKBENCH_MAXIMUM_RESULT_PUBLICATIONS = 64;

const WorkbenchTabMoveDirectionSchema = z.enum(['previous', 'next', 'first', 'last']);
const WorkbenchPanelSplitDirectionSchema = z.enum(['above', 'below', 'left', 'right']);

const WorkbenchTabIdSetSchema = z
	.array(WorkbenchTabIdSchema)
	.min(1)
	.max(WORKBENCH_MAXIMUM_TABS_PER_CONTEXT)
	.refine((ids) => new Set(ids).size === ids.length)
	.readonly();

const WorkbenchPanelTerminalTargetSchema = z.discriminatedUnion('kind', [
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
	z
		.strictObject({
			kind: z.literal('tab.close-if-idle'),
			terminalContextId: WorkbenchTerminalContextIdSchema,
			tabId: WorkbenchTabIdSchema,
		})
		.readonly(),
	z.strictObject({ kind: z.literal('project.create'), label: WorkbenchLabelSchema }).readonly(),
	z
		.strictObject({ kind: z.literal('project.dissolve'), projectId: WorkbenchProjectIdSchema })
		.readonly(),
	z
		.strictObject({
			kind: z.literal('project.dissolve-if-idle'),
			projectId: WorkbenchProjectIdSchema,
		})
		.readonly(),
	z
		.strictObject({
			kind: z.literal('workspace.set-project'),
			workspaceId: WorkbenchWorkspaceIdSchema,
			projectId: WorkbenchProjectIdSchema.nullable(),
		})
		.readonly(),
	z
		.strictObject({
			kind: z.literal('workspace.create-project'),
			workspaceId: WorkbenchWorkspaceIdSchema,
			label: WorkbenchLabelSchema,
		})
		.readonly(),
	WorkbenchWorkspaceLaunchFieldsSchema.safeExtend({
		kind: z.literal('workspace.launch'),
		placement: WorkbenchWorkspacePlacementSchema,
		label: WorkspaceLabelSchema,
		automaticLabel: z.boolean().optional(),
		geometry: WorkbenchTerminalGeometrySchema,
	})
		.transform(({ automaticLabel, ...value }) => {
			const canonical = normalizeWorkbenchTerminalLaunch(value);
			return {
				...canonical,
				...(automaticLabel ? { automaticLabel } : {}),
				options: { ...canonical.options, cwd: value.options.cwd },
			};
		})
		.readonly(),
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
			projectId: WorkbenchProjectIdSchema.nullable(),
			label: WorkspaceLabelSchema,
		})
		.readonly(),
	z
		.strictObject({
			kind: z.literal('workspace.rename'),
			workspaceId: WorkbenchWorkspaceIdSchema,
			label: WorkspaceLabelSchema,
		})
		.readonly(),
	z
		.strictObject({
			kind: z.literal('workspace.move'),
			workspaceId: WorkbenchWorkspaceIdSchema,
			insertIndex: z.number().int().min(0).max(1024),
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
			workspaceId: WorkbenchWorkspaceIdSchema,
		})
		.readonly(),
	z
		.strictObject({
			kind: z.literal('workspace.close-if-idle'),
			workspaceId: WorkbenchWorkspaceIdSchema,
		})
		.readonly(),
	z
		.strictObject({
			kind: z.literal('workspace.primary-directory.accept'),
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
			kind: z.literal('navigation.configure'),
			operation: z.discriminatedUnion('kind', [
				z
					.strictObject({
						kind: z.literal('move-project'),
						projectId: WorkbenchProjectIdSchema,
						beforeProjectId: WorkbenchProjectIdSchema.nullable(),
					})
					.readonly(),
				z
					.strictObject({
						kind: z.literal('move-workspace'),
						workspaceId: WorkbenchWorkspaceIdSchema,
						projectId: WorkbenchProjectIdSchema.nullable(),
						beforeWorkspaceId: WorkbenchWorkspaceIdSchema.nullable(),
					})
					.readonly(),
			]),
		})
		.readonly(),
	z
		.strictObject({
			kind: z.literal('tabs.configure'),
			terminalContextId: WorkbenchTerminalContextIdSchema,
			operation: z.discriminatedUnion('kind', [
				z
					.strictObject({
						kind: z.literal('move-tabs-by-drag'),
						tabIds: WorkbenchTabIdSetSchema,
						insertIndex: z.number().int().min(0).max(WORKBENCH_MAXIMUM_TABS_PER_CONTEXT),
						groupId: WorkbenchTabGroupIdSchema.nullable(),
					})
					.readonly(),
				z
					.strictObject({
						kind: z.literal('create-group-for-tabs'),
						tabIds: WorkbenchTabIdSetSchema,
						label: WorkbenchTabGroupLabelSchema,
						color: WorkbenchTabGroupSchema.unwrap().shape.color,
					})
					.readonly(),
				z
					.strictObject({
						kind: z.literal('set-tabs-group'),
						tabIds: WorkbenchTabIdSetSchema,
						groupId: WorkbenchTabGroupIdSchema.nullable(),
					})
					.readonly(),
				z
					.strictObject({
						kind: z.literal('set-tabs-pinned'),
						tabIds: WorkbenchTabIdSetSchema,
						pinned: z.boolean(),
					})
					.readonly(),
				z
					.strictObject({
						kind: z.literal('move-tab'),
						tabId: WorkbenchTabIdSchema,
						direction: z.enum(['previous', 'next', 'first', 'last']),
						adjacentGroupCollapsed: z.boolean(),
					})
					.readonly(),
				z
					.strictObject({
						kind: z.literal('create-group'),
						tabId: WorkbenchTabIdSchema,
						label: WorkbenchTabGroupLabelSchema,
						color: WorkbenchTabGroupSchema.unwrap().shape.color,
					})
					.readonly(),
				z
					.strictObject({
						kind: z.literal('set-group'),
						tabId: WorkbenchTabIdSchema,
						groupId: WorkbenchTabGroupIdSchema.nullable(),
					})
					.readonly(),
				z
					.strictObject({
						kind: z.literal('update-group'),
						groupId: WorkbenchTabGroupIdSchema,
						label: WorkbenchTabGroupLabelSchema,
						color: WorkbenchTabGroupSchema.unwrap().shape.color,
					})
					.readonly(),
				z
					.strictObject({ kind: z.literal('ungroup'), groupId: WorkbenchTabGroupIdSchema })
					.readonly(),
				z
					.strictObject({
						kind: z.literal('set-pinned'),
						tabId: WorkbenchTabIdSchema,
						pinned: z.boolean(),
					})
					.readonly(),
				z
					.strictObject({
						kind: z.literal('move-group'),
						groupId: WorkbenchTabGroupIdSchema,
						direction: z.enum(['previous', 'next']),
					})
					.readonly(),
			]),
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
		placement: z
			.discriminatedUnion('kind', [
				z.strictObject({ kind: z.literal('after-tab'), tabId: WorkbenchTabIdSchema }),
				z.strictObject({ kind: z.literal('in-group'), groupId: WorkbenchTabGroupIdSchema }),
				z.strictObject({ kind: z.literal('new-group') }),
				z.strictObject({ kind: z.literal('duplicate'), tabId: WorkbenchTabIdSchema }),
			])
			.nullish(),
		automaticLabel: z.boolean().optional(),
		terminalContextId: WorkbenchTerminalContextIdSchema,
		label: WorkbenchLabelSchema,
		geometry: WorkbenchTerminalGeometrySchema,
	})
		.transform(({ automaticLabel, placement, ...action }) => ({
			...normalizeWorkbenchTerminalLaunch(action),
			...(automaticLabel ? { automaticLabel } : {}),
			...(placement ? { placement } : {}),
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
	/** Starts a Presentation a restore held for an unavailable directory: there again, or at home. */
	z
		.strictObject({
			kind: z.literal('panel.restore-terminal'),
			terminalContextId: WorkbenchTerminalContextIdSchema,
			tabId: WorkbenchTabIdSchema,
			panelId: WorkbenchPanelIdSchema,
			presentationId: TerminalPresentationIdSchema,
			directory: z.enum(['saved', 'home']),
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
			kind: z.literal('panel.close-if-idle'),
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
						projectId: WorkbenchProjectIdSchema.nullable(),
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

const WorkbenchCommandSchema = z
	.strictObject({
		kind: z.literal('workbench.command'),
		...correlation,
		sessionId: SessionIdSchema,
		expectedRevision: WorkbenchRevisionSchema,
		action: WorkbenchActionSchema,
	})
	.readonly();

const WorkbenchPublicationReadIntentSchema = z
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
		WorkbenchWorktreeListReadSchema,
		WorkbenchWorktreeQuerySchema,
		WorkspaceMetadataRequestSchema,
		WorkspaceGitRequestSchema,
		CurrentDirectoryRequestSchema,
		AgentManifestsRequestSchema,
		ConfigurationRequestSchema,
		CloseOperationReadSchema,
		SettingsRequestSchema,
		TerminalEditorRequestSchema,
		ConfiguredCommandRequestSchema,
		SemanticEventsRequestSchema,
		PaneGraphicsRequestSchema,
		PluginRequestSchema,
		AgentIntegrationRequestSchema,
		WindowTitleRequestSchema,
		NotificationRequestSchema,
		PanelAgentRequestSchema,
		PanelMetadataRequestSchema,
		WorkbenchWorkspaceGroupCloseRequestSchema,
		WorkbenchTabSetCloseRequestSchema,
		WorkbenchTabDuplicateRequestSchema,
		WorkbenchLayoutHistoryRequestSchema,
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
		if (value.kind === 'workbench.agent-integration')
			maximum = WORKBENCH_MAXIMUM_ENCODED_AGENT_INTEGRATION_REQUEST_BYTES;
		if (value.kind === 'workbench.command' && value.action.kind === 'tab.create-layout')
			maximum = WORKBENCH_MAXIMUM_ENCODED_LAYOUT_COMMAND_BYTES;
		if (
			value.kind === 'workbench.command' &&
			(value.action.kind === 'panel.create-terminal' ||
				value.action.kind === 'workspace.launch' ||
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

const WorkbenchSnapshotOutcomeSchema = z.discriminatedUnion('kind', [
	z.strictObject({ kind: z.literal('available'), snapshot: WorkbenchSnapshotSchema }).readonly(),
	deniedSchema,
	z
		.strictObject({
			kind: z.literal('unavailable'),
			reason: WorkbenchSnapshotUnavailableReasonSchema,
		})
		.readonly(),
]);

const WorkbenchCommandResultSchema = z
	.strictObject({
		kind: z.literal('workbench.command.result'),
		...correlation,
		outcome: WorkbenchCommandOutcomeSchema,
	})
	.readonly();
const WorkbenchSnapshotResultSchema = z
	.strictObject({
		kind: z.literal('workbench.snapshot.result'),
		...correlation,
		outcome: WorkbenchSnapshotOutcomeSchema,
	})
	.readonly();

const WorkbenchPublicationUnavailableReasonSchema = z.enum(['service-unavailable']);

const WorkbenchPublicationDocumentSchema = z.union([WorkbenchSnapshotSchema, WorkbenchDeltaSchema]);

const WorkbenchPublicationOutcomeSchema = z.discriminatedUnion('kind', [
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

const WorkbenchPublicationResultSchema = z
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
		WorkbenchWorktreeListResultSchema,
		WorkbenchWorktreeQueryResultSchema,
		WorkspaceMetadataResultSchema,
		WorkspaceGitResultSchema,
		CurrentDirectoryResultSchema,
		AgentManifestsResultSchema,
		ConfigurationResultSchema,
		CloseOperationResultSchema,
		SettingsResultSchema,
		TerminalEditorResultSchema,
		ConfiguredCommandResultSchema,
		SemanticEventsResultSchema,
		PaneGraphicsResultSchema,
		PluginResultSchema,
		AgentIntegrationResultSchema,
		WindowTitleResultSchema,
		NotificationResultSchema,
		PanelAgentResultSchema,
		PanelMetadataResultSchema,
		WorkbenchWorkspaceGroupCloseResultSchema,
		WorkbenchTabSetCloseResultSchema,
		WorkbenchTabDuplicateResultSchema,
		WorkbenchLayoutHistoryResultSchema,
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
		} else if (value.kind === 'workbench.tab-set-close.result') {
			maximum = WORKBENCH_MAXIMUM_ENCODED_TAB_SET_CLOSE_RESULT_BYTES;
		} else if (value.kind === 'workbench.tab-duplicate.result') {
			maximum = WORKBENCH_MAXIMUM_ENCODED_TAB_DUPLICATE_RESULT_BYTES;
		} else if (value.kind === 'workbench.agent-manifests.result') {
			maximum = WORKBENCH_MAXIMUM_ENCODED_AGENT_MANIFESTS_BYTES;
		} else if (value.kind === 'workbench.settings.result') {
			maximum = WORKBENCH_MAXIMUM_NATIVE_SETTINGS_REQUEST_BYTES;
		} else if (value.kind === 'workbench.configuration.result') {
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
		} else if (value.kind === 'workbench.worktree-list.result') {
			maximum = WORKBENCH_MAXIMUM_ENCODED_WORKTREE_LIST_RESULT_BYTES;
		} else if (value.kind === 'workbench.current-directory.result') {
			maximum = WORKBENCH_MAXIMUM_CURRENT_DIRECTORY_BYTES;
		} else if (value.kind === 'workbench.agent-integration.result') {
			maximum = WORKBENCH_MAXIMUM_ENCODED_AGENT_INTEGRATION_RESULT_BYTES;
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

export type WorkbenchCommand = z.infer<typeof WorkbenchCommandSchema>;
export type WorkbenchRequest = z.infer<typeof WorkbenchRequestSchema>;

export type WorkbenchCommandResult = z.infer<typeof WorkbenchCommandResultSchema>;

export type WorkbenchResponse = z.infer<typeof WorkbenchResponseSchema>;

function encodedBytes(value: unknown): number {
	return new TextEncoder().encode(JSON.stringify(value)).length;
}
