import type {
	WorkbenchAgentCatalogSource,
	WorkbenchAuthority,
	WorkbenchAuthorityChange,
	WorkbenchAuthorityCreation,
	WorkbenchAuthorityPanelTerminalReceipt,
	WorkbenchAuthoritySplitRatioChange,
	WorkbenchAuthorityTabReceipt,
	WorkbenchConfiguredCommands,
	WorkbenchFocusedPanel,
	WorkbenchFocusNewPanelSource,
	WorkbenchProjectCreationReceipt,
} from '../../workbench';
import {
	WORKBENCH_PROTOCOL_VERSION,
	type UserLaunchProfileCatalog,
	type WorkbenchSnapshot,
} from '@orchbee/workbench-client-protocol';

import { createWorkbenchRequestId } from './createWorkbenchRequestId';
import { handOverServerWorkbenchAction } from './handOverServerWorkbenchAction';
import { readServerWorkbenchAction } from './readServerWorkbenchAction';
import { createServerWorkbenchClose } from './createServerWorkbenchClose';
import { createServerWorktrees } from './createServerWorktrees';
import { createServerAgentResumeNotices } from './createServerAgentResumeNotices';
import { createServerAgentStartNotices } from './createServerAgentStartNotices';
import { createServerHeldRestores } from './createServerHeldRestores';
import { createServerEarlierLayouts } from './createServerEarlierLayouts';
import { duplicateServerTabs } from './duplicateServerTabs';
import {
	SERVER_MINIMUM_SPLIT_PERCENT,
	changeServerSplitRatios,
	panelTerminalCreateAction,
	panelTerminalCreated,
} from './server-workbench-panels';
import { serverProfileLaunch, tabCreateAction, tabsChangeAction } from './server-workbench-tabs';
import type { ServerLeave } from './ServerLeaving';
import { serverWorkbenchResult } from './serverWorkbenchResult';
import { shownPanelTerminal } from './shownPanelTerminal';
import type { ServerWorkbenchAction } from './ServerWorkbenchSession';
import { workspaceLaunchAction, type ServerWorkspacePlacement } from './workspace-launch';

/** The Server answers within its two-second sampling window; waiting longer only holds New Workspace back. */
const CURRENT_DIRECTORY_MILLISECONDS = 5000;

/** The Session's saved launch profiles as Settings keeps them current. */
export interface ServerLaunchProfileSource {
	getSnapshot(): { readonly catalog: UserLaunchProfileCatalog };
	subscribe(listener: () => void): () => void;
}

interface ServerWorkbenchAuthorityOptions {
	/** The Session's Agent catalog, which Start Agent reads over this window's connection. */
	readonly agentCatalog?: WorkbenchAgentCatalogSource | undefined;
	/** Absent where this window publishes no view for a command to start from. */
	readonly configuredCommands?: WorkbenchConfiguredCommands | undefined;
	/** Where this Client works when nothing it shows names a directory. */
	readonly defaultDirectory: string;
	readonly defaultWorkspaceLabel: () => string;
	readonly focusedWorkspaceId: () => string | null;
	/** Absent where every new Panel takes the focus, as the default configuration sets it. */
	readonly focusNewPanel?: WorkbenchFocusNewPanelSource | undefined;
	/** Without them, every Tab starts with the default launch profile. */
	readonly launchProfiles?: ServerLaunchProfileSource;
	/** Takes out of view what a followed close removes; absent where no close leaves early. */
	readonly leave?: ServerLeave | undefined;
	readonly session: Parameters<typeof createServerWorkbenchClose>[0];
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
		case 'panel-renamed':
			return readServerWorkbenchAction({
				kind: 'panel.rename',
				panelId: change.panelId,
				label: change.label,
				tabId: change.tabId,
				terminalContextId: change.terminalContextId,
			});
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
		case 'workspace-moved':
			return readServerWorkbenchAction({
				kind: 'navigation.configure',
				operation: {
					beforeWorkspaceId: change.beforeWorkspaceId,
					kind: 'move-workspace',
					projectId: change.projectId,
					workspaceId: change.workspaceId,
				},
			});
		case 'workspace-updated':
			return readServerWorkbenchAction({
				kind: 'workspace.rename',
				label: change.name,
				workspaceId: change.workspaceId,
			});
	}
}

/**
 * Where New Workspace opens: where the person is working, otherwise where this Client works. The
 * Server reads the directory the focused Panel's terminal is in now when the creation names that
 * Panel, otherwise the focused Workspace's first terminal; without an answer, the last directory it
 * followed or the Workspace's own stands in.
 */
async function defaultWorkspaceDirectory(
	session: ServerWorkbenchAuthorityOptions['session'],
	focusedWorkspaceId: string | null,
	focusedPanel: WorkbenchFocusedPanel | null,
	defaultDirectory: string,
): Promise<string> {
	const { snapshot } = session;
	const workspace = snapshot.navigation.workspaces.find(
		(candidate) => candidate.workspaceId === focusedWorkspaceId,
	);
	if (workspace === undefined) return defaultDirectory;
	const panel =
		focusedPanel === null || focusedPanel.workspaceId !== workspace.workspaceId
			? null
			: shownPanelTerminal(snapshot, workspace.workspaceId, focusedPanel);
	const requestId = createWorkbenchRequestId();
	const answer = await session.ask(
		{
			kind: 'workbench.current-directory',
			protocolVersion: WORKBENCH_PROTOCOL_VERSION,
			requestId,
			sessionId: snapshot.sessionId,
			workspaceId: workspace.workspaceId,
			...(panel === null ? {} : { panel }),
		},
		CURRENT_DIRECTORY_MILLISECONDS,
	);
	if (
		answer.kind === 'answered' &&
		answer.response.kind === 'workbench.current-directory.result' &&
		answer.response.requestId === requestId &&
		answer.response.outcome.kind === 'observed'
	) {
		return answer.response.outcome.directory;
	}
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
	workingDirectory: string,
): WorkspaceLaunchPlan | null {
	switch (creation.type) {
		case 'project-created':
			return {
				directory: creation.path,
				placement: { kind: 'new-project', label: creation.projectName },
			};
		case 'workspace-created-at-path':
			return {
				directory: creation.path,
				placement: { kind: 'project', projectId: creation.projectId },
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
				directory: workingDirectory,
				placement:
					creation.projectId === null
						? { kind: 'ungrouped' }
						: { kind: 'project', projectId: creation.projectId },
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
	agentCatalog,
	configuredCommands,
	defaultDirectory,
	defaultWorkspaceLabel,
	focusedWorkspaceId,
	focusNewPanel,
	launchProfiles,
	leave,
	session,
}: ServerWorkbenchAuthorityOptions): WorkbenchAuthority {
	return {
		...createServerWorkbenchClose(session, leave),
		...(agentCatalog === undefined ? {} : { agentCatalog }),
		...(configuredCommands === undefined ? {} : { configuredCommands }),
		...(focusNewPanel === undefined ? {} : { focusNewPanel }),
		...(launchProfiles === undefined
			? {}
			: {
					launchProfiles: {
						getSnapshot: () => launchProfiles.getSnapshot().catalog,
						subscribe: (listener) => launchProfiles.subscribe(listener),
					},
				}),
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
		create: async (context, creation) => {
			// A read names no revision, so it is asked before the command's turn instead of in it.
			const workingDirectory =
				creation.type === 'workspace-created-default'
					? await defaultWorkspaceDirectory(
							session,
							focusedWorkspaceId(),
							creation.focusedPanel,
							defaultDirectory,
						)
					: defaultDirectory;
			let directory = '';
			const submission = await session.submit((snapshot) => {
				const plan = launchPlan(creation, snapshot, workingDirectory);
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
		createTab: async (context, creation) => {
			const profileLaunch =
				creation.profile === undefined
					? { launch: undefined }
					: serverProfileLaunch(creation.profile, launchProfiles?.getSnapshot().catalog);
			if (profileLaunch === null) return { code: 'launch-profile-unavailable', kind: 'blocked' };
			return serverWorkbenchResult<WorkbenchAuthorityTabReceipt>(
				context,
				await session.submit((snapshot) =>
					handOverServerWorkbenchAction(
						context,
						tabCreateAction(creation, snapshot, profileLaunch.launch),
					),
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
			);
		},
		duplicateTabs: (context, duplication) => duplicateServerTabs(session, context, duplication),
		agentResumeNotices: createServerAgentResumeNotices(session),
		agentStartNotices: createServerAgentStartNotices(session),
		earlierLayouts: createServerEarlierLayouts(session),
		heldRestores: createServerHeldRestores(session),
		worktrees: createServerWorktrees(session),
	};
}
