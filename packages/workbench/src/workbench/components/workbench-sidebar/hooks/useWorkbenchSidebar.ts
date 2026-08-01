import { useMemoizedFn } from '@orchbee/foundation/react';
import * as React from 'react';

import type {
	ProjectWorkspaceState,
	WorkbenchProject,
	WorkbenchWorkspace,
} from '../../../../project-workspace';
import type { WorkbenchWorkspaceManagementTarget } from '../types/workbench-sidebar-types';

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
	readonly project: WorkbenchProject;
	readonly setName: (name: string) => void;
	readonly setPrimaryDirectoryId: (directoryId: string) => void;
	readonly workspace: WorkbenchWorkspace;
}

interface WorkbenchSidebarView {
	readonly projectCreation: ProjectCreationView | null;
	readonly projectManagement: ProjectManagementView | null;
	readonly showProjectCreation: () => void;
	readonly showProjectManager: (projectId: string) => void;
	readonly showWorkspaceManager: (projectId: string, workspaceId: string) => void;
	readonly workspaceManagement: WorkspaceManagementView | null;
}

export function useWorkbenchSidebar(state: ProjectWorkspaceState): WorkbenchSidebarView {
	const [creatingProject, setCreatingProject] = React.useState(false);
	const [projectPath, setProjectPath] = React.useState('');
	const [managedProjectId, setManagedProjectId] = React.useState<string | null>(null);
	const [projectName, setProjectName] = React.useState('');
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
	const showWorkspaceManager = useMemoizedFn((projectId: string, workspaceId: string) => {
		const project = state.projects.find((candidate) => candidate.id === projectId);
		const workspace = project?.workspaces.find((candidate) => candidate.id === workspaceId);
		if (workspace === undefined) return;
		setWorkspaceName(workspace.name);
		setWorkspacePrimaryDirectoryId(workspace.primaryDirectoryId);
		setManagedWorkspace({ projectId, workspaceId });
	});
	const closeProjectManager = useMemoizedFn(() => setManagedProjectId(null));
	const closeProjectCreation = useMemoizedFn(() => setCreatingProject(false));
	const closeWorkspaceManager = useMemoizedFn(() => setManagedWorkspace(null));
	const managedProject = state.projects.find((project) => project.id === managedProjectId) ?? null;
	const managedWorkspaceProject =
		managedWorkspace === null
			? null
			: (state.projects.find((project) => project.id === managedWorkspace.projectId) ?? null);
	const managedWorkspaceValue =
		managedWorkspace === null
			? null
			: (managedWorkspaceProject?.workspaces.find(
					(workspace) => workspace.id === managedWorkspace.workspaceId,
				) ?? null);

	return {
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
			managedWorkspaceProject === null || managedWorkspaceValue === null
				? null
				: {
						close: closeWorkspaceManager,
						name: workspaceName,
						primaryDirectoryId: workspacePrimaryDirectoryId,
						project: managedWorkspaceProject,
						setName: setWorkspaceName,
						setPrimaryDirectoryId: setWorkspacePrimaryDirectoryId,
						workspace: managedWorkspaceValue,
					},
	};
}
