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
	readonly projectCloseAll: string;
	readonly workspaceNew: string;
	readonly add: string;
	readonly cancel: string;
	readonly close: string;
	readonly collapseProject: string;
	readonly collapseRecents: string;
	readonly connectProject: string;
	readonly connectViaSsh: string;
	readonly disconnectProject: string;
	readonly create: string;
	readonly projectCreate: string;
	readonly projectCreateDescription: string;
	readonly projectCreatePath: string;
	readonly projectCreatePathDescription: string;
	readonly projectCreatePathPlaceholder: string;
	readonly projectCreateTitle: string;
	readonly projectConnectionStatus: Readonly<
		Record<
			| 'blocked'
			| 'connected'
			| 'connecting'
			| 'disconnected'
			| 'interaction-required'
			| 'retrying'
			| 'unavailable',
			string
		>
	>;
	readonly createWorkspace: string;
	readonly deleteWorkspace: string;
	readonly duplicateWorkspace: string;
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
	readonly projectFolders: string;
	readonly projectManage: string;
	readonly projectManageAddFolder: string;
	readonly projectManageAddLocalFolder: string;
	readonly projectManageAddSshFolder: string;
	readonly projectManageDescription: string;
	readonly projectManageFolderAlreadyAdded: string;
	readonly projectManageFolders: string;
	readonly projectManageName: string;
	readonly projectManageNamePlaceholder: string;
	readonly projectManagePrimaryFolder: string;
	readonly projectManageSshFolderBadge: string;
	readonly projectManageSshFolderDescription: string;
	readonly projectManageTitle: string;
	readonly projectOptions: string;
	readonly projectWorktrees: string;
	readonly recents: string;
	readonly removeProject: string;
	readonly retryProjectConnection: string;
	readonly removeProjectFolder: string;
	readonly save: string;
	readonly selectFolder: string;
	readonly setProjectPrimaryFolder: string;
	readonly workspaceManage: string;
	readonly workspaceManageDescription: string;
	readonly workspaceManageNameOnlyDescription: string;
	readonly workspaceManageName: string;
	readonly workspaceManageNamePlaceholder: string;
	readonly workspaceManagePrimaryFolder: string;
	readonly workspaceManageTitle: string;
	readonly workspaceOptions: string;
}

export interface WorkbenchWorkspaceManagementTarget {
	readonly workspaceId: string;
}
