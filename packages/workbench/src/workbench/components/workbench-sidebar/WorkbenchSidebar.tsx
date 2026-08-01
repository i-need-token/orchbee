import { Button, Kbd, ProductIcon } from '@orchbee/ui/components';
import * as React from 'react';

import type { ProjectWorkspaceState } from '../../../project-workspace';
import type { WorkbenchConfirmationAnchor } from '../../types/workbench-confirmation';
import { CreateProjectDialog } from './components/create-project-dialog/CreateProjectDialog';
import { ProjectCreateMenu } from './components/project-create-menu/ProjectCreateMenu';
import { ProjectManagerDialog } from './components/project-manager-dialog/ProjectManagerDialog';
import { ProjectNavigationItem } from './components/project-navigation-item/ProjectNavigationItem';
import { WorkspaceManagerDialog } from './components/workspace-manager-dialog/WorkspaceManagerDialog';
import { useWorkbenchSidebar } from './hooks/useWorkbenchSidebar';
import { useNavigationReorderDrag } from './hooks/useNavigationReorderDrag';
import type { WorkbenchProjectNavigationMessages } from './types/workbench-sidebar-types';
import styles from './WorkbenchSidebar.module.css';

interface WorkbenchSidebarProps {
	readonly activateStandalone: () => void;
	readonly activateWorkspace: (projectId: string, workspaceId: string) => void;
	readonly addProjectDirectory: (projectId: string, path: string) => void;
	readonly createWorkspace: (projectId: string, directoryId: string) => void;
	readonly createProject: (path: string) => void;
	readonly deleteWorkspace: (
		projectId: string,
		workspaceId: string,
		anchor: WorkbenchConfirmationAnchor,
	) => void;
	readonly duplicateWorkspace: (projectId: string, workspaceId: string) => void;
	readonly label: string;
	readonly onSearch: (returnFocus: HTMLElement) => void;
	readonly onSettings?: (() => void) | undefined;
	readonly projectLabel: string;
	readonly projectNavigationMessages: WorkbenchProjectNavigationMessages;
	readonly projectWorkspaceState: ProjectWorkspaceState;
	readonly removeProject: (projectId: string, anchor: WorkbenchConfirmationAnchor) => void;
	readonly removeProjectDirectory: (projectId: string, directoryId: string) => void;
	readonly reorderProject: (projectId: string, beforeProjectId: string | null) => void;
	readonly reorderWorkspace: (
		projectId: string,
		workspaceId: string,
		beforeWorkspaceId: string | null,
	) => void;
	readonly renameProject: (projectId: string, name: string) => void;
	readonly searchLabel: string;
	readonly searchShortcutId: string;
	readonly searchShortcutLabel: string;
	readonly setProjectPrimaryDirectory: (projectId: string, directoryId: string) => void;
	readonly settingsLabel: string;
	readonly settingsShortcutId: string;
	readonly settingsShortcutLabel: string;
	readonly terminalLabel: string;
	readonly toggleProjectExpansion: (projectId: string) => void;
	readonly updateWorkspace: (
		projectId: string,
		workspaceId: string,
		name: string,
		primaryDirectoryId: string,
	) => void;
}

export function WorkbenchSidebar({
	activateStandalone,
	activateWorkspace,
	addProjectDirectory,
	createWorkspace,
	createProject,
	deleteWorkspace,
	duplicateWorkspace,
	label,
	onSearch,
	onSettings,
	projectLabel,
	projectNavigationMessages,
	projectWorkspaceState,
	removeProject,
	removeProjectDirectory,
	reorderProject,
	reorderWorkspace,
	renameProject,
	searchLabel,
	searchShortcutId,
	searchShortcutLabel,
	setProjectPrimaryDirectory,
	settingsLabel,
	settingsShortcutId,
	settingsShortcutLabel,
	terminalLabel,
	toggleProjectExpansion,
	updateWorkspace,
}: WorkbenchSidebarProps): React.ReactNode {
	const view = useWorkbenchSidebar(projectWorkspaceState);
	const projectReorder = useNavigationReorderDrag(projectWorkspaceState.projects, reorderProject);
	const standaloneActive = projectWorkspaceState.activeLocation.kind === 'standalone';
	const projectCreation = view.projectCreation;
	const projectManagement = view.projectManagement;
	const workspaceManagement = view.workspaceManagement;
	return (
		<>
			<aside aria-label={label} className={styles['root']} data-slot='workbench-sidebar'>
				<ul className={styles['primaryList']}>
					<li>
						<Button
							aria-current={standaloneActive ? 'page' : undefined}
							className={styles['primaryItem']}
							data-active={standaloneActive || undefined}
							data-slot='workbench-sidebar-terminal'
							size='sm'
							variant='ghost'
							onClick={activateStandalone}
						>
							<span className={styles['icon']}>
								<ProductIcon name='terminal' />
							</span>
							<span className={styles['label']}>{terminalLabel}</span>
						</Button>
					</li>
					<li>
						<Button
							className={styles['primaryItem']}
							data-shortcut-action={searchShortcutId}
							data-slot='workbench-sidebar-search'
							size='sm'
							variant='ghost'
							onClick={(event) => onSearch(event.currentTarget)}
						>
							<span className={styles['icon']}>
								<ProductIcon name='search' />
							</span>
							<span className={styles['label']}>{searchLabel}</span>
							<Kbd className={styles['shortcut']}>{searchShortcutLabel}</Kbd>
						</Button>
					</li>
				</ul>
				<section className={styles['projectSection']} data-navigation-reorder-scroll='true'>
					<div className={styles['projectRegion']}>
						<div className={styles['sectionHeader']}>
							<h2 className={styles['sectionHeading']}>{projectLabel}</h2>
							<div
								className={styles['projectCreate']}
								data-empty={projectWorkspaceState.projects.length === 0 || undefined}
								data-slot='workbench-sidebar-project-create'
							>
								<ProjectCreateMenu
									messages={projectNavigationMessages}
									onSelectFolder={view.showProjectCreation}
								/>
							</div>
						</div>
						<ul
							ref={projectReorder.listRef}
							className={styles['projectList']}
							data-project-reorder-dragging={projectReorder.draggingItemId ?? undefined}
						>
							{projectWorkspaceState.projects.map((project) => (
								<ProjectNavigationItem
									key={project.id}
									activateWorkspace={activateWorkspace}
									activeLocation={projectWorkspaceState.activeLocation}
									createWorkspace={createWorkspace}
									deleteWorkspace={deleteWorkspace}
									displacement={
										projectReorder.displacedItemIds.includes(project.id)
											? projectReorder.displacementDirection
											: null
									}
									duplicateWorkspace={duplicateWorkspace}
									expanded={
										projectWorkspaceState.expandedProjectIds.includes(project.id) &&
										projectReorder.draggingItemId !== project.id
									}
									dragging={projectReorder.draggingItemId === project.id}
									messages={projectNavigationMessages}
									project={project}
									onPointerDown={projectReorder.onItemPointerDown}
									reorderActive={projectReorder.draggingItemId !== null}
									reorderWorkspace={reorderWorkspace}
									removeProject={removeProject}
									showProjectManager={view.showProjectManager}
									showWorkspaceManager={view.showWorkspaceManager}
									toggleProjectExpansion={toggleProjectExpansion}
								/>
							))}
						</ul>
					</div>
				</section>
				<div className={styles['footer']}>
					<Button
						aria-disabled={onSettings === undefined}
						className={styles['primaryItem']}
						data-shortcut-action={settingsShortcutId}
						data-slot='workbench-sidebar-settings'
						size='sm'
						variant='ghost'
						onClick={onSettings}
					>
						<span className={styles['icon']}>
							<ProductIcon name='settings' />
						</span>
						<span className={styles['label']}>{settingsLabel}</span>
						<Kbd className={styles['shortcut']}>{settingsShortcutLabel}</Kbd>
					</Button>
				</div>
			</aside>
			{projectCreation === null ? null : (
				<CreateProjectDialog
					messages={projectNavigationMessages}
					onClose={projectCreation.close}
					onPathChange={projectCreation.setPath}
					onSubmit={(path) => {
						createProject(path);
						projectCreation.close();
					}}
					path={projectCreation.path}
				/>
			)}
			{projectManagement === null ? null : (
				<ProjectManagerDialog
					messages={projectNavigationMessages}
					name={projectManagement.name}
					onAddDirectory={(path) => addProjectDirectory(projectManagement.project.id, path)}
					onClose={projectManagement.close}
					onNameChange={projectManagement.setName}
					onRemoveDirectory={(directoryId) =>
						removeProjectDirectory(projectManagement.project.id, directoryId)
					}
					onSave={(name) => {
						renameProject(projectManagement.project.id, name);
						projectManagement.close();
					}}
					onSetPrimaryDirectory={(directoryId) =>
						setProjectPrimaryDirectory(projectManagement.project.id, directoryId)
					}
					project={projectManagement.project}
				/>
			)}
			{workspaceManagement === null ? null : (
				<WorkspaceManagerDialog
					messages={projectNavigationMessages}
					name={workspaceManagement.name}
					onClose={workspaceManagement.close}
					onNameChange={workspaceManagement.setName}
					onPrimaryDirectoryChange={workspaceManagement.setPrimaryDirectoryId}
					onSave={(name, primaryDirectoryId) => {
						updateWorkspace(
							workspaceManagement.project.id,
							workspaceManagement.workspace.id,
							name,
							primaryDirectoryId,
						);
						workspaceManagement.close();
					}}
					primaryDirectoryId={workspaceManagement.primaryDirectoryId}
					project={workspaceManagement.project}
					workspace={workspaceManagement.workspace}
				/>
			)}
		</>
	);
}
