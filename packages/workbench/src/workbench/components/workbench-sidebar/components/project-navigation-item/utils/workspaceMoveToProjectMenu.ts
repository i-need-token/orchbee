import type {
	ProjectWorkspaceState,
	WorkbenchWorkspace,
} from '../../../../../../project-workspace';
import type {
	WorkbenchActionMenuCommand,
	WorkbenchActionMenuEntry,
} from '../../../../../types/workbench-action-menu';
import type { WorkbenchProjectNavigationMessages } from '../../../types/workbench-sidebar-types';

interface WorkspaceMoveToProjectMenuOptions {
	readonly messages: WorkbenchProjectNavigationMessages;
	readonly setWorkspaceProject: (workspaceId: string, projectId: string | null) => void;
	readonly showWorkspaceProjectCreation: (workspaceId: string) => void;
	readonly state: ProjectWorkspaceState;
	readonly workspace: WorkbenchWorkspace;
}

/**
 * A Workspace's Move to Project submenu, as the TUI composes it: the other Projects in navigation
 * order, then New Project and, for a grouped Workspace, Remove from Project.
 */
export function workspaceMoveToProjectMenu({
	messages,
	setWorkspaceProject,
	showWorkspaceProjectCreation,
	state,
	workspace,
}: WorkspaceMoveToProjectMenuOptions): WorkbenchActionMenuEntry {
	const organization: WorkbenchActionMenuCommand[] = [
		{
			icon: 'add',
			id: 'new-project',
			kind: 'command',
			label: messages.workspaceMenuNewProject,
			onSelect: () => showWorkspaceProjectCreation(workspace.id),
		},
	];
	if (workspace.projectId !== null) {
		organization.push({
			icon: 'close',
			id: 'ungroup',
			kind: 'command',
			label: messages.workspaceUngroup,
			onSelect: () => setWorkspaceProject(workspace.id, null),
		});
	}
	return {
		groups: [
			{
				commands: state.projects
					.filter((project) => project.id !== workspace.projectId)
					.map(
						(project): WorkbenchActionMenuCommand => ({
							icon: 'folder',
							id: project.id,
							kind: 'command',
							label: project.name,
							onSelect: () => setWorkspaceProject(workspace.id, project.id),
						}),
					),
				id: 'projects',
			},
			{ commands: organization, id: 'organization' },
		].filter((group) => group.commands.length > 0),
		icon: 'folder',
		id: 'workspace-project',
		kind: 'submenu',
		label: messages.workspaceMoveToProject,
	};
}
