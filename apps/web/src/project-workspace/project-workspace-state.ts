import { assertWorkbenchSharedProjectWorkspaceInvariants } from '@orchbee/workbench-client-protocol';

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
interface WorkbenchResourceScope {
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
	/**
	 * Whether Recents, the group of Workspaces outside any Project, shows them. Like a Project, it
	 * opens when one of its Workspaces is activated or created here.
	 */
	readonly recentsExpanded: boolean;
	readonly resourceScopes: readonly WorkbenchResourceScope[];
	readonly standaloneTerminalContextId: string;
	readonly workspaces: readonly WorkbenchWorkspace[];
}

/** What this Client changes on its own: which location it shows and which groups it expands. */
export type ProjectWorkspaceEvent =
	| { readonly type: 'project-expansion-toggled'; readonly projectId: string }
	| { readonly type: 'recents-expansion-toggled' }
	| { readonly type: 'standalone-activated' }
	| { readonly type: 'workspace-activated'; readonly workspaceId: string };

type ProjectWorkspaceRejection = 'project-not-found' | 'workspace-not-found';

export interface ProjectWorkspaceTransition {
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
		recentsExpanded: true,
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
		case 'project-expansion-toggled':
			if (!hasProject(state, event.projectId)) return rejected(state, 'project-not-found');
			return applied({
				...state,
				expandedProjectIds: state.expandedProjectIds.includes(event.projectId)
					? state.expandedProjectIds.filter((id) => id !== event.projectId)
					: [...state.expandedProjectIds, event.projectId],
			});
		case 'recents-expansion-toggled':
			return applied({ ...state, recentsExpanded: !state.recentsExpanded });
		case 'standalone-activated':
			return state.activeLocation.kind === 'standalone'
				? unchanged(state)
				: applied({ ...state, activeLocation: { kind: 'standalone' } });
		case 'workspace-activated':
			return activateWorkspace(state, event.workspaceId);
	}
}

function hasProject(state: ProjectWorkspaceState, projectId: string | null): boolean {
	return projectId === null || state.projects.some((project) => project.id === projectId);
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
	const recentsExpanded = state.recentsExpanded || workspace.projectId === null;
	if (
		state.activeLocation.kind === 'workspace' &&
		state.activeLocation.workspaceId === workspaceId &&
		expandedProjectIds === state.expandedProjectIds &&
		recentsExpanded === state.recentsExpanded
	)
		return unchanged(state);
	return applied({
		...state,
		activeLocation: { kind: 'workspace', workspaceId },
		expandedProjectIds,
		recentsExpanded,
	});
}

function includeExpandedProject(
	state: ProjectWorkspaceState,
	projectId: string,
): readonly string[] {
	return state.expandedProjectIds.includes(projectId)
		? state.expandedProjectIds
		: [...state.expandedProjectIds, projectId];
}

function applied(state: ProjectWorkspaceState): ProjectWorkspaceTransition {
	return { outcome: 'applied', rejection: null, state };
}

function unchanged(state: ProjectWorkspaceState): ProjectWorkspaceTransition {
	return { outcome: 'unchanged', rejection: null, state };
}

function rejected(
	state: ProjectWorkspaceState,
	rejection: ProjectWorkspaceRejection,
): ProjectWorkspaceTransition {
	return { outcome: 'rejected', rejection, state };
}

function assertProjectWorkspaceInvariants(state: ProjectWorkspaceState): void {
	assertWorkbenchSharedProjectWorkspaceInvariants(state);
	const projectIds = new Set(state.projects.map((project) => project.id));
	const workspaceIds = new Set(state.workspaces.map((workspace) => workspace.id));
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
