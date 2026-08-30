import { Button, ContextMenu, ContextMenuTrigger, ProductIcon } from '@orchbee/ui/components';
import type { PointerEvent, ReactNode } from 'react';

import type { ProjectWorkspaceState, WorkbenchProject } from '../../../../../project-workspace';
import type { WorkbenchAgentNavigationInput } from '../../../../model/project-workbench-agent-overview';
import type { WorkbenchTerminalLocationActivationOrigin } from '../../../../model/workbench-terminal-location-activation';
import { WorkbenchAgentIdentity } from '../../../workbench-agent-identity/WorkbenchAgentIdentity';
import { WorkbenchActionContextMenuContent } from '../../../workbench-action-context-menu-content/WorkbenchActionContextMenuContent';
import type {
	WorkbenchActionMenuCommand,
	WorkbenchActionMenuEntry,
} from '../../../../types/workbench-action-menu';
import type {
	WorkbenchProjectWorktreeCapability,
	WorkbenchProjectWorktreeDiscovery,
} from '../../../../types/workbench-project-worktree';
import type { WorkbenchEnvironmentProjectConnectionStatus } from '../../../../types/workbench-environment-project-connection';
import type { WorkbenchWorkspaceAgentOverviewView } from '../../../../types/workbench-agent-overview';
import { useNavigationReorderDrag } from '../../hooks/useNavigationReorderDrag';
import type { WorkbenchProjectNavigationMessages } from '../../types/workbench-sidebar-types';
import { WorkbenchSidebarDropdownMenu } from '../workbench-sidebar-dropdown-menu/WorkbenchSidebarDropdownMenu';
import { WorkspaceAgentList } from './components/workspace-agent-list/WorkspaceAgentList';
import styles from './ProjectNavigationItem.module.css';

interface ProjectNavigationItemProps {
	readonly agentGradientPrefix: string;
	readonly activateWorkspace: (
		projectId: string,
		workspaceId: string,
		origin: WorkbenchTerminalLocationActivationOrigin,
	) => void;
	readonly activeLocation: ProjectWorkspaceState['activeLocation'];
	readonly adoptWorktree?:
		| ((input: Parameters<WorkbenchProjectWorktreeCapability['adopt']>[0]) => void)
		| undefined;
	readonly createWorkspace: (projectId: string, directoryId: string) => void;
	readonly connectEnvironment?: ((projectId: string) => void) | undefined;
	readonly createWorktree?: ((projectId: string) => void) | undefined;
	readonly deleteWorkspace: (projectId: string, workspaceId: string) => void;
	readonly displacement: 'down' | 'up' | null;
	readonly disconnectEnvironment?: ((projectId: string) => void) | undefined;
	readonly duplicateWorkspace: (projectId: string, workspaceId: string) => void;
	readonly expandedAgentWorkspaceId: string | null;
	readonly dragging: boolean;
	readonly expanded: boolean;
	readonly environmentConnection: WorkbenchEnvironmentProjectConnectionStatus | null;
	readonly messages: WorkbenchProjectNavigationMessages;
	readonly navigateAgent: (input: WorkbenchAgentNavigationInput) => void;
	readonly project: WorkbenchProject;
	readonly refreshWorktrees?: (() => void) | undefined;
	readonly onPointerDown: (projectId: string, event: PointerEvent<HTMLDivElement>) => void;
	readonly reorderActive: boolean;
	readonly reorderWorkspace: (
		projectId: string,
		workspaceId: string,
		beforeWorkspaceId: string | null,
	) => void;
	readonly removeProject: (projectId: string) => void;
	readonly showProjectManager: (projectId: string) => void;
	readonly showWorkspaceManager: (projectId: string, workspaceId: string) => void;
	readonly toggleProjectExpansion: (projectId: string) => void;
	readonly toggleWorkspaceAgentExpansion: (workspaceId: string) => void;
	readonly workspaceAgentOverviews: readonly WorkbenchWorkspaceAgentOverviewView[];
	readonly worktreeDiscovery: WorkbenchProjectWorktreeDiscovery | null;
	readonly worktreeDiscoveryFailed: boolean;
	readonly worktreeOperationPending: boolean;
}

export function ProjectNavigationItem({
	agentGradientPrefix,
	activateWorkspace,
	activeLocation,
	adoptWorktree,
	createWorkspace,
	connectEnvironment,
	createWorktree,
	deleteWorkspace,
	displacement,
	disconnectEnvironment,
	duplicateWorkspace,
	expandedAgentWorkspaceId,
	dragging,
	expanded,
	environmentConnection,
	messages,
	navigateAgent,
	project,
	refreshWorktrees,
	onPointerDown,
	reorderActive,
	reorderWorkspace,
	removeProject,
	showProjectManager,
	showWorkspaceManager,
	toggleProjectExpansion,
	toggleWorkspaceAgentExpansion,
	workspaceAgentOverviews,
	worktreeDiscovery,
	worktreeDiscoveryFailed,
	worktreeOperationPending,
}: ProjectNavigationItemProps): ReactNode {
	const representsActiveWorkspace =
		!expanded && activeLocation.kind === 'workspace' && activeLocation.projectId === project.id;
	const workspaceReorder = useNavigationReorderDrag(project.workspaces, (workspaceId, beforeId) =>
		reorderWorkspace(project.id, workspaceId, beforeId),
	);
	const projectFolders = project.directories.filter((directory) => directory.kind === 'folder');
	const worktrees = project.directories.filter((directory) => directory.kind === 'worktree');
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
			onSelect: () => createWorkspace(project.id, directory.id),
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
							projectId: project.id,
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
	const environmentConnectionEntries: readonly WorkbenchActionMenuEntry[] = (() => {
		if (environmentConnection === null) return [];
		const statusLabel = messages.projectConnectionStatus[environmentConnection.state];
		if (environmentConnection.state === 'connected') {
			return [
				{
					disabled: disconnectEnvironment === undefined,
					icon: 'close',
					id: 'disconnect-environment-project',
					kind: 'command',
					label: messages.disconnectProject,
					onSelect: () => disconnectEnvironment?.(project.id),
				},
				{ id: 'environment-connection-separator', kind: 'separator' },
			];
		}
		if (
			environmentConnection.state === 'connecting' ||
			environmentConnection.state === 'interaction-required' ||
			environmentConnection.state === 'retrying'
		) {
			return [
				{
					disabled: true,
					icon: 'terminal',
					id: 'environment-connection-pending',
					kind: 'command',
					label: statusLabel,
					onSelect: () => undefined,
				},
				{ id: 'environment-connection-separator', kind: 'separator' },
			];
		}
		return [
			{
				disabled: connectEnvironment === undefined,
				icon: 'terminal',
				id: 'connect-environment-project',
				kind: 'command',
				label:
					environmentConnection.state === 'disconnected'
						? messages.connectProject
						: messages.retryProjectConnection,
				onSelect: () => connectEnvironment?.(project.id),
			},
			{ id: 'environment-connection-separator', kind: 'separator' },
		];
	})();
	const projectMenuEntries: readonly WorkbenchActionMenuEntry[] = [
		...environmentConnectionEntries,
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
										onSelect: () => createWorktree(project.id),
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
							onSelect: () => createWorkspace(project.id, directory.id),
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
										onSelect: () => createWorkspace(project.id, directory.id),
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
		{ id: 'project-management-separator', kind: 'separator' },
		{
			icon: 'settings',
			id: 'manage-project',
			kind: 'command',
			label: messages.projectManage,
			onSelect: () => showProjectManager(project.id),
		},
		{ id: 'remove-project-separator', kind: 'separator' },
		{
			destructive: true,
			icon: 'trash',
			id: 'remove-project',
			kind: 'command',
			label: messages.removeProject,
			onSelect: () => removeProject(project.id),
		},
	];
	return (
		<li
			className={styles['project']}
			data-dragging={dragging || undefined}
			data-navigation-reorder-id={project.id}
			data-project-id={project.id}
			data-project-reorder-active={reorderActive || undefined}
			data-project-reorder-displacement={displacement ?? undefined}
		>
			<ContextMenu
				onOpenChange={(open) => {
					if (open) refreshWorktrees?.();
				}}
			>
				<ContextMenuTrigger
					render={
						<div
							className={styles['projectRow']}
							data-navigation-reorder-row='true'
							data-project-reorder-row='true'
							onContextMenu={() => refreshWorktrees?.()}
							onPointerDown={(event) => onPointerDown(project.id, event)}
						/>
					}
				>
					<Button
						aria-expanded={expanded}
						aria-label={`${expanded ? messages.collapseProject : messages.expandProject}: ${project.name}${environmentConnection === null ? '' : `. ${messages.projectConnectionStatus[environmentConnection.state]}`}`}
						className={styles['projectItem']}
						data-active={representsActiveWorkspace || undefined}
						data-slot='workbench-sidebar-project'
						size='sm'
						variant='ghost'
						onClick={() => toggleProjectExpansion(project.id)}
					>
						<span className={styles['icon']}>
							<ProductIcon name='folder' />
						</span>
						<span className={styles['projectName']}>{project.name}</span>
						{environmentConnection === null ? null : (
							<span
								aria-hidden='true'
								className={styles['connectionStatus']}
								data-state={environmentConnection.state}
								title={messages.projectConnectionStatus[environmentConnection.state]}
							/>
						)}
						<span
							aria-hidden='true'
							className={styles['disclosure']}
							data-expanded={expanded || undefined}
						>
							<ProductIcon name='chevron-right' />
						</span>
					</Button>
					<WorkbenchSidebarDropdownMenu
						entries={projectMenuEntries}
						onOpenChange={(open) => {
							if (open) refreshWorktrees?.();
						}}
						trigger={
							<Button
								aria-label={`${messages.projectOptions}: ${project.name}`}
								className={styles['rowMenu']}
								data-navigation-reorder-excluded='true'
								size='icon-xs'
								variant='ghost'
								onClick={(event) => {
									if (event.currentTarget.getAttribute('aria-expanded') !== 'true') {
										refreshWorktrees?.();
									}
								}}
							/>
						}
						triggerContent={<ProductIcon name='ellipsis' />}
					/>
				</ContextMenuTrigger>
				<WorkbenchActionContextMenuContent entries={projectMenuEntries} />
			</ContextMenu>
			{expanded && (
				<ul
					ref={workspaceReorder.listRef}
					className={styles['workspaceList']}
					data-workspace-reorder-dragging={workspaceReorder.draggingItemId ?? undefined}
				>
					{project.workspaces.map((workspace) => {
						const agentOverview = workspaceAgentOverviews.find(
							(overview) => overview.workspaceId === workspace.id,
						);
						const agentExpanded = expandedAgentWorkspaceId === workspace.id;
						const active =
							activeLocation.kind === 'workspace' &&
							activeLocation.projectId === project.id &&
							activeLocation.workspaceId === workspace.id;
						const workspaceMenuEntries: readonly WorkbenchActionMenuEntry[] = [
							{
								icon: 'copy',
								id: 'duplicate-workspace',
								kind: 'command',
								label: messages.duplicateWorkspace,
								onSelect: () => duplicateWorkspace(project.id, workspace.id),
							},
							{
								icon: 'settings',
								id: 'manage-workspace',
								kind: 'command',
								label: messages.workspaceManage,
								onSelect: () => showWorkspaceManager(project.id, workspace.id),
							},
							{ id: 'delete-workspace-separator', kind: 'separator' },
							{
								destructive: true,
								icon: 'trash',
								id: 'delete-workspace',
								kind: 'command',
								label: messages.deleteWorkspace,
								onSelect: () => deleteWorkspace(project.id, workspace.id),
							},
						];
						const workspaceDragging = workspaceReorder.draggingItemId === workspace.id;
						const workspaceDisplacement = workspaceReorder.displacedItemIds.includes(workspace.id)
							? workspaceReorder.displacementDirection
							: null;
						return (
							<li
								key={workspace.id}
								className={styles['workspace']}
								data-dragging={workspaceDragging || undefined}
								data-navigation-reorder-id={workspace.id}
								data-workspace-id={workspace.id}
								data-workspace-reorder-active={
									workspaceReorder.draggingItemId !== null || undefined
								}
								data-workspace-reorder-displacement={workspaceDisplacement ?? undefined}
							>
								<ContextMenu>
									<ContextMenuTrigger
										render={
											<div
												className={styles['workspaceRow']}
												data-navigation-reorder-row='true'
												onPointerDown={(event) =>
													workspaceReorder.onItemPointerDown(workspace.id, event)
												}
											/>
										}
									>
										<Button
											aria-label={
												agentOverview === undefined
													? workspace.name
													: `${workspace.name}. ${agentOverview.countLabel}. ${agentOverview.status.provider}: ${agentOverview.status.status}`
											}
											aria-current={active ? 'page' : undefined}
											className={styles['workspaceItem']}
											data-active={active || undefined}
											data-agent-summary={agentOverview === undefined ? undefined : 'true'}
											data-slot='workbench-sidebar-workspace'
											size='sm'
											variant='ghost'
											onClick={(event) =>
												activateWorkspace(
													project.id,
													workspace.id,
													event.detail > 0 ? 'pointer' : 'other',
												)
											}
										>
											<span className={styles['icon']}>
												{agentOverview === undefined ? (
													<ProductIcon name='layout' />
												) : (
													<WorkbenchAgentIdentity
														gradientPrefix={`${agentGradientPrefix}-${workspace.id}`}
														providerIds={agentOverview.providerIds}
														status={agentOverview.status}
													/>
												)}
											</span>
											<span className={styles['workspaceName']}>{workspace.name}</span>
											{agentOverview === undefined ? null : (
												<span
													aria-hidden='true'
													className={styles['agentCount']}
													title={agentOverview.countLabel}
												>
													{agentOverview.visibleCountLabel}
												</span>
											)}
										</Button>
										{agentOverview === undefined ? null : (
											<Button
												aria-expanded={agentExpanded}
												aria-label={
													agentExpanded ? agentOverview.collapseLabel : agentOverview.expandLabel
												}
												className={styles['agentDisclosure']}
												data-navigation-reorder-excluded='true'
												size='icon-xs'
												variant='ghost'
												onClick={() => toggleWorkspaceAgentExpansion(workspace.id)}
											>
												<span
													aria-hidden='true'
													className={styles['agentDisclosureIcon']}
													data-expanded={agentExpanded || undefined}
												>
													<ProductIcon name='chevron-right' />
												</span>
											</Button>
										)}
										<WorkbenchSidebarDropdownMenu
											entries={workspaceMenuEntries}
											trigger={
												<Button
													aria-label={`${messages.workspaceOptions}: ${workspace.name}`}
													className={styles['rowMenu']}
													data-navigation-reorder-excluded='true'
													size='icon-xs'
													variant='ghost'
												/>
											}
											triggerContent={<ProductIcon name='ellipsis' />}
										/>
									</ContextMenuTrigger>
									<WorkbenchActionContextMenuContent entries={workspaceMenuEntries} />
								</ContextMenu>
								{agentOverview !== undefined &&
									agentExpanded &&
									workspaceReorder.draggingItemId === null && (
										<WorkspaceAgentList
											gradientPrefix={`${agentGradientPrefix}-${workspace.id}-detail`}
											navigateAgent={(sessionId) =>
												navigateAgent({
													projectId: project.id,
													sessionId,
													workspaceId: workspace.id,
												})
											}
											overview={agentOverview}
										/>
									)}
							</li>
						);
					})}
				</ul>
			)}
		</li>
	);
}
