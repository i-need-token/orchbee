import type {
	ProjectWorkspaceState,
	WorkbenchWorkspace,
} from '../../../../../../project-workspace';
import type {
	WorkbenchActionMenuCommand,
	WorkbenchActionMenuEntry,
} from '../../../../../types/workbench-action-menu';
import type {
	WorkbenchProjectWorktreeCapability,
	WorkbenchProjectWorktreeDiscovery,
} from '../../../../../types/workbench-project-worktree';
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
	readonly createWorktree: ((workspaceId: string) => void) | undefined;
	readonly adoptWorktree:
		| ((input: Parameters<WorkbenchProjectWorktreeCapability['adopt']>[0]) => void)
		| undefined;
	readonly refreshWorktrees: ((workspaceId: string) => void) | undefined;
	readonly worktreeDiscovery: WorkbenchProjectWorktreeDiscovery | null;
	readonly worktreeDiscoveryFailed: boolean;
	readonly worktreeOperationPending: boolean;
	readonly setWorkspaceProject: (workspaceId: string, projectId: string | null) => void;
	readonly showWorkspaceProjectCreation: (workspaceId: string) => void;
}

export function workspaceNavigationMenu({
	state,
	workspace,
	messages,
	createWorkspace,
	createWorktree,
	adoptWorktree,
	refreshWorktrees,
	worktreeDiscovery,
	worktreeDiscoveryFailed,
	worktreeOperationPending,
	setWorkspaceProject,
	showWorkspaceProjectCreation,
}: WorkspaceNavigationMenuOptions): readonly WorkbenchActionMenuEntry[] {
	const resources = state.resourceScopes.find((scope) => scope.id === workspace.resourceScopeId);
	if (resources === undefined) return [];
	const projectFolders = resources.directories.filter((directory) => directory.kind === 'folder');
	const worktrees = resources.directories.filter((directory) => directory.kind === 'worktree');
	const groupedDirectoryIds = new Set<string>();
	const repositoryWorktreeGroups = (worktreeDiscovery?.repositories ?? []).flatMap((repository) => {
		const representedWorktrees = worktrees.filter(
			(directory) => directory.git.repositoryId === repository.repositoryId,
		);
		for (const directory of representedWorktrees) groupedDirectoryIds.add(directory.id);
		const commands: WorkbenchActionMenuCommand[] = representedWorktrees.map((directory) => ({
			disabled: !directory.available,
			icon: 'git-branch',
			id: `worktree-${directory.id}`,
			kind: 'command',
			label: directory.name,
			onSelect: () =>
				createWorkspace({
					resourceScopeId: resources.id,
					directoryId: directory.id,
					projectId: workspace.projectId,
				}),
		}));
		if (repository.worktreeInventory.kind === 'available') {
			commands.push(
				...repository.worktreeInventory.worktrees.map((worktree) => ({
					disabled: worktreeOperationPending,
					icon: 'git-branch' as const,
					id: `discovered-${repository.repositoryId}-${worktree.worktreeId}`,
					kind: 'command' as const,
					label: worktree.label,
					onSelect: () =>
						adoptWorktree?.({
							sourceWorkspaceId: workspace.id,
							repositoryId: repository.repositoryId,
							sourceDirectoryId: repository.sourceDirectoryId,
							worktreeId: worktree.worktreeId,
						}),
				})),
			);
		} else {
			commands.push({
				disabled: true,
				icon: 'git-branch',
				id: `worktree-inventory-unavailable-${repository.repositoryId}`,
				kind: 'command',
				label: messages.projectWorktreeInventoryUnavailable,
				onSelect: () => undefined,
			});
		}
		return commands.length === 0
			? []
			: [
					{
						commands,
						id: `repository-worktrees-${repository.repositoryId}`,
						label: repository.label,
					},
				];
	});
	const ungroupedWorktrees = worktrees.filter(
		(directory) => !groupedDirectoryIds.has(directory.id),
	);
	const worktreeStatusGroups = (() => {
		if (refreshWorktrees === undefined || worktreeDiscovery !== null) return [];
		if (worktreeOperationPending) {
			return [
				{
					commands: [
						{
							disabled: true,
							icon: 'git-branch' as const,
							id: 'worktree-inventory-loading',
							kind: 'command' as const,
							label: messages.projectWorktreeLoading,
							onSelect: () => undefined,
						},
					],
					id: 'worktree-inventory-status',
				},
			];
		}
		if (!worktreeDiscoveryFailed) return [];
		return [
			{
				commands: [
					{
						disabled: true,
						icon: 'git-branch' as const,
						id: 'worktree-inventory-failed',
						kind: 'command' as const,
						label: messages.projectWorktreeInventoryUnavailable,
						onSelect: () => undefined,
					},
				],
				id: 'worktree-inventory-status',
			},
		];
	})();
	return [
		{
			groups: [
				...(createWorktree === undefined
					? []
					: [
							{
								commands: [
									{
										icon: 'git-branch',
										id: 'new-managed-worktree',
										kind: 'command',
										label: messages.newWorktree,
										disabled: worktreeOperationPending,
										onSelect: () => createWorktree(workspace.id),
									} satisfies WorkbenchActionMenuCommand,
								],
								id: 'project-new-worktree',
							},
						]),
				{
					commands: projectFolders.map(
						(directory): WorkbenchActionMenuCommand => ({
							disabled: !directory.available,
							icon: 'folder',
							id: `folder-${directory.id}`,
							kind: 'command',
							label: directory.name,
							onSelect: () =>
								createWorkspace({
									resourceScopeId: resources.id,
									directoryId: directory.id,
									projectId: workspace.projectId,
								}),
						}),
					),
					id: 'project-folders',
					label: messages.projectFolders,
				},
				...repositoryWorktreeGroups,
				...(ungroupedWorktrees.length === 0
					? []
					: [
							{
								commands: ungroupedWorktrees.map(
									(directory): WorkbenchActionMenuCommand => ({
										disabled: !directory.available,
										icon: 'git-branch',
										id: `worktree-${directory.id}`,
										kind: 'command',
										label: directory.name,
										onSelect: () =>
											createWorkspace({
												resourceScopeId: resources.id,
												directoryId: directory.id,
												projectId: workspace.projectId,
											}),
									}),
								),
								id: 'project-worktrees',
								label: messages.projectWorktrees,
							},
						]),
				...worktreeStatusGroups,
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
