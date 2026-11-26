import {
	ContextMenu,
	ContextMenuTrigger,
	ProductIcon,
	Tooltip,
	TooltipContent,
	TooltipTrigger,
} from '@orchbee/ui/components';
import type { PointerEvent, ReactNode } from 'react';
import type { TerminalPresentationId } from '@orchbee/workbench-client-protocol';

import type { ProjectWorkspaceState, WorkbenchProject } from '../../../../../project-workspace';
import type { WorkbenchMachineMark } from '../../../../types/workbench-other-machines';
import type { WorkbenchAgentNavigationInput } from '../../../../model/project-workbench-agent-overview';
import type {
	WorkbenchAgentConfirmationChooseInput,
	WorkbenchPanelTargetInput,
} from '../../../../actions/workbench-action-ids';

import type { WorkbenchTerminalLocationActivationOrigin } from '../../../../model/workbench-terminal-location-activation';
import { WorkbenchActionContextMenuContent } from '../../../workbench-action-context-menu-content/WorkbenchActionContextMenuContent';
import type { WorkbenchActionMenuEntry } from '../../../../types/workbench-action-menu';
import type { WorkbenchWorkspaceAgentOverviewView } from '../../../../types/workbench-agent-overview';
import type { SidebarDragItem } from '../../model/sidebar-drag-destination';
import type { WorkbenchSidebarWorkspaceLocation } from '../../hooks/useWorkbenchSidebarWorkspaceLocations';
import type { WorkspaceAgentMenu } from '../../hooks/useWorkspaceAgentMenu';
import type { ProjectWorktreeRows, WorkspaceWorktreeMenu } from '../../hooks/useWorkspaceWorktrees';
import type {
	WorkbenchProjectNavigationMessages,
	WorkbenchSidebarWorkspaceCreation,
	WorkspaceMachineMenu,
} from '../../types/workbench-sidebar-types';
import { SidebarTruncationTooltip } from '../sidebar-truncation-tooltip/SidebarTruncationTooltip';
import { WorkbenchSidebarDropdownMenu } from '../workbench-sidebar-dropdown-menu/WorkbenchSidebarDropdownMenu';
import { WorkspaceNavigationRow } from './components/workspace-navigation-row/WorkspaceNavigationRow';
import { OtherMachineWorkspaceRow } from './components/other-machine-workspace-row/OtherMachineWorkspaceRow';
import { useProjectNavigationMenuFocus } from './hooks/useProjectNavigationMenuFocus';
import {
	navigationMenuAriaKeyShortcuts,
	navigationRowAriaKeyShortcuts,
} from './utils/navigation-menu-shortcuts';
import {
	handleNavigationRowKeyDown,
	suppressDuplicateNavigationContextMenu,
} from './utils/navigation-row-menu';
import { projectNewWorkspaceMenu } from './utils/projectNewWorkspaceMenu';
import styles from './ProjectNavigationItem.module.css';

interface ProjectNavigationItemProps {
	readonly activateWorkspace: (
		workspaceId: string,
		origin: WorkbenchTerminalLocationActivationOrigin,
	) => void;
	readonly activeLocation: ProjectWorkspaceState['activeLocation'];
	readonly createWorkspace: (input: WorkbenchSidebarWorkspaceCreation) => void;
	readonly state: ProjectWorkspaceState;
	readonly setWorkspaceProject: (workspaceId: string, projectId: string | null) => void;
	readonly showWorkspaceProjectCreation: (workspaceId: string) => void;
	readonly deleteWorkspace: (workspaceId: string) => void;
	/** Held by a drag: a Project keeps its look and moves with its Workspaces. */
	readonly dragging: boolean;
	readonly expanded: boolean;
	/**
	 * A Project only other machines hold: its heading names it and folds, with no menu, tools or
	 * drag, which act on the Session on screen.
	 */
	readonly foreign: boolean;
	/** The machine on screen while it is another computer, whose mark its Workspaces carry. */
	readonly onScreenMark: WorkbenchMachineMark | null;
	readonly openOtherWorkspace: (machine: string, workspaceId: string) => void;
	/** Other machines' Workspaces in this group, before and after the Session on screen's. */
	readonly otherRows: {
		readonly leading: readonly SidebarOtherMachineRow[];
		readonly trailing: readonly SidebarOtherMachineRow[];
	};
	/** The Workspace a drag holds; a folded group shows it alone while it stands there. */
	readonly heldWorkspaceId: string | null;
	readonly messages: WorkbenchProjectNavigationMessages;
	readonly navigateAgent: (input: WorkbenchAgentNavigationInput) => void;
	readonly acknowledgeAgentCompletion?:
		| ((presentationId: TerminalPresentationId) => void)
		| undefined;
	readonly chooseAgentConfirmation: (input: WorkbenchAgentConfirmationChooseInput) => void;
	readonly closeAgentPanel: (input: WorkbenchPanelTargetInput) => void;
	readonly renameAgentPanel: (input: WorkbenchPanelTargetInput) => void;
	readonly project: WorkbenchProject | null;
	readonly projectWorktreeRows: (sourceWorkspaceId: string | null) => ProjectWorktreeRows;
	/**
	 * Reads a Workspace's repository again. A Workspace menu calls it for its Workspace when it opens,
	 * and a Project menu for the Project's first Workspace.
	 */
	readonly refreshWorktrees: (workspaceId: string) => void;
	readonly onEntryPointerDown: (item: SidebarDragItem, event: PointerEvent<HTMLElement>) => void;
	readonly removeProject: (projectId: string) => void;
	readonly showProjectRename: (projectId: string) => void;
	readonly showWorkspaceRename: (workspaceId: string) => void;
	readonly toggleProjectExpansion: (projectId: string) => void;
	readonly toggleRecentsExpansion: () => void;
	readonly toggleWorkspaceAgents: (workspaceId: string) => void;
	readonly workspaceAgentOverviews: readonly WorkbenchWorkspaceAgentOverviewView[];
	readonly agentMenu?: WorkspaceAgentMenu | undefined;
	/** Change machine in each Workspace's menu, where the window holds other machines' Sessions. */
	readonly machineMenu?: WorkspaceMachineMenu | undefined;
	readonly workspaceLocations: ReadonlyMap<string, WorkbenchSidebarWorkspaceLocation>;
	readonly worktreeMenu: (workspaceId: string) => WorkspaceWorktreeMenu;
}

/** One other machine's Workspace in a group. */
export interface SidebarOtherMachineRow {
	readonly machine: string;
	readonly mark: WorkbenchMachineMark | null;
	readonly name: string;
	/** The state word of a machine that is not connected; null while connected. */
	readonly word: string | null;
	readonly workspaceId: string;
}

const NO_OTHER_ROWS: ProjectNavigationItemProps['otherRows'] = { leading: [], trailing: [] };

/** The + menu: New Workspace's rows as one menu, one click shorter, its groups kept apart. */
function flattenSubmenu(entry: WorkbenchActionMenuEntry): readonly WorkbenchActionMenuEntry[] {
	if (entry.kind !== 'submenu') return [entry];
	return entry.groups.flatMap((group, index): WorkbenchActionMenuEntry[] => [
		...(index === 0 ? [] : [{ id: `${group.id}-separator`, kind: 'separator' as const }]),
		...group.commands,
	]);
}

export function ProjectNavigationItem({
	activateWorkspace,
	activeLocation,
	createWorkspace,
	state,
	setWorkspaceProject,
	showWorkspaceProjectCreation,
	deleteWorkspace,
	dragging,
	expanded,
	foreign,
	onScreenMark,
	openOtherWorkspace,
	otherRows,
	heldWorkspaceId,
	acknowledgeAgentCompletion,
	chooseAgentConfirmation,
	closeAgentPanel,
	messages,
	navigateAgent,
	renameAgentPanel,
	project,
	projectWorktreeRows,
	refreshWorktrees,
	onEntryPointerDown,
	removeProject,
	showProjectRename,
	showWorkspaceRename,
	toggleProjectExpansion,
	toggleRecentsExpansion,
	toggleWorkspaceAgents,
	workspaceAgentOverviews,
	agentMenu,
	machineMenu,
	workspaceLocations,
	worktreeMenu,
}: ProjectNavigationItemProps): ReactNode {
	const menuFocus = useProjectNavigationMenuFocus();
	const workspaces = foreign
		? []
		: state.workspaces.filter((workspace) => workspace.projectId === (project?.id ?? null));
	// A folded heading is never current, but sidebar focus lands on the one hiding the active Workspace.
	const holdsHiddenActiveWorkspace =
		!expanded &&
		activeLocation.kind === 'workspace' &&
		workspaces.some((workspace) => workspace.id === activeLocation.workspaceId);
	// A folded group stays folded under a drag, yet shows the held Workspace standing in it.
	const shownWorkspaces = expanded
		? workspaces
		: workspaces.filter((workspace) => workspace.id === heldWorkspaceId);
	const shownOtherRows = expanded ? otherRows : NO_OTHER_ROWS;
	const otherRow = (row: SidebarOtherMachineRow): ReactNode => (
		<OtherMachineWorkspaceRow
			key={`${row.machine}/${row.workspaceId}`}
			mark={row.mark}
			name={row.name}
			onOpen={() => openOtherWorkspace(row.machine, row.workspaceId)}
			word={row.word}
		/>
	);
	// A Project's Worktree rows read the repository of its first Workspace, as the TUI's do.
	const worktreeSourceId = workspaces[0]?.id ?? null;
	const refreshProjectWorktrees = (open: boolean) => {
		if (open && worktreeSourceId !== null) refreshWorktrees(worktreeSourceId);
	};
	const newWorkspaceMenu =
		project === null
			? null
			: projectNewWorkspaceMenu({
					createWorkspace,
					messages,
					project,
					worktrees: projectWorktreeRows(worktreeSourceId),
				});
	const projectMenuEntries: readonly WorkbenchActionMenuEntry[] =
		project === null || newWorkspaceMenu === null
			? []
			: [
					newWorkspaceMenu,
					{
						icon: 'edit',
						id: 'rename-project',
						kind: 'command',
						label: messages.projectMenuRename,
						onSelect: () => showProjectRename(project.id),
					},
					{ id: 'dissolve-project-separator', kind: 'separator' },
					// Dissolving keeps every Workspace, so it is not destructive.
					{
						icon: 'ungroup',
						id: 'dissolve-project',
						kind: 'command',
						label: messages.projectMenuDissolve,
						onSelect: () => removeProject(project.id),
					},
				];
	const name = project?.name ?? messages.recents;
	const projectFoldLabel = expanded ? messages.collapseProject : messages.expandProject;
	const recentsFoldLabel = expanded ? messages.collapseRecents : messages.expandRecents;
	const chevronLabel = project === null ? recentsFoldLabel : `${projectFoldLabel}: ${project.name}`;
	// Headings are never current and never filled; only the chevron folds the group. It shows while
	// the group is folded, and while open only under the pointer or focus, as the TUI's does.
	const headingContent = (
		<>
			<SidebarTruncationTooltip content={name} render={<span className={styles['headingName']} />}>
				{name}
			</SidebarTruncationTooltip>
			<Tooltip>
				<TooltipTrigger
					render={
						<button
							aria-expanded={expanded}
							aria-keyshortcuts={project === null ? undefined : navigationRowAriaKeyShortcuts}
							aria-label={chevronLabel}
							className={styles['chevron']}
							data-holds-active-workspace={holdsHiddenActiveWorkspace || undefined}
							data-navigation-reorder-excluded='true'
							data-sidebar-nav='heading'
							data-slot={
								project === null ? 'workbench-sidebar-recents' : 'workbench-sidebar-project'
							}
							tabIndex={-1}
							type='button'
							onClick={() =>
								project === null ? toggleRecentsExpansion() : toggleProjectExpansion(project.id)
							}
						/>
					}
				>
					<span className={styles['chevronIcon']} data-open={expanded || undefined}>
						<ProductIcon name='chevron-right' size='disclosure' />
					</span>
				</TooltipTrigger>
				<TooltipContent side='bottom'>
					{project === null ? recentsFoldLabel : projectFoldLabel}
				</TooltipContent>
			</Tooltip>
		</>
	);
	return (
		<li
			className={styles['project']}
			data-dragging={dragging || undefined}
			data-project-id={foreign ? undefined : project?.id}
		>
			{project === null || newWorkspaceMenu === null || foreign ? (
				// Recents only folds: it has no menu and is neither held nor a destination.
				<div className={styles['heading']} data-recents-row='true'>
					{headingContent}
				</div>
			) : (
				<ContextMenu
					onOpenChange={(open, details) => {
						refreshProjectWorktrees(open);
						menuFocus.onOpenChange(null, open, details);
					}}
				>
					<ContextMenuTrigger
						render={
							<div
								className={styles['heading']}
								data-drag-entry='project'
								data-drag-id={project.id}
								data-navigation-reorder-row='true'
								data-project-reorder-row='true'
								onContextMenuCapture={suppressDuplicateNavigationContextMenu}
								onKeyDown={(event) =>
									handleNavigationRowKeyDown(event, () => showProjectRename(project.id))
								}
								onPointerDown={(event) =>
									onEntryPointerDown({ id: project.id, kind: 'project' }, event)
								}
							/>
						}
					>
						{headingContent}
						<span className={styles['tools']}>
							<Tooltip>
								<WorkbenchSidebarDropdownMenu
									onOpenChange={refreshProjectWorktrees}
									entries={flattenSubmenu(newWorkspaceMenu)}
									trigger={
										<TooltipTrigger
											render={
												<button
													aria-label={`${messages.createWorkspace}: ${project.name}`}
													className={styles['tool']}
													data-navigation-reorder-excluded='true'
													tabIndex={-1}
													type='button'
												/>
											}
										/>
									}
									triggerContent={<ProductIcon name='add' />}
								/>
								<TooltipContent side='bottom'>{messages.createWorkspace}</TooltipContent>
							</Tooltip>
							<Tooltip>
								<WorkbenchSidebarDropdownMenu
									onOpenChange={refreshProjectWorktrees}
									entries={projectMenuEntries}
									trigger={
										<TooltipTrigger
											render={
												<button
													aria-label={`${messages.projectOptions}: ${project.name}`}
													aria-keyshortcuts={navigationMenuAriaKeyShortcuts}
													className={styles['tool']}
													data-navigation-menu-trigger='true'
													data-navigation-reorder-excluded='true'
													tabIndex={-1}
													type='button'
												/>
											}
										/>
									}
									triggerContent={<ProductIcon name='ellipsis' />}
								/>
								<TooltipContent side='bottom'>{messages.projectOptions}</TooltipContent>
							</Tooltip>
						</span>
					</ContextMenuTrigger>
					<WorkbenchActionContextMenuContent
						entries={projectMenuEntries}
						finalFocus={(closeType) => menuFocus.finalFocus(null, closeType)}
					/>
				</ContextMenu>
			)}
			{shownWorkspaces.length + shownOtherRows.leading.length + shownOtherRows.trailing.length >
				0 && (
				<ul className={styles['workspaceList']}>
					{shownOtherRows.leading.map(otherRow)}
					{shownWorkspaces.map((workspace) => (
						<WorkspaceNavigationRow
							key={workspace.id}
							active={
								activeLocation.kind === 'workspace' && activeLocation.workspaceId === workspace.id
							}
							activateWorkspace={activateWorkspace}
							agentOverview={workspaceAgentOverviews.find(
								(overview) => overview.workspaceId === workspace.id,
							)}
							agentMenu={agentMenu}
							machineMenu={machineMenu}
							deleteWorkspace={deleteWorkspace}
							dragging={workspace.id === heldWorkspaceId}
							location={workspaceLocations.get(workspace.id)}
							menuFocus={menuFocus}
							messages={messages}
							navigateAgent={navigateAgent}
							acknowledgeAgentCompletion={acknowledgeAgentCompletion}
							chooseAgentConfirmation={chooseAgentConfirmation}
							closeAgentPanel={closeAgentPanel}
							renameAgentPanel={renameAgentPanel}
							onPointerDown={(event) =>
								onEntryPointerDown({ id: workspace.id, kind: 'workspace' }, event)
							}
							refreshWorktrees={refreshWorktrees}
							setWorkspaceProject={setWorkspaceProject}
							showWorkspaceProjectCreation={showWorkspaceProjectCreation}
							showWorkspaceRename={showWorkspaceRename}
							state={state}
							toggleWorkspaceAgents={toggleWorkspaceAgents}
							workspace={workspace}
							worktrees={worktreeMenu(workspace.id)}
							machineMark={onScreenMark}
						/>
					))}
					{shownOtherRows.trailing.map(otherRow)}
				</ul>
			)}
		</li>
	);
}
