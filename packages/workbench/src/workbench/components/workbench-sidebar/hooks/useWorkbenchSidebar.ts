import { useMemoizedFn } from '@orchbee/foundation/react';
import * as React from 'react';

import type {
	ProjectWorkspaceState,
	WorkbenchProject,
	WorkbenchWorkspace,
	WorkbenchResourceScope,
} from '../../../../project-workspace';
import type {
	WorkbenchEnvironmentProjectConnectionCapability,
	WorkbenchEnvironmentProjectConnectionStatus,
} from '../../../types/workbench-environment-project-connection';
import type { WorkbenchWorkspaceManagementTarget } from '../types/workbench-sidebar-types';

const EMPTY_CONNECTION_STATUSES: readonly WorkbenchEnvironmentProjectConnectionStatus[] = [];

interface ProjectManagementView {
	readonly close: () => void;
	readonly name: string;
	readonly project: WorkbenchProject;
	readonly setName: (name: string) => void;
}

interface ProjectCreationView {
	readonly close: () => void;
	readonly path: string;
	readonly setPath: (path: string) => void;
}

interface WorkspaceManagementView {
	readonly close: () => void;
	readonly name: string;
	readonly primaryDirectoryId: string;
	readonly resources: WorkbenchResourceScope;
	readonly setName: (name: string) => void;
	readonly setPrimaryDirectoryId: (directoryId: string) => void;
	readonly workspace: WorkbenchWorkspace;
}

interface WorkbenchSidebarView {
	readonly connectionStatusByProjectId: ReadonlyMap<
		string,
		WorkbenchEnvironmentProjectConnectionStatus
	>;
	readonly workspaceProjectCreation: {
		readonly workspaceId: string;
		readonly name: string;
		readonly setName: (name: string) => void;
		readonly close: () => void;
	} | null;
	readonly showWorkspaceProjectCreation: (workspaceId: string) => void;
	readonly projectCreation: ProjectCreationView | null;
	readonly projectManagement: ProjectManagementView | null;
	readonly showProjectCreation: () => void;
	readonly showProjectManager: (projectId: string) => void;
	readonly showWorkspaceManager: (workspaceId: string) => void;
	readonly workspaceManagement: WorkspaceManagementView | null;
}

export function useWorkbenchSidebar(
	state: ProjectWorkspaceState,
	environmentProjectConnection?: WorkbenchEnvironmentProjectConnectionCapability,
): WorkbenchSidebarView {
	const connectionStore = React.useMemo(
		() => ({
			getSnapshot: () => environmentProjectConnection?.getSnapshot() ?? EMPTY_CONNECTION_STATUSES,
			subscribe: (listener: () => void) =>
				environmentProjectConnection?.subscribe(listener) ?? (() => undefined),
		}),
		[environmentProjectConnection],
	);
	const connectionStatuses = React.useSyncExternalStore(
		connectionStore.subscribe,
		connectionStore.getSnapshot,
		connectionStore.getSnapshot,
	);
	const [creatingProject, setCreatingProject] = React.useState(false);
	const [projectPath, setProjectPath] = React.useState('');
	const [managedProjectId, setManagedProjectId] = React.useState<string | null>(null);
	const [projectName, setProjectName] = React.useState('');
	const [groupingWorkspaceId, setGroupingWorkspaceId] = React.useState<string | null>(null);
	const [groupName, setGroupName] = React.useState('');
	const [managedWorkspace, setManagedWorkspace] =
		React.useState<WorkbenchWorkspaceManagementTarget | null>(null);
	const [workspaceName, setWorkspaceName] = React.useState('');
	const [workspacePrimaryDirectoryId, setWorkspacePrimaryDirectoryId] = React.useState('');
	const showProjectManager = useMemoizedFn((projectId: string) => {
		const project = state.projects.find((candidate) => candidate.id === projectId);
		if (project === undefined) return;
		setProjectName(project.name);
		setManagedProjectId(projectId);
	});
	const showProjectCreation = useMemoizedFn(() => {
		setProjectPath('');
		setCreatingProject(true);
	});
	const showWorkspaceManager = useMemoizedFn((workspaceId: string) => {
		const workspace = state.workspaces.find((candidate) => candidate.id === workspaceId);
		if (workspace === undefined) return;
		setWorkspaceName(workspace.name);
		setWorkspacePrimaryDirectoryId(workspace.primaryDirectoryId);
		setManagedWorkspace({ workspaceId });
	});
	const closeProjectManager = useMemoizedFn(() => setManagedProjectId(null));
	const closeProjectCreation = useMemoizedFn(() => setCreatingProject(false));
	const closeWorkspaceManager = useMemoizedFn(() => setManagedWorkspace(null));
	const managedProject = state.projects.find((project) => project.id === managedProjectId) ?? null;
	const managedWorkspaceValue =
		state.workspaces.find((workspace) => workspace.id === managedWorkspace?.workspaceId) ?? null;
	const managedWorkspaceResources =
		state.resourceScopes.find((scope) => scope.id === managedWorkspaceValue?.resourceScopeId) ??
		null;

	return {
		workspaceProjectCreation:
			groupingWorkspaceId !== null &&
			state.workspaces.some((workspace) => workspace.id === groupingWorkspaceId)
				? {
						workspaceId: groupingWorkspaceId,
						name: groupName,
						setName: setGroupName,
						close: () => setGroupingWorkspaceId(null),
					}
				: null,
		showWorkspaceProjectCreation: (workspaceId) => {
			const workspace = state.workspaces.find((candidate) => candidate.id === workspaceId);
			if (workspace === undefined) return;
			setGroupName(workspace.name);
			setGroupingWorkspaceId(workspaceId);
		},
		connectionStatusByProjectId: new Map(
			connectionStatuses.map((status) => [status.projectId, status] as const),
		),
		projectCreation: creatingProject
			? { close: closeProjectCreation, path: projectPath, setPath: setProjectPath }
			: null,
		projectManagement:
			managedProject === null
				? null
				: {
						close: closeProjectManager,
						name: projectName,
						project: managedProject,
						setName: setProjectName,
					},
		showProjectManager,
		showProjectCreation,
		showWorkspaceManager,
		workspaceManagement:
			managedWorkspaceResources === null || managedWorkspaceValue === null
				? null
				: {
						close: closeWorkspaceManager,
						name: workspaceName,
						primaryDirectoryId: managedWorkspaceResources.directories.some(
							(directory) => directory.id === workspacePrimaryDirectoryId,
						)
							? workspacePrimaryDirectoryId
							: managedWorkspaceValue.primaryDirectoryId,
						resources: managedWorkspaceResources,
						setName: setWorkspaceName,
						setPrimaryDirectoryId: setWorkspacePrimaryDirectoryId,
						workspace: managedWorkspaceValue,
					},
	};
}
