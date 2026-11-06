import type {
	ProjectWorkspaceState,
	WorkbenchDirectory,
	WorkbenchProject,
} from '../../../../../../project-workspace';
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
	readonly state: ProjectWorkspaceState;
	readonly worktrees: ProjectWorktreeRows;
}

/** Recorded paths may differ only in ASCII case on hosts that ignore it, as the TUI compares them. */
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

/** The directory each of the Project's Workspaces records, in navigation order, each path once. */
function recordedFolders(state: ProjectWorkspaceState, projectId: string): WorkbenchDirectory[] {
	const folders: WorkbenchDirectory[] = [];
	for (const workspace of state.workspaces) {
		const directory = workspace.directory;
		if (workspace.projectId !== projectId || directory === null) continue;
		const key = asciiLowerCase(directory.path);
		if (folders.some((folder) => asciiLowerCase(folder.path) === key)) continue;
		folders.push(directory);
	}
	return folders;
}

/**
 * A Project's New Workspace submenu, as the TUI composes it: New Worktree from the repository of the
 * Project's first Workspace, a Folders group with the directories its Workspaces record, and a Git
 * Worktrees group once Git answers. While the Project records no folder, New Workspace leads it, so
 * the submenu always holds a row that works.
 */
export function projectNewWorkspaceMenu({
	createWorkspace,
	messages,
	project,
	state,
	worktrees,
}: ProjectNewWorkspaceMenuOptions): WorkbenchActionMenuEntry {
	const folders = recordedFolders(state, project.id);
	const names = folders.map((folder) => asciiLowerCase(folderName(folder.path)));
	const folderRows = folders.map(
		(folder, index): WorkbenchActionMenuCommand => ({
			icon: 'folder',
			id: `folder-${folder.id}`,
			kind: 'command',
			// A shared folder name is ambiguous, so those rows show their whole path.
			label:
				names.filter((name) => name === names[index]).length > 1
					? folder.path
					: folderName(folder.path),
			onSelect: () => createWorkspace({ directoryId: folder.id, projectId: project.id }),
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
