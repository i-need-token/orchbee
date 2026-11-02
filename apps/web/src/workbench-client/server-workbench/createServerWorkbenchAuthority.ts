import type {
	WorkbenchAuthority,
	WorkbenchAuthorityChange,
	WorkbenchAuthorityCreation,
	WorkbenchAuthorityPanelTerminalReceipt,
	WorkbenchAuthoritySplitRatioChange,
	WorkbenchAuthorityTabReceipt,
	WorkbenchProjectCreationReceipt,
} from '@orchbee/workbench';
import type { WorkbenchSnapshot } from '@orchbee/workbench-client-protocol';

import { handOverServerWorkbenchAction } from './handOverServerWorkbenchAction';
import { readServerWorkbenchAction } from './readServerWorkbenchAction';
import { closeServerWorkbench, watchServerWorkbenchClose } from './server-workbench-close';
import {
	SERVER_MINIMUM_SPLIT_PERCENT,
	changeServerSplitRatios,
	panelTerminalCreateAction,
	panelTerminalCreated,
} from './server-workbench-panels';
import { tabCreateAction, tabsChangeAction } from './server-workbench-tabs';
import { serverResourceScopeId } from './serverResourceScopeId';
import { serverWorkbenchResult } from './serverWorkbenchResult';
import type { ServerWorkbenchAction } from './ServerWorkbenchSession';
import { workspaceLaunchAction, type ServerWorkspacePlacement } from './workspace-launch';

interface ServerWorkbenchAuthorityOptions {
	/** Where this Client works when nothing it shows names a directory. */
	readonly defaultDirectory: string;
	readonly defaultWorkspaceLabel: () => string;
	readonly focusedWorkspaceId: () => string | null;
	readonly session: Parameters<typeof closeServerWorkbench>[0];
}

interface WorkspaceLaunchPlan {
	readonly directory: string;
	readonly placement: ServerWorkspacePlacement;
}

function changeAction(
	change: Exclude<WorkbenchAuthorityChange, WorkbenchAuthoritySplitRatioChange>,
	snapshot: WorkbenchSnapshot,
): ServerWorkbenchAction | null {
	switch (change.type) {
		case 'panel-moved':
			return readServerWorkbenchAction({
				direction: change.direction,
				kind: 'panel.move',
				panelId: change.panelId,
				tabId: change.tabId,
				targetPanelId: change.targetPanelId,
				terminalContextId: change.terminalContextId,
			});
		case 'panels-swapped':
			return readServerWorkbenchAction({
				kind: 'panel.swap',
				panelId: change.panelId,
				tabId: change.tabId,
				targetPanelId: change.targetPanelId,
				terminalContextId: change.terminalContextId,
			});
		case 'tab-renamed':
			return readServerWorkbenchAction({
				kind: 'tab.rename',
				label: change.label,
				tabId: change.tabId,
				terminalContextId: change.terminalContextId,
			});
		case 'tabs-changed':
			return tabsChangeAction(change, snapshot);
		case 'project-dissolved':
			return readServerWorkbenchAction({ kind: 'project.dissolve', projectId: change.projectId });
		case 'project-renamed':
			return readServerWorkbenchAction({
				kind: 'project.rename',
				label: change.name,
				projectId: change.projectId,
			});
		case 'project-reordered':
			return readServerWorkbenchAction({
				kind: 'navigation.configure',
				operation: {
					beforeProjectId: change.beforeProjectId,
					kind: 'move-project',
					projectId: change.projectId,
				},
			});
		case 'workspace-project-created':
			return readServerWorkbenchAction({
				kind: 'workspace.create-project',
				label: change.name,
				workspaceId: change.workspaceId,
			});
		case 'workspace-project-set':
			return readServerWorkbenchAction({
				kind: 'workspace.set-project',
				projectId: change.projectId,
				workspaceId: change.workspaceId,
			});
		case 'workspace-reordered': {
			const workspace = snapshot.navigation.workspaces.find(
				(candidate) => candidate.workspaceId === change.workspaceId,
			);
			return workspace === undefined
				? null
				: readServerWorkbenchAction({
						kind: 'navigation.configure',
						operation: {
							beforeWorkspaceId: change.beforeWorkspaceId,
							kind: 'move-workspace',
							projectId: workspace.projectId,
							workspaceId: change.workspaceId,
						},
					});
		}
		// A Server Workspace's directory is not this Client's to change, so only the name travels.
		case 'workspace-updated':
			return readServerWorkbenchAction({
				kind: 'workspace.rename',
				label: change.name,
				workspaceId: change.workspaceId,
			});
	}
}

/** Where New Workspace opens: where the person is working, otherwise where this Client works. */
function defaultWorkspaceDirectory(
	snapshot: WorkbenchSnapshot,
	focusedWorkspaceId: string | null,
	defaultDirectory: string,
): string {
	const workspace = snapshot.navigation.workspaces.find(
		(candidate) => candidate.workspaceId === focusedWorkspaceId,
	);
	if (workspace === undefined) return defaultDirectory;
	return (
		workspace.terminalDirectory ??
		snapshot.navigation.directories.find(
			(directory) => directory.directoryId === workspace.primaryDirectoryId,
		)?.path ??
		defaultDirectory
	);
}

function launchPlan(
	creation: WorkbenchAuthorityCreation,
	snapshot: WorkbenchSnapshot,
	focusedWorkspaceId: string | null,
	defaultDirectory: string,
): WorkspaceLaunchPlan | null {
	switch (creation.type) {
		case 'project-created':
			return {
				directory: creation.path,
				placement: { kind: 'new-project', label: creation.projectName },
			};
		case 'workspace-created': {
			const directory = snapshot.navigation.directories.find(
				(candidate) => candidate.directoryId === creation.directoryId,
			);
			if (directory === undefined) return null;
			return {
				directory: directory.path,
				placement:
					creation.projectId === null
						? { kind: 'ungrouped' }
						: { kind: 'project', projectId: creation.projectId },
			};
		}
		case 'workspace-created-default':
			return {
				directory: defaultWorkspaceDirectory(snapshot, focusedWorkspaceId, defaultDirectory),
				placement: { kind: 'ungrouped' },
			};
	}
}

/**
 * This Client's Workbench vocabulary, said to the Server. Every command is decided from the snapshot
 * it will name, one at a time, and a result settles once a publication carries what it reports. A
 * refused or unknown command is never repeated, so the person decides from what they can see. Only
 * a ratio goes again, when a commit elsewhere moved the revision and left the split as it was.
 */
export function createServerWorkbenchAuthority({
	defaultDirectory,
	defaultWorkspaceLabel,
	focusedWorkspaceId,
	session,
}: ServerWorkbenchAuthorityOptions): WorkbenchAuthority {
	return {
		minimumSplitPercent: SERVER_MINIMUM_SPLIT_PERCENT,
		change: async (context, change) => {
			switch (change.type) {
				case 'panel-size-adjusted':
				case 'panels-equalized':
				case 'split-ratio-set':
					return changeServerSplitRatios(session, context, change);
				default:
					return serverWorkbenchResult<void>(
						context,
						await session.submit((snapshot) =>
							handOverServerWorkbenchAction(context, changeAction(change, snapshot)),
						),
						() => ({ kind: 'completed' }),
					);
			}
		},
		close: (context, closure) => closeServerWorkbench(session, context, closure),
		create: async (context, creation) => {
			let directory = '';
			const submission = await session.submit((snapshot) => {
				const plan = launchPlan(creation, snapshot, focusedWorkspaceId(), defaultDirectory);
				if (plan === null) return null;
				directory = plan.directory;
				return handOverServerWorkbenchAction(
					context,
					workspaceLaunchAction({
						directory: plan.directory,
						fallbackLabel: defaultWorkspaceLabel(),
						placement: plan.placement,
						snapshot,
					}),
				);
			});
			return serverWorkbenchResult<WorkbenchProjectCreationReceipt>(
				context,
				submission,
				(outcome) =>
					outcome.kind === 'workspace-launch-pending' || outcome.kind === 'workspace-launched'
						? {
								kind: 'completed',
								value: {
									directoryId: outcome.directoryId,
									path: directory,
									presentationId: outcome.presentationId,
									projectId: outcome.projectId,
									resourceScopeId: serverResourceScopeId(outcome.workspaceId),
									terminalContextId: outcome.terminalContextId,
									workspaceId: outcome.workspaceId,
								},
							}
						: { kind: 'completed' },
			);
		},
		createPanelTerminal: async (context, creation) =>
			serverWorkbenchResult<WorkbenchAuthorityPanelTerminalReceipt>(
				context,
				await session.submit((snapshot) =>
					handOverServerWorkbenchAction(context, panelTerminalCreateAction(creation, snapshot)),
				),
				panelTerminalCreated,
			),
		createTab: async (context, creation) =>
			serverWorkbenchResult<WorkbenchAuthorityTabReceipt>(
				context,
				await session.submit((snapshot) =>
					handOverServerWorkbenchAction(context, tabCreateAction(creation, snapshot)),
				),
				(outcome) =>
					outcome.kind === 'tab-create-pending' || outcome.kind === 'tab-created'
						? {
								kind: 'completed',
								value: {
									tabId: outcome.tabId,
									terminalContextId: outcome.terminalContextId,
									terminalStarted:
										outcome.kind === 'tab-created' && outcome.terminalOutcome.kind === 'started',
								},
							}
						: { kind: 'completed' },
			),
		watchClose: (closure, onChanged) => watchServerWorkbenchClose(session, closure, onChanged),
	};
}
