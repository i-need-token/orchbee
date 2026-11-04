interface WorkbenchRouteWorkspaceSnapshot {
	readonly activeTabId: string | null;
	readonly id: string;
	readonly projectId: string | null;
	readonly tabIds: readonly string[];
}

interface WorkbenchRouteProjectSnapshot {
	readonly id: string;
}

export interface WorkbenchRouteSnapshot {
	readonly activeLocation:
		| { readonly kind: 'no-workspace' }
		| {
				readonly kind: 'workspace';
				readonly workspaceId: string;
		  };
	readonly projects: readonly WorkbenchRouteProjectSnapshot[];
	readonly workspaces: readonly WorkbenchRouteWorkspaceSnapshot[];
}

/** No Workspace is a location only while none exist, as in the TUI. */
export type WorkbenchRouteLocation =
	| { readonly kind: 'no-workspace' }
	| {
			readonly kind: 'workspace';
			readonly tabId: string | null;
			readonly workspaceId: string;
	  };

export type WorkbenchRouteRequest =
	| { readonly kind: 'root' }
	| { readonly kind: 'projects' }
	| { readonly kind: 'project'; readonly projectId: string }
	| {
			readonly kind: 'workspace';
			readonly tabId?: string | undefined;
			readonly workspaceId: string;
	  };

export type WorkbenchRouteResolution =
	| { readonly kind: 'ready'; readonly location: WorkbenchRouteLocation }
	| { readonly kind: 'redirect'; readonly location: WorkbenchRouteLocation }
	| {
			readonly kind: 'unavailable';
			readonly reason: 'project-not-found' | 'tab-not-found' | 'workspace-not-found';
	  };

const NO_WORKSPACE_LOCATION: WorkbenchRouteLocation = { kind: 'no-workspace' };

export function canonicalWorkbenchRoute(snapshot: WorkbenchRouteSnapshot): WorkbenchRouteLocation {
	const activeLocation = snapshot.activeLocation;
	if (activeLocation.kind === 'no-workspace') return preferredLocation(snapshot);
	const workspace = snapshot.workspaces.find(
		(candidate) => candidate.id === activeLocation.workspaceId,
	);
	return workspace === undefined
		? preferredLocation(snapshot)
		: workspaceLocation(workspace, 'active');
}

export function resolveWorkbenchRoute(
	snapshot: WorkbenchRouteSnapshot,
	request: WorkbenchRouteRequest,
): WorkbenchRouteResolution {
	switch (request.kind) {
		case 'root': {
			// The root path itself shows No Workspace, so it redirects only to a Workspace.
			const location = canonicalWorkbenchRoute(snapshot);
			return location.kind === 'no-workspace'
				? { kind: 'ready', location }
				: { kind: 'redirect', location };
		}
		case 'projects':
			return { kind: 'redirect', location: preferredLocation(snapshot) };
		case 'project': {
			const project = snapshot.projects.find((candidate) => candidate.id === request.projectId);
			if (project === undefined) return { kind: 'unavailable', reason: 'project-not-found' };
			const workspace = snapshot.workspaces.find((candidate) => candidate.projectId === project.id);
			return workspace === undefined
				? { kind: 'unavailable', reason: 'workspace-not-found' }
				: { kind: 'redirect', location: workspaceLocation(workspace, 'first') };
		}
		case 'workspace':
			return resolveWorkspaceRoute(snapshot, request);
	}
}

export function workbenchRouteLocationsEqual(
	left: WorkbenchRouteLocation,
	right: WorkbenchRouteLocation,
): boolean {
	return left.kind === 'no-workspace'
		? right.kind === 'no-workspace'
		: right.kind === 'workspace' &&
				left.workspaceId === right.workspaceId &&
				left.tabId === right.tabId;
}

export function workbenchRoutePath(location: WorkbenchRouteLocation): string {
	if (location.kind === 'no-workspace') return '/';
	const workspacePath = `/workspaces/${encodeURIComponent(location.workspaceId)}`;
	return location.tabId === null
		? workspacePath
		: `${workspacePath}/tabs/${encodeURIComponent(location.tabId)}`;
}

/** Whether what the location names still exists. */
export function workbenchRouteLocationExists(
	snapshot: WorkbenchRouteSnapshot,
	location: WorkbenchRouteLocation,
): boolean {
	if (location.kind === 'no-workspace') return snapshot.workspaces.length === 0;
	const workspace = snapshot.workspaces.find((candidate) => candidate.id === location.workspaceId);
	return (
		workspace !== undefined &&
		(location.tabId === null || workspace.tabIds.includes(location.tabId))
	);
}

function resolveWorkspaceRoute(
	snapshot: WorkbenchRouteSnapshot,
	request: Extract<WorkbenchRouteRequest, { readonly kind: 'workspace' }>,
): WorkbenchRouteResolution {
	const workspace = snapshot.workspaces.find((candidate) => candidate.id === request.workspaceId);
	if (workspace === undefined) return { kind: 'unavailable', reason: 'workspace-not-found' };
	if (request.tabId !== undefined) {
		return workspace.tabIds.includes(request.tabId)
			? {
					kind: 'ready',
					location: {
						kind: 'workspace',
						tabId: request.tabId,
						workspaceId: workspace.id,
					},
				}
			: { kind: 'unavailable', reason: 'tab-not-found' };
	}
	const location = workspaceLocation(workspace, 'first');
	return location.tabId === null ? { kind: 'ready', location } : { kind: 'redirect', location };
}

function preferredLocation(snapshot: WorkbenchRouteSnapshot): WorkbenchRouteLocation {
	const workspace = snapshot.workspaces[0];
	return workspace === undefined ? NO_WORKSPACE_LOCATION : workspaceLocation(workspace, 'first');
}

function workspaceLocation(
	workspace: WorkbenchRouteWorkspaceSnapshot,
	selection: 'active' | 'first',
): Extract<WorkbenchRouteLocation, { readonly kind: 'workspace' }> {
	return {
		kind: 'workspace',
		tabId: selectedTabId(workspace, selection),
		workspaceId: workspace.id,
	};
}

function selectedTabId(
	workspace: WorkbenchRouteWorkspaceSnapshot,
	selection: 'active' | 'first',
): string | null {
	if (
		selection === 'active' &&
		workspace.activeTabId !== null &&
		workspace.tabIds.includes(workspace.activeTabId)
	) {
		return workspace.activeTabId;
	}
	return workspace.tabIds[0] ?? null;
}
