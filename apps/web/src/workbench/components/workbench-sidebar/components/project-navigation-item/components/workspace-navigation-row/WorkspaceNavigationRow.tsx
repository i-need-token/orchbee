import {
	ContextMenu,
	ContextMenuTrigger,
	ProductIcon,
	Tooltip,
	TooltipContent,
	TooltipTrigger,
} from '@orchbee/ui/components';
import type { MouseEvent, PointerEvent, ReactNode } from 'react';

import type {
	ProjectWorkspaceState,
	WorkbenchWorkspace,
} from '../../../../../../../project-workspace';
import type { WorkbenchAgentNavigationInput } from '../../../../../../model/project-workbench-agent-overview';
import type { WorkbenchTerminalLocationActivationOrigin } from '../../../../../../model/workbench-terminal-location-activation';
import type { WorkbenchPanelTargetInput } from '../../../../../../actions/workbench-action-ids';
import type { WorkbenchActionMenuEntry } from '../../../../../../types/workbench-action-menu';
import type { WorkbenchWorkspaceAgentOverviewView } from '../../../../../../types/workbench-agent-overview';
import { WorkbenchActionContextMenuContent } from '../../../../../workbench-action-context-menu-content/WorkbenchActionContextMenuContent';
import type { WorkbenchSidebarWorkspaceLocation } from '../../../../hooks/useWorkbenchSidebarWorkspaceLocations';
import type { WorkspaceWorktreeMenu } from '../../../../hooks/useWorkspaceWorktrees';
import type { WorkbenchProjectNavigationMessages } from '../../../../types/workbench-sidebar-types';
import { SidebarTruncationTooltip } from '../../../sidebar-truncation-tooltip/SidebarTruncationTooltip';
import { WorkbenchSidebarDropdownMenu } from '../../../workbench-sidebar-dropdown-menu/WorkbenchSidebarDropdownMenu';
import type { ProjectNavigationMenuFocus } from '../../hooks/useProjectNavigationMenuFocus';
import {
	navigationMenuAriaKeyShortcuts,
	navigationRowAriaKeyShortcuts,
} from '../../utils/navigation-menu-shortcuts';
import {
	handleNavigationRowKeyDown,
	suppressDuplicateNavigationContextMenu,
} from '../../utils/navigation-row-menu';
import { workspaceMoveToProjectMenu } from '../../utils/workspaceMoveToProjectMenu';
import { WorkspaceAgentList } from '../workspace-agent-list/WorkspaceAgentList';
import { WorkspaceAgentStatusLine } from '../workspace-agent-status-line/WorkspaceAgentStatusLine';
import styles from './WorkspaceNavigationRow.module.css';

/** Clears the second line, so a cut name's tooltip opens below the whole row. */
const NAME_TOOLTIP_OFFSET = 20;

/**
 * A Workspace's Agents in words. Folded, the row speaks for its hidden Agent rows: every state its
 * second line shows, and their unread marker.
 */
function workspaceAgentSummary(overview: WorkbenchWorkspaceAgentOverviewView): string {
	if (overview.expanded) {
		return `${overview.countLabel}. ${overview.agentLabel}: ${overview.status.status}`;
	}
	const unread = overview.unreadLabel === undefined ? '' : ` · ${overview.unreadLabel}`;
	return `${overview.countLabel}. ${overview.agentLabel}: ${overview.foldedStatus.label}${unread}`;
}

interface WorkspaceNavigationRowProps {
	readonly active: boolean;
	readonly activateWorkspace: (
		workspaceId: string,
		origin: WorkbenchTerminalLocationActivationOrigin,
	) => void;
	readonly agentOverview: WorkbenchWorkspaceAgentOverviewView | undefined;
	readonly deleteWorkspace: (workspaceId: string) => void;
	/** Held by a drag: it stands where it would land, its disclosure and Agent rows tucked away. */
	readonly dragging: boolean;
	readonly location: WorkbenchSidebarWorkspaceLocation | undefined;
	readonly menuFocus: ProjectNavigationMenuFocus;
	readonly messages: WorkbenchProjectNavigationMessages;
	readonly navigateAgent: (input: WorkbenchAgentNavigationInput) => void;
	readonly closeAgentPanel: (input: WorkbenchPanelTargetInput) => void;
	readonly renameAgentPanel: (input: WorkbenchPanelTargetInput) => void;
	readonly onPointerDown: (event: PointerEvent<HTMLDivElement>) => void;
	readonly refreshWorktrees: (workspaceId: string) => void;
	readonly setWorkspaceProject: (workspaceId: string, projectId: string | null) => void;
	readonly showWorkspaceProjectCreation: (workspaceId: string) => void;
	readonly showWorkspaceRename: (workspaceId: string) => void;
	readonly state: ProjectWorkspaceState;
	readonly toggleWorkspaceAgents: (workspaceId: string) => void;
	readonly workspace: WorkbenchWorkspace;
	readonly worktrees: WorkspaceWorktreeMenu;
}

/**
 * One Workspace: the name and its fold chevron, then the branch or folder, or the folded Agents'
 * states. The end area is reserved at all times, so the name truncates at one width; it holds ↑↓
 * and the unread dot at rest, and ⋯ and × under the pointer or focus.
 */
export function WorkspaceNavigationRow({
	active,
	activateWorkspace,
	agentOverview,
	deleteWorkspace,
	dragging,
	location,
	menuFocus,
	messages,
	closeAgentPanel,
	navigateAgent,
	onPointerDown,
	renameAgentPanel,
	refreshWorktrees,
	setWorkspaceProject,
	showWorkspaceProjectCreation,
	showWorkspaceRename,
	state,
	toggleWorkspaceAgents,
	workspace,
	worktrees,
}: WorkspaceNavigationRowProps): ReactNode {
	const expanded = agentOverview?.expanded ?? false;
	// A folded Workspace's second line lists its Agents' states in place of its location, and its
	// end area repeats their unread marker while the Agent rows are hidden.
	const folded = agentOverview !== undefined && !expanded;
	const unread = folded && agentOverview.unreadLabel !== undefined;
	// A row whose directory just moved keeps its old name until its new location settles.
	const name = location?.heldName ?? workspace.name;
	const line = location?.line?.text ?? null;
	const ahead = location?.ahead ?? 0;
	const behind = location?.behind ?? 0;
	const menuEntries: readonly WorkbenchActionMenuEntry[] = [
		workspaceMoveToProjectMenu({
			messages,
			setWorkspaceProject,
			showWorkspaceProjectCreation,
			state,
			workspace,
		}),
		{
			icon: 'edit',
			id: 'rename-workspace',
			kind: 'command',
			label: messages.workspaceMenuRename,
			onSelect: () => showWorkspaceRename(workspace.id),
		},
		...worktrees.section,
		{ id: 'close-workspace-separator', kind: 'separator' },
		{
			destructive: true,
			icon: 'close',
			id: 'close-workspace',
			kind: 'command',
			label: messages.workspaceMenuClose,
			onSelect: () => deleteWorkspace(workspace.id),
		},
		// Discovery only ever appends this row, so no row above it moves.
		...worktrees.removal,
	];
	// The whole row opens the Workspace; its chevron and tools act on their own, and portaled menus
	// and tooltips sit outside the row.
	const activate = (event: MouseEvent<HTMLDivElement>) => {
		if (!(event.target instanceof Element) || !event.currentTarget.contains(event.target)) return;
		const control = event.target.closest('button');
		if (control !== null && !control.hasAttribute('data-sidebar-nav')) return;
		activateWorkspace(workspace.id, event.detail > 0 ? 'pointer' : 'other');
	};
	return (
		<li
			className={styles['workspace']}
			data-dragging={dragging || undefined}
			data-drag-entry='workspace'
			data-drag-id={workspace.id}
			data-workspace-id={workspace.id}
		>
			<ContextMenu
				onOpenChange={(open, details) => {
					if (open) refreshWorktrees(workspace.id);
					menuFocus.onOpenChange(workspace.id, open, details);
				}}
			>
				<ContextMenuTrigger
					render={
						<div
							className={styles['row']}
							data-current={active || undefined}
							data-navigation-reorder-row='true'
							onClick={activate}
							onContextMenuCapture={suppressDuplicateNavigationContextMenu}
							onKeyDown={(event) =>
								handleNavigationRowKeyDown(event, () => showWorkspaceRename(workspace.id))
							}
							onPointerDown={onPointerDown}
						/>
					}
				>
					<button
						aria-current={active ? 'page' : undefined}
						aria-keyshortcuts={navigationRowAriaKeyShortcuts}
						aria-label={[
							name,
							folded ? null : line,
							location?.divergenceLabel,
							agentOverview === undefined ? null : workspaceAgentSummary(agentOverview),
						]
							.filter((part) => typeof part === 'string')
							.join('. ')}
						className={styles['rowMain']}
						data-sidebar-nav='workspace'
						data-slot='workbench-sidebar-workspace'
						tabIndex={-1}
						type='button'
					/>
					<span className={styles['line1']}>
						<SidebarTruncationTooltip
							content={name}
							render={<span className={styles['name']} />}
							sideOffset={NAME_TOOLTIP_OFFSET}
						>
							{name}
						</SidebarTruncationTooltip>
						{agentOverview === undefined || dragging ? null : (
							<Tooltip>
								<TooltipTrigger
									render={
										<button
											aria-expanded={expanded}
											aria-label={
												expanded ? agentOverview.collapseLabel : agentOverview.expandLabel
											}
											className={styles['chevron']}
											data-navigation-reorder-excluded='true'
											tabIndex={-1}
											type='button'
											onClick={() => toggleWorkspaceAgents(workspace.id)}
										/>
									}
								>
									<span className={styles['chevronIcon']} data-open={expanded || undefined}>
										<ProductIcon name='chevron-right' size='disclosure' />
									</span>
								</TooltipTrigger>
								<TooltipContent side='bottom'>
									{expanded ? agentOverview.collapseLabel : agentOverview.expandLabel}
								</TooltipContent>
							</Tooltip>
						)}
					</span>
					<span className={styles['line2']} data-agent-status={folded ? '' : undefined}>
						{folded ? <WorkspaceAgentStatusLine overview={agentOverview} /> : null}
						{!folded && line !== null ? (
							<Tooltip disabled={(location?.directory ?? null) === null}>
								<TooltipTrigger render={<span className={styles['lineText']} />}>
									{line}
								</TooltipTrigger>
								<TooltipContent align='start' side='bottom'>
									{location?.directory}
								</TooltipContent>
							</Tooltip>
						) : null}
					</span>
					{ahead > 0 || behind > 0 || unread ? (
						// Commits to push read as success and commits to pull as a caution, as in common Git
						// tools; the arrows tell them apart without color.
						<span aria-hidden='true' className={styles['endInfo']}>
							{ahead > 0 ? <span className={styles['ahead']}>↑{ahead}</span> : null}
							{behind > 0 ? <span className={styles['behind']}>↓{behind}</span> : null}
							{unread ? <span className={styles['dot']} /> : null}
						</span>
					) : null}
					<span className={styles['tools']}>
						<Tooltip>
							<WorkbenchSidebarDropdownMenu
								onOpenChange={(open) => {
									if (open) refreshWorktrees(workspace.id);
								}}
								entries={menuEntries}
								trigger={
									<TooltipTrigger
										render={
											<button
												aria-label={`${messages.workspaceOptions}: ${name}`}
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
							<TooltipContent side='bottom'>{messages.workspaceOptions}</TooltipContent>
						</Tooltip>
						<Tooltip>
							<TooltipTrigger
								render={
									<button
										aria-label={`${messages.workspaceMenuClose}: ${name}`}
										className={styles['tool']}
										data-danger=''
										data-navigation-reorder-excluded='true'
										tabIndex={-1}
										type='button'
										onClick={() => deleteWorkspace(workspace.id)}
									/>
								}
							>
								<ProductIcon name='close' />
							</TooltipTrigger>
							<TooltipContent side='bottom'>{messages.closeWorkspace}</TooltipContent>
						</Tooltip>
					</span>
				</ContextMenuTrigger>
				<WorkbenchActionContextMenuContent
					entries={menuEntries}
					finalFocus={() => menuFocus.finalFocus(workspace.id)}
				/>
			</ContextMenu>
			{/* A held Workspace moves as its row alone; its Agent rows return where it lands. */}
			{agentOverview !== undefined && expanded && !dragging && (
				<WorkspaceAgentList
					navigateAgent={(presentationId) =>
						navigateAgent({ presentationId, workspaceId: workspace.id })
					}
					overview={agentOverview}
					closeAgentPanel={closeAgentPanel}
					messages={messages}
					renameAgentPanel={renameAgentPanel}
				/>
			)}
		</li>
	);
}
