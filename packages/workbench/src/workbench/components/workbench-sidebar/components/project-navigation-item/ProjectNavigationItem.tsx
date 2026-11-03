import { Button, ContextMenu, ContextMenuTrigger, ProductIcon } from '@orchbee/ui/components';
import type { PointerEvent, ReactNode } from 'react';

import type { ProjectWorkspaceState, WorkbenchProject } from '../../../../../project-workspace';
import type { WorkbenchAgentNavigationInput } from '../../../../model/project-workbench-agent-overview';
import type { WorkbenchTerminalLocationActivationOrigin } from '../../../../model/workbench-terminal-location-activation';
import { WorkbenchAgentIdentity } from '../../../workbench-agent-identity/WorkbenchAgentIdentity';
import { WorkbenchActionContextMenuContent } from '../../../workbench-action-context-menu-content/WorkbenchActionContextMenuContent';
import type { WorkbenchActionMenuEntry } from '../../../../types/workbench-action-menu';
import type { WorkbenchEnvironmentProjectConnectionStatus } from '../../../../types/workbench-environment-project-connection';
import type { WorkbenchWorkspaceAgentOverviewView } from '../../../../types/workbench-agent-overview';
import { useNavigationReorderDrag } from '../../hooks/useNavigationReorderDrag';
import type { WorkbenchSidebarWorkspaceLocation } from '../../hooks/useWorkbenchSidebarWorkspaceLocations';
import type { WorkspaceWorktreeMenu } from '../../hooks/useWorkspaceWorktrees';
import type { WorkbenchProjectNavigationMessages } from '../../types/workbench-sidebar-types';
import { WorkbenchSidebarDropdownMenu } from '../workbench-sidebar-dropdown-menu/WorkbenchSidebarDropdownMenu';
import { WorkspaceAgentList } from './components/workspace-agent-list/WorkspaceAgentList';
import { WorkspaceAgentStatusLine } from './components/workspace-agent-status-line/WorkspaceAgentStatusLine';
import { WorkspaceLocationLine } from './components/workspace-location-line/WorkspaceLocationLine';
import { useProjectNavigationMenuFocus } from './hooks/useProjectNavigationMenuFocus';
import { navigationMenuAriaKeyShortcuts } from './utils/navigation-menu-shortcuts';
import {
	openNavigationRowMenuFromKeyboard,
	suppressDuplicateNavigationContextMenu,
} from './utils/navigation-row-menu';
import { workspaceNavigationMenu } from './utils/workspaceNavigationMenu';
import styles from './ProjectNavigationItem.module.css';

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

interface ProjectNavigationItemProps {
	readonly agentGradientPrefix: string;
	readonly activateWorkspace: (
		workspaceId: string,
		origin: WorkbenchTerminalLocationActivationOrigin,
	) => void;
	readonly activeLocation: ProjectWorkspaceState['activeLocation'];
	readonly createWorkspace: (input: {
		resourceScopeId: string;
		directoryId: string;
		projectId: string | null;
	}) => void;
	readonly state: ProjectWorkspaceState;
	readonly setWorkspaceProject: (workspaceId: string, projectId: string | null) => void;
	readonly showWorkspaceProjectCreation: (workspaceId: string) => void;
	readonly closeProjectWorkspaces: (projectId: string) => void;
	readonly connectEnvironment?: ((projectId: string) => void) | undefined;
	readonly deleteWorkspace: (workspaceId: string) => void;
	readonly displacement: 'down' | 'up' | null;
	readonly disconnectEnvironment?: ((projectId: string) => void) | undefined;
	readonly duplicateWorkspace?: ((workspaceId: string) => void) | undefined;
	readonly dragging: boolean;
	readonly expanded: boolean;
	readonly environmentConnection: WorkbenchEnvironmentProjectConnectionStatus | null;
	readonly messages: WorkbenchProjectNavigationMessages;
	readonly navigateAgent: (input: WorkbenchAgentNavigationInput) => void;
	readonly project: WorkbenchProject | null;
	/** Reads a Workspace's repository again; every Workspace menu calls it when it opens. */
	readonly refreshWorktrees: (workspaceId: string) => void;
	readonly onPointerDown: (projectId: string, event: PointerEvent<HTMLDivElement>) => void;
	readonly reorderActive: boolean;
	readonly reorderWorkspace: (
		workspaceId: string,
		beforeWorkspaceId: string | null,
	) => Promise<unknown>;
	readonly removeProject: (projectId: string) => void;
	readonly showProjectManager: (projectId: string) => void;
	readonly showWorkspaceManager: (workspaceId: string) => void;
	readonly toggleProjectExpansion: (projectId: string) => void;
	readonly toggleRecentsExpansion: () => void;
	readonly toggleWorkspaceAgents: (workspaceId: string) => void;
	readonly workspaceAgentOverviews: readonly WorkbenchWorkspaceAgentOverviewView[];
	readonly workspaceLocations: ReadonlyMap<string, WorkbenchSidebarWorkspaceLocation>;
	readonly worktreeMenu: (workspaceId: string) => WorkspaceWorktreeMenu;
}

export function ProjectNavigationItem({
	agentGradientPrefix,
	activateWorkspace,
	activeLocation,
	createWorkspace,
	state,
	setWorkspaceProject,
	showWorkspaceProjectCreation,
	closeProjectWorkspaces,
	connectEnvironment,
	deleteWorkspace,
	displacement,
	disconnectEnvironment,
	duplicateWorkspace,
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
	toggleRecentsExpansion,
	toggleWorkspaceAgents,
	workspaceAgentOverviews,
	workspaceLocations,
	worktreeMenu,
}: ProjectNavigationItemProps): ReactNode {
	const menuFocus = useProjectNavigationMenuFocus();
	const workspaces = state.workspaces.filter(
		(workspace) => workspace.projectId === (project?.id ?? null),
	);
	// A folded heading is never current, but sidebar focus lands on the one hiding the active Workspace.
	const holdsHiddenActiveWorkspace =
		!expanded &&
		activeLocation.kind === 'workspace' &&
		workspaces.some((workspace) => workspace.id === activeLocation.workspaceId);
	const workspaceReorder = useNavigationReorderDrag(workspaces, reorderWorkspace);
	const environmentConnectionEntries: readonly WorkbenchActionMenuEntry[] = (() => {
		if (environmentConnection === null || project === null) return [];
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
	const projectMenuEntries: readonly WorkbenchActionMenuEntry[] =
		project === null
			? []
			: [
					...environmentConnectionEntries,
					{
						icon: 'settings',
						id: 'manage-project',
						kind: 'command',
						label: messages.projectManage,
						onSelect: () => showProjectManager(project.id),
					},
					{
						icon: 'close',
						id: 'remove-project',
						kind: 'command',
						label: messages.removeProject,
						onSelect: () => removeProject(project.id),
					},
					{ id: 'close-project-separator', kind: 'separator' },
					{
						destructive: true,
						icon: 'trash',
						id: 'close-project-workspaces',
						kind: 'command',
						label: messages.projectCloseAll,
						disabled: workspaces.length === 0,
						onSelect: () => closeProjectWorkspaces(project.id),
					},
				];
	return (
		<li
			className={styles['project']}
			data-dragging={dragging || undefined}
			data-navigation-reorder-id={project?.id}
			data-project-id={project?.id}
			data-project-reorder-active={reorderActive || undefined}
			data-project-reorder-displacement={displacement ?? undefined}
		>
			{project === null && (
				// Recents only folds: it has no menu and neither starts nor receives a drag.
				<div className={styles['projectRow']} data-recents-row='true'>
					<Button
						aria-expanded={expanded}
						aria-label={expanded ? messages.collapseRecents : messages.expandRecents}
						className={styles['projectItem']}
						data-holds-active-workspace={holdsHiddenActiveWorkspace || undefined}
						data-slot='workbench-sidebar-recents'
						size='sm'
						variant='ghost'
						onClick={toggleRecentsExpansion}
					>
						<span aria-hidden='true' className={styles['icon']} />
						<span className={styles['projectName']}>{messages.recents}</span>
						<span
							aria-hidden='true'
							className={styles['disclosure']}
							data-expanded={expanded || undefined}
						>
							<ProductIcon name='chevron-right' />
						</span>
					</Button>
				</div>
			)}
			{project !== null && (
				<ContextMenu
					onOpenChange={(open, details) => {
						menuFocus.onOpenChange(null, open, details);
					}}
				>
					<ContextMenuTrigger
						render={
							<div
								className={styles['projectRow']}
								data-navigation-reorder-row='true'
								data-project-reorder-row='true'
								onContextMenuCapture={suppressDuplicateNavigationContextMenu}
								onKeyDown={openNavigationRowMenuFromKeyboard}
								onPointerDown={(event) => onPointerDown(project.id, event)}
							/>
						}
					>
						<Button
							aria-expanded={expanded}
							aria-keyshortcuts={navigationMenuAriaKeyShortcuts}
							aria-label={`${expanded ? messages.collapseProject : messages.expandProject}: ${project.name}${environmentConnection === null ? '' : `. ${messages.projectConnectionStatus[environmentConnection.state]}`}`}
							className={styles['projectItem']}
							data-holds-active-workspace={holdsHiddenActiveWorkspace || undefined}
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
							trigger={
								<Button
									aria-label={`${messages.projectOptions}: ${project.name}`}
									aria-keyshortcuts={navigationMenuAriaKeyShortcuts}
									className={styles['rowMenu']}
									data-navigation-menu-trigger='true'
									data-navigation-reorder-excluded='true'
									size='icon-xs'
									variant='ghost'
								/>
							}
							triggerContent={<ProductIcon name='ellipsis' />}
						/>
					</ContextMenuTrigger>
					<WorkbenchActionContextMenuContent
						entries={projectMenuEntries}
						finalFocus={() => menuFocus.finalFocus(null)}
					/>
				</ContextMenu>
			)}
			{expanded && (
				<ul
					ref={workspaceReorder.listRef}
					className={styles['workspaceList']}
					data-workspace-reorder-dragging={workspaceReorder.draggingItemId ?? undefined}
				>
					{workspaces.map((workspace) => {
						const agentOverview = workspaceAgentOverviews.find(
							(overview) => overview.workspaceId === workspace.id,
						);
						const agentExpanded = agentOverview?.expanded ?? false;
						// A folded Workspace's second line lists its Agents' states in place of its location,
						// and its row repeats their unread marker while the Agent rows are hidden.
						const agentsFolded = agentOverview !== undefined && !agentExpanded;
						const location = workspaceLocations.get(workspace.id);
						// A row whose directory just moved keeps its old name until its new location settles.
						const name = location?.heldName ?? workspace.name;
						const ahead = location?.ahead ?? 0;
						const behind = location?.behind ?? 0;
						const active =
							activeLocation.kind === 'workspace' && activeLocation.workspaceId === workspace.id;
						const worktrees = worktreeMenu(workspace.id);
						const workspaceMenuEntries: readonly WorkbenchActionMenuEntry[] = [
							...workspaceNavigationMenu({
								state,
								workspace,
								messages,
								createWorkspace,
								setWorkspaceProject,
								showWorkspaceProjectCreation,
							}),
							...(duplicateWorkspace === undefined
								? []
								: [
										{
											icon: 'copy' as const,
											id: 'duplicate-workspace',
											kind: 'command' as const,
											label: messages.duplicateWorkspace,
											onSelect: () => duplicateWorkspace(workspace.id),
										},
									]),
							{
								icon: 'settings',
								id: 'manage-workspace',
								kind: 'command',
								label: messages.workspaceManage,
								onSelect: () => showWorkspaceManager(workspace.id),
							},
							...worktrees.section,
							{ id: 'delete-workspace-separator', kind: 'separator' },
							{
								destructive: true,
								icon: 'trash',
								id: 'delete-workspace',
								kind: 'command',
								label: messages.deleteWorkspace,
								onSelect: () => deleteWorkspace(workspace.id),
							},
							// Discovery only ever appends this row, so no row above it moves.
							...worktrees.removal,
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
								data-workspace-reorder-active={workspaceReorder.displacementAnimated || undefined}
								data-workspace-reorder-displacement={workspaceDisplacement ?? undefined}
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
												className={styles['workspaceRow']}
												data-navigation-reorder-row='true'
												onContextMenuCapture={suppressDuplicateNavigationContextMenu}
												onKeyDown={openNavigationRowMenuFromKeyboard}
												onPointerDown={(event) =>
													workspaceReorder.onItemPointerDown(workspace.id, event)
												}
											/>
										}
									>
										<Button
											aria-keyshortcuts={navigationMenuAriaKeyShortcuts}
											aria-label={[
												name,
												agentsFolded ? null : location?.line?.text,
												location?.divergenceLabel,
												agentOverview === undefined ? null : workspaceAgentSummary(agentOverview),
											]
												.filter((part) => typeof part === 'string')
												.join('. ')}
											aria-current={active ? 'page' : undefined}
											className={styles['workspaceItem']}
											data-active={active || undefined}
											data-agent-summary={agentOverview === undefined ? undefined : 'true'}
											data-slot='workbench-sidebar-workspace'
											size='sm'
											variant='ghost'
											onClick={(event) =>
												activateWorkspace(workspace.id, event.detail > 0 ? 'pointer' : 'other')
											}
										>
											<span className={styles['icon']}>
												{agentOverview === undefined ? (
													<ProductIcon name='layout' />
												) : (
													<WorkbenchAgentIdentity
														agents={agentOverview.agents}
														gradientPrefix={`${agentGradientPrefix}-${workspace.id}`}
														status={agentOverview.status}
													/>
												)}
											</span>
											<span className={styles['workspaceText']}>
												<span className={styles['workspaceIdentity']}>
													<span className={styles['workspaceName']}>{name}</span>
													{ahead > 0 || behind > 0 ? (
														// Commits to push read as success and commits to pull as a caution, as in
														// common Git tools; the arrows tell them apart without color.
														<span aria-hidden='true' className={styles['divergence']}>
															{ahead > 0 ? <span className={styles['ahead']}>↑{ahead}</span> : null}
															{behind > 0 ? (
																<span className={styles['behind']}>↓{behind}</span>
															) : null}
														</span>
													) : null}
													{agentOverview === undefined ? null : (
														<span
															aria-hidden='true'
															className={styles['unread']}
															data-agent-unread={
																agentsFolded && agentOverview.unreadLabel !== undefined
																	? ''
																	: undefined
															}
															title={agentsFolded ? agentOverview.unreadLabel : undefined}
														>
															●
														</span>
													)}
													{agentOverview === undefined ? null : (
														<span
															aria-hidden='true'
															className={styles['agentCount']}
															title={agentOverview.countLabel}
														>
															{agentOverview.visibleCountLabel}
														</span>
													)}
												</span>
												{agentsFolded ? (
													<span className={styles['workspaceLocation']}>
														<WorkspaceAgentStatusLine status={agentOverview.foldedStatus} />
													</span>
												) : null}
												{!agentsFolded && location !== undefined && location.line !== null ? (
													<span className={styles['workspaceLocation']}>
														<WorkspaceLocationLine line={location.line} />
													</span>
												) : null}
											</span>
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
												onClick={() => toggleWorkspaceAgents(workspace.id)}
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
											onOpenChange={(open) => {
												if (open) refreshWorktrees(workspace.id);
											}}
											entries={workspaceMenuEntries}
											trigger={
												<Button
													aria-label={`${messages.workspaceOptions}: ${workspace.name}`}
													aria-keyshortcuts={navigationMenuAriaKeyShortcuts}
													className={styles['rowMenu']}
													data-navigation-menu-trigger='true'
													data-navigation-reorder-excluded='true'
													size='icon-xs'
													variant='ghost'
												/>
											}
											triggerContent={<ProductIcon name='ellipsis' />}
										/>
									</ContextMenuTrigger>
									<WorkbenchActionContextMenuContent
										entries={workspaceMenuEntries}
										finalFocus={() => menuFocus.finalFocus(workspace.id)}
									/>
								</ContextMenu>
								{/* A dragged Workspace leaves with its row alone, as a dragged Project folds. */}
								{agentOverview !== undefined && agentExpanded && !workspaceDragging && (
									<WorkspaceAgentList
										gradientPrefix={`${agentGradientPrefix}-${workspace.id}-detail`}
										navigateAgent={(presentationId) =>
											navigateAgent({
												presentationId,
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
