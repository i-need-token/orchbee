import type { WorkbenchProjectFolderAcquisitionFailureCode } from '../../../types/workbench-project-folder-acquisition';

export interface WorkbenchProjectNavigationMessages {
	readonly add: string;
	readonly cancel: string;
	readonly close: string;
	readonly collapseProject: string;
	readonly create: string;
	readonly projectCreate: string;
	readonly projectCreateDescription: string;
	readonly projectCreatePath: string;
	readonly projectCreatePathDescription: string;
	readonly projectCreatePathPlaceholder: string;
	readonly projectCreateTitle: string;
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
	readonly projectFolders: string;
	readonly projectManage: string;
	readonly projectManageAddFolder: string;
	readonly projectManageDescription: string;
	readonly projectManageFolderAlreadyAdded: string;
	readonly projectManageFolders: string;
	readonly projectManageName: string;
	readonly projectManageNamePlaceholder: string;
	readonly projectManagePrimaryFolder: string;
	readonly projectManageTitle: string;
	readonly projectOptions: string;
	readonly projectWorktrees: string;
	readonly removeProject: string;
	readonly removeProjectFolder: string;
	readonly save: string;
	readonly selectFolder: string;
	readonly setProjectPrimaryFolder: string;
	readonly workspaceManage: string;
	readonly workspaceManageDescription: string;
	readonly workspaceManageName: string;
	readonly workspaceManageNamePlaceholder: string;
	readonly workspaceManagePrimaryFolder: string;
	readonly workspaceManageTitle: string;
	readonly workspaceOptions: string;
}

export interface WorkbenchWorkspaceManagementTarget {
	readonly projectId: string;
	readonly workspaceId: string;
}
