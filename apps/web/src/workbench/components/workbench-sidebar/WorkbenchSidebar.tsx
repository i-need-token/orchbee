import {
	Button,
	Kbd,
	ProductIcon,
	Tooltip,
	TooltipContent,
	TooltipTrigger,
} from '@orchbee/ui/components';
import * as React from 'react';

import type { ProjectWorkspaceState } from '../../../project-workspace';
import type { WorkbenchAgentNavigationInput } from '../../model/project-workbench-agent-overview';
import type {
	WorkbenchAgentConfirmationChooseInput,
	WorkbenchPanelTargetInput,
} from '../../actions/workbench-action-ids';

import type { WorkbenchTerminalLocationActivationOrigin } from '../../model/workbench-terminal-location-activation';
import { NameDialog } from '../name-dialog/NameDialog';
import { ProjectNavigationItem } from './components/project-navigation-item/ProjectNavigationItem';
import { useWorkbenchSidebar } from './hooks/useWorkbenchSidebar';
import { useSidebarDrag } from './hooks/useSidebarDrag';
import { useSidebarScrollbar } from './hooks/useSidebarScrollbar';
import { useWorkbenchSidebarFocusRequest } from './hooks/useWorkbenchSidebarFocusRequest';
import { useWorkbenchSidebarKeyboard } from './hooks/useWorkbenchSidebarKeyboard';
import { useWorkbenchSidebarWorkspaceLocations } from './hooks/useWorkbenchSidebarWorkspaceLocations';
import { useWorkspaceAgentMenu } from './hooks/useWorkspaceAgentMenu';
import { useWorkspaceCreationActivation } from './hooks/useWorkspaceCreationActivation';
import { useWorkspaceWorktrees } from './hooks/useWorkspaceWorktrees';
import type {
	WorkbenchProjectNavigationMessages,
	WorkbenchSidebarWorkspaceCreation,
	WorkbenchWorktreeCommands,
} from './types/workbench-sidebar-types';
import type { WorkbenchWorkspaceAgentOverviewView } from '../../types/workbench-agent-overview';
import type { WorkbenchWorkspaceAgentLaunch } from '../../types/workbench-agent-start';
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
	readonly deleteWorkspace: (workspaceId: string) => void;
	/**
	 * A development or test build's marker at the footer's end: its word, the checkout name when the
	 * build came from one, and the tooltip that names both in full.
	 */
	readonly environmentMarker?:
		| {
				readonly checkout: string | null;
				readonly description: string | null;
				readonly label: string;
		  }
		| undefined;
	readonly navigateAgent: (input: WorkbenchAgentNavigationInput) => void;
	readonly chooseAgentConfirmation: (input: WorkbenchAgentConfirmationChooseInput) => void;
	readonly closeAgentPanel: (input: WorkbenchPanelTargetInput) => void;
	readonly renameAgentPanel: (input: WorkbenchPanelTargetInput) => void;
	readonly focusRequest: number;
	readonly label: string;
	readonly newWorkspaceLabel: string;
	readonly newWorkspaceShortcutId: string;
	readonly newWorkspaceShortcutLabel: string | undefined;
	/** Absent when the window withholds Search, which removes its item and key. */
	readonly onSearch?: ((returnFocus: HTMLElement) => void) | undefined;
	readonly onSettings?: (() => void) | undefined;
	readonly onShortcuts?: (() => void) | undefined;
	readonly projectLabel: string;
	readonly projectNavigationMessages: WorkbenchProjectNavigationMessages;
	readonly projectWorkspaceState: ProjectWorkspaceState;
	readonly subscribeNavigation: (listener: () => void) => () => void;
	readonly removeProject: (projectId: string) => void;
	/** The modifier that moves a row with ↑ and ↓: ⌘ on macOS, where ⌃↑ opens Mission Control. */
	readonly reorderModifier: 'ctrl' | 'meta';
	readonly reorderProject: (projectId: string, beforeProjectId: string | null) => Promise<unknown>;
	/** Moves a Workspace before another of `projectId`'s, or to the end of that list when null. */
	readonly moveWorkspace: (
		workspaceId: string,
		projectId: string | null,
		beforeWorkspaceId: string | null,
	) => Promise<unknown>;
	readonly renameProject: (projectId: string, name: string) => void;
	readonly returnToTerminal?: (() => void) | undefined;
	readonly searchLabel: string;
	readonly searchShortcutId: string;
	readonly searchShortcutLabel: string | undefined;
	readonly settingsLabel: string;
	readonly settingsShortcutId: string;
	readonly settingsShortcutLabel: string | undefined;
	readonly toggleProjectExpansion: (projectId: string) => void;
	readonly toggleRecentsExpansion: () => void;
	readonly toggleWorkspaceAgents: (workspaceId: string) => void;
	/**
	 * The footer's Update control at the row's end, present only while a newer version is offered:
	 * its label, the tooltip that names the version, and what a press invokes.
	 */
	readonly update?:
		| { readonly label: string; readonly onUpdate: () => void; readonly tooltip: string }
		| undefined;
	readonly updateWorkspace: (workspaceId: string, name: string) => void;
	readonly workspaceAgentOverviews: readonly WorkbenchWorkspaceAgentOverviewView[];
	readonly workspaceAgentLaunch?: WorkbenchWorkspaceAgentLaunch | undefined;
	readonly workspaceLocationSource?: WorkbenchWorkspaceLocationSource | undefined;
	/** Absent where the authority cannot reach Git for its Workspaces. */
	readonly worktreeCommands?: WorkbenchWorktreeCommands | undefined;
}

export function WorkbenchSidebar({
	activateWorkspace,
	createWorkspace,
	setWorkspaceProject,
	createWorkspaceProject,
	deleteWorkspace,
	environmentMarker,
	chooseAgentConfirmation,
	closeAgentPanel,
	navigateAgent,
	renameAgentPanel,
	focusRequest,
	label,
	newWorkspaceLabel,
	newWorkspaceShortcutId,
	newWorkspaceShortcutLabel,
	onSearch,
	onSettings,
	onShortcuts,
	projectLabel,
	projectNavigationMessages,
	projectWorkspaceState,
	subscribeNavigation,
	removeProject,
	reorderModifier,
	reorderProject,
	moveWorkspace,
	renameProject,
	returnToTerminal,
	searchLabel,
	searchShortcutId,
	searchShortcutLabel,
	settingsLabel,
	settingsShortcutId,
	settingsShortcutLabel,
	toggleProjectExpansion,
	toggleRecentsExpansion,
	toggleWorkspaceAgents,
	update,
	updateWorkspace,
	workspaceAgentOverviews,
	workspaceAgentLaunch,
	workspaceLocationSource,
	worktreeCommands,
}: WorkbenchSidebarProps): React.ReactNode {
	const workspaceLocations = useWorkbenchSidebarWorkspaceLocations(workspaceLocationSource);
	const agentMenu = useWorkspaceAgentMenu(workspaceAgentLaunch);
	const rootReference = useWorkbenchSidebarFocusRequest(focusRequest);
	const view = useWorkbenchSidebar(projectWorkspaceState);
	const locationNameMessages = {
		cancel: projectNavigationMessages.cancel,
		close: projectNavigationMessages.close,
		nameTooLong: projectNavigationMessages.nameTooLong,
	};
	const locationNameBlank = {
		kind: 'refuse',
		reason: projectNavigationMessages.nameRequired,
	} as const;
	const workspaceActivation = useWorkspaceCreationActivation({
		subscribeNavigation,
		activateWorkspace: (workspaceId) => activateWorkspace(workspaceId, 'other'),
		projectWorkspaceState,
	});
	const drag = useSidebarDrag({
		holdWorkspace: (workspaceId) => {
			const { activeLocation } = projectWorkspaceState;
			if (activeLocation.kind === 'workspace' && activeLocation.workspaceId === workspaceId) return;
			workspaceActivation.cancel();
			activateWorkspace(workspaceId, 'other');
		},
		moveWorkspace,
		reorderProject,
		state: projectWorkspaceState,
	});
	// While a drag shows a move, the list shows its order; everything else stays canonical.
	const shownState = React.useMemo(
		() =>
			drag.order === projectWorkspaceState
				? projectWorkspaceState
				: { ...projectWorkspaceState, ...drag.order },
		[drag.order, projectWorkspaceState],
	);
	const heldWorkspaceId = drag.held?.kind === 'workspace' ? drag.held.id : null;
	// Recents, the Workspaces outside any Project, is the last group, listed only while it has one.
	const navigationGroups = shownState.workspaces.some((workspace) => workspace.projectId === null)
		? [...shownState.projects, null]
		: shownState.projects;
	const projectRename = view.projectRename;
	const workspaceProjectCreation = view.workspaceProjectCreation;
	const workspaceRename = view.workspaceRename;
	const keyboard = useWorkbenchSidebarKeyboard({
		activateWorkspace: (workspaceId) => {
			workspaceActivation.cancel();
			activateWorkspace(workspaceId, 'keyboard');
		},
		createWorkspace: () => {
			workspaceActivation.cancel();
			createWorkspace();
		},
		onSearch,
		onShortcuts,
		reorderModifier,
		reorderProject,
		moveWorkspace,
		returnToTerminal,
		state: projectWorkspaceState,
	});
	const scrollbar = useSidebarScrollbar(keyboard.listReference);
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
							data-shortcut-action={newWorkspaceShortcutId}
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
							{newWorkspaceShortcutLabel === undefined ? null : (
								<Kbd className={styles['shortcut']}>{newWorkspaceShortcutLabel}</Kbd>
							)}
						</Button>
					</li>
					{onSearch === undefined ? null : (
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
					)}
				</ul>
				<div ref={scrollbar.sectionReference} className={styles['projectSection']}>
					<div
						ref={keyboard.listReference}
						className={styles['projectRegion']}
						data-navigation-reorder-scroll='true'
						onBlur={keyboard.onBlur}
						onFocus={keyboard.onFocus}
						onKeyDown={keyboard.onKeyDown}
					>
						{/* Project and Recents headings name the groups; the list shows no heading of its own. */}
						<ul
							aria-label={projectLabel}
							className={styles['projectList']}
							data-sidebar-dragging={drag.held === null ? undefined : drag.held.kind}
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
									expanded={
										project === null
											? projectWorkspaceState.recentsExpanded
											: projectWorkspaceState.expandedProjectIds.includes(project.id)
									}
									dragging={
										project !== null && drag.held?.kind === 'project' && drag.held.id === project.id
									}
									heldWorkspaceId={heldWorkspaceId}
									messages={projectNavigationMessages}
									navigateAgent={(input) => {
										workspaceActivation.cancel();
										navigateAgent(input);
									}}
									chooseAgentConfirmation={chooseAgentConfirmation}
									closeAgentPanel={closeAgentPanel}
									renameAgentPanel={renameAgentPanel}
									project={project}
									projectWorktreeRows={workspaceWorktrees.projectRows}
									refreshWorktrees={workspaceWorktrees.refresh}
									onEntryPointerDown={drag.onEntryPointerDown}
									removeProject={removeProject}
									showProjectRename={view.showProjectRename}
									showWorkspaceRename={view.showWorkspaceRename}
									toggleProjectExpansion={toggleProjectExpansion}
									toggleRecentsExpansion={toggleRecentsExpansion}
									toggleWorkspaceAgents={toggleWorkspaceAgents}
									workspaceAgentOverviews={workspaceAgentOverviews}
									agentMenu={agentMenu}
									workspaceLocations={workspaceLocations}
									worktreeMenu={workspaceWorktrees.menu}
									state={shownState}
									setWorkspaceProject={setWorkspaceProject}
									showWorkspaceProjectCreation={view.showWorkspaceProjectCreation}
								/>
							))}
						</ul>
						{/* The drop area exists only while a Project is dragged, after the last one. */}
						{drag.dropArea === null ? null : (
							<div
								className={styles['dropArea']}
								data-hovered={drag.dropAreaHovered || undefined}
								data-sidebar-drop-area={drag.dropArea}
							>
								{projectNavigationMessages.dropEndOfProjects}
							</div>
						)}
					</div>
					<span aria-hidden='true' className={styles['scrollEdge']} data-edge='start' />
					<span aria-hidden='true' className={styles['scrollEdge']} data-edge='end' />
					<div
						ref={scrollbar.scrollbarReference}
						aria-hidden='true'
						className={styles['scrollbar']}
						data-slot='workbench-sidebar-scrollbar'
					>
						<span className={styles['scrollThumb']} />
					</div>
				</div>
				{/* Holds the raised copy of a dragged row, so it outlives the row's move between lists. */}
				<div aria-hidden='true' className={styles['dragLayer']} data-sidebar-drag-layer='true' />
				<div className={styles['footer']}>
					{/* Settings: the gear alone at the footer's start, as in Cursor; its tooltip names it. */}
					<Tooltip>
						<TooltipTrigger
							render={
								<button
									aria-disabled={onSettings === undefined}
									aria-label={settingsLabel}
									className={styles['settings']}
									data-shortcut-action={settingsShortcutId}
									data-slot='workbench-sidebar-settings'
									type='button'
									onClick={onSettings}
								/>
							}
						>
							<ProductIcon name='settings' />
						</TooltipTrigger>
						<TooltipContent side='top'>
							{settingsLabel}
							{settingsShortcutLabel === undefined ? null : <Kbd>{settingsShortcutLabel}</Kbd>}
						</TooltipContent>
					</Tooltip>
					{environmentMarker === undefined ? null : (
						<Tooltip disabled={environmentMarker.description === null}>
							<TooltipTrigger
								render={
									<span
										className={styles['environment']}
										data-slot='workbench-sidebar-environment'
									/>
								}
							>
								{/* Anchors the first line, so even the word wraps out of sight when it cannot fit. */}
								<span aria-hidden='true' className={styles['environmentAnchor']} />
								<span className={styles['environmentPart']}>
									<span className={styles['environmentText']}>{environmentMarker.label}</span>
								</span>
								{environmentMarker.checkout === null ? null : (
									<span className={styles['environmentPart']} data-checkout=''>
										<span aria-hidden='true' className={styles['environmentText']}>
											·
										</span>
										<span className={styles['environmentText']}>{environmentMarker.checkout}</span>
									</span>
								)}
							</TooltipTrigger>
							<TooltipContent side='top'>{environmentMarker.description}</TooltipContent>
						</Tooltip>
					)}
					{/* Update: the primary button that ends the row while a newer version is offered. */}
					{update === undefined ? null : (
						<Tooltip>
							<TooltipTrigger
								render={
									<Button
										className={styles['update']}
										data-slot='workbench-sidebar-update'
										size='sm'
										type='button'
										onClick={update.onUpdate}
									/>
								}
							>
								<span className={styles['updateText']}>{update.label}</span>
							</TooltipTrigger>
							<TooltipContent side='top'>{update.tooltip}</TooltipContent>
						</Tooltip>
					)}
				</div>
			</aside>
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
		</>
	);
}
