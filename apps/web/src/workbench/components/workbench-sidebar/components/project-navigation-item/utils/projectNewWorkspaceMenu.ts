import type { WorkbenchProject } from '../../../../../../project-workspace';
import type {
	WorkbenchActionMenuCommand,
	WorkbenchActionMenuEntry,
} from '../../../../../types/workbench-action-menu';
import type { ProjectWorktreeRows } from '../../../hooks/useWorkspaceWorktrees';
import type {
	WorkbenchProjectNavigationMessages,
	WorkbenchSidebarWorkspaceCreation,
} from '../../../types/workbench-sidebar-types';

interface ProjectNewWorkspaceMenuOptions {
	readonly createWorkspace: (input: WorkbenchSidebarWorkspaceCreation) => void;
	readonly messages: WorkbenchProjectNavigationMessages;
	readonly project: WorkbenchProject;
	readonly worktrees: ProjectWorktreeRows;
}

/** Folder names may differ only in ASCII case on hosts that ignore it, as the TUI compares them. */
function asciiLowerCase(value: string): string {
	return value.replaceAll(/[A-Z]/gu, (letter) => letter.toLowerCase());
}

function folderName(path: string): string {
	const name = path
		.replace(/[/\\]+$/u, '')
		.split(/[/\\]/u)
		.at(-1);
	return name === undefined || name === '' ? path : name;
}

/**
 * A Project's New Workspace submenu, as the TUI composes it: New Workspace, the directory choice,
 * New Worktree from the repository of the Project's first Workspace, a Folders group with the
 * Project's folders, where its Workspaces stand, and a Git Worktrees group once Git answers.
 */
export function projectNewWorkspaceMenu({
	createWorkspace,
	messages,
	project,
	worktrees,
}: ProjectNewWorkspaceMenuOptions): WorkbenchActionMenuEntry {
	const names = project.folders.map((folder) => asciiLowerCase(folderName(folder)));
	const folderRows = project.folders.map(
		(folder, index): WorkbenchActionMenuCommand => ({
			icon: 'folder',
			id: `folder-${index}`,
			kind: 'command',
			// A shared folder name is ambiguous, so those rows show their whole path.
			label: names.filter((name) => name === names[index]).length > 1 ? folder : folderName(folder),
			onSelect: () => createWorkspace({ path: folder, projectId: project.id }),
		}),
	);
	const leadingRows: WorkbenchActionMenuCommand[] = [
		{
			icon: 'add',
			id: 'project-workspace',
			kind: 'command',
			label: messages.createWorkspace,
			onSelect: () => createWorkspace({ projectId: project.id }),
		},
		{
			icon: 'folder',
			id: 'project-workspace-directory',
			kind: 'command',
			label: messages.createWorkspaceAtDirectory,
			onSelect: () => createWorkspace({ projectId: project.id, selectDirectory: true }),
		},
	];
	if (worktrees.create !== null) leadingRows.push(worktrees.create);
	return {
		groups: [
			{ commands: leadingRows, id: 'project-create' },
			{ commands: folderRows, id: 'project-folders', label: messages.projectFolders },
			{ commands: worktrees.checkouts, id: 'project-worktrees', label: messages.projectWorktrees },
		].filter((group) => group.commands.length > 0),
		icon: 'add',
		id: 'create-workspace',
		kind: 'submenu',
		label: messages.createWorkspace,
	};
}
