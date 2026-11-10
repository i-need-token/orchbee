export type WorkbenchProjectFolderAcquisitionFailureCode =
	| 'already-exists'
	| 'busy'
	| 'control-unavailable'
	| 'dialog-failed'
	| 'invalid-name'
	| 'not-found'
	| 'permission-denied'
	| 'stale-parent'
	| 'validation-failed';

export type WorkbenchProjectFolderAcquisitionResult =
	| { readonly kind: 'cancelled' }
	| { readonly code: WorkbenchProjectFolderAcquisitionFailureCode; readonly kind: 'failed' }
	| { readonly kind: 'selected'; readonly path: string };

export interface WorkbenchProjectFolderAcquisition {
	readonly newFolder?: WorkbenchNewProjectFolderCapability | undefined;
	selectExistingFolder(): Promise<WorkbenchProjectFolderAcquisitionResult>;
}

export interface WorkbenchProjectCreationReceipt {
	readonly directoryId: string;
	readonly path: string;
	readonly presentationId: string;
	readonly projectId: string | null;
	readonly terminalContextId: string;
	readonly workspaceId: string;
}

export interface WorkbenchNewProjectFolderParent {
	readonly path: string;
	readonly selectionId: string;
}

export type WorkbenchNewProjectFolderParentResult =
	| { readonly kind: 'parent-selected'; readonly parent: WorkbenchNewProjectFolderParent }
	| Exclude<WorkbenchProjectFolderAcquisitionResult, { readonly kind: 'selected' }>;

export type WorkbenchNewProjectFolderResult =
	| { readonly kind: 'created'; readonly path: string; readonly requestId: string }
	| {
			readonly kind: 'created-unavailable';
			readonly path: string;
			readonly requestId: string;
			readonly code: WorkbenchProjectFolderAcquisitionFailureCode;
	  }
	| { readonly kind: 'failed'; readonly code: WorkbenchProjectFolderAcquisitionFailureCode }
	| { readonly kind: 'outcome-unknown'; readonly requestId: string };

export interface WorkbenchNewProjectFolderCapability {
	readonly signal: AbortSignal;
	selectParent(): Promise<WorkbenchNewProjectFolderParentResult>;
	releaseParent(parent: WorkbenchNewProjectFolderParent): void;
	create(
		parent: WorkbenchNewProjectFolderParent,
		name: string,
	): Promise<WorkbenchNewProjectFolderResult>;
	confirmProject(receipt: WorkbenchProjectCreationReceipt): Promise<boolean>;
	acknowledge(requestId: string): void;
}
