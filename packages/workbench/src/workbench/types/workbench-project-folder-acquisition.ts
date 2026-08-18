export type WorkbenchProjectFolderAcquisitionFailureCode =
	| 'already-added'
	| 'busy'
	| 'control-unavailable'
	| 'dialog-failed'
	| 'not-directory'
	| 'not-found'
	| 'permission-denied'
	| 'project-unavailable'
	| 'validation-failed';

export type WorkbenchProjectFolderAcquisitionResult =
	| { readonly kind: 'cancelled' }
	| { readonly code: WorkbenchProjectFolderAcquisitionFailureCode; readonly kind: 'failed' }
	| { readonly kind: 'selected'; readonly path: string };

export interface WorkbenchProjectFolderAcquisition {
	selectExistingFolder(): Promise<WorkbenchProjectFolderAcquisitionResult>;
}
