import type { ActionExecution } from '@orchbee/actions';

import type {
	WorkbenchWorktreeCheckoutInput,
	WorkbenchWorktreeOpening,
	WorkbenchWorktreeRemovalInput,
	WorkbenchWorktreeResumeInput,
} from '../../../actions/workbench-action-ids';
import type { WorkbenchActionMenuEntry } from '../../../types/workbench-action-menu';
import type { WorkbenchWorktreeDiscoveryResult } from '../../../types/workbench-worktrees';

/** What a Workspace menu reads about Worktrees and the Actions it invokes on them. */
export interface WorkbenchWorktreeCommands {
	readonly create: (workspaceId: string) => ActionExecution<WorkbenchWorktreeOpening>;
	readonly discover: (workspaceId: string) => Promise<WorkbenchWorktreeDiscoveryResult>;
	readonly open: (
		input: WorkbenchWorktreeCheckoutInput,
	) => ActionExecution<WorkbenchWorktreeOpening>;
	readonly remove: (input: WorkbenchWorktreeRemovalInput) => ActionExecution<void>;
	readonly resume: (input: WorkbenchWorktreeResumeInput) => ActionExecution<void>;
}

export interface WorkbenchProjectNavigationMessages {
	readonly agentMenuClose: string;
	/** The Agent row's Allow confirmations submenu and its two answers that name no Agent type. */
	readonly agentMenuConfirmations: string;
	readonly agentMenuConfirmationsOff: string;
	readonly agentMenuConfirmationsRun: string;
	readonly agentMenuRename: string;
	readonly workspaceMoveToProject: string;
	readonly workspaceCreateProject: string;
	readonly workspaceUngroup: string;
	readonly cancel: string;
	readonly close: string;
	/** The × tool's tooltip on a Workspace row. */
	readonly closeWorkspace: string;
	readonly collapseProject: string;
	readonly collapseRecents: string;
	readonly create: string;
	readonly createWorkspace: string;
	readonly createWorkspaceAtDirectory: string;
	/** The drop area that ends a Project drag at the end of the list. */
	readonly dropEndOfProjects: string;
	readonly expandProject: string;
	readonly expandRecents: string;
	readonly nameRequired: string;
	readonly nameTooLong: string;
	readonly renameSubmit: string;
	readonly projectFolders: string;
	readonly projectMenuDissolve: string;
	readonly projectMenuRename: string;
	readonly projectOptions: string;
	readonly projectRenameTitle: string;
	readonly projectWorktrees: string;
	readonly recents: string;
	readonly workspaceMenuClose: string;
	readonly workspaceMenuNewProject: string;
	readonly workspaceMenuRename: string;
	readonly workspaceOptions: string;
	readonly workspaceRenameTitle: string;
}

/**
 * A Workspace a sidebar menu creates in a Project: a default one, one in a directory the person
 * chooses, or one in one of the Project's folders.
 */
export type WorkbenchSidebarWorkspaceCreation =
	| { readonly projectId: string; readonly selectDirectory: true }
	| { readonly projectId: string }
	| { readonly path: string; readonly projectId: string };

/** Change machine in a Workspace's menu. */
export interface WorkspaceMachineMenu {
	/** The menu that offers it opened or closed: its order is taken on opening. */
	readonly opened: (open: boolean) => void;
	readonly submenu: (workspace: {
		readonly id: string;
		readonly name: string;
	}) => WorkbenchActionMenuEntry;
}
