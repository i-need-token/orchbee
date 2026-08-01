export type WorkbenchDirectorySource = 'created' | 'discovered' | 'selected';

interface WorkbenchDirectoryGitMetadataBase {
	readonly branch: string | null;
	readonly head: string;
	readonly repositoryId: string;
}

export interface WorkbenchRepositoryDirectoryGitMetadata extends WorkbenchDirectoryGitMetadataBase {
	readonly worktree: false;
}

export interface WorkbenchWorktreeDirectoryGitMetadata extends WorkbenchDirectoryGitMetadataBase {
	readonly worktree: true;
}

export type WorkbenchDirectoryGitMetadata =
	| WorkbenchRepositoryDirectoryGitMetadata
	| WorkbenchWorktreeDirectoryGitMetadata;

interface WorkbenchDirectoryBase {
	readonly available: boolean;
	readonly id: string;
	readonly name: string;
	readonly path: string;
	readonly source: WorkbenchDirectorySource;
}

export interface WorkbenchFolderDirectory extends WorkbenchDirectoryBase {
	readonly git: WorkbenchRepositoryDirectoryGitMetadata | null;
	readonly kind: 'folder';
}

export interface WorkbenchWorktreeDirectory extends WorkbenchDirectoryBase {
	readonly git: WorkbenchWorktreeDirectoryGitMetadata;
	readonly kind: 'worktree';
}

export type WorkbenchDirectory = WorkbenchFolderDirectory | WorkbenchWorktreeDirectory;

export interface WorkbenchLaunchConfigurationReference {
	readonly id: string;
	readonly name: string;
}

export interface WorkbenchWorkspace {
	readonly id: string;
	readonly launchConfigurationId: string | null;
	readonly name: string;
	readonly primaryDirectoryId: string;
	readonly terminalContextId: string;
}

export interface WorkbenchProject {
	readonly directories: readonly WorkbenchDirectory[];
	readonly id: string;
	readonly launchConfigurations: readonly WorkbenchLaunchConfigurationReference[];
	readonly name: string;
	readonly primaryDirectoryId: string;
	readonly workspaces: readonly WorkbenchWorkspace[];
}

export type WorkbenchActiveLocation =
	| { readonly kind: 'standalone' }
	| {
			readonly kind: 'workspace';
			readonly projectId: string;
			readonly workspaceId: string;
	  };

export interface ProjectWorkspaceState {
	readonly activeLocation: WorkbenchActiveLocation;
	readonly expandedProjectIds: readonly string[];
	readonly nextProjectSequence: number;
	readonly nextTerminalContextSequence: number;
	readonly nextWorkspaceSequence: number;
	readonly projects: readonly WorkbenchProject[];
	readonly standaloneTerminalContextId: string;
}

export type ProjectWorkspaceEvent =
	| {
			readonly directory: WorkbenchDirectory;
			readonly launchConfigurations: readonly WorkbenchLaunchConfigurationReference[];
			readonly primaryWorkspaceName: string;
			readonly primaryWorkspaceLaunchConfigurationId: string | null;
			readonly projectName: string;
			readonly terminalTitle: string;
			readonly type: 'project-created';
	  }
	| {
			readonly projectId: string;
			readonly type: 'project-expansion-toggled';
	  }
	| {
			readonly beforeProjectId: string | null;
			readonly projectId: string;
			readonly type: 'project-reordered';
	  }
	| {
			readonly projectId: string;
			readonly type: 'project-renamed';
			readonly name: string;
	  }
	| {
			readonly directory: WorkbenchDirectory;
			readonly projectId: string;
			readonly type: 'directory-added';
	  }
	| {
			readonly directory: WorkbenchDirectory;
			readonly projectId: string;
			readonly type: 'directory-updated';
	  }
	| {
			readonly directoryIds: readonly string[];
			readonly projectId: string;
			readonly type: 'directories-removed';
	  }
	| {
			readonly launchConfigurations: readonly WorkbenchLaunchConfigurationReference[];
			readonly projectId: string;
			readonly type: 'project-launch-configurations-reconciled';
	  }
	| {
			readonly directoryId: string;
			readonly projectId: string;
			readonly type: 'project-primary-directory-set';
	  }
	| {
			readonly type: 'standalone-activated';
	  }
	| {
			readonly projectId: string;
			readonly type: 'workspace-activated';
			readonly workspaceId: string;
	  }
	| {
			readonly beforeWorkspaceId: string | null;
			readonly projectId: string;
			readonly type: 'workspace-reordered';
			readonly workspaceId: string;
	  }
	| {
			readonly directoryId: string;
			readonly launchConfigurationId: string | null;
			readonly name: string;
			readonly projectId: string;
			readonly terminalTitle: string;
			readonly type: 'workspace-created';
	  }
	| {
			readonly copyName: string;
			readonly projectId: string;
			readonly type: 'workspace-duplicated';
			readonly workspaceId: string;
	  }
	| {
			readonly launchConfigurationId: string | null;
			readonly name: string;
			readonly primaryDirectoryId: string;
			readonly projectId: string;
			readonly type: 'workspace-updated';
			readonly workspaceId: string;
	  }
	| {
			readonly projectId: string;
			readonly type: 'workspace-deleted';
			readonly workspaceId: string;
	  }
	| {
			readonly projectId: string;
			readonly type: 'project-removed';
	  };

export type ProjectWorkspaceRejection =
	| 'directory-already-exists'
	| 'directory-invalid'
	| 'directory-not-found'
	| 'directory-removal-not-allowed'
	| 'directory-unavailable'
	| 'directory-update-not-allowed'
	| 'last-directory'
	| 'launch-configuration-invalid'
	| 'launch-configuration-not-found'
	| 'project-not-found'
	| 'workspace-not-found';

export type TerminalContextCommand =
	| {
			readonly cwd: string;
			readonly terminalContextId: string;
			readonly terminalTitle: string;
			readonly type: 'terminal-context-created';
	  }
	| {
			readonly sourceTerminalContextId: string;
			readonly targetTerminalContextId: string;
			readonly type: 'terminal-context-cloned';
	  }
	| {
			readonly terminalContextId: string;
			readonly type: 'terminal-context-disposed';
	  };

export interface ProjectWorkspaceTransition {
	readonly commands: readonly TerminalContextCommand[];
	readonly outcome: 'applied' | 'rejected' | 'unchanged';
	readonly rejection: ProjectWorkspaceRejection | null;
	readonly state: ProjectWorkspaceState;
}

interface ProjectUpdate {
	readonly project: WorkbenchProject;
	readonly state: ProjectWorkspaceState;
}

export function createProjectWorkspaceState(): ProjectWorkspaceState {
	return {
		activeLocation: { kind: 'standalone' },
		expandedProjectIds: [],
		nextProjectSequence: 1,
		nextTerminalContextSequence: 2,
		nextWorkspaceSequence: 1,
		projects: [],
		standaloneTerminalContextId: 'terminal-context-1',
	};
}

export function transitionProjectWorkspace(
	state: ProjectWorkspaceState,
	event: ProjectWorkspaceEvent,
): ProjectWorkspaceTransition {
	let result: ProjectWorkspaceTransition;
	switch (event.type) {
		case 'project-created':
			result = createProject(state, event);
			break;
		case 'project-expansion-toggled':
			result = toggleProjectExpansion(state, event.projectId);
			break;
		case 'project-reordered':
			result = reorderProject(state, event.projectId, event.beforeProjectId);
			break;
		case 'project-renamed':
			result = updateProjectName(state, event.projectId, event.name);
			break;
		case 'directory-added':
			result = addDirectory(state, event.projectId, event.directory);
			break;
		case 'directory-updated':
			result = updateDirectory(state, event.projectId, event.directory);
			break;
		case 'directories-removed':
			result = removeDirectories(state, event.projectId, event.directoryIds);
			break;
		case 'project-launch-configurations-reconciled':
			result = reconcileProjectLaunchConfigurations(
				state,
				event.projectId,
				event.launchConfigurations,
			);
			break;
		case 'project-primary-directory-set':
			result = setProjectPrimaryDirectory(state, event.projectId, event.directoryId);
			break;
		case 'standalone-activated':
			result =
				state.activeLocation.kind === 'standalone'
					? unchanged(state)
					: applied({ ...state, activeLocation: { kind: 'standalone' } });
			break;
		case 'workspace-activated':
			result = activateWorkspace(state, event.projectId, event.workspaceId);
			break;
		case 'workspace-reordered':
			result = reorderWorkspace(state, event.projectId, event.workspaceId, event.beforeWorkspaceId);
			break;
		case 'workspace-created':
			result = createWorkspace(state, event);
			break;
		case 'workspace-duplicated':
			result = duplicateWorkspace(state, event);
			break;
		case 'workspace-updated':
			result = updateWorkspace(state, event);
			break;
		case 'workspace-deleted':
			result = deleteWorkspace(state, event.projectId, event.workspaceId);
			break;
		case 'project-removed':
			result = removeProject(state, event.projectId);
			break;
	}
	assertProjectWorkspaceInvariants(result.state);
	return result;
}

function createProject(
	state: ProjectWorkspaceState,
	event: Extract<ProjectWorkspaceEvent, { type: 'project-created' }>,
): ProjectWorkspaceTransition {
	if (!isValidDirectoryDescriptor(event.directory)) {
		return rejected(state, 'directory-invalid');
	}
	if (!event.directory.available) {
		return rejected(state, 'directory-unavailable');
	}
	if (!areValidLaunchConfigurations(event.launchConfigurations)) {
		return rejected(state, 'launch-configuration-invalid');
	}
	if (
		!hasLaunchConfiguration(event.launchConfigurations, event.primaryWorkspaceLaunchConfigurationId)
	) {
		return rejected(state, 'launch-configuration-not-found');
	}
	const projectId = `project-${state.nextProjectSequence}`;
	const workspaceId = `workspace-${state.nextWorkspaceSequence}`;
	const terminalContextId = `terminal-context-${state.nextTerminalContextSequence}`;
	const workspace: WorkbenchWorkspace = {
		id: workspaceId,
		launchConfigurationId: event.primaryWorkspaceLaunchConfigurationId,
		name: event.primaryWorkspaceName,
		primaryDirectoryId: event.directory.id,
		terminalContextId,
	};
	const project: WorkbenchProject = {
		directories: [event.directory],
		id: projectId,
		launchConfigurations: [...event.launchConfigurations],
		name: event.projectName,
		primaryDirectoryId: event.directory.id,
		workspaces: [workspace],
	};
	return applied(
		{
			...state,
			activeLocation: { kind: 'workspace', projectId, workspaceId },
			expandedProjectIds: [...state.expandedProjectIds, projectId],
			nextProjectSequence: state.nextProjectSequence + 1,
			nextTerminalContextSequence: state.nextTerminalContextSequence + 1,
			nextWorkspaceSequence: state.nextWorkspaceSequence + 1,
			projects: [...state.projects, project],
		},
		[
			{
				cwd: event.directory.path,
				terminalContextId,
				terminalTitle: event.terminalTitle,
				type: 'terminal-context-created',
			},
		],
	);
}

function toggleProjectExpansion(
	state: ProjectWorkspaceState,
	projectId: string,
): ProjectWorkspaceTransition {
	if (findProject(state, projectId) === null) return rejected(state, 'project-not-found');
	const expandedProjectIds = state.expandedProjectIds.includes(projectId)
		? state.expandedProjectIds.filter((id) => id !== projectId)
		: [...state.expandedProjectIds, projectId];
	return applied({ ...state, expandedProjectIds });
}

function reorderProject(
	state: ProjectWorkspaceState,
	projectId: string,
	beforeProjectId: string | null,
): ProjectWorkspaceTransition {
	const projects = reorderProjects(state.projects, projectId, beforeProjectId);
	if (projects === null) return rejected(state, 'project-not-found');
	return projects === state.projects ? unchanged(state) : applied({ ...state, projects });
}

function reorderProjects(
	projects: readonly WorkbenchProject[],
	projectId: string,
	beforeProjectId: string | null,
): readonly WorkbenchProject[] | null {
	return reorderItems(projects, projectId, beforeProjectId);
}

function updateProjectName(
	state: ProjectWorkspaceState,
	projectId: string,
	name: string,
): ProjectWorkspaceTransition {
	const update = updateKnownProject(state, projectId, (project) =>
		project.name === name ? project : { ...project, name },
	);
	if (update === null) return rejected(state, 'project-not-found');
	return update.state === state ? unchanged(state) : applied(update.state);
}

function addDirectory(
	state: ProjectWorkspaceState,
	projectId: string,
	directory: WorkbenchDirectory,
): ProjectWorkspaceTransition {
	const project = findProject(state, projectId);
	if (project === null) return rejected(state, 'project-not-found');
	if (!isValidDirectoryDescriptor(directory)) return rejected(state, 'directory-invalid');
	if (
		project.directories.some(
			(candidate) => candidate.id === directory.id || candidate.path === directory.path,
		)
	) {
		return rejected(state, 'directory-already-exists');
	}
	const update = requireProjectUpdate(
		updateKnownProject(state, projectId, (currentProject) => ({
			...currentProject,
			directories: [...currentProject.directories, directory],
		})),
	);
	return applied(update.state);
}

function updateDirectory(
	state: ProjectWorkspaceState,
	projectId: string,
	directory: WorkbenchDirectory,
): ProjectWorkspaceTransition {
	const project = findProject(state, projectId);
	if (project === null) return rejected(state, 'project-not-found');
	if (!isValidDirectoryDescriptor(directory)) return rejected(state, 'directory-invalid');
	const currentDirectory = findDirectory(project, directory.id);
	if (currentDirectory === null) return rejected(state, 'directory-not-found');
	if (currentDirectory.kind !== directory.kind || currentDirectory.source !== directory.source) {
		return rejected(state, 'directory-update-not-allowed');
	}
	if (
		project.directories.some(
			(candidate) => candidate.id !== directory.id && candidate.path === directory.path,
		)
	) {
		return rejected(state, 'directory-already-exists');
	}
	if (directoriesEqual(currentDirectory, directory)) return unchanged(state);
	const update = requireProjectUpdate(
		updateKnownProject(state, projectId, (currentProject) => ({
			...currentProject,
			directories: currentProject.directories.map((candidate) =>
				candidate.id === directory.id ? directory : candidate,
			),
		})),
	);
	return applied(update.state);
}

function removeDirectories(
	state: ProjectWorkspaceState,
	projectId: string,
	directoryIds: readonly string[],
): ProjectWorkspaceTransition {
	const project = findProject(state, projectId);
	if (project === null) return rejected(state, 'project-not-found');
	const removedIds = new Set(directoryIds);
	if (removedIds.size === 0) return unchanged(state);
	const removedDirectories = project.directories.filter((directory) =>
		removedIds.has(directory.id),
	);
	if (removedDirectories.length !== removedIds.size) return rejected(state, 'directory-not-found');
	if (removedDirectories.some((directory) => directory.kind === 'worktree')) {
		return rejected(state, 'directory-removal-not-allowed');
	}
	const directories = project.directories.filter((directory) => !removedIds.has(directory.id));
	if (directories.length === 0) return rejected(state, 'last-directory');
	const nextPrimaryDirectoryId = removedIds.has(project.primaryDirectoryId)
		? requireDirectoryFallback(project.directories, removedIds, project.primaryDirectoryId)
		: project.primaryDirectoryId;
	const workspaces: WorkbenchWorkspace[] = [];
	for (const workspace of project.workspaces) {
		workspaces.push(
			removedIds.has(workspace.primaryDirectoryId)
				? {
						...workspace,
						primaryDirectoryId: requireDirectoryFallback(
							project.directories,
							removedIds,
							workspace.primaryDirectoryId,
						),
					}
				: workspace,
		);
	}
	const update = requireProjectUpdate(
		updateKnownProject(state, projectId, (currentProject) => ({
			...currentProject,
			directories,
			primaryDirectoryId: nextPrimaryDirectoryId,
			workspaces,
		})),
	);
	return applied(update.state);
}

function reconcileProjectLaunchConfigurations(
	state: ProjectWorkspaceState,
	projectId: string,
	launchConfigurations: readonly WorkbenchLaunchConfigurationReference[],
): ProjectWorkspaceTransition {
	const project = findProject(state, projectId);
	if (project === null) return rejected(state, 'project-not-found');
	if (!areValidLaunchConfigurations(launchConfigurations)) {
		return rejected(state, 'launch-configuration-invalid');
	}
	if (launchConfigurationsEqual(project.launchConfigurations, launchConfigurations)) {
		return unchanged(state);
	}
	const launchConfigurationIds = new Set(
		launchConfigurations.map((launchConfiguration) => launchConfiguration.id),
	);
	const update = requireProjectUpdate(
		updateKnownProject(state, projectId, (currentProject) => ({
			...currentProject,
			launchConfigurations: [...launchConfigurations],
			workspaces: currentProject.workspaces.map((workspace) =>
				workspace.launchConfigurationId === null ||
				launchConfigurationIds.has(workspace.launchConfigurationId)
					? workspace
					: { ...workspace, launchConfigurationId: null },
			),
		})),
	);
	return applied(update.state);
}

function setProjectPrimaryDirectory(
	state: ProjectWorkspaceState,
	projectId: string,
	directoryId: string,
): ProjectWorkspaceTransition {
	const project = findProject(state, projectId);
	if (project === null) return rejected(state, 'project-not-found');
	const directory = findDirectory(project, directoryId);
	if (directory === null) return rejected(state, 'directory-not-found');
	if (!directory.available) return rejected(state, 'directory-unavailable');
	if (project.primaryDirectoryId === directoryId) return unchanged(state);
	const update = requireProjectUpdate(
		updateKnownProject(state, projectId, (currentProject) => ({
			...currentProject,
			primaryDirectoryId: directoryId,
		})),
	);
	return applied(update.state);
}

function activateWorkspace(
	state: ProjectWorkspaceState,
	projectId: string,
	workspaceId: string,
): ProjectWorkspaceTransition {
	const project = findProject(state, projectId);
	if (project === null) return rejected(state, 'project-not-found');
	if (findWorkspace(project, workspaceId) === null) return rejected(state, 'workspace-not-found');
	const nextLocation: WorkbenchActiveLocation = { kind: 'workspace', projectId, workspaceId };
	const expandedProjectIds = state.expandedProjectIds.includes(projectId)
		? state.expandedProjectIds
		: [...state.expandedProjectIds, projectId];
	if (
		state.activeLocation.kind === 'workspace' &&
		state.activeLocation.projectId === projectId &&
		state.activeLocation.workspaceId === workspaceId &&
		expandedProjectIds === state.expandedProjectIds
	) {
		return unchanged(state);
	}
	return applied({ ...state, activeLocation: nextLocation, expandedProjectIds });
}

function reorderWorkspace(
	state: ProjectWorkspaceState,
	projectId: string,
	workspaceId: string,
	beforeWorkspaceId: string | null,
): ProjectWorkspaceTransition {
	const project = findProject(state, projectId);
	if (project === null) return rejected(state, 'project-not-found');
	const workspaces = reorderItems(project.workspaces, workspaceId, beforeWorkspaceId);
	if (workspaces === null) return rejected(state, 'workspace-not-found');
	if (workspaces === project.workspaces) return unchanged(state);
	const update = requireProjectUpdate(
		updateKnownProject(state, projectId, (currentProject) => ({
			...currentProject,
			workspaces,
		})),
	);
	return applied(update.state);
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

function createWorkspace(
	state: ProjectWorkspaceState,
	event: Extract<ProjectWorkspaceEvent, { type: 'workspace-created' }>,
): ProjectWorkspaceTransition {
	const project = findProject(state, event.projectId);
	if (project === null) return rejected(state, 'project-not-found');
	const directory = findDirectory(project, event.directoryId);
	if (directory === null) return rejected(state, 'directory-not-found');
	if (!directory.available) return rejected(state, 'directory-unavailable');
	if (!hasLaunchConfiguration(project.launchConfigurations, event.launchConfigurationId)) {
		return rejected(state, 'launch-configuration-not-found');
	}
	const workspaceId = `workspace-${state.nextWorkspaceSequence}`;
	const terminalContextId = `terminal-context-${state.nextTerminalContextSequence}`;
	const workspace: WorkbenchWorkspace = {
		id: workspaceId,
		launchConfigurationId: event.launchConfigurationId,
		name: event.name,
		primaryDirectoryId: directory.id,
		terminalContextId,
	};
	const update = requireProjectUpdate(
		updateKnownProject(state, event.projectId, (currentProject) => ({
			...currentProject,
			workspaces: [...currentProject.workspaces, workspace],
		})),
	);
	return applied(
		{
			...update.state,
			activeLocation: {
				kind: 'workspace',
				projectId: event.projectId,
				workspaceId,
			},
			expandedProjectIds: includeExpandedProject(update.state, event.projectId),
			nextTerminalContextSequence: state.nextTerminalContextSequence + 1,
			nextWorkspaceSequence: state.nextWorkspaceSequence + 1,
		},
		[
			{
				cwd: directory.path,
				terminalContextId,
				terminalTitle: event.terminalTitle,
				type: 'terminal-context-created',
			},
		],
	);
}

function duplicateWorkspace(
	state: ProjectWorkspaceState,
	event: Extract<ProjectWorkspaceEvent, { type: 'workspace-duplicated' }>,
): ProjectWorkspaceTransition {
	const project = findProject(state, event.projectId);
	if (project === null) return rejected(state, 'project-not-found');
	const sourceWorkspace = findWorkspace(project, event.workspaceId);
	if (sourceWorkspace === null) return rejected(state, 'workspace-not-found');
	const workspaceId = `workspace-${state.nextWorkspaceSequence}`;
	const terminalContextId = `terminal-context-${state.nextTerminalContextSequence}`;
	const workspace: WorkbenchWorkspace = {
		id: workspaceId,
		launchConfigurationId: sourceWorkspace.launchConfigurationId,
		name: event.copyName,
		primaryDirectoryId: sourceWorkspace.primaryDirectoryId,
		terminalContextId,
	};
	const update = requireProjectUpdate(
		updateKnownProject(state, event.projectId, (currentProject) => ({
			...currentProject,
			workspaces: [...currentProject.workspaces, workspace],
		})),
	);
	return applied(
		{
			...update.state,
			activeLocation: {
				kind: 'workspace',
				projectId: event.projectId,
				workspaceId,
			},
			expandedProjectIds: includeExpandedProject(update.state, event.projectId),
			nextTerminalContextSequence: state.nextTerminalContextSequence + 1,
			nextWorkspaceSequence: state.nextWorkspaceSequence + 1,
		},
		[
			{
				sourceTerminalContextId: sourceWorkspace.terminalContextId,
				targetTerminalContextId: terminalContextId,
				type: 'terminal-context-cloned',
			},
		],
	);
}

function updateWorkspace(
	state: ProjectWorkspaceState,
	event: Extract<ProjectWorkspaceEvent, { type: 'workspace-updated' }>,
): ProjectWorkspaceTransition {
	const project = findProject(state, event.projectId);
	if (project === null) return rejected(state, 'project-not-found');
	const workspace = findWorkspace(project, event.workspaceId);
	if (workspace === null) return rejected(state, 'workspace-not-found');
	const directory = findDirectory(project, event.primaryDirectoryId);
	if (directory === null) return rejected(state, 'directory-not-found');
	if (!directory.available) return rejected(state, 'directory-unavailable');
	if (!hasLaunchConfiguration(project.launchConfigurations, event.launchConfigurationId)) {
		return rejected(state, 'launch-configuration-not-found');
	}
	if (
		workspace.launchConfigurationId === event.launchConfigurationId &&
		workspace.name === event.name &&
		workspace.primaryDirectoryId === event.primaryDirectoryId
	) {
		return unchanged(state);
	}
	const update = requireProjectUpdate(
		updateKnownProject(state, event.projectId, (currentProject) => ({
			...currentProject,
			workspaces: currentProject.workspaces.map((candidate) =>
				candidate.id === event.workspaceId
					? {
							...candidate,
							launchConfigurationId: event.launchConfigurationId,
							name: event.name,
							primaryDirectoryId: event.primaryDirectoryId,
						}
					: candidate,
			),
		})),
	);
	return applied(update.state);
}

function deleteWorkspace(
	state: ProjectWorkspaceState,
	projectId: string,
	workspaceId: string,
): ProjectWorkspaceTransition {
	const project = findProject(state, projectId);
	if (project === null) return rejected(state, 'project-not-found');
	const workspace = findWorkspace(project, workspaceId);
	if (workspace === null) return rejected(state, 'workspace-not-found');
	const update = requireProjectUpdate(
		updateKnownProject(state, projectId, (currentProject) => ({
			...currentProject,
			workspaces: currentProject.workspaces.filter((candidate) => candidate.id !== workspaceId),
		})),
	);
	const deletesActiveWorkspace =
		state.activeLocation.kind === 'workspace' &&
		state.activeLocation.projectId === projectId &&
		state.activeLocation.workspaceId === workspaceId;
	return applied(
		deletesActiveWorkspace
			? { ...update.state, activeLocation: { kind: 'standalone' } }
			: update.state,
		[
			{
				terminalContextId: workspace.terminalContextId,
				type: 'terminal-context-disposed',
			},
		],
	);
}

function removeProject(
	state: ProjectWorkspaceState,
	projectId: string,
): ProjectWorkspaceTransition {
	const project = findProject(state, projectId);
	if (project === null) return rejected(state, 'project-not-found');
	const removesActiveProject =
		state.activeLocation.kind === 'workspace' && state.activeLocation.projectId === projectId;
	return applied(
		{
			...state,
			activeLocation: removesActiveProject ? { kind: 'standalone' } : state.activeLocation,
			expandedProjectIds: state.expandedProjectIds.filter((id) => id !== projectId),
			projects: state.projects.filter((candidate) => candidate.id !== projectId),
		},
		project.workspaces.map((workspace) => ({
			terminalContextId: workspace.terminalContextId,
			type: 'terminal-context-disposed',
		})),
	);
}

function findProject(state: ProjectWorkspaceState, projectId: string): WorkbenchProject | null {
	return state.projects.find((project) => project.id === projectId) ?? null;
}

function findDirectory(project: WorkbenchProject, directoryId: string): WorkbenchDirectory | null {
	return project.directories.find((directory) => directory.id === directoryId) ?? null;
}

function findWorkspace(project: WorkbenchProject, workspaceId: string): WorkbenchWorkspace | null {
	return project.workspaces.find((workspace) => workspace.id === workspaceId) ?? null;
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
	return directory.kind === 'worktree'
		? directory.git?.worktree === true
		: directory.kind === 'folder' && (directory.git === null || directory.git?.worktree === false);
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

function updateKnownProject(
	state: ProjectWorkspaceState,
	projectId: string,
	update: (project: WorkbenchProject) => WorkbenchProject,
): ProjectUpdate | null {
	const projectIndex = state.projects.findIndex((project) => project.id === projectId);
	const project = state.projects[projectIndex];
	if (project === undefined) return null;
	const nextProject = update(project);
	if (nextProject === project) return { project, state };
	const projects = [...state.projects];
	projects[projectIndex] = nextProject;
	return { project: nextProject, state: { ...state, projects } };
}

function requireProjectUpdate(update: ProjectUpdate | null): ProjectUpdate {
	if (update === null) throw new Error('A known Project update unexpectedly lost its Project.');
	return update;
}

function requireDirectoryFallback(
	directories: readonly WorkbenchDirectory[],
	removedIds: ReadonlySet<string>,
	removedDirectoryId: string,
): string {
	const removedIndex = directories.findIndex((directory) => directory.id === removedDirectoryId);
	if (removedIndex === -1) {
		throw new Error('A removed primary Directory was not present in its Project.');
	}
	for (let offset = 1; offset <= directories.length; offset += 1) {
		const candidate = directories[(removedIndex + offset) % directories.length];
		if (candidate !== undefined && !removedIds.has(candidate.id)) return candidate.id;
	}
	throw new Error('Directory fallback requires one surviving Project Directory.');
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

function assertProjectWorkspaceInvariants(state: ProjectWorkspaceState): void {
	const projectIds = new Set<string>();
	const workspaceIds = new Set<string>();
	const terminalContextIds = new Set<string>([state.standaloneTerminalContextId]);
	for (const project of state.projects) {
		if (projectIds.has(project.id)) throw new Error(`Duplicate Project ID ${project.id}.`);
		projectIds.add(project.id);
		if (project.directories.length === 0) {
			throw new Error(`Project ${project.id} has no Directory.`);
		}
		const directoryIds = new Set<string>();
		const directoryPaths = new Set<string>();
		for (const directory of project.directories) {
			if (!isValidDirectoryDescriptor(directory)) {
				throw new Error(`Project ${project.id} has an invalid Directory ${directory.id}.`);
			}
			if (directoryIds.has(directory.id)) {
				throw new Error(`Project ${project.id} has duplicate Directory ID ${directory.id}.`);
			}
			if (directoryPaths.has(directory.path)) {
				throw new Error(`Project ${project.id} has duplicate Directory path ${directory.path}.`);
			}
			directoryIds.add(directory.id);
			directoryPaths.add(directory.path);
		}
		if (!directoryIds.has(project.primaryDirectoryId)) {
			throw new Error(`Project ${project.id} has a missing primary Directory.`);
		}
		if (!areValidLaunchConfigurations(project.launchConfigurations)) {
			throw new Error(`Project ${project.id} has invalid launch configurations.`);
		}
		for (const workspace of project.workspaces) {
			if (workspaceIds.has(workspace.id)) {
				throw new Error(`Duplicate Workspace ID ${workspace.id}.`);
			}
			if (!directoryIds.has(workspace.primaryDirectoryId)) {
				throw new Error(`Workspace ${workspace.id} has a missing primary Directory.`);
			}
			if (!hasLaunchConfiguration(project.launchConfigurations, workspace.launchConfigurationId)) {
				throw new Error(`Workspace ${workspace.id} has a missing launch configuration.`);
			}
			if (terminalContextIds.has(workspace.terminalContextId)) {
				throw new Error(`Duplicate terminal-context ID ${workspace.terminalContextId}.`);
			}
			workspaceIds.add(workspace.id);
			terminalContextIds.add(workspace.terminalContextId);
		}
	}
	for (const expandedProjectId of state.expandedProjectIds) {
		if (!projectIds.has(expandedProjectId)) {
			throw new Error(`Expanded Project ${expandedProjectId} does not exist.`);
		}
	}
	if (state.activeLocation.kind === 'workspace') {
		const project = findProject(state, state.activeLocation.projectId);
		if (project === null || findWorkspace(project, state.activeLocation.workspaceId) === null) {
			throw new Error('The active Workspace location does not exist.');
		}
	}
}
