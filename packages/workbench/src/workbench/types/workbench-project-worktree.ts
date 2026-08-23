export type WorkbenchProjectWorktreeFailureCode =
	| 'base-not-found'
	| 'branch-already-exists'
	| 'busy'
	| 'compensation-failed'
	| 'control-unavailable'
	| 'creation-failed'
	| 'discovery-failed'
	| 'document-conflict'
	| 'git-unavailable'
	| 'invalid-branch'
	| 'project-not-found'
	| 'recovery-required'
	| 'repository-not-found'
	| 'source-unavailable'
	| 'target-conflict';

export interface WorkbenchProjectWorktreeRepository {
	readonly defaultBaseRef: string;
	readonly label: string;
	readonly localBaseRefs: readonly string[];
	readonly repositoryId: string;
	readonly sourceDirectoryId: string;
	readonly suggestedBranchName: string;
}

export interface WorkbenchProjectWorktreeDiscovery {
	readonly managedRoot: string;
	readonly repositories: readonly WorkbenchProjectWorktreeRepository[];
}

export type WorkbenchProjectWorktreeDiscoveryResult =
	| { readonly discovery: WorkbenchProjectWorktreeDiscovery; readonly kind: 'discovered' }
	| { readonly code: WorkbenchProjectWorktreeFailureCode; readonly kind: 'failed' };

export type WorkbenchProjectWorktreeCreationResult =
	| {
			readonly directoryId: string;
			readonly kind: 'created';
			readonly workspaceId: string | null;
	  }
	| { readonly code: WorkbenchProjectWorktreeFailureCode; readonly kind: 'failed' };

export interface WorkbenchProjectWorktreeCapability {
	create(input: {
		readonly baseRef: string;
		readonly branchName: string;
		readonly createWorkspace: boolean;
		readonly projectId: string;
		readonly repositoryId: string;
		readonly sourceDirectoryId: string;
	}): Promise<WorkbenchProjectWorktreeCreationResult>;
	discover(input: {
		readonly preferredDirectoryId: string | null;
		readonly projectId: string;
	}): Promise<WorkbenchProjectWorktreeDiscoveryResult>;
}
