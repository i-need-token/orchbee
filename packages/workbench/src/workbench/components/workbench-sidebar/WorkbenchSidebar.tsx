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
import { useWorkbenchSidebarFocusRequest } from './hooks/useWorkbenchSidebarFocusRequest';
import { useWorkbenchSidebarAgentOverview } from './hooks/useWorkbenchSidebarAgentOverview';
import { useWorkspaceCreationActivation } from './hooks/useWorkspaceCreationActivation';
import { useWorkspaceWorktrees } from './hooks/useWorkspaceWorktrees';
import type {
	WorkbenchProjectNavigationMessages,
	WorkbenchWorktreeCommands,
} from './types/workbench-sidebar-types';
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
	readonly projectWorkspaceState: ProjectWorkspaceState;
	readonly subscribeNavigation: (listener: () => void) => () => void;
	readonly removeProject: (projectId: string) => void;
	readonly removeResourceDirectory: (resourceScopeId: string, directoryId: string) => void;
	readonly reorderProject: (projectId: string, beforeProjectId: string | null) => Promise<unknown>;
	readonly reorderWorkspace: (
		workspaceId: string,
		beforeWorkspaceId: string | null,
	) => Promise<unknown>;
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
	readonly toggleRecentsExpansion: () => void;
	readonly updateWorkspace: (workspaceId: string, name: string, primaryDirectoryId: string) => void;
	readonly workspaceAgentOverviews: readonly WorkbenchWorkspaceAgentOverviewView[];
	/** A Workspace whose resources are decided elsewhere can only be renamed here. */
	readonly workspaceResourcesEditable: boolean;
	/** Absent where the authority cannot reach Git for its Workspaces. */
	readonly worktreeCommands?: WorkbenchWorktreeCommands | undefined;
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
	toggleRecentsExpansion,
	updateWorkspace,
	workspaceAgentOverviews,
	workspaceResourcesEditable,
	worktreeCommands,
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
	// Recents, the Workspaces outside any Project, is the last group, listed only while it has one.
	const navigationGroups = projectWorkspaceState.workspaces.some(
		(workspace) => workspace.projectId === null,
	)
		? [...projectWorkspaceState.projects, null]
		: projectWorkspaceState.projects;
	const standaloneActive = projectWorkspaceState.activeLocation.kind === 'standalone';
	const projectCreation = view.projectCreation;
	const projectManagement = view.projectManagement;
	const workspaceManagement = view.workspaceManagement;
	const workspaceActivation = useWorkspaceCreationActivation({
		subscribeNavigation,
		activateWorkspace: (workspaceId) => activateWorkspace(workspaceId, 'other'),
		projectWorkspaceState,
	});
	const workspaceWorktrees = useWorkspaceWorktrees({
		beginWorkspaceActivation: workspaceActivation.begin,
		commands: worktreeCommands,
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
				<div className={styles['projectSection']}>
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
						{/* Project and Recents headings name the groups; the list shows no heading of its own. */}
						<ul
							ref={projectReorder.listRef}
							aria-label={projectLabel}
							className={styles['projectList']}
							data-project-reorder-dragging={projectReorder.draggingItemId ?? undefined}
						>
							{navigationGroups.map((project) => (
								<ProjectNavigationItem
									key={project?.id ?? 'ungrouped'}
									agentGradientPrefix={`${agentOverview.agentGradientPrefix}-${project?.id ?? 'ungrouped'}`}
									activateWorkspace={(workspaceId, origin) => {
										workspaceActivation.cancel();
										activateWorkspace(workspaceId, origin);
									}}
									activeLocation={projectWorkspaceState.activeLocation}
									createWorkspace={createWorkspace}
									deleteWorkspace={deleteWorkspace}
									displacement={
										project !== null && projectReorder.displacedItemIds.includes(project.id)
											? projectReorder.displacementDirection
											: null
									}
									duplicateWorkspace={workspaceResourcesEditable ? duplicateWorkspace : undefined}
									expandedAgentWorkspaceId={agentOverview.expandedWorkspace?.workspaceId ?? null}
									environmentConnection={
										project === null
											? null
											: (view.connectionStatusByProjectId.get(project.id) ?? null)
									}
									connectEnvironment={environmentProjectConnection?.connect}
									disconnectEnvironment={environmentProjectConnection?.disconnect}
									expanded={
										project === null
											? projectWorkspaceState.recentsExpanded
											: projectWorkspaceState.expandedProjectIds.includes(project.id) &&
												projectReorder.draggingItemId !== project.id
									}
									dragging={projectReorder.draggingItemId === project?.id}
									messages={projectNavigationMessages}
									navigateAgent={(input) => {
										workspaceActivation.cancel();
										navigateAgent(input);
									}}
									project={project}
									refreshWorktrees={workspaceWorktrees.refresh}
									onPointerDown={projectReorder.onItemPointerDown}
									reorderActive={projectReorder.displacementAnimated}
									reorderWorkspace={reorderWorkspace}
									removeProject={removeProject}
									showProjectManager={view.showProjectManager}
									showWorkspaceManager={view.showWorkspaceManager}
									toggleProjectExpansion={toggleProjectExpansion}
									toggleRecentsExpansion={toggleRecentsExpansion}
									toggleWorkspaceAgentExpansion={agentOverview.toggleWorkspace}
									workspaceAgentOverviews={workspaceAgentOverviews}
									worktreeMenu={workspaceWorktrees.menu}
									state={projectWorkspaceState}
									setWorkspaceProject={setWorkspaceProject}
									showWorkspaceProjectCreation={view.showWorkspaceProjectCreation}
									closeProjectWorkspaces={closeProjectWorkspaces}
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
				</div>
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
					directoriesEditable={workspaceResourcesEditable}
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
		</>
	);
}
