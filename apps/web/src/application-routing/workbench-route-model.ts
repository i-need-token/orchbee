interface WorkbenchRouteTerminalContextSnapshot {
	readonly activeTabId: string | null;
	readonly tabIds: readonly string[];
}

interface WorkbenchRouteWorkspaceSnapshot extends WorkbenchRouteTerminalContextSnapshot {
	readonly projectId: string | null;
	readonly id: string;
}

interface WorkbenchRouteProjectSnapshot {
	readonly id: string;
}

export interface WorkbenchRouteSnapshot {
	readonly activeLocation:
		| { readonly kind: 'standalone' }
		| {
				readonly kind: 'workspace';
				readonly workspaceId: string;
		  };
	readonly projects: readonly WorkbenchRouteProjectSnapshot[];
	readonly workspaces: readonly WorkbenchRouteWorkspaceSnapshot[];
	readonly standalone: WorkbenchRouteTerminalContextSnapshot;
}

export type WorkbenchRouteLocation =
	| {
			readonly kind: 'standalone';
			readonly tabId: string | null;
	  }
	| {
			readonly kind: 'workspace';
			readonly tabId: string | null;
			readonly workspaceId: string;
	  };

export type WorkbenchRouteRequest =
	| { readonly kind: 'root' }
	| { readonly kind: 'standalone'; readonly tabId?: string | undefined }
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

export function canonicalWorkbenchRoute(snapshot: WorkbenchRouteSnapshot): WorkbenchRouteLocation {
	if (snapshot.activeLocation.kind === 'standalone') {
		return standaloneLocation(snapshot.standalone, 'active');
	}
	const workspaceId = snapshot.activeLocation.workspaceId;
	const workspace = snapshot.workspaces.find((candidate) => candidate.id === workspaceId);
	return workspace === undefined
		? standaloneLocation(snapshot.standalone, 'active')
		: workspaceLocation(workspace, 'active');
}

export function resolveWorkbenchRoute(
	snapshot: WorkbenchRouteSnapshot,
	request: WorkbenchRouteRequest,
): WorkbenchRouteResolution {
	switch (request.kind) {
		case 'root':
			return { kind: 'redirect', location: canonicalWorkbenchRoute(snapshot) };
		case 'standalone':
			return resolveStandaloneRoute(snapshot.standalone, request.tabId);
		case 'projects': {
			const workspace = snapshot.workspaces[0];
			return workspace === undefined
				? {
						kind: 'redirect',
						location: standaloneLocation(snapshot.standalone, 'first'),
					}
				: { kind: 'redirect', location: workspaceLocation(workspace, 'first') };
		}
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
	return left.kind === 'standalone'
		? right.kind === 'standalone' && left.tabId === right.tabId
		: right.kind === 'workspace' &&
				left.workspaceId === right.workspaceId &&
				left.tabId === right.tabId;
}

export function workbenchRoutePath(location: WorkbenchRouteLocation): string {
	if (location.kind === 'standalone') {
		return location.tabId === null
			? '/terminal'
			: `/terminal/tabs/${encodeURIComponent(location.tabId)}`;
	}
	const workspacePath = `/workspaces/${encodeURIComponent(location.workspaceId)}`;
	return location.tabId === null
		? workspacePath
		: `${workspacePath}/tabs/${encodeURIComponent(location.tabId)}`;
}

function resolveStandaloneRoute(
	context: WorkbenchRouteTerminalContextSnapshot,
	tabId: string | undefined,
): WorkbenchRouteResolution {
	if (tabId !== undefined) {
		return context.tabIds.includes(tabId)
			? { kind: 'ready', location: { kind: 'standalone', tabId } }
			: { kind: 'unavailable', reason: 'tab-not-found' };
	}
	const location = standaloneLocation(context, 'first');
	return location.tabId === null ? { kind: 'ready', location } : { kind: 'redirect', location };
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

function standaloneLocation(
	context: WorkbenchRouteTerminalContextSnapshot,
	selection: 'active' | 'first',
): Extract<WorkbenchRouteLocation, { readonly kind: 'standalone' }> {
	return {
		kind: 'standalone',
		tabId: selectedTabId(context, selection),
	};
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
	context: WorkbenchRouteTerminalContextSnapshot,
	selection: 'active' | 'first',
): string | null {
	if (
		selection === 'active' &&
		context.activeTabId !== null &&
		context.tabIds.includes(context.activeTabId)
	) {
		return context.activeTabId;
	}
	return context.tabIds[0] ?? null;
}
