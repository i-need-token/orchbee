/** The folder a Workspace records, where its new terminals start. */
export interface WorkbenchDirectory {
	readonly id: string;
	readonly path: string;
}

export interface WorkbenchWorkspace {
	/** Null while the Workspace records no folder, so no Tab can start in it. */
	readonly directory: WorkbenchDirectory | null;
	readonly id: string;
	readonly name: string;
	readonly projectId: string | null;
	readonly terminalContextId: string;
}

export interface WorkbenchProject {
	readonly id: string;
	readonly name: string;
}

/**
 * Where this Client is. As in the TUI, the no-Workspace location holds no terminal context, and this
 * Client is there only while no Workspace exists.
 */
export type WorkbenchActiveLocation =
	| { readonly kind: 'no-workspace' }
	| {
			readonly kind: 'workspace';
			readonly workspaceId: string;
	  };

export interface ProjectWorkspaceState {
	readonly activeLocation: WorkbenchActiveLocation;
	readonly expandedProjectIds: readonly string[];
	readonly projects: readonly WorkbenchProject[];
	/**
	 * Whether Recents, the group of Workspaces outside any Project, shows them. Like a Project, it
	 * opens when one of its Workspaces is activated or created here.
	 */
	readonly recentsExpanded: boolean;
	readonly workspaces: readonly WorkbenchWorkspace[];
}

/** What this Client changes on its own: which location it shows and which groups it expands. */
export type ProjectWorkspaceEvent =
	| { readonly type: 'project-expansion-toggled'; readonly projectId: string }
	| { readonly type: 'recents-expansion-toggled' }
	| { readonly type: 'workspace-activated'; readonly workspaceId: string };

type ProjectWorkspaceRejection = 'project-not-found' | 'workspace-not-found';

export interface ProjectWorkspaceTransition {
	readonly outcome: 'applied' | 'rejected' | 'unchanged';
	readonly rejection: ProjectWorkspaceRejection | null;
	readonly state: ProjectWorkspaceState;
}

export function createProjectWorkspaceState(): ProjectWorkspaceState {
	return {
		activeLocation: { kind: 'no-workspace' },
		expandedProjectIds: [],
		projects: [],
		recentsExpanded: true,
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
