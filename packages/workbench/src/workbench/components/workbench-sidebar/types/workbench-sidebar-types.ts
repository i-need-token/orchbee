import type { WorkbenchProjectFolderAcquisitionFailureCode } from '../../../types/workbench-project-folder-acquisition';
import type { WorkbenchProjectWorktreeFailureCode } from '../../../types/workbench-project-worktree';

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
	readonly newWorktree: string;
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
	readonly projectWorktreeBase: string;
	readonly projectWorktreeBaseNoMatches: string;
	readonly projectWorktreeBranch: string;
	readonly projectWorktreeCreateWorkspace: string;
	readonly projectWorktreeDescription: string;
	readonly projectWorktreeFailureMessages: Readonly<
		Record<WorkbenchProjectWorktreeFailureCode, string>
	>;
	readonly projectWorktreeFailureTitle: string;
	readonly projectWorktreeInventoryUnavailable: string;
	readonly projectWorktreeLoading: string;
	readonly projectWorktreeLocation: string;
	readonly projectWorktreeLocationAutomatic: (root: string) => string;
	readonly projectWorktreeNoRepositories: string;
	readonly projectWorktreeRepository: string;
	readonly projectWorktreeTitle: string;
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
