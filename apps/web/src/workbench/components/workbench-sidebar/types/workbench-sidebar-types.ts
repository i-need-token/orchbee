import type { ActionExecution } from '@orchbee/actions';

import type {
	WorkbenchWorktreeCheckoutInput,
	WorkbenchWorktreeOpening,
	WorkbenchWorktreeRemovalInput,
	WorkbenchWorktreeResumeInput,
} from '../../../actions/workbench-action-ids';
import type { WorkbenchProjectFolderAcquisitionFailureCode } from '../../../types/workbench-project-folder-acquisition';
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
	readonly workspaceMoveToProject: string;
	readonly workspaceCreateProject: string;
	readonly workspaceUngroup: string;
	readonly cancel: string;
	readonly close: string;
	readonly collapseProject: string;
	readonly collapseRecents: string;
	readonly create: string;
	readonly projectCreate: string;
	readonly projectCreateDescription: string;
	readonly projectCreatePath: string;
	readonly projectCreatePathDescription: string;
	readonly projectCreatePathPlaceholder: string;
	readonly projectCreateTitle: string;
	readonly createWorkspace: string;
	readonly expandProject: string;
	readonly expandRecents: string;
	readonly folderAcquisitionFailureDismiss: string;
	readonly folderAcquisitionFailureMessages: Readonly<
		Record<WorkbenchProjectFolderAcquisitionFailureCode, string>
	>;
	readonly folderAcquisitionFailureTitle: string;
	readonly newFolder: string;
	readonly newFolderTitle: string;
	readonly newFolderDescription: string;
	readonly newFolderParent: string;
	readonly newFolderName: string;
	readonly newFolderNameHint: string;
	readonly newFolderCreating: string;
	readonly newFolderOpening: string;
	readonly newFolderRetained: string;
	readonly newFolderUnknown: string;
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
	readonly selectFolder: string;
	readonly workspaceMenuClose: string;
	readonly workspaceMenuNewProject: string;
	readonly workspaceMenuRename: string;
	readonly workspaceOptions: string;
	readonly workspaceRenameTitle: string;
}

/**
 * A Workspace a sidebar menu creates: a default one in a Project, or one in a directory a Workspace
 * recorded.
 */
export type WorkbenchSidebarWorkspaceCreation =
	| { readonly projectId: string }
	| {
			readonly directoryId: string;
			readonly projectId: string | null;
	  };
