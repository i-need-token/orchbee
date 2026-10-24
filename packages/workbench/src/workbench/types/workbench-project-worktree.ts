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
	| 'target-conflict'
	| 'worktree-retained'
	| 'worktree-not-found';

interface WorkbenchProjectExistingWorktree {
	readonly label: string;
	readonly worktreeId: string;
}

type WorkbenchProjectWorktreeInventory =
	| {
			readonly kind: 'available';
			readonly worktrees: readonly WorkbenchProjectExistingWorktree[];
	  }
	| { readonly kind: 'unavailable' };

interface WorkbenchProjectWorktreeRepository {
	readonly defaultBaseRef: string;
	readonly label: string;
	readonly localBaseRefs: readonly string[];
	readonly repositoryId: string;
	readonly sourceDirectoryId: string;
	readonly suggestedBranchName: string;
	readonly worktreeInventory: WorkbenchProjectWorktreeInventory;
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

export type WorkbenchProjectWorktreeAdoptionResult =
	| {
			readonly directoryId: string;
			readonly kind: 'adopted';
			readonly workspaceId: string;
	  }
	| { readonly code: WorkbenchProjectWorktreeFailureCode; readonly kind: 'failed' };

export interface WorkbenchProjectWorktreeCapability {
	adopt(input: {
		readonly sourceWorkspaceId: string;
		readonly repositoryId: string;
		readonly sourceDirectoryId: string;
		readonly worktreeId: string;
	}): Promise<WorkbenchProjectWorktreeAdoptionResult>;
	create(input: {
		readonly baseRef: string;
		readonly branchName: string;
		readonly createWorkspace: boolean;
		readonly sourceWorkspaceId: string;
		readonly repositoryId: string;
		readonly sourceDirectoryId: string;
	}): Promise<WorkbenchProjectWorktreeCreationResult>;
	discover(input: {
		readonly preferredDirectoryId: string | null;
		readonly sourceWorkspaceId: string;
	}): Promise<WorkbenchProjectWorktreeDiscoveryResult>;
}
