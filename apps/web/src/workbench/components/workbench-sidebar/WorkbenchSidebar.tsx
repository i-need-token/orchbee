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
import type {
	WorkbenchProjectFolderAcquisition,
	WorkbenchProjectCreationReceipt,
} from '../../types/workbench-project-folder-acquisition';
import { CreateProjectDialog } from './components/create-project-dialog/CreateProjectDialog';
import { CreateProjectFolderDialog } from './components/create-project-folder-dialog/CreateProjectFolderDialog';
import { useNewProjectFolder } from './hooks/useNewProjectFolder';
import { NameDialog } from '../name-dialog/NameDialog';
import { ProjectCreateMenu } from './components/project-create-menu/ProjectCreateMenu';
import { ProjectNavigationItem } from './components/project-navigation-item/ProjectNavigationItem';
import { useWorkbenchSidebar } from './hooks/useWorkbenchSidebar';
import { useNavigationReorderDrag } from './hooks/useNavigationReorderDrag';
import { useProjectFolderAcquisition } from './hooks/useProjectFolderAcquisition';
import { useWorkbenchSidebarFocusRequest } from './hooks/useWorkbenchSidebarFocusRequest';
import { useWorkbenchSidebarWorkspaceLocations } from './hooks/useWorkbenchSidebarWorkspaceLocations';
import { useWorkspaceCreationActivation } from './hooks/useWorkspaceCreationActivation';
import { useWorkspaceWorktrees } from './hooks/useWorkspaceWorktrees';
import type {
	WorkbenchProjectNavigationMessages,
	WorkbenchSidebarWorkspaceCreation,
	WorkbenchWorktreeCommands,
} from './types/workbench-sidebar-types';
import type { WorkbenchWorkspaceAgentOverviewView } from '../../types/workbench-agent-overview';
import type { WorkbenchWorkspaceLocationSource } from '../../types/workbench-workspace-location';
import styles from './WorkbenchSidebar.module.css';

interface WorkbenchSidebarProps {
	readonly activateWorkspace: (
		workspaceId: string,
		origin: WorkbenchTerminalLocationActivationOrigin,
	) => void;
	readonly createWorkspace: (input?: WorkbenchSidebarWorkspaceCreation) => void;
	readonly setWorkspaceProject: (workspaceId: string, projectId: string | null) => void;
	readonly createWorkspaceProject: (workspaceId: string, name: string) => void;
	readonly createProject: (path: string) => ActionExecution<WorkbenchProjectCreationReceipt>;
	readonly activateCreatedProject: (
		receipt: WorkbenchProjectCreationReceipt,
	) => ActionExecution<void>;
	readonly isCreatedProjectActive: (receipt: WorkbenchProjectCreationReceipt) => boolean;
	readonly deleteWorkspace: (workspaceId: string) => void;
	readonly navigateAgent: (input: WorkbenchAgentNavigationInput) => void;
	readonly focusRequest: number;
	readonly label: string;
	readonly newWorkspaceLabel: string;
	readonly onSearch: (returnFocus: HTMLElement) => void;
	readonly onSettings?: (() => void) | undefined;
	readonly projectLabel: string;
	readonly projectNavigationMessages: WorkbenchProjectNavigationMessages;
	readonly projectFolderAcquisition?: WorkbenchProjectFolderAcquisition | undefined;
	readonly projectWorkspaceState: ProjectWorkspaceState;
	readonly subscribeNavigation: (listener: () => void) => () => void;
	readonly removeProject: (projectId: string) => void;
	readonly reorderProject: (projectId: string, beforeProjectId: string | null) => Promise<unknown>;
	readonly reorderWorkspace: (
		workspaceId: string,
		beforeWorkspaceId: string | null,
	) => Promise<unknown>;
	readonly renameProject: (projectId: string, name: string) => void;
	readonly searchLabel: string;
	readonly searchShortcutId: string;
	readonly searchShortcutLabel: string | undefined;
	readonly settingsLabel: string;
	readonly settingsShortcutId: string;
	readonly settingsShortcutLabel: string | undefined;
	readonly toggleProjectExpansion: (projectId: string) => void;
	readonly toggleRecentsExpansion: () => void;
	readonly toggleWorkspaceAgents: (workspaceId: string) => void;
	readonly updateWorkspace: (workspaceId: string, name: string) => void;
	readonly workspaceAgentOverviews: readonly WorkbenchWorkspaceAgentOverviewView[];
	readonly workspaceLocationSource?: WorkbenchWorkspaceLocationSource | undefined;
	/** Absent where the authority cannot reach Git for its Workspaces. */
	readonly worktreeCommands?: WorkbenchWorktreeCommands | undefined;
}

export function WorkbenchSidebar({
	activateWorkspace,
	createWorkspace,
	setWorkspaceProject,
	createWorkspaceProject,
	createProject,
	activateCreatedProject,
	isCreatedProjectActive,
	deleteWorkspace,
	navigateAgent,
	focusRequest,
	label,
	newWorkspaceLabel,
	onSearch,
	onSettings,
	projectLabel,
	projectNavigationMessages,
	projectFolderAcquisition,
	projectWorkspaceState,
	subscribeNavigation,
	removeProject,
	reorderProject,
	reorderWorkspace,
	renameProject,
	searchLabel,
	searchShortcutId,
	searchShortcutLabel,
	settingsLabel,
	settingsShortcutId,
	settingsShortcutLabel,
	toggleProjectExpansion,
	toggleRecentsExpansion,
	toggleWorkspaceAgents,
	updateWorkspace,
	workspaceAgentOverviews,
	workspaceLocationSource,
	worktreeCommands,
}: WorkbenchSidebarProps): ReactNode {
	const workspaceLocations = useWorkbenchSidebarWorkspaceLocations(workspaceLocationSource);
	const rootReference = useWorkbenchSidebarFocusRequest(focusRequest);
	const view = useWorkbenchSidebar(projectWorkspaceState);
	const folderAcquisition = useProjectFolderAcquisition({
		acquisition: projectFolderAcquisition,
		createProject,
	});
	const locationNameMessages = {
		cancel: projectNavigationMessages.cancel,
		close: projectNavigationMessages.close,
		nameTooLong: projectNavigationMessages.nameTooLong,
	};
	const locationNameBlank = {
		kind: 'refuse',
		reason: projectNavigationMessages.nameRequired,
	} as const;
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
	const projectCreation = view.projectCreation;
	const projectRename = view.projectRename;
	const workspaceProjectCreation = view.workspaceProjectCreation;
	const workspaceRename = view.workspaceRename;
	const workspaceActivation = useWorkspaceCreationActivation({
		subscribeNavigation,
		activateWorkspace: (workspaceId) => activateWorkspace(workspaceId, 'other'),
		projectWorkspaceState,
	});
	const workspaceWorktrees = useWorkspaceWorktrees({
		beginWorkspaceActivation: workspaceActivation.begin,
		commands: worktreeCommands,
		locations: workspaceLocations,
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
							{searchShortcutLabel === undefined ? null : (
								<Kbd className={styles['shortcut']}>{searchShortcutLabel}</Kbd>
							)}
						</Button>
					</li>
				</ul>
				<div className={styles['projectSection']}>
					<div className={styles['projectRegion']} data-navigation-reorder-scroll='true'>
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
									projectWorktreeRows={workspaceWorktrees.projectRows}
									refreshWorktrees={workspaceWorktrees.refresh}
									onPointerDown={projectReorder.onItemPointerDown}
									reorderActive={projectReorder.displacementAnimated}
									reorderWorkspace={reorderWorkspace}
									removeProject={removeProject}
									showProjectRename={view.showProjectRename}
									showWorkspaceRename={view.showWorkspaceRename}
									toggleProjectExpansion={toggleProjectExpansion}
									toggleRecentsExpansion={toggleRecentsExpansion}
									toggleWorkspaceAgents={toggleWorkspaceAgents}
									workspaceAgentOverviews={workspaceAgentOverviews}
									workspaceLocations={workspaceLocations}
									worktreeMenu={workspaceWorktrees.menu}
									state={projectWorkspaceState}
									setWorkspaceProject={setWorkspaceProject}
									showWorkspaceProjectCreation={view.showWorkspaceProjectCreation}
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
						{settingsShortcutLabel === undefined ? null : (
							<Kbd className={styles['shortcut']}>{settingsShortcutLabel}</Kbd>
						)}
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
			{projectRename === null ? null : (
				<NameDialog
					blank={locationNameBlank}
					messages={locationNameMessages}
					name={projectRename.name}
					onClose={projectRename.close}
					onNameChange={projectRename.setName}
					onSubmit={(name) => {
						renameProject(projectRename.project.id, name);
						projectRename.close();
					}}
					submitLabel={projectNavigationMessages.renameSubmit}
					title={projectNavigationMessages.projectRenameTitle}
				/>
			)}
			{workspaceProjectCreation === null ? null : (
				<NameDialog
					blank={locationNameBlank}
					messages={locationNameMessages}
					name={workspaceProjectCreation.name}
					onClose={workspaceProjectCreation.close}
					onNameChange={workspaceProjectCreation.setName}
					onSubmit={(name) => {
						createWorkspaceProject(workspaceProjectCreation.workspaceId, name);
						workspaceProjectCreation.close();
					}}
					submitLabel={projectNavigationMessages.create}
					title={projectNavigationMessages.workspaceCreateProject}
				/>
			)}
			{workspaceRename === null ? null : (
				<NameDialog
					blank={locationNameBlank}
					messages={locationNameMessages}
					name={workspaceRename.name}
					onClose={workspaceRename.close}
					onNameChange={workspaceRename.setName}
					onSubmit={(name) => {
						updateWorkspace(workspaceRename.workspace.id, name);
						workspaceRename.close();
					}}
					submitLabel={projectNavigationMessages.renameSubmit}
					title={projectNavigationMessages.workspaceRenameTitle}
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
