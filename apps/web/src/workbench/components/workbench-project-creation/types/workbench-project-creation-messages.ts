import type { WorkbenchProjectFolderAcquisitionFailureCode } from '../../../types/workbench-project-folder-acquisition';

/** What the Project creation dialogs say, prepared in the window's language. */
export interface WorkbenchProjectCreationMessages {
	readonly cancel: string;
	readonly close: string;
	readonly create: string;
	readonly folderAcquisitionFailureDismiss: string;
	readonly folderAcquisitionFailureMessages: Readonly<
		Record<WorkbenchProjectFolderAcquisitionFailureCode, string>
	>;
	readonly folderAcquisitionFailureTitle: string;
	readonly newFolderCreating: string;
	readonly newFolderDescription: string;
	readonly newFolderName: string;
	readonly newFolderNameHint: string;
	readonly newFolderOpening: string;
	readonly newFolderParent: string;
	readonly newFolderRetained: string;
	readonly newFolderTitle: string;
	readonly newFolderUnknown: string;
	readonly projectCreateDescription: string;
	readonly projectCreatePath: string;
	readonly projectCreatePathDescription: string;
	readonly projectCreatePathPlaceholder: string;
	readonly projectCreateTitle: string;
	readonly selectFolder: string;
}
