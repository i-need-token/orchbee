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
import type { ActionExecution } from '@orchbee/actions';

import type { ProjectWorkspaceState } from '../../../project-workspace';
import type { WorkbenchAgentNavigationInput } from '../../model/project-workbench-agent-overview';
import type { WorkbenchTerminalLocationActivationOrigin } from '../../model/workbench-terminal-location-activation';
import type { WorkbenchEnvironmentProjectConnectionCapability } from '../../types/workbench-environment-project-connection';
import type {
	WorkbenchProjectFolderAcquisition,
	WorkbenchProjectCreationReceipt,
} from '../../types/workbench-project-folder-acquisition';
import type { WorkbenchProjectWorktreeCapability } from '../../types/workbench-project-worktree';
import { CreateWorktreeDialog } from './components/create-worktree-dialog/CreateWorktreeDialog';
import { CreateProjectDialog } from './components/create-project-dialog/CreateProjectDialog';
import { CreateProjectFolderDialog } from './components/create-project-folder-dialog/CreateProjectFolderDialog';
import { useNewProjectFolder } from './hooks/useNewProjectFolder';
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
import { useWorkbenchSidebarAgentOverview } from './hooks/useWorkbenchSidebarAgentOverview';
import { useWorkspaceCreationActivation } from './hooks/useWorkspaceCreationActivation';
import type { WorkbenchProjectNavigationMessages } from './types/workbench-sidebar-types';
import type { WorkbenchWorkspaceAgentOverviewView } from '../../types/workbench-agent-overview';
import styles from './WorkbenchSidebar.module.css';

interface WorkbenchSidebarProps {
	readonly activateStandalone: (origin: WorkbenchTerminalLocationActivationOrigin) => void;
	/** A host whose terminals all live in Workspaces has nothing to offer here. */
	readonly standaloneTerminalAvailable: boolean;
	readonly activateWorkspace: (
		workspaceId: string,
		origin: WorkbenchTerminalLocationActivationOrigin,
	) => void;
	readonly addResourceDirectory: (resourceScopeId: string, path: string) => void;
	readonly createWorkspace: (input?: {
		readonly resourceScopeId: string;
		readonly projectId: string | null;
		readonly directoryId: string;
	}) => void;
	readonly setWorkspaceProject: (workspaceId: string, projectId: string | null) => void;
	readonly createWorkspaceProject: (workspaceId: string, name: string) => void;
	readonly closeProjectWorkspaces: (projectId: string) => void;
	readonly createProject: (path: string) => ActionExecution<WorkbenchProjectCreationReceipt>;
	readonly activateCreatedProject: (
		receipt: WorkbenchProjectCreationReceipt,
	) => ActionExecution<void>;
	readonly isCreatedProjectActive: (receipt: WorkbenchProjectCreationReceipt) => boolean;
	readonly deleteWorkspace: (workspaceId: string) => void;
	readonly duplicateWorkspace: (workspaceId: string) => void;
	readonly navigateAgent: (input: WorkbenchAgentNavigationInput) => void;
	readonly focusRequest: number;
	readonly environmentProjectConnection?:
		| WorkbenchEnvironmentProjectConnectionCapability
		| undefined;
	readonly label: string;
	readonly newWorkspaceLabel: string;
	readonly onSearch: (returnFocus: HTMLElement) => void;
	readonly onSettings?: (() => void) | undefined;
	readonly onConnectViaSsh?: (() => void) | undefined;
	readonly onAddResourceFolderViaSsh?: ((resourceScopeId: string) => void) | undefined;
	readonly projectLabel: string;
	readonly projectNavigationMessages: WorkbenchProjectNavigationMessages;
	readonly projectFolderAcquisition?: WorkbenchProjectFolderAcquisition | undefined;
	readonly projectWorktree?: WorkbenchProjectWorktreeCapability | undefined;
	readonly projectWorkspaceState: ProjectWorkspaceState;
	readonly subscribeNavigation: (listener: () => void) => () => void;
	readonly removeProject: (projectId: string) => void;
	readonly removeResourceDirectory: (resourceScopeId: string, directoryId: string) => void;
	readonly reorderProject: (projectId: string, beforeProjectId: string | null) => void;
	readonly reorderWorkspace: (workspaceId: string, beforeWorkspaceId: string | null) => void;
	readonly renameProject: (projectId: string, name: string) => void;
	readonly searchLabel: string;
	readonly searchShortcutId: string;
	readonly searchShortcutLabel: string;
	readonly setResourcePrimaryDirectory: (resourceScopeId: string, directoryId: string) => void;
	readonly settingsLabel: string;
	readonly settingsShortcutId: string;
	readonly settingsShortcutLabel: string;
	readonly terminalLabel: string;
	readonly toggleProjectExpansion: (projectId: string) => void;
	readonly updateWorkspace: (workspaceId: string, name: string, primaryDirectoryId: string) => void;
	readonly workspaceAgentOverviews: readonly WorkbenchWorkspaceAgentOverviewView[];
}

export function WorkbenchSidebar({
	activateStandalone,
	standaloneTerminalAvailable,
	activateWorkspace,
	addResourceDirectory,
	createWorkspace,
	setWorkspaceProject,
	createWorkspaceProject,
	closeProjectWorkspaces,
	createProject,
	activateCreatedProject,
	isCreatedProjectActive,
	deleteWorkspace,
	duplicateWorkspace,
	navigateAgent,
	focusRequest,
	environmentProjectConnection,
	label,
	newWorkspaceLabel,
	onSearch,
	onSettings,
	onConnectViaSsh,
	onAddResourceFolderViaSsh,
	projectLabel,
	projectNavigationMessages,
	projectFolderAcquisition,
	projectWorktree,
	projectWorkspaceState,
	subscribeNavigation,
	removeProject,
	removeResourceDirectory,
	reorderProject,
	reorderWorkspace,
	renameProject,
	searchLabel,
	searchShortcutId,
	searchShortcutLabel,
	setResourcePrimaryDirectory,
	settingsLabel,
	settingsShortcutId,
	settingsShortcutLabel,
	terminalLabel,
	toggleProjectExpansion,
	updateWorkspace,
	workspaceAgentOverviews,
}: WorkbenchSidebarProps): ReactNode {
	const agentOverview = useWorkbenchSidebarAgentOverview({
		expandedProjectIds: projectWorkspaceState.expandedProjectIds,
		workspaces: workspaceAgentOverviews,
	});
	const rootReference = useWorkbenchSidebarFocusRequest(focusRequest);
	const view = useWorkbenchSidebar(projectWorkspaceState, environmentProjectConnection);
	const folderAcquisition = useProjectFolderAcquisition({
		acquisition: projectFolderAcquisition,
		addResourceDirectory,
		createProject,
		projectWorkspaceState,
	});
	const newFolder = useNewProjectFolder({
		activateCreatedProject,
		capability: projectFolderAcquisition?.newFolder,
		createProject,
		isCreatedProjectActive,
		selectExisting: folderAcquisition.requestCreateProject,
	});
	const folderFailure =
		folderAcquisition.failure ?? (newFolder.state.kind === 'failed' ? newFolder.state.code : null);
	const dismissFolderFailure = () => {
		folderAcquisition.dismissFailure();
		newFolder.close();
	};
	const projectReorder = useNavigationReorderDrag(projectWorkspaceState.projects, reorderProject);
	const standaloneActive = projectWorkspaceState.activeLocation.kind === 'standalone';
	const projectCreation = view.projectCreation;
	const projectManagement = view.projectManagement;
	const workspaceManagement = view.workspaceManagement;
	const workspaceActivation = useWorkspaceCreationActivation({
		subscribeNavigation,
		activateWorkspace: (workspaceId) => activateWorkspace(workspaceId, 'other'),
		projectWorkspaceState,
	});
	const worktreeCreation = useProjectWorktreeCreation({
		capability: projectWorktree,
		projectWorkspaceState,
		beginWorkspaceActivation: workspaceActivation.begin,
	});
	const worktreeMenu = useProjectWorktreeMenu({
		capability: projectWorktree,
		projectWorkspaceState,
		beginWorkspaceActivation: workspaceActivation.begin,
	});
	return (
		<>
			<aside
				aria-label={label}
				className={styles['root']}
				data-slot='workbench-sidebar'
				ref={rootReference}
			>
				<ul className={styles['primaryList']} data-slot='workbench-sidebar-actions'>
					<li>
						<Button
							className={styles['primaryItem']}
							data-slot='workbench-sidebar-new-workspace'
							size='sm'
							variant='ghost'
							onClick={() => {
								workspaceActivation.cancel();
								createWorkspace();
							}}
						>
							<span className={styles['icon']}>
								<ProductIcon name='add' />
							</span>
							<span className={styles['label']}>{newWorkspaceLabel}</span>
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
				<section className={styles['projectSection']}>
					<div className={styles['sectionHeader']}>
						<h2 className={styles['sectionHeading']}>
							{projectLabel}{' '}
							<span className={styles['sectionCount']}>
								{projectWorkspaceState.workspaces.length}
							</span>
						</h2>
					</div>
					<div className={styles['projectRegion']} data-navigation-reorder-scroll='true'>
						{standaloneTerminalAvailable ? (
							<div>
								<Button
									aria-current={standaloneActive ? 'page' : undefined}
									className={styles['primaryItem']}
									data-active={standaloneActive || undefined}
									data-slot='workbench-sidebar-terminal'
									size='sm'
									variant='ghost'
									onClick={(event) => {
										workspaceActivation.cancel();
										activateStandalone(event.detail > 0 ? 'pointer' : 'other');
									}}
								>
									<span className={styles['icon']}>
										<ProductIcon name='terminal' />
									</span>
									<span className={styles['label']}>{terminalLabel}</span>
								</Button>
							</div>
						) : null}
						<ul
							ref={projectReorder.listRef}
							className={styles['projectList']}
							data-project-reorder-dragging={projectReorder.draggingItemId ?? undefined}
						>
							{[null, ...projectWorkspaceState.projects].map((project) => (
								<ProjectNavigationItem
									key={project?.id ?? 'ungrouped'}
									agentGradientPrefix={`${agentOverview.agentGradientPrefix}-${project?.id ?? 'ungrouped'}`}
									activateWorkspace={(workspaceId, origin) => {
										workspaceActivation.cancel();
										activateWorkspace(workspaceId, origin);
									}}
									activeLocation={projectWorkspaceState.activeLocation}
									adoptWorktree={projectWorktree === undefined ? undefined : worktreeMenu.adopt}
									createWorkspace={createWorkspace}
									createWorktree={projectWorktree === undefined ? undefined : worktreeCreation.open}
									deleteWorkspace={deleteWorkspace}
									displacement={
										project !== null && projectReorder.displacedItemIds.includes(project.id)
											? projectReorder.displacementDirection
											: null
									}
									duplicateWorkspace={duplicateWorkspace}
									expandedAgentWorkspaceId={agentOverview.expandedWorkspace?.workspaceId ?? null}
									environmentConnection={
										project === null
											? null
											: (view.connectionStatusByProjectId.get(project.id) ?? null)
									}
									connectEnvironment={environmentProjectConnection?.connect}
									disconnectEnvironment={environmentProjectConnection?.disconnect}
									expanded={
										project !== null &&
										projectWorkspaceState.expandedProjectIds.includes(project.id) &&
										projectReorder.draggingItemId !== project.id
									}
									dragging={projectReorder.draggingItemId === project?.id}
									messages={projectNavigationMessages}
									navigateAgent={(input) => {
										workspaceActivation.cancel();
										navigateAgent(input);
									}}
									project={project}
									refreshWorktrees={
										projectWorktree === undefined ? undefined : worktreeMenu.refresh
									}
									onPointerDown={projectReorder.onItemPointerDown}
									reorderActive={projectReorder.draggingItemId !== null}
									reorderWorkspace={reorderWorkspace}
									removeProject={removeProject}
									showProjectManager={view.showProjectManager}
									showWorkspaceManager={view.showWorkspaceManager}
									toggleProjectExpansion={toggleProjectExpansion}
									toggleWorkspaceAgentExpansion={agentOverview.toggleWorkspace}
									workspaceAgentOverviews={workspaceAgentOverviews}
									worktreeDiscoveryByWorkspace={worktreeMenu.discoveryByWorkspace}
									worktreeDiscoveryFailures={worktreeMenu.discoveryFailureWorkspaceIds}
									state={projectWorkspaceState}
									setWorkspaceProject={setWorkspaceProject}
									showWorkspaceProjectCreation={view.showWorkspaceProjectCreation}
									closeProjectWorkspaces={closeProjectWorkspaces}
									worktreeOperationPending={worktreeMenu.busy}
								/>
							))}
						</ul>
						<div
							className={styles['projectCreate']}
							data-empty={projectWorkspaceState.projects.length === 0 || undefined}
							data-slot='workbench-sidebar-project-create'
						>
							<ProjectCreateMenu
								selectionPending={folderAcquisition.selecting || newFolder.busy}
								onNewFolder={
									projectFolderAcquisition?.newFolder === undefined ? undefined : newFolder.open
								}
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
			{'parent' in newFolder.state ? (
				<CreateProjectFolderDialog
					messages={projectNavigationMessages}
					onClose={newFolder.close}
					onNameChange={newFolder.setName}
					onRecover={newFolder.recover}
					onSubmit={newFolder.submit}
					state={newFolder.state}
				/>
			) : null}
			{projectManagement === null ? null : (
				<ProjectManagerDialog
					messages={projectNavigationMessages}
					name={projectManagement.name}
					onClose={projectManagement.close}
					onNameChange={projectManagement.setName}
					onSave={(name) => {
						renameProject(projectManagement.project.id, name);
						projectManagement.close();
					}}
				/>
			)}
			{view.workspaceProjectCreation === null ? null : (
				<ProjectManagerDialog
					messages={projectNavigationMessages}
					submitLabel={projectNavigationMessages.create}
					title={projectNavigationMessages.workspaceCreateProject}
					name={view.workspaceProjectCreation.name}
					onClose={view.workspaceProjectCreation.close}
					onNameChange={view.workspaceProjectCreation.setName}
					onSave={(name) => {
						if (view.workspaceProjectCreation === null) return;
						createWorkspaceProject(view.workspaceProjectCreation.workspaceId, name);
						view.workspaceProjectCreation.close();
					}}
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
						updateWorkspace(workspaceManagement.workspace.id, name, primaryDirectoryId);
						workspaceManagement.close();
					}}
					primaryDirectoryId={workspaceManagement.primaryDirectoryId}
					resources={workspaceManagement.resources}
					directorySelectionPending={folderAcquisition.selecting}
					onAddDirectory={(path) => addResourceDirectory(workspaceManagement.resources.id, path)}
					onRemoveDirectory={(directoryId) =>
						removeResourceDirectory(workspaceManagement.resources.id, directoryId)
					}
					onSetPrimaryDirectory={(directoryId) =>
						setResourcePrimaryDirectory(workspaceManagement.resources.id, directoryId)
					}
					onSelectDirectory={
						projectFolderAcquisition === undefined
							? undefined
							: () => folderAcquisition.requestAddResourceFolder(workspaceManagement.resources.id)
					}
					onSelectSshDirectory={
						onAddResourceFolderViaSsh === undefined
							? undefined
							: () => onAddResourceFolderViaSsh(workspaceManagement.resources.id)
					}
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
			{folderFailure === null ? null : (
				<AlertDialog
					onOpenChange={(open) => {
						if (!open) dismissFolderFailure();
					}}
					open
				>
					<AlertDialogContent
						className={styles['alertContent']}
						data-slot='workbench-project-folder-acquisition-failure'
					>
						<AlertDialogHeader className={styles['alertHeader']}>
							<AlertDialogTitle className={styles['alertTitle']}>
								{projectNavigationMessages.folderAcquisitionFailureTitle}
							</AlertDialogTitle>
							<AlertDialogDescription className={styles['alertDescription']}>
								{projectNavigationMessages.folderAcquisitionFailureMessages[folderFailure]}
							</AlertDialogDescription>
						</AlertDialogHeader>
						<AlertDialogFooter className={styles['alertFooter']}>
							<AlertDialogAction onClick={dismissFolderFailure}>
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
					<AlertDialogContent
						className={styles['alertContent']}
						data-slot='workbench-project-worktree-adoption-failure'
					>
						<AlertDialogHeader className={styles['alertHeader']}>
							<AlertDialogTitle className={styles['alertTitle']}>
								{projectNavigationMessages.projectWorktreeFailureTitle}
							</AlertDialogTitle>
							<AlertDialogDescription className={styles['alertDescription']}>
								{projectNavigationMessages.projectWorktreeFailureMessages[worktreeMenu.failure]}
							</AlertDialogDescription>
						</AlertDialogHeader>
						<AlertDialogFooter className={styles['alertFooter']}>
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
