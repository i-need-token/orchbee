import { useMemoizedFn } from '@orchbee/foundation/react';
import * as React from 'react';

import type {
	ProjectWorkspaceState,
	WorkbenchProject,
	WorkbenchWorkspace,
} from '../../../../project-workspace';

interface ProjectRenameView {
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

interface WorkspaceRenameView {
	readonly close: () => void;
	readonly name: string;
	readonly setName: (name: string) => void;
	readonly workspace: WorkbenchWorkspace;
}

interface WorkbenchSidebarView {
	readonly workspaceProjectCreation: {
		readonly workspaceId: string;
		readonly name: string;
		readonly setName: (name: string) => void;
		readonly close: () => void;
	} | null;
	readonly showWorkspaceProjectCreation: (workspaceId: string) => void;
	readonly projectCreation: ProjectCreationView | null;
	readonly projectRename: ProjectRenameView | null;
	readonly showProjectCreation: () => void;
	readonly showProjectRename: (projectId: string) => void;
	readonly showWorkspaceRename: (workspaceId: string) => void;
	readonly workspaceRename: WorkspaceRenameView | null;
}

export function useWorkbenchSidebar(state: ProjectWorkspaceState): WorkbenchSidebarView {
	const [creatingProject, setCreatingProject] = React.useState(false);
	const [projectPath, setProjectPath] = React.useState('');
	const [renamedProjectId, setRenamedProjectId] = React.useState<string | null>(null);
	const [projectName, setProjectName] = React.useState('');
	const [groupingWorkspaceId, setGroupingWorkspaceId] = React.useState<string | null>(null);
	const [groupName, setGroupName] = React.useState('');
	const [renamedWorkspaceId, setRenamedWorkspaceId] = React.useState<string | null>(null);
	const [workspaceName, setWorkspaceName] = React.useState('');
	const showProjectRename = useMemoizedFn((projectId: string) => {
		const project = state.projects.find((candidate) => candidate.id === projectId);
		if (project === undefined) return;
		setProjectName(project.name);
		setRenamedProjectId(projectId);
	});
	const showProjectCreation = useMemoizedFn(() => {
		setProjectPath('');
		setCreatingProject(true);
	});
	const showWorkspaceRename = useMemoizedFn((workspaceId: string) => {
		const workspace = state.workspaces.find((candidate) => candidate.id === workspaceId);
		if (workspace === undefined) return;
		setWorkspaceName(workspace.name);
		setRenamedWorkspaceId(workspaceId);
	});
	const closeProjectRename = useMemoizedFn(() => setRenamedProjectId(null));
	const closeProjectCreation = useMemoizedFn(() => setCreatingProject(false));
	const closeWorkspaceRename = useMemoizedFn(() => setRenamedWorkspaceId(null));
	// A rename whose target left closes with it.
	const renamedProject = state.projects.find((project) => project.id === renamedProjectId) ?? null;
	const renamedWorkspace =
		state.workspaces.find((workspace) => workspace.id === renamedWorkspaceId) ?? null;

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
		projectCreation: creatingProject
			? { close: closeProjectCreation, path: projectPath, setPath: setProjectPath }
			: null,
		projectRename:
			renamedProject === null
				? null
				: {
						close: closeProjectRename,
						name: projectName,
						project: renamedProject,
						setName: setProjectName,
					},
		showProjectRename,
		showProjectCreation,
		showWorkspaceRename,
		workspaceRename:
			renamedWorkspace === null
				? null
				: {
						close: closeWorkspaceRename,
						name: workspaceName,
						setName: setWorkspaceName,
						workspace: renamedWorkspace,
					},
	};
}
