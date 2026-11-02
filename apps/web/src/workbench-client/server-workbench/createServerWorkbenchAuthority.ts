import type { ActionResult } from '@orchbee/actions';
import type {
	WorkbenchAuthority,
	WorkbenchAuthorityChange,
	WorkbenchAuthorityContext,
	WorkbenchAuthorityCreation,
	WorkbenchProjectCreationReceipt,
} from '@orchbee/workbench';
import type { WorkbenchSnapshot } from '@orchbee/workbench-client-protocol';

import { handOverServerWorkbenchAction } from './handOverServerWorkbenchAction';
import { readServerWorkbenchAction } from './readServerWorkbenchAction';
import { closeServerWorkbench, watchServerWorkbenchClose } from './server-workbench-close';
import { serverResourceScopeId } from './serverResourceScopeId';
import type {
	ServerWorkbenchAction,
	ServerWorkbenchOutcome,
	ServerWorkbenchSubmission,
} from './ServerWorkbenchSession';
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
	change: WorkbenchAuthorityChange,
	snapshot: WorkbenchSnapshot,
): ServerWorkbenchAction | null {
	switch (change.type) {
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

function resultOf<Output>(
	context: WorkbenchAuthorityContext,
	submission: ServerWorkbenchSubmission,
	committed: (outcome: ServerWorkbenchOutcome) => ActionResult<Output>,
): ActionResult<Output> {
	switch (submission.kind) {
		case 'not-submitted':
			return context.signal.aborted
				? { kind: 'cancelled' }
				: { code: 'not-submitted', kind: 'blocked' };
		case 'outcome-unknown':
			return { failure: { code: 'outcome-unknown' }, kind: 'failed' };
		case 'settled':
			break;
	}
	const { outcome } = submission;
	switch (outcome.kind) {
		case 'rejected':
			return { code: outcome.reason, kind: 'blocked' };
		case 'denied':
		case 'launch-profile-ambiguous':
		case 'launch-profile-unavailable':
			return { code: outcome.kind, kind: 'blocked' };
		case 'unavailable':
			return { failure: { code: outcome.reason }, kind: 'failed' };
		case 'unchanged':
			return { kind: 'unchanged' };
		default:
			return committed(outcome);
	}
}

/**
 * This Client's Workbench vocabulary, said to the Server. Every command is decided from the snapshot
 * it will name, one at a time, and a result settles once a publication carries what it reports. A
 * refused or unknown command is never repeated: the person decides from what they can see.
 */
export function createServerWorkbenchAuthority({
	defaultDirectory,
	defaultWorkspaceLabel,
	focusedWorkspaceId,
	session,
}: ServerWorkbenchAuthorityOptions): WorkbenchAuthority {
	return {
		change: async (context, change) =>
			resultOf<void>(
				context,
				await session.submit((snapshot) =>
					handOverServerWorkbenchAction(context, changeAction(change, snapshot)),
				),
				() => ({ kind: 'completed' }),
			),
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
			return resultOf<WorkbenchProjectCreationReceipt>(context, submission, (outcome) =>
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
		watchClose: (closure, onChanged) => watchServerWorkbenchClose(session, closure, onChanged),
	};
}
