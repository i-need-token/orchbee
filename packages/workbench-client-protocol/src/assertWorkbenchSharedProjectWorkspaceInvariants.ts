// Shared-document validation and the canonical Workbench transitions apply these same rules,
// so a structure that one side accepts cannot be rejected by the other.

type WorkbenchSharedDirectoryPlacement =
	| { readonly kind: 'local' }
	| { readonly environmentId: string; readonly kind: 'environment' };

interface WorkbenchSharedDirectoryLocation {
	readonly path: string;
	readonly placement: WorkbenchSharedDirectoryPlacement;
}

interface WorkbenchSharedDirectoryBase extends WorkbenchSharedDirectoryLocation {
	readonly available: boolean;
	readonly id: string;
}

type WorkbenchSharedDirectory =
	| (WorkbenchSharedDirectoryBase & {
			readonly git: { readonly worktree: false } | null;
			readonly kind: 'folder';
	  })
	| (WorkbenchSharedDirectoryBase & {
			readonly git: { readonly worktree: true };
			readonly kind: 'worktree';
	  });

interface WorkbenchSharedLaunchConfiguration {
	readonly id: string;
	readonly name: string;
}

interface WorkbenchSharedResourceScope {
	readonly directories: readonly WorkbenchSharedDirectory[];
	readonly id: string;
	readonly launchConfigurations: readonly WorkbenchSharedLaunchConfiguration[];
	readonly primaryDirectoryId: string;
}

interface WorkbenchSharedWorkspace {
	readonly id: string;
	readonly launchConfigurationId: string | null;
	readonly primaryDirectoryId: string;
	readonly projectId: string | null;
	readonly resourceScopeId: string;
	readonly terminalContextId: string;
}

interface WorkbenchSharedProjectWorkspace {
	readonly projects: readonly { readonly id: string }[];
	readonly resourceScopes: readonly WorkbenchSharedResourceScope[];
	readonly standaloneTerminalContextId: string;
	readonly workspaces: readonly WorkbenchSharedWorkspace[];
}

function areValidWorkbenchSharedLaunchConfigurations(
	launchConfigurations: readonly WorkbenchSharedLaunchConfiguration[],
): boolean {
	const ids = new Set<string>();
	for (const launchConfiguration of launchConfigurations) {
		if (
			launchConfiguration.id.length === 0 ||
			launchConfiguration.name.trim().length === 0 ||
			ids.has(launchConfiguration.id)
		) {
			return false;
		}
		ids.add(launchConfiguration.id);
	}
	return true;
}

function hasWorkbenchSharedLaunchConfiguration(
	launchConfigurations: readonly WorkbenchSharedLaunchConfiguration[],
	launchConfigurationId: string | null,
): boolean {
	return (
		launchConfigurationId === null ||
		launchConfigurations.some(
			(launchConfiguration) => launchConfiguration.id === launchConfigurationId,
		)
	);
}

function isValidWorkbenchSharedDirectory(directory: WorkbenchSharedDirectory): boolean {
	if (
		directory.placement.kind === 'environment' &&
		(directory.placement.environmentId.length === 0 || directory.available)
	) {
		return false;
	}
	return directory.kind === 'worktree'
		? directory.git?.worktree === true
		: directory.kind === 'folder' && (directory.git === null || directory.git?.worktree === false);
}

function workbenchSharedDirectoryPlacementsEqual(
	left: WorkbenchSharedDirectoryPlacement,
	right: WorkbenchSharedDirectoryPlacement,
): boolean {
	return (
		left.kind === right.kind &&
		(left.kind === 'local' ||
			(right.kind === 'environment' && left.environmentId === right.environmentId))
	);
}

function workbenchSharedDirectoriesShareLocation(
	left: WorkbenchSharedDirectoryLocation,
	right: WorkbenchSharedDirectoryLocation,
): boolean {
	return (
		left.path === right.path &&
		workbenchSharedDirectoryPlacementsEqual(left.placement, right.placement)
	);
}

export function assertWorkbenchSharedProjectWorkspaceInvariants(
	projectWorkspace: WorkbenchSharedProjectWorkspace,
): void {
	const projectIds = new Set<string>();
	for (const project of projectWorkspace.projects) {
		if (projectIds.has(project.id)) throw new Error(`Duplicate Project ID ${project.id}.`);
		projectIds.add(project.id);
	}
	const scopes = new Map<string, WorkbenchSharedResourceScope>();
	for (const scope of projectWorkspace.resourceScopes) {
		if (scopes.has(scope.id)) throw new Error(`Duplicate resource scope ID ${scope.id}.`);
		scopes.set(scope.id, scope);
		if (scope.directories.length === 0)
			throw new Error(`Resource scope ${scope.id} has no Directory.`);
		const ids = new Set<string>();
		const locations: WorkbenchSharedDirectory[] = [];
		for (const directory of scope.directories) {
			if (!isValidWorkbenchSharedDirectory(directory))
				throw new Error(`Resource scope ${scope.id} has an invalid Directory ${directory.id}.`);
			if (ids.has(directory.id))
				throw new Error(`Resource scope ${scope.id} has duplicate Directory ID ${directory.id}.`);
			if (
				locations.some((candidate) => workbenchSharedDirectoriesShareLocation(candidate, directory))
			)
				throw new Error(
					`Resource scope ${scope.id} has duplicate Directory path ${directory.path}.`,
				);
			ids.add(directory.id);
			locations.push(directory);
		}
		if (!ids.has(scope.primaryDirectoryId))
			throw new Error(`Resource scope ${scope.id} has a missing primary Directory.`);
		if (!areValidWorkbenchSharedLaunchConfigurations(scope.launchConfigurations))
			throw new Error(`Resource scope ${scope.id} has invalid launch configurations.`);
	}
	const workspaceIds = new Set<string>();
	const contextIds = new Set<string>([projectWorkspace.standaloneTerminalContextId]);
	for (const workspace of projectWorkspace.workspaces) {
		if (workspaceIds.has(workspace.id)) throw new Error(`Duplicate Workspace ID ${workspace.id}.`);
		workspaceIds.add(workspace.id);
		if (workspace.projectId !== null && !projectIds.has(workspace.projectId))
			throw new Error(`Workspace ${workspace.id} has a missing Project.`);
		const scope = scopes.get(workspace.resourceScopeId);
		if (scope === undefined)
			throw new Error(`Workspace ${workspace.id} has a missing resource scope.`);
		if (!scope.directories.some((directory) => directory.id === workspace.primaryDirectoryId))
			throw new Error(`Workspace ${workspace.id} has a missing primary Directory.`);
		if (
			!hasWorkbenchSharedLaunchConfiguration(
				scope.launchConfigurations,
				workspace.launchConfigurationId,
			)
		)
			throw new Error(`Workspace ${workspace.id} has a missing launch configuration.`);
		if (contextIds.has(workspace.terminalContextId))
			throw new Error(`Duplicate terminal-context ID ${workspace.terminalContextId}.`);
		contextIds.add(workspace.terminalContextId);
	}
}
