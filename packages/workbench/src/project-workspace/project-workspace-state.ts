type WorkbenchDirectorySource = 'created' | 'discovered' | 'selected';

type WorkbenchDirectoryPlacement =
	| { readonly kind: 'local' }
	| { readonly environmentId: string; readonly kind: 'environment' };

interface WorkbenchDirectoryGitMetadataBase {
	readonly branch: string | null;
	readonly head: string;
	readonly repositoryId: string;
}

interface WorkbenchRepositoryDirectoryGitMetadata extends WorkbenchDirectoryGitMetadataBase {
	readonly worktree: false;
}

interface WorkbenchWorktreeDirectoryGitMetadata extends WorkbenchDirectoryGitMetadataBase {
	readonly worktree: true;
}

interface WorkbenchDirectoryBase {
	readonly available: boolean;
	readonly id: string;
	readonly name: string;
	readonly path: string;
	readonly placement: WorkbenchDirectoryPlacement;
	readonly source: WorkbenchDirectorySource;
}

interface WorkbenchFolderDirectory extends WorkbenchDirectoryBase {
	readonly git: WorkbenchRepositoryDirectoryGitMetadata | null;
	readonly kind: 'folder';
}

interface WorkbenchWorktreeDirectory extends WorkbenchDirectoryBase {
	readonly git: WorkbenchWorktreeDirectoryGitMetadata;
	readonly kind: 'worktree';
}

export type WorkbenchDirectory = WorkbenchFolderDirectory | WorkbenchWorktreeDirectory;

interface WorkbenchLaunchConfigurationReference {
	readonly id: string;
	readonly name: string;
}

export interface WorkbenchWorkspace {
	readonly id: string;
	readonly launchConfigurationId: string | null;
	readonly name: string;
	readonly primaryDirectoryId: string;
	readonly projectId: string | null;
	readonly resourceScopeId: string;
	readonly terminalContextId: string;
}

export interface WorkbenchProject {
	readonly id: string;
	readonly name: string;
}

/** Immutable resource identity; grouping never transfers or merges this namespace. */
export interface WorkbenchResourceScope {
	readonly directories: readonly WorkbenchDirectory[];
	readonly id: string;
	readonly launchConfigurations: readonly WorkbenchLaunchConfigurationReference[];
	readonly name: string;
	readonly primaryDirectoryId: string;
}

export type WorkbenchActiveLocation =
	| { readonly kind: 'standalone' }
	| {
			readonly kind: 'workspace';
			readonly workspaceId: string;
	  };

export interface ProjectWorkspaceState {
	readonly activeLocation: WorkbenchActiveLocation;
	readonly expandedProjectIds: readonly string[];
	readonly nextProjectSequence: number;
	readonly nextResourceScopeSequence: number;
	readonly nextTerminalContextSequence: number;
	readonly nextWorkspaceSequence: number;
	readonly projects: readonly WorkbenchProject[];
	readonly resourceScopes: readonly WorkbenchResourceScope[];
	readonly standaloneTerminalContextId: string;
	readonly workspaces: readonly WorkbenchWorkspace[];
}

export type ProjectWorkspaceEvent =
	| {
			readonly type: 'project-created';
			readonly directory: WorkbenchDirectory;
			readonly launchConfigurations: readonly WorkbenchLaunchConfigurationReference[];
			readonly primaryWorkspaceName: string;
			readonly primaryWorkspaceLaunchConfigurationId: string | null;
			readonly projectName: string;
			readonly terminalTitle: string;
	  }
	| {
			readonly type: 'workspace-created-default';
			readonly directory: WorkbenchDirectory;
			readonly name: string;
			readonly terminalTitle: string;
	  }
	| {
			readonly type: 'environment-project-registered';
			readonly directory: WorkbenchFolderDirectory;
			readonly projectName: string;
	  }
	| {
			readonly type: 'environment-directory-registered';
			readonly directory: WorkbenchFolderDirectory;
			readonly resourceScopeId: string;
	  }
	| {
			readonly type: 'environment-workspace-registered';
			readonly directoryId: string;
			readonly resourceScopeId: string;
			readonly projectId: string | null;
			readonly workspaceName: string;
	  }
	| { readonly type: 'project-expansion-toggled'; readonly projectId: string }
	| {
			readonly type: 'project-reordered';
			readonly projectId: string;
			readonly beforeProjectId: string | null;
	  }
	| { readonly type: 'project-renamed'; readonly projectId: string; readonly name: string }
	| { readonly type: 'project-dissolved'; readonly projectId: string }
	| { readonly type: 'project-removed'; readonly projectId: string }
	| { readonly type: 'project-workspaces-closed'; readonly projectId: string }
	| {
			readonly type: 'workspace-project-set';
			readonly workspaceId: string;
			readonly projectId: string | null;
	  }
	| {
			readonly type: 'workspace-project-created';
			readonly workspaceId: string;
			readonly name: string;
	  }
	| {
			readonly type: 'directory-added' | 'directory-updated';
			readonly resourceScopeId: string;
			readonly directory: WorkbenchDirectory;
	  }
	| {
			readonly type: 'directories-removed';
			readonly resourceScopeId: string;
			readonly directoryIds: readonly string[];
	  }
	| {
			readonly type: 'resource-launch-configurations-reconciled';
			readonly resourceScopeId: string;
			readonly launchConfigurations: readonly WorkbenchLaunchConfigurationReference[];
	  }
	| {
			readonly type: 'resource-primary-directory-set';
			readonly resourceScopeId: string;
			readonly directoryId: string;
	  }
	| { readonly type: 'standalone-activated' }
	| { readonly type: 'workspace-activated' | 'workspace-deleted'; readonly workspaceId: string }
	| {
			readonly type: 'workspace-reordered';
			readonly workspaceId: string;
			readonly beforeWorkspaceId: string | null;
	  }
	| {
			readonly type: 'workspace-created';
			readonly resourceScopeId: string;
			readonly projectId: string | null;
			readonly directoryId: string;
			readonly launchConfigurationId: string | null;
			readonly name: string;
			readonly terminalTitle: string;
	  }
	| {
			readonly type: 'workspace-duplicated';
			readonly workspaceId: string;
			readonly copyName: string;
	  }
	| {
			readonly type: 'workspace-updated';
			readonly workspaceId: string;
			readonly launchConfigurationId: string | null;
			readonly primaryDirectoryId: string;
			readonly name: string;
	  }
	| {
			readonly type: 'project-worktree-created';
			readonly createWorkspace: boolean;
			readonly sourceWorkspaceId: string;
			readonly sourceDirectoryId: string;
			readonly directory: WorkbenchWorktreeDirectory;
			readonly terminalTitle: string;
			readonly workspaceName: string;
			readonly projectName: string;
	  }
	| {
			readonly type: 'project-worktree-adopted';
			readonly sourceWorkspaceId: string;
			readonly sourceDirectoryId: string;
			readonly directory: WorkbenchWorktreeDirectory;
			readonly terminalTitle: string;
			readonly workspaceName: string;
			readonly projectName: string;
	  };

type ProjectWorkspaceRejection =
	| 'directory-already-exists'
	| 'directory-environment-conflict'
	| 'directory-invalid'
	| 'directory-not-found'
	| 'directory-removal-not-allowed'
	| 'directory-unavailable'
	| 'directory-update-not-allowed'
	| 'last-directory'
	| 'launch-configuration-invalid'
	| 'launch-configuration-not-found'
	| 'project-not-found'
	| 'resource-scope-not-found'
	| 'workspace-not-found'
	| 'workspace-project-conflict';

export type TerminalContextCommand =
	| { readonly type: 'terminal-context-created-empty'; readonly terminalContextId: string }
	| {
			readonly type: 'terminal-context-created';
			readonly cwd: string;
			readonly terminalContextId: string;
			readonly terminalTitle: string;
	  }
	| {
			readonly type: 'terminal-context-cloned';
			readonly sourceTerminalContextId: string;
			readonly targetTerminalContextId: string;
	  }
	| { readonly type: 'terminal-context-disposed'; readonly terminalContextId: string };

export interface ProjectWorkspaceTransition {
	readonly commands: readonly TerminalContextCommand[];
	readonly outcome: 'applied' | 'rejected' | 'unchanged';
	readonly rejection: ProjectWorkspaceRejection | null;
	readonly state: ProjectWorkspaceState;
}

export function createProjectWorkspaceState(): ProjectWorkspaceState {
	return {
		activeLocation: { kind: 'standalone' },
		expandedProjectIds: [],
		nextProjectSequence: 1,
		nextResourceScopeSequence: 1,
		nextTerminalContextSequence: 2,
		nextWorkspaceSequence: 1,
		projects: [],
		resourceScopes: [],
		standaloneTerminalContextId: 'terminal-context-1',
		workspaces: [],
	};
}

export function findWorkbenchWorkspace(
	state: ProjectWorkspaceState,
	workspaceId: string,
): WorkbenchWorkspace | undefined {
	return state.workspaces.find((workspace) => workspace.id === workspaceId);
}

function findWorkbenchResourceScope(
	state: ProjectWorkspaceState,
	resourceScopeId: string,
): WorkbenchResourceScope | undefined {
	return state.resourceScopes.find((scope) => scope.id === resourceScopeId);
}

export function findWorkspaceResources(
	state: ProjectWorkspaceState,
	workspaceId: string,
): WorkbenchResourceScope | undefined {
	const workspace = findWorkbenchWorkspace(state, workspaceId);
	return workspace === undefined
		? undefined
		: findWorkbenchResourceScope(state, workspace.resourceScopeId);
}

export function transitionProjectWorkspace(
	state: ProjectWorkspaceState,
	event: ProjectWorkspaceEvent,
): ProjectWorkspaceTransition {
	const result = transition(state, event);
	assertProjectWorkspaceInvariants(result.state);
	return result;
}

function transition(
	state: ProjectWorkspaceState,
	event: ProjectWorkspaceEvent,
): ProjectWorkspaceTransition {
	switch (event.type) {
		case 'project-created':
		case 'workspace-created-default':
			return createInitialWorkspace(state, event);
		case 'environment-project-registered':
			return registerEnvironmentProject(state, event);
		case 'environment-directory-registered':
			return addDirectory(state, event.resourceScopeId, event.directory, true);
		case 'environment-workspace-registered':
			return createWorkspace(
				state,
				{
					...event,
					name: event.workspaceName,
					launchConfigurationId: null,
					terminalTitle: '',
				},
				true,
			);
		case 'project-expansion-toggled':
			if (!hasProject(state, event.projectId)) return rejected(state, 'project-not-found');
			return applied({
				...state,
				expandedProjectIds: state.expandedProjectIds.includes(event.projectId)
					? state.expandedProjectIds.filter((id) => id !== event.projectId)
					: [...state.expandedProjectIds, event.projectId],
			});
		case 'project-reordered': {
			const projects = reorderItems(state.projects, event.projectId, event.beforeProjectId);
			if (projects === null) return rejected(state, 'project-not-found');
			return projects === state.projects ? unchanged(state) : applied({ ...state, projects });
		}
		case 'project-renamed':
			if (!hasProject(state, event.projectId)) return rejected(state, 'project-not-found');
			if (state.projects.find((project) => project.id === event.projectId)?.name === event.name)
				return unchanged(state);
			return applied({
				...state,
				projects: state.projects.map((project) =>
					project.id === event.projectId ? { ...project, name: event.name } : project,
				),
			});
		case 'project-dissolved':
		case 'project-removed':
			return dissolveProject(state, event.projectId);
		case 'project-workspaces-closed':
			return closeProjectWorkspaces(state, event.projectId);
		case 'workspace-project-set':
			return setWorkspaceProject(state, event.workspaceId, event.projectId);
		case 'workspace-project-created': {
			if (findWorkbenchWorkspace(state, event.workspaceId) === undefined)
				return rejected(state, 'workspace-not-found');
			const projectId = `project-${availableProjectSequence(state)}`;
			return setWorkspaceProject(
				{
					...state,
					projects: [...state.projects, { id: projectId, name: event.name }],
					nextProjectSequence: availableProjectSequence(state) + 1,
				},
				event.workspaceId,
				projectId,
			);
		}
		case 'directory-added':
			return addDirectory(state, event.resourceScopeId, event.directory);
		case 'directory-updated':
			return updateDirectory(state, event.resourceScopeId, event.directory);
		case 'directories-removed':
			return removeDirectories(state, event.resourceScopeId, event.directoryIds);
		case 'resource-launch-configurations-reconciled':
			return reconcileLaunchConfigurations(
				state,
				event.resourceScopeId,
				event.launchConfigurations,
			);
		case 'resource-primary-directory-set':
			return setResourcePrimaryDirectory(state, event.resourceScopeId, event.directoryId);
		case 'standalone-activated':
			return state.activeLocation.kind === 'standalone'
				? unchanged(state)
				: applied({ ...state, activeLocation: { kind: 'standalone' } });
		case 'workspace-activated':
			return activateWorkspace(state, event.workspaceId);
		case 'workspace-reordered':
			return reorderWorkspace(state, event.workspaceId, event.beforeWorkspaceId);
		case 'workspace-created':
			return createWorkspace(state, event);
		case 'workspace-duplicated':
			return duplicateWorkspace(state, event.workspaceId, event.copyName);
		case 'workspace-updated':
			return updateWorkspace(state, event);
		case 'workspace-deleted':
			return deleteWorkspace(state, event.workspaceId);
		case 'project-worktree-created':
		case 'project-worktree-adopted':
			return registerWorktree(state, event);
	}
}

function createInitialWorkspace(
	state: ProjectWorkspaceState,
	event: Extract<ProjectWorkspaceEvent, { type: 'project-created' | 'workspace-created-default' }>,
): ProjectWorkspaceTransition {
	if (!isValidDirectoryDescriptor(event.directory)) return rejected(state, 'directory-invalid');
	if (!isUsableLocalDirectory(event.directory)) return rejected(state, 'directory-unavailable');
	const launchConfigurations = event.type === 'project-created' ? event.launchConfigurations : [];
	const launchConfigurationId =
		event.type === 'project-created' ? event.primaryWorkspaceLaunchConfigurationId : null;
	if (!areValidLaunchConfigurations(launchConfigurations))
		return rejected(state, 'launch-configuration-invalid');
	if (!hasLaunchConfiguration(launchConfigurations, launchConfigurationId))
		return rejected(state, 'launch-configuration-not-found');
	const resourceScopeId = `resource-scope-${availableResourceScopeSequence(state)}`;
	const projectId =
		event.type === 'project-created' ? `project-${availableProjectSequence(state)}` : null;
	const scope: WorkbenchResourceScope = {
		id: resourceScopeId,
		name: event.directory.name,
		directories: [event.directory],
		launchConfigurations: [...launchConfigurations],
		primaryDirectoryId: event.directory.id,
	};
	const prepared = {
		...state,
		resourceScopes: [...state.resourceScopes, scope],
		nextResourceScopeSequence: availableResourceScopeSequence(state) + 1,
		projects:
			projectId !== null && event.type === 'project-created'
				? [...state.projects, { id: projectId, name: event.projectName }]
				: state.projects,
		nextProjectSequence: availableProjectSequence(state) + (projectId === null ? 0 : 1),
	};
	return createWorkspace(prepared, {
		resourceScopeId,
		projectId,
		directoryId: event.directory.id,
		launchConfigurationId,
		name: event.type === 'project-created' ? event.primaryWorkspaceName : event.name,
		terminalTitle: event.terminalTitle,
	});
}

function registerEnvironmentProject(
	state: ProjectWorkspaceState,
	event: Extract<ProjectWorkspaceEvent, { type: 'environment-project-registered' }>,
): ProjectWorkspaceTransition {
	if (!isValidRemoteProjectDirectory(event.directory)) return rejected(state, 'directory-invalid');
	if (
		state.resourceScopes.some((scope) =>
			scope.directories.some((directory) => directoriesShareLocation(directory, event.directory)),
		)
	)
		return unchanged(state);
	const projectId = `project-${availableProjectSequence(state)}`;
	return applied({
		...state,
		nextProjectSequence: availableProjectSequence(state) + 1,
		nextResourceScopeSequence: availableResourceScopeSequence(state) + 1,
		projects: [...state.projects, { id: projectId, name: event.projectName }],
		resourceScopes: [
			...state.resourceScopes,
			{
				id: `resource-scope-${availableResourceScopeSequence(state)}`,
				name: event.projectName,
				directories: [event.directory],
				primaryDirectoryId: event.directory.id,
				launchConfigurations: [],
			},
		],
	});
}

function hasProject(state: ProjectWorkspaceState, projectId: string | null): boolean {
	return projectId === null || state.projects.some((project) => project.id === projectId);
}

function setWorkspaceProject(
	state: ProjectWorkspaceState,
	workspaceId: string,
	projectId: string | null,
): ProjectWorkspaceTransition {
	const workspace = findWorkbenchWorkspace(state, workspaceId);
	if (workspace === undefined) return rejected(state, 'workspace-not-found');
	if (!hasProject(state, projectId)) return rejected(state, 'project-not-found');
	if (workspace.projectId === projectId) return unchanged(state);
	return applied({
		...state,
		workspaces: state.workspaces.map((candidate) =>
			candidate.id === workspaceId ? { ...candidate, projectId } : candidate,
		),
	});
}

function dissolveProject(
	state: ProjectWorkspaceState,
	projectId: string,
): ProjectWorkspaceTransition {
	if (!hasProject(state, projectId)) return rejected(state, 'project-not-found');
	return applied({
		...state,
		projects: state.projects.filter((project) => project.id !== projectId),
		expandedProjectIds: state.expandedProjectIds.filter((id) => id !== projectId),
		workspaces: state.workspaces.map((workspace) =>
			workspace.projectId === projectId ? { ...workspace, projectId: null } : workspace,
		),
	});
}

function closeProjectWorkspaces(
	state: ProjectWorkspaceState,
	projectId: string,
): ProjectWorkspaceTransition {
	if (!hasProject(state, projectId)) return rejected(state, 'project-not-found');
	const members = state.workspaces.filter((workspace) => workspace.projectId === projectId);
	if (members.length === 0) return unchanged(state);
	const memberIds = new Set(members.map((workspace) => workspace.id));
	return applied(
		{
			...state,
			workspaces: state.workspaces.filter((workspace) => !memberIds.has(workspace.id)),
			activeLocation:
				state.activeLocation.kind === 'workspace' && memberIds.has(state.activeLocation.workspaceId)
					? { kind: 'standalone' }
					: state.activeLocation,
		},
		members.map((workspace) => ({
			type: 'terminal-context-disposed',
			terminalContextId: workspace.terminalContextId,
		})),
	);
}

function updateScope(
	state: ProjectWorkspaceState,
	scope: WorkbenchResourceScope,
): ProjectWorkspaceState {
	return {
		...state,
		resourceScopes: state.resourceScopes.map((candidate) =>
			candidate.id === scope.id ? scope : candidate,
		),
	};
}

function addDirectory(
	state: ProjectWorkspaceState,
	scopeId: string,
	directory: WorkbenchDirectory,
	remote = false,
): ProjectWorkspaceTransition {
	const scope = findWorkbenchResourceScope(state, scopeId);
	if (scope === undefined) return rejected(state, 'resource-scope-not-found');
	if (
		!isValidDirectoryDescriptor(directory) ||
		(remote
			? directory.kind !== 'folder' || !isValidRemoteProjectDirectory(directory)
			: directory.placement.kind !== 'local')
	)
		return rejected(state, 'directory-invalid');
	if (remote) {
		if (
			state.resourceScopes.some(
				(other) =>
					other.id !== scope.id &&
					other.directories.some((candidate) => directoriesShareLocation(candidate, directory)),
			)
		)
			return rejected(state, 'directory-already-exists');
		if (directory.placement.kind === 'environment') {
			const environmentId = directory.placement.environmentId;
			if (
				scope.directories.some(
					(candidate) =>
						candidate.placement.kind === 'environment' &&
						candidate.placement.environmentId !== environmentId,
				)
			)
				return rejected(state, 'directory-environment-conflict');
		}
		if (scope.directories.some((candidate) => directoriesShareLocation(candidate, directory)))
			return unchanged(state);
	}
	if (
		scope.directories.some(
			(candidate) =>
				candidate.id === directory.id || directoriesShareLocation(candidate, directory),
		)
	)
		return rejected(state, 'directory-already-exists');
	return applied(updateScope(state, { ...scope, directories: [...scope.directories, directory] }));
}

function updateDirectory(
	state: ProjectWorkspaceState,
	scopeId: string,
	directory: WorkbenchDirectory,
): ProjectWorkspaceTransition {
	const scope = findWorkbenchResourceScope(state, scopeId);
	if (scope === undefined) return rejected(state, 'resource-scope-not-found');
	if (!isValidDirectoryDescriptor(directory)) return rejected(state, 'directory-invalid');
	const current = scope.directories.find((candidate) => candidate.id === directory.id);
	if (current === undefined) return rejected(state, 'directory-not-found');
	if (
		current.kind !== directory.kind ||
		current.source !== directory.source ||
		!directoryPlacementsEqual(current.placement, directory.placement)
	)
		return rejected(state, 'directory-update-not-allowed');
	if (
		scope.directories.some(
			(candidate) =>
				candidate.id !== directory.id && directoriesShareLocation(candidate, directory),
		)
	)
		return rejected(state, 'directory-already-exists');
	if (directoriesEqual(current, directory)) return unchanged(state);
	return applied(
		updateScope(state, {
			...scope,
			directories: scope.directories.map((candidate) =>
				candidate.id === directory.id ? directory : candidate,
			),
		}),
	);
}

function removeDirectories(
	state: ProjectWorkspaceState,
	scopeId: string,
	directoryIds: readonly string[],
): ProjectWorkspaceTransition {
	const scope = findWorkbenchResourceScope(state, scopeId);
	if (scope === undefined) return rejected(state, 'resource-scope-not-found');
	const removedIds = new Set(directoryIds);
	if (removedIds.size === 0) return unchanged(state);
	const removed = scope.directories.filter((directory) => removedIds.has(directory.id));
	if (removed.length !== removedIds.size) return rejected(state, 'directory-not-found');
	if (removed.some((directory) => directory.kind === 'worktree'))
		return rejected(state, 'directory-removal-not-allowed');
	const directories = scope.directories.filter((directory) => !removedIds.has(directory.id));
	if (directories.length === 0) return rejected(state, 'last-directory');
	const workspaces: WorkbenchWorkspace[] = [];
	for (const workspace of state.workspaces) {
		if (workspace.resourceScopeId !== scopeId || !removedIds.has(workspace.primaryDirectoryId)) {
			workspaces.push(workspace);
			continue;
		}
		const primaryDirectoryId = findDirectoryFallback(
			scope.directories,
			removedIds,
			workspace.primaryDirectoryId,
			(directory) => directory.placement.kind === 'local',
		);
		if (primaryDirectoryId === null) return rejected(state, 'directory-removal-not-allowed');
		workspaces.push({ ...workspace, primaryDirectoryId });
	}
	return applied({
		...updateScope(state, {
			...scope,
			directories,
			primaryDirectoryId: removedIds.has(scope.primaryDirectoryId)
				? requireDirectoryFallback(scope.directories, removedIds, scope.primaryDirectoryId)
				: scope.primaryDirectoryId,
		}),
		workspaces,
	});
}

function reconcileLaunchConfigurations(
	state: ProjectWorkspaceState,
	scopeId: string,
	launchConfigurations: readonly WorkbenchLaunchConfigurationReference[],
): ProjectWorkspaceTransition {
	const scope = findWorkbenchResourceScope(state, scopeId);
	if (scope === undefined) return rejected(state, 'resource-scope-not-found');
	if (!areValidLaunchConfigurations(launchConfigurations))
		return rejected(state, 'launch-configuration-invalid');
	if (launchConfigurationsEqual(scope.launchConfigurations, launchConfigurations))
		return unchanged(state);
	return applied({
		...updateScope(state, { ...scope, launchConfigurations: [...launchConfigurations] }),
		workspaces: state.workspaces.map((workspace) =>
			workspace.resourceScopeId !== scopeId ||
			hasLaunchConfiguration(launchConfigurations, workspace.launchConfigurationId)
				? workspace
				: { ...workspace, launchConfigurationId: null },
		),
	});
}

function setResourcePrimaryDirectory(
	state: ProjectWorkspaceState,
	scopeId: string,
	directoryId: string,
): ProjectWorkspaceTransition {
	const scope = findWorkbenchResourceScope(state, scopeId);
	if (scope === undefined) return rejected(state, 'resource-scope-not-found');
	const directory = scope.directories.find((candidate) => candidate.id === directoryId);
	if (directory === undefined) return rejected(state, 'directory-not-found');
	if (!isUsableLocalDirectory(directory)) return rejected(state, 'directory-unavailable');
	return scope.primaryDirectoryId === directoryId
		? unchanged(state)
		: applied(updateScope(state, { ...scope, primaryDirectoryId: directoryId }));
}

function activateWorkspace(
	state: ProjectWorkspaceState,
	workspaceId: string,
): ProjectWorkspaceTransition {
	const workspace = findWorkbenchWorkspace(state, workspaceId);
	if (workspace === undefined) return rejected(state, 'workspace-not-found');
	const expandedProjectIds =
		workspace.projectId === null
			? state.expandedProjectIds
			: includeExpandedProject(state, workspace.projectId);
	if (
		state.activeLocation.kind === 'workspace' &&
		state.activeLocation.workspaceId === workspaceId &&
		expandedProjectIds === state.expandedProjectIds
	)
		return unchanged(state);
	return applied({
		...state,
		activeLocation: { kind: 'workspace', workspaceId },
		expandedProjectIds,
	});
}

function reorderWorkspace(
	state: ProjectWorkspaceState,
	workspaceId: string,
	beforeWorkspaceId: string | null,
): ProjectWorkspaceTransition {
	const workspace = findWorkbenchWorkspace(state, workspaceId);
	if (workspace === undefined) return rejected(state, 'workspace-not-found');
	const members = state.workspaces.filter(
		(candidate) => candidate.projectId === workspace.projectId,
	);
	const ordered = reorderItems(members, workspaceId, beforeWorkspaceId);
	if (ordered === null) return rejected(state, 'workspace-not-found');
	if (ordered === members) return unchanged(state);
	let nextIndex = 0;
	return applied({
		...state,
		workspaces: state.workspaces.map((candidate) => {
			if (candidate.projectId !== workspace.projectId) return candidate;
			const next = ordered[nextIndex++];
			if (next === undefined) throw new Error('Workspace order lost a member.');
			return next;
		}),
	});
}

interface WorkspaceCreation {
	readonly resourceScopeId: string;
	readonly projectId: string | null;
	readonly directoryId: string;
	readonly launchConfigurationId: string | null;
	readonly name: string;
	readonly terminalTitle: string;
}

function createWorkspace(
	state: ProjectWorkspaceState,
	input: WorkspaceCreation,
	remote = false,
): ProjectWorkspaceTransition {
	const scope = findWorkbenchResourceScope(state, input.resourceScopeId);
	if (scope === undefined) return rejected(state, 'resource-scope-not-found');
	if (!hasProject(state, input.projectId)) return rejected(state, 'project-not-found');
	const directory = scope.directories.find((candidate) => candidate.id === input.directoryId);
	if (directory === undefined) return rejected(state, 'directory-not-found');
	if (remote ? directory.placement.kind !== 'environment' : !isUsableLocalDirectory(directory))
		return rejected(state, 'directory-unavailable');
	if (!hasLaunchConfiguration(scope.launchConfigurations, input.launchConfigurationId))
		return rejected(state, 'launch-configuration-not-found');
	if (
		remote &&
		state.workspaces.some(
			(workspace) =>
				workspace.resourceScopeId === scope.id && workspace.primaryDirectoryId === directory.id,
		)
	)
		return unchanged(state);
	const workspace: WorkbenchWorkspace = {
		id: `workspace-${availableWorkspaceSequence(state)}`,
		resourceScopeId: scope.id,
		projectId: input.projectId,
		launchConfigurationId: input.launchConfigurationId,
		name: input.name,
		primaryDirectoryId: directory.id,
		terminalContextId: `terminal-context-${availableTerminalContextSequence(state)}`,
	};
	return applied(
		{
			...state,
			workspaces: [...state.workspaces, workspace],
			activeLocation: remote
				? state.activeLocation
				: { kind: 'workspace', workspaceId: workspace.id },
			expandedProjectIds:
				remote || input.projectId === null
					? state.expandedProjectIds
					: includeExpandedProject(state, input.projectId),
			nextTerminalContextSequence: availableTerminalContextSequence(state) + 1,
			nextWorkspaceSequence: availableWorkspaceSequence(state) + 1,
		},
		[
			remote
				? { type: 'terminal-context-created-empty', terminalContextId: workspace.terminalContextId }
				: {
						type: 'terminal-context-created',
						terminalContextId: workspace.terminalContextId,
						cwd: directory.path,
						terminalTitle: input.terminalTitle,
					},
		],
	);
}

function duplicateWorkspace(
	state: ProjectWorkspaceState,
	workspaceId: string,
	copyName: string,
): ProjectWorkspaceTransition {
	const source = findWorkbenchWorkspace(state, workspaceId);
	if (source === undefined) return rejected(state, 'workspace-not-found');
	const result = createWorkspace(state, {
		resourceScopeId: source.resourceScopeId,
		projectId: source.projectId,
		directoryId: source.primaryDirectoryId,
		launchConfigurationId: source.launchConfigurationId,
		name: copyName,
		terminalTitle: '',
	});
	if (result.outcome !== 'applied') return result;
	return {
		...result,
		commands: [
			{
				type: 'terminal-context-cloned',
				sourceTerminalContextId: source.terminalContextId,
				targetTerminalContextId: `terminal-context-${availableTerminalContextSequence(state)}`,
			},
		],
	};
}

function updateWorkspace(
	state: ProjectWorkspaceState,
	event: Extract<ProjectWorkspaceEvent, { type: 'workspace-updated' }>,
): ProjectWorkspaceTransition {
	const workspace = findWorkbenchWorkspace(state, event.workspaceId);
	if (workspace === undefined) return rejected(state, 'workspace-not-found');
	const scope = findWorkbenchResourceScope(state, workspace.resourceScopeId);
	const directory = scope?.directories.find(
		(candidate) => candidate.id === event.primaryDirectoryId,
	);
	if (scope === undefined || directory === undefined) return rejected(state, 'directory-not-found');
	if (!isUsableLocalDirectory(directory)) return rejected(state, 'directory-unavailable');
	if (!hasLaunchConfiguration(scope.launchConfigurations, event.launchConfigurationId))
		return rejected(state, 'launch-configuration-not-found');
	if (
		workspace.name === event.name &&
		workspace.primaryDirectoryId === event.primaryDirectoryId &&
		workspace.launchConfigurationId === event.launchConfigurationId
	)
		return unchanged(state);
	return applied({
		...state,
		workspaces: state.workspaces.map((candidate) =>
			candidate.id === workspace.id
				? {
						...workspace,
						name: event.name,
						primaryDirectoryId: event.primaryDirectoryId,
						launchConfigurationId: event.launchConfigurationId,
					}
				: candidate,
		),
	});
}

function deleteWorkspace(
	state: ProjectWorkspaceState,
	workspaceId: string,
): ProjectWorkspaceTransition {
	const workspace = findWorkbenchWorkspace(state, workspaceId);
	if (workspace === undefined) return rejected(state, 'workspace-not-found');
	return applied(
		{
			...state,
			workspaces: state.workspaces.filter((candidate) => candidate.id !== workspaceId),
			activeLocation:
				state.activeLocation.kind === 'workspace' &&
				state.activeLocation.workspaceId === workspaceId
					? { kind: 'standalone' }
					: state.activeLocation,
		},
		[{ type: 'terminal-context-disposed', terminalContextId: workspace.terminalContextId }],
	);
}

function registerWorktree(
	state: ProjectWorkspaceState,
	event: Extract<
		ProjectWorkspaceEvent,
		{ type: 'project-worktree-created' | 'project-worktree-adopted' }
	>,
): ProjectWorkspaceTransition {
	const source = findWorkbenchWorkspace(state, event.sourceWorkspaceId);
	if (source === undefined) return rejected(state, 'workspace-not-found');
	const scope = findWorkbenchResourceScope(state, source.resourceScopeId);
	const sourceDirectory = scope?.directories.find(
		(candidate) => candidate.id === event.sourceDirectoryId,
	);
	if (scope === undefined || sourceDirectory === undefined)
		return rejected(state, 'directory-not-found');
	if (!isUsableLocalDirectory(sourceDirectory)) return rejected(state, 'directory-unavailable');
	if (
		!isValidDirectoryDescriptor(event.directory) ||
		!isUsableLocalDirectory(event.directory) ||
		event.directory.source !==
			(event.type === 'project-worktree-created' ? 'created' : 'discovered') ||
		(event.type === 'project-worktree-created' && event.directory.git.branch === null) ||
		(sourceDirectory.git !== null &&
			sourceDirectory.git.repositoryId !== event.directory.git.repositoryId)
	)
		return rejected(state, 'directory-invalid');
	// Git identity is physical and independent of Project and resource namespace.
	const existingWorkspace = state.workspaces.find((workspace) => {
		const resources = findWorkbenchResourceScope(state, workspace.resourceScopeId);
		const directory = resources?.directories.find(
			(candidate) => candidate.id === workspace.primaryDirectoryId,
		);
		return (
			directory?.kind === 'worktree' &&
			directory.git.repositoryId === event.directory.git.repositoryId &&
			directoriesShareLocation(directory, event.directory)
		);
	});
	if (existingWorkspace !== undefined) return unchanged(state);
	const existingDirectory = scope.directories.find((candidate) =>
		directoriesShareLocation(candidate, event.directory),
	);
	if (
		existingDirectory === undefined &&
		scope.directories.some((candidate) => candidate.id === event.directory.id)
	)
		return rejected(state, 'directory-already-exists');
	if (
		existingDirectory !== undefined &&
		(!existingDirectory.available ||
			(existingDirectory.git !== null &&
				existingDirectory.git.repositoryId !== event.directory.git.repositoryId))
	)
		return rejected(state, 'directory-already-exists');
	const directory =
		existingDirectory === undefined
			? event.directory
			: { ...event.directory, id: existingDirectory.id };
	const withDirectory = updateScope(state, {
		...scope,
		directories:
			existingDirectory === undefined
				? [...scope.directories, directory]
				: scope.directories.map((candidate) =>
						candidate.id === existingDirectory.id ? directory : candidate,
					),
	});
	if (event.type === 'project-worktree-created' && !event.createWorkspace)
		return withDirectory === state ? unchanged(state) : applied(withDirectory);
	const projectId = source.projectId ?? `project-${availableProjectSequence(state)}`;
	const grouped =
		source.projectId === null
			? {
					...withDirectory,
					projects: [...withDirectory.projects, { id: projectId, name: event.projectName }],
					nextProjectSequence: availableProjectSequence(state) + 1,
					workspaces: withDirectory.workspaces.map((workspace) =>
						workspace.id === source.id ? { ...workspace, projectId } : workspace,
					),
				}
			: withDirectory;
	const created = createWorkspace(grouped, {
		resourceScopeId: scope.id,
		projectId,
		directoryId: directory.id,
		launchConfigurationId: null,
		name: event.workspaceName,
		terminalTitle: event.terminalTitle,
	});
	// Publication is authoritative; client activation belongs to the initiating intent owner.
	return {
		...created,
		state: {
			...created.state,
			activeLocation: state.activeLocation,
			expandedProjectIds: state.expandedProjectIds,
		},
	};
}

function reorderItems<Item extends { readonly id: string }>(
	items: readonly Item[],
	itemId: string,
	beforeItemId: string | null,
): readonly Item[] | null {
	const sourceIndex = items.findIndex((item) => item.id === itemId);
	if (sourceIndex < 0) return null;
	if (beforeItemId === itemId) return items;
	if (beforeItemId !== null && !items.some((item) => item.id === beforeItemId)) return null;
	const sourceItem = items[sourceIndex];
	if (sourceItem === undefined) return null;
	const remainingItems = items.filter((item) => item.id !== itemId);
	const targetIndex =
		beforeItemId === null
			? remainingItems.length
			: remainingItems.findIndex((item) => item.id === beforeItemId);
	if (targetIndex < 0) return null;
	const reorderedItems = [...remainingItems];
	reorderedItems.splice(targetIndex, 0, sourceItem);
	return reorderedItems.every((item, index) => item === items[index]) ? items : reorderedItems;
}

function areValidLaunchConfigurations(
	launchConfigurations: readonly WorkbenchLaunchConfigurationReference[],
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

function directoriesEqual(left: WorkbenchDirectory, right: WorkbenchDirectory): boolean {
	return (
		left.available === right.available &&
		left.git?.branch === right.git?.branch &&
		left.git?.head === right.git?.head &&
		left.git?.repositoryId === right.git?.repositoryId &&
		left.git?.worktree === right.git?.worktree &&
		left.id === right.id &&
		left.kind === right.kind &&
		left.name === right.name &&
		left.path === right.path &&
		directoryPlacementsEqual(left.placement, right.placement) &&
		left.source === right.source
	);
}

function hasLaunchConfiguration(
	launchConfigurations: readonly WorkbenchLaunchConfigurationReference[],
	launchConfigurationId: string | null,
): boolean {
	return (
		launchConfigurationId === null ||
		launchConfigurations.some(
			(launchConfiguration) => launchConfiguration.id === launchConfigurationId,
		)
	);
}

function isValidDirectoryDescriptor(directory: WorkbenchDirectory): boolean {
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

function isValidRemoteProjectDirectory(
	directory: WorkbenchFolderDirectory,
): directory is WorkbenchFolderDirectory & {
	readonly placement: Extract<WorkbenchDirectoryPlacement, { kind: 'environment' }>;
} {
	return (
		isValidDirectoryDescriptor(directory) &&
		directory.placement.kind === 'environment' &&
		!directory.available &&
		directory.git === null &&
		directory.source === 'selected'
	);
}

function isUsableLocalDirectory(directory: WorkbenchDirectory): boolean {
	return directory.available && directory.placement.kind === 'local';
}

function directoryPlacementsEqual(
	left: WorkbenchDirectoryPlacement,
	right: WorkbenchDirectoryPlacement,
): boolean {
	return (
		left.kind === right.kind &&
		(left.kind === 'local' ||
			(right.kind === 'environment' && left.environmentId === right.environmentId))
	);
}

function directoriesShareLocation(left: WorkbenchDirectory, right: WorkbenchDirectory): boolean {
	return left.path === right.path && directoryPlacementsEqual(left.placement, right.placement);
}

function launchConfigurationsEqual(
	left: readonly WorkbenchLaunchConfigurationReference[],
	right: readonly WorkbenchLaunchConfigurationReference[],
): boolean {
	return (
		left.length === right.length &&
		left.every(
			(launchConfiguration, index) =>
				launchConfiguration.id === right[index]?.id &&
				launchConfiguration.name === right[index]?.name,
		)
	);
}

function requireDirectoryFallback(
	directories: readonly WorkbenchDirectory[],
	removedIds: ReadonlySet<string>,
	removedDirectoryId: string,
): string {
	const fallback = findDirectoryFallback(directories, removedIds, removedDirectoryId);
	if (fallback !== null) return fallback;
	throw new Error('Directory fallback requires one surviving Project Directory.');
}

function findDirectoryFallback(
	directories: readonly WorkbenchDirectory[],
	removedIds: ReadonlySet<string>,
	removedDirectoryId: string,
	accept: (directory: WorkbenchDirectory) => boolean = () => true,
): string | null {
	const removedIndex = directories.findIndex((directory) => directory.id === removedDirectoryId);
	if (removedIndex === -1) {
		throw new Error('A removed primary Directory was not present in its Project.');
	}
	for (let offset = 1; offset <= directories.length; offset += 1) {
		const candidate = directories[(removedIndex + offset) % directories.length];
		if (candidate !== undefined && !removedIds.has(candidate.id) && accept(candidate)) {
			return candidate.id;
		}
	}
	return null;
}

function includeExpandedProject(
	state: ProjectWorkspaceState,
	projectId: string,
): readonly string[] {
	return state.expandedProjectIds.includes(projectId)
		? state.expandedProjectIds
		: [...state.expandedProjectIds, projectId];
}

function applied(
	state: ProjectWorkspaceState,
	commands: readonly TerminalContextCommand[] = [],
): ProjectWorkspaceTransition {
	return { commands, outcome: 'applied', rejection: null, state };
}

function unchanged(state: ProjectWorkspaceState): ProjectWorkspaceTransition {
	return { commands: [], outcome: 'unchanged', rejection: null, state };
}

function rejected(
	state: ProjectWorkspaceState,
	rejection: ProjectWorkspaceRejection,
): ProjectWorkspaceTransition {
	return { commands: [], outcome: 'rejected', rejection, state };
}

export function assertProjectWorkspaceInvariants(state: ProjectWorkspaceState): void {
	const projectIds = new Set<string>();
	for (const project of state.projects) {
		if (projectIds.has(project.id)) throw new Error(`Duplicate Project ID ${project.id}.`);
		projectIds.add(project.id);
	}
	const scopes = new Map<string, WorkbenchResourceScope>();
	for (const scope of state.resourceScopes) {
		if (scopes.has(scope.id)) throw new Error(`Duplicate resource scope ID ${scope.id}.`);
		scopes.set(scope.id, scope);
		if (scope.directories.length === 0)
			throw new Error(`Resource scope ${scope.id} has no Directory.`);
		const ids = new Set<string>();
		const locations: WorkbenchDirectory[] = [];
		for (const directory of scope.directories) {
			if (!isValidDirectoryDescriptor(directory))
				throw new Error(`Resource scope ${scope.id} has an invalid Directory ${directory.id}.`);
			if (ids.has(directory.id))
				throw new Error(`Resource scope ${scope.id} has duplicate Directory ID ${directory.id}.`);
			if (locations.some((candidate) => directoriesShareLocation(candidate, directory)))
				throw new Error(
					`Resource scope ${scope.id} has duplicate Directory path ${directory.path}.`,
				);
			ids.add(directory.id);
			locations.push(directory);
		}
		if (!ids.has(scope.primaryDirectoryId))
			throw new Error(`Resource scope ${scope.id} has a missing primary Directory.`);
		if (!areValidLaunchConfigurations(scope.launchConfigurations))
			throw new Error(`Resource scope ${scope.id} has invalid launch configurations.`);
	}
	const workspaceIds = new Set<string>();
	const contextIds = new Set<string>([state.standaloneTerminalContextId]);
	for (const workspace of state.workspaces) {
		if (workspaceIds.has(workspace.id)) throw new Error(`Duplicate Workspace ID ${workspace.id}.`);
		workspaceIds.add(workspace.id);
		if (workspace.projectId !== null && !projectIds.has(workspace.projectId))
			throw new Error(`Workspace ${workspace.id} has a missing Project.`);
		const scope = scopes.get(workspace.resourceScopeId);
		if (scope === undefined)
			throw new Error(`Workspace ${workspace.id} has a missing resource scope.`);
		if (!scope.directories.some((directory) => directory.id === workspace.primaryDirectoryId))
			throw new Error(`Workspace ${workspace.id} has a missing primary Directory.`);
		if (!hasLaunchConfiguration(scope.launchConfigurations, workspace.launchConfigurationId))
			throw new Error(`Workspace ${workspace.id} has a missing launch configuration.`);
		if (contextIds.has(workspace.terminalContextId))
			throw new Error(`Duplicate terminal-context ID ${workspace.terminalContextId}.`);
		contextIds.add(workspace.terminalContextId);
	}
	const expanded = new Set<string>();
	for (const id of state.expandedProjectIds) {
		if (!projectIds.has(id) || expanded.has(id))
			throw new Error(`Expanded Project ${id} is missing or duplicated.`);
		expanded.add(id);
	}
	if (
		state.activeLocation.kind === 'workspace' &&
		!workspaceIds.has(state.activeLocation.workspaceId)
	)
		throw new Error('The active Workspace location does not exist.');
}

function availableResourceScopeSequence(state: ProjectWorkspaceState): number {
	return nextUnusedSequence(
		'resource-scope',
		state.nextResourceScopeSequence,
		state.resourceScopes.map((scope) => scope.id),
	);
}

function availableProjectSequence(state: ProjectWorkspaceState): number {
	return nextUnusedSequence(
		'project',
		state.nextProjectSequence,
		state.projects.map((project) => project.id),
	);
}

function availableWorkspaceSequence(state: ProjectWorkspaceState): number {
	return nextUnusedSequence(
		'workspace',
		state.nextWorkspaceSequence,
		state.workspaces.map((workspace) => workspace.id),
	);
}

function availableTerminalContextSequence(state: ProjectWorkspaceState): number {
	return nextUnusedSequence('terminal-context', state.nextTerminalContextSequence, [
		state.standaloneTerminalContextId,
		...state.workspaces.map((workspace) => workspace.terminalContextId),
	]);
}

function nextUnusedSequence(prefix: string, initial: number, ids: readonly string[]): number {
	const occupied = new Set(ids);
	let sequence = initial;
	while (occupied.has(`${prefix}-${sequence}`)) sequence += 1;
	return sequence;
}
