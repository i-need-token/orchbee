import {
	Button,
	ContextMenu,
	ContextMenuTrigger,
	DropdownMenu,
	DropdownMenuTrigger,
	ProductIcon,
} from '@orchbee/ui/components';
import * as React from 'react';

import type { ProjectWorkspaceState, WorkbenchProject } from '../../../../../project-workspace';
import { WorkbenchActionContextMenuContent } from '../../../workbench-action-context-menu-content/WorkbenchActionContextMenuContent';
import { WorkbenchActionDropdownMenuContent } from '../../../workbench-action-dropdown-menu-content/WorkbenchActionDropdownMenuContent';
import type {
	WorkbenchActionMenuCommand,
	WorkbenchActionMenuEntry,
} from '../../../../types/workbench-action-menu';
import type { WorkbenchConfirmationAnchor } from '../../../../types/workbench-confirmation';
import { workbenchConfirmationAnchorFromMenuEvent } from '../../../../utils/workbenchConfirmationAnchorFromMenuEvent';
import { useNavigationReorderDrag } from '../../hooks/useNavigationReorderDrag';
import type { WorkbenchProjectNavigationMessages } from '../../types/workbench-sidebar-types';
import styles from './ProjectNavigationItem.module.css';

interface ProjectNavigationItemProps {
	readonly activateWorkspace: (projectId: string, workspaceId: string) => void;
	readonly activeLocation: ProjectWorkspaceState['activeLocation'];
	readonly createWorkspace: (projectId: string, directoryId: string) => void;
	readonly deleteWorkspace: (
		projectId: string,
		workspaceId: string,
		anchor: WorkbenchConfirmationAnchor,
	) => void;
	readonly displacement: 'down' | 'up' | null;
	readonly duplicateWorkspace: (projectId: string, workspaceId: string) => void;
	readonly dragging: boolean;
	readonly expanded: boolean;
	readonly messages: WorkbenchProjectNavigationMessages;
	readonly project: WorkbenchProject;
	readonly onPointerDown: (projectId: string, event: React.PointerEvent<HTMLDivElement>) => void;
	readonly reorderActive: boolean;
	readonly reorderWorkspace: (
		projectId: string,
		workspaceId: string,
		beforeWorkspaceId: string | null,
	) => void;
	readonly removeProject: (projectId: string, anchor: WorkbenchConfirmationAnchor) => void;
	readonly showProjectManager: (projectId: string) => void;
	readonly showWorkspaceManager: (projectId: string, workspaceId: string) => void;
	readonly toggleProjectExpansion: (projectId: string) => void;
}

export function ProjectNavigationItem({
	activateWorkspace,
	activeLocation,
	createWorkspace,
	deleteWorkspace,
	displacement,
	duplicateWorkspace,
	dragging,
	expanded,
	messages,
	project,
	onPointerDown,
	reorderActive,
	reorderWorkspace,
	removeProject,
	showProjectManager,
	showWorkspaceManager,
	toggleProjectExpansion,
}: ProjectNavigationItemProps): React.ReactNode {
	const workspaceReorder = useNavigationReorderDrag(project.workspaces, (workspaceId, beforeId) =>
		reorderWorkspace(project.id, workspaceId, beforeId),
	);
	const projectFolders = project.directories.filter((directory) => directory.kind === 'folder');
	const worktrees = project.directories.filter((directory) => directory.kind === 'worktree');
	const projectMenuEntries: readonly WorkbenchActionMenuEntry[] = [
		{
			groups: [
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
				{
					commands: worktrees.map(
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
			onSelect: (event) =>
				removeProject(project.id, workbenchConfirmationAnchorFromMenuEvent(event)),
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
			<ContextMenu>
				<ContextMenuTrigger
					render={
						<div
							className={styles['projectRow']}
							data-navigation-reorder-row='true'
							data-project-reorder-row='true'
							onPointerDown={(event) => onPointerDown(project.id, event)}
						/>
					}
				>
					<Button
						aria-expanded={expanded}
						aria-label={`${expanded ? messages.collapseProject : messages.expandProject}: ${project.name}`}
						className={styles['projectItem']}
						data-slot='workbench-sidebar-project'
						size='sm'
						variant='ghost'
						onClick={() => toggleProjectExpansion(project.id)}
					>
						<span className={styles['icon']}>
							<ProductIcon name='folder' />
						</span>
						<span className={styles['projectName']}>{project.name}</span>
						<span
							aria-hidden='true'
							className={styles['disclosure']}
							data-expanded={expanded || undefined}
						>
							<ProductIcon name='chevron-right' />
						</span>
					</Button>
					<DropdownMenu>
						<DropdownMenuTrigger
							render={
								<Button
									aria-label={`${messages.projectOptions}: ${project.name}`}
									className={styles['rowMenu']}
									data-navigation-reorder-excluded='true'
									size='icon-xs'
									variant='ghost'
								/>
							}
						>
							<ProductIcon name='ellipsis' />
						</DropdownMenuTrigger>
						<WorkbenchActionDropdownMenuContent entries={projectMenuEntries} />
					</DropdownMenu>
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
								onSelect: (event) =>
									deleteWorkspace(
										project.id,
										workspace.id,
										workbenchConfirmationAnchorFromMenuEvent(event),
									),
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
											aria-current={active ? 'page' : undefined}
											className={styles['workspaceItem']}
											data-active={active || undefined}
											data-slot='workbench-sidebar-workspace'
											size='sm'
											variant='ghost'
											onClick={() => activateWorkspace(project.id, workspace.id)}
										>
											<span className={styles['icon']}>
												<ProductIcon name='layout' />
											</span>
											<span className={styles['workspaceName']}>{workspace.name}</span>
										</Button>
										<DropdownMenu>
											<DropdownMenuTrigger
												render={
													<Button
														aria-label={`${messages.workspaceOptions}: ${workspace.name}`}
														className={styles['rowMenu']}
														data-navigation-reorder-excluded='true'
														size='icon-xs'
														variant='ghost'
													/>
												}
											>
												<ProductIcon name='ellipsis' />
											</DropdownMenuTrigger>
											<WorkbenchActionDropdownMenuContent entries={workspaceMenuEntries} />
										</DropdownMenu>
									</ContextMenuTrigger>
									<WorkbenchActionContextMenuContent entries={workspaceMenuEntries} />
								</ContextMenu>
							</li>
						);
					})}
				</ul>
			)}
		</li>
	);
}
