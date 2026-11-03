import type {
	ProjectWorkspaceState,
	WorkbenchWorkspace,
} from '../../../../../../project-workspace';
import type {
	WorkbenchActionMenuCommand,
	WorkbenchActionMenuEntry,
} from '../../../../../types/workbench-action-menu';
import type { WorkbenchProjectNavigationMessages } from '../../../types/workbench-sidebar-types';

interface WorkspaceNavigationMenuOptions {
	readonly state: ProjectWorkspaceState;
	readonly workspace: WorkbenchWorkspace;
	readonly messages: WorkbenchProjectNavigationMessages;
	readonly createWorkspace: (input: {
		resourceScopeId: string;
		directoryId: string;
		projectId: string | null;
	}) => void;
	readonly setWorkspaceProject: (workspaceId: string, projectId: string | null) => void;
	readonly showWorkspaceProjectCreation: (workspaceId: string) => void;
}

export function workspaceNavigationMenu({
	state,
	workspace,
	messages,
	createWorkspace,
	setWorkspaceProject,
	showWorkspaceProjectCreation,
}: WorkspaceNavigationMenuOptions): readonly WorkbenchActionMenuEntry[] {
	const resources = state.resourceScopes.find((scope) => scope.id === workspace.resourceScopeId);
	if (resources === undefined) return [];
	const directoryCommands = (kind: 'folder' | 'worktree'): WorkbenchActionMenuCommand[] =>
		resources.directories
			.filter((directory) => directory.kind === kind)
			.map((directory) => ({
				disabled: !directory.available,
				icon: kind === 'folder' ? 'folder' : 'git-branch',
				id: `${kind}-${directory.id}`,
				kind: 'command',
				label: directory.name,
				onSelect: () =>
					createWorkspace({
						resourceScopeId: resources.id,
						directoryId: directory.id,
						projectId: workspace.projectId,
					}),
			}));
	return [
		{
			groups: [
				{
					commands: directoryCommands('folder'),
					id: 'project-folders',
					label: messages.projectFolders,
				},
				{
					commands: directoryCommands('worktree'),
					id: 'project-worktrees',
					label: messages.projectWorktrees,
				},
			].filter((group) => group.commands.length > 0),
			icon: 'add',
			id: 'create-workspace',
			kind: 'submenu',
			label: messages.createWorkspace,
		},
		{
			kind: 'submenu',
			id: 'workspace-project',
			icon: 'folder',
			label: messages.workspaceMoveToProject,
			groups: [
				{
					id: 'projects',
					commands: state.projects.map(
						(project): WorkbenchActionMenuCommand => ({
							kind: 'command',
							id: project.id,
							icon: 'folder',
							label: project.name,
							disabled: project.id === workspace.projectId,
							onSelect: () => setWorkspaceProject(workspace.id, project.id),
						}),
					),
				},
				{
					id: 'organization',
					commands: [
						{
							kind: 'command',
							id: 'new-project',
							icon: 'add',
							label: messages.workspaceCreateProject,
							onSelect: () => showWorkspaceProjectCreation(workspace.id),
						} satisfies WorkbenchActionMenuCommand,
						{
							kind: 'command',
							id: 'ungroup',
							icon: 'close',
							label: messages.workspaceUngroup,
							disabled: workspace.projectId === null,
							onSelect: () => setWorkspaceProject(workspace.id, null),
						} satisfies WorkbenchActionMenuCommand,
					],
				},
			].filter((group) => group.commands.length > 0),
		},
	];
}
