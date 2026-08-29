import {
	AlertDialog,
	AlertDialogAction,
	AlertDialogContent,
	AlertDialogDescription,
	AlertDialogFooter,
	AlertDialogHeader,
	AlertDialogTitle,
	Button,
	Kbd,
	ProductIcon,
} from '@orchbee/ui/components';
import type { ReactNode } from 'react';

import type { ProjectWorkspaceState } from '../../../project-workspace';
import type { WorkbenchEnvironmentProjectConnectionCapability } from '../../types/workbench-environment-project-connection';
import type { WorkbenchProjectFolderAcquisition } from '../../types/workbench-project-folder-acquisition';
import type { WorkbenchProjectWorktreeCapability } from '../../types/workbench-project-worktree';
import { CreateWorktreeDialog } from './components/create-worktree-dialog/CreateWorktreeDialog';
import { CreateProjectDialog } from './components/create-project-dialog/CreateProjectDialog';
import { ProjectCreateMenu } from './components/project-create-menu/ProjectCreateMenu';
import { ProjectManagerDialog } from './components/project-manager-dialog/ProjectManagerDialog';
import { ProjectNavigationItem } from './components/project-navigation-item/ProjectNavigationItem';
import { WorkspaceManagerDialog } from './components/workspace-manager-dialog/WorkspaceManagerDialog';
import { useWorkbenchSidebar } from './hooks/useWorkbenchSidebar';
import { useNavigationReorderDrag } from './hooks/useNavigationReorderDrag';
import { useProjectFolderAcquisition } from './hooks/useProjectFolderAcquisition';
import { useProjectWorktreeCreation } from './hooks/useProjectWorktreeCreation';
import { useProjectWorktreeMenu } from './hooks/useProjectWorktreeMenu';
import { useWorkbenchSidebarFocusRequest } from './hooks/useWorkbenchSidebarFocusRequest';
import type { WorkbenchProjectNavigationMessages } from './types/workbench-sidebar-types';
import styles from './WorkbenchSidebar.module.css';

interface WorkbenchSidebarProps {
	readonly activateStandalone: () => void;
	readonly activateWorkspace: (projectId: string, workspaceId: string) => void;
	readonly addProjectDirectory: (projectId: string, path: string) => void;
	readonly createWorkspace: (projectId: string, directoryId: string) => void;
	readonly createProject: (path: string) => void;
	readonly deleteWorkspace: (projectId: string, workspaceId: string) => void;
	readonly duplicateWorkspace: (projectId: string, workspaceId: string) => void;
	readonly focusRequest: number;
	readonly environmentProjectConnection?:
		| WorkbenchEnvironmentProjectConnectionCapability
		| undefined;
	readonly label: string;
	readonly onSearch: (returnFocus: HTMLElement) => void;
	readonly onSettings?: (() => void) | undefined;
	readonly onConnectViaSsh?: (() => void) | undefined;
	readonly projectLabel: string;
	readonly projectNavigationMessages: WorkbenchProjectNavigationMessages;
	readonly projectFolderAcquisition?: WorkbenchProjectFolderAcquisition | undefined;
	readonly projectWorktree?: WorkbenchProjectWorktreeCapability | undefined;
	readonly projectWorkspaceState: ProjectWorkspaceState;
	readonly removeProject: (projectId: string) => void;
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
	focusRequest,
	environmentProjectConnection,
	label,
	onSearch,
	onSettings,
	onConnectViaSsh,
	projectLabel,
	projectNavigationMessages,
	projectFolderAcquisition,
	projectWorktree,
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
}: WorkbenchSidebarProps): ReactNode {
	const rootReference = useWorkbenchSidebarFocusRequest(focusRequest);
	const view = useWorkbenchSidebar(projectWorkspaceState, environmentProjectConnection);
	const folderAcquisition = useProjectFolderAcquisition({
		acquisition: projectFolderAcquisition,
		addProjectDirectory,
		createProject,
		projectWorkspaceState,
	});
	const projectReorder = useNavigationReorderDrag(projectWorkspaceState.projects, reorderProject);
	const standaloneActive = projectWorkspaceState.activeLocation.kind === 'standalone';
	const projectCreation = view.projectCreation;
	const projectManagement = view.projectManagement;
	const workspaceManagement = view.workspaceManagement;
	const worktreeCreation = useProjectWorktreeCreation({
		capability: projectWorktree,
		projectWorkspaceState,
	});
	const worktreeMenu = useProjectWorktreeMenu({
		capability: projectWorktree,
		projectWorkspaceState,
	});
	return (
		<>
			<aside
				aria-label={label}
				className={styles['root']}
				data-slot='workbench-sidebar'
				ref={rootReference}
			>
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
									selectionPending={folderAcquisition.selecting}
									messages={projectNavigationMessages}
									onConnectViaSsh={onConnectViaSsh}
									onSelectFolder={
										projectFolderAcquisition === undefined
											? view.showProjectCreation
											: folderAcquisition.requestCreateProject
									}
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
									adoptWorktree={projectWorktree === undefined ? undefined : worktreeMenu.adopt}
									createWorkspace={createWorkspace}
									createWorktree={projectWorktree === undefined ? undefined : worktreeCreation.open}
									deleteWorkspace={deleteWorkspace}
									displacement={
										projectReorder.displacedItemIds.includes(project.id)
											? projectReorder.displacementDirection
											: null
									}
									duplicateWorkspace={duplicateWorkspace}
									environmentConnection={view.connectionStatusByProjectId.get(project.id) ?? null}
									connectEnvironment={environmentProjectConnection?.connect}
									disconnectEnvironment={environmentProjectConnection?.disconnect}
									expanded={
										projectWorkspaceState.expandedProjectIds.includes(project.id) &&
										projectReorder.draggingItemId !== project.id
									}
									dragging={projectReorder.draggingItemId === project.id}
									messages={projectNavigationMessages}
									project={project}
									refreshWorktrees={
										projectWorktree === undefined
											? undefined
											: () => worktreeMenu.refresh(project.id)
									}
									onPointerDown={projectReorder.onItemPointerDown}
									reorderActive={projectReorder.draggingItemId !== null}
									reorderWorkspace={reorderWorkspace}
									removeProject={removeProject}
									showProjectManager={view.showProjectManager}
									showWorkspaceManager={view.showWorkspaceManager}
									toggleProjectExpansion={toggleProjectExpansion}
									worktreeDiscovery={worktreeMenu.discoveryByProject.get(project.id) ?? null}
									worktreeDiscoveryFailed={worktreeMenu.discoveryFailureProjectIds.has(project.id)}
									worktreeOperationPending={worktreeMenu.busy}
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
					directorySelectionPending={folderAcquisition.selecting}
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
					onSelectDirectory={
						projectFolderAcquisition === undefined
							? undefined
							: () => folderAcquisition.requestAddProjectFolder(projectManagement.project.id)
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
			{worktreeCreation.dialog === null ? null : (
				<CreateWorktreeDialog
					baseRef={worktreeCreation.dialog.baseRef}
					branchName={worktreeCreation.dialog.branchName}
					createWorkspace={worktreeCreation.dialog.createWorkspace}
					discovery={worktreeCreation.dialog.discovery}
					failure={worktreeCreation.dialog.failure}
					loading={worktreeCreation.dialog.phase === 'discovering'}
					messages={projectNavigationMessages}
					repositoryId={worktreeCreation.dialog.repositoryId}
					submitting={worktreeCreation.dialog.phase === 'submitting'}
					onBaseRefChange={worktreeCreation.setBaseRef}
					onBranchNameChange={worktreeCreation.setBranchName}
					onClose={worktreeCreation.close}
					onCreateWorkspaceChange={worktreeCreation.setCreateWorkspace}
					onRepositoryChange={worktreeCreation.setRepository}
					onSubmit={worktreeCreation.submit}
				/>
			)}
			{folderAcquisition.failure === null ? null : (
				<AlertDialog
					onOpenChange={(open) => {
						if (!open) folderAcquisition.dismissFailure();
					}}
					open
				>
					<AlertDialogContent data-slot='workbench-project-folder-acquisition-failure'>
						<AlertDialogHeader>
							<AlertDialogTitle>
								{projectNavigationMessages.folderAcquisitionFailureTitle}
							</AlertDialogTitle>
							<AlertDialogDescription>
								{
									projectNavigationMessages.folderAcquisitionFailureMessages[
										folderAcquisition.failure
									]
								}
							</AlertDialogDescription>
						</AlertDialogHeader>
						<AlertDialogFooter>
							<AlertDialogAction onClick={folderAcquisition.dismissFailure}>
								{projectNavigationMessages.folderAcquisitionFailureDismiss}
							</AlertDialogAction>
						</AlertDialogFooter>
					</AlertDialogContent>
				</AlertDialog>
			)}
			{worktreeMenu.failure === null ? null : (
				<AlertDialog
					onOpenChange={(open) => {
						if (!open) worktreeMenu.dismissFailure();
					}}
					open
				>
					<AlertDialogContent data-slot='workbench-project-worktree-adoption-failure'>
						<AlertDialogHeader>
							<AlertDialogTitle>
								{projectNavigationMessages.projectWorktreeFailureTitle}
							</AlertDialogTitle>
							<AlertDialogDescription>
								{projectNavigationMessages.projectWorktreeFailureMessages[worktreeMenu.failure]}
							</AlertDialogDescription>
						</AlertDialogHeader>
						<AlertDialogFooter>
							<AlertDialogAction onClick={worktreeMenu.dismissFailure}>
								{projectNavigationMessages.close}
							</AlertDialogAction>
						</AlertDialogFooter>
					</AlertDialogContent>
				</AlertDialog>
			)}
		</>
	);
}
