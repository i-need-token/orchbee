import {
	ContextMenu,
	ContextMenuTrigger,
	Tooltip,
	TooltipContent,
	TooltipTrigger,
} from '@orchbee/ui/components';
import type { TerminalPresentationId } from '@orchbee/workbench-client-protocol';
import type { MouseEvent, ReactNode } from 'react';

import type { WorkbenchPanelTargetInput } from '../../../../../../actions/workbench-action-ids';
import { WorkbenchAgentStateGlyph } from '../../../../../workbench-agent-state-glyph/WorkbenchAgentStateGlyph';
import type { WorkbenchActionMenuEntry } from '../../../../../../types/workbench-action-menu';
import type { WorkbenchWorkspaceAgentOverviewView } from '../../../../../../types/workbench-agent-overview';
import { WorkbenchActionContextMenuContent } from '../../../../../workbench-action-context-menu-content/WorkbenchActionContextMenuContent';
import type { WorkbenchProjectNavigationMessages } from '../../../../types/workbench-sidebar-types';
import { SidebarTruncationTooltip } from '../../../sidebar-truncation-tooltip/SidebarTruncationTooltip';
import styles from './WorkspaceAgentList.module.css';

interface WorkspaceAgentListProps {
	readonly closeAgentPanel: (input: WorkbenchPanelTargetInput) => void;
	readonly messages: WorkbenchProjectNavigationMessages;
	readonly navigateAgent: (presentationId: TerminalPresentationId) => void;
	readonly overview: WorkbenchWorkspaceAgentOverviewView;
	readonly renameAgentPanel: (input: WorkbenchPanelTargetInput) => void;
}

/**
 * A Workspace's Agents, one line each as in the TUI: the state glyph, title · provider, and the
 * unread dot. The glyph names its state under the pointer and the dot says Unread. A right-click
 * opens the Agent's menu: Rename, which only a Panel's primary terminal offers, then Close, which
 * asks first.
 */
export function WorkspaceAgentList({
	closeAgentPanel,
	messages,
	navigateAgent,
	overview,
	renameAgentPanel,
}: WorkspaceAgentListProps): ReactNode {
	return (
		<ul className={styles['list']} data-slot='workbench-sidebar-workspace-agents'>
			{overview.rows.map((row, index) => {
				const title = row.titleLabel === '' ? row.agentLabel : row.titleLabel;
				const provider = row.titleLabel === '' ? null : row.agentLabel;
				const panel = {
					panelId: row.panelId,
					tabId: row.tabId,
					terminalContextId: row.terminalContextId,
				};
				const renameEntries: readonly WorkbenchActionMenuEntry[] = row.primary
					? [
							{
								icon: 'edit',
								id: 'rename-agent',
								kind: 'command',
								label: messages.agentMenuRename,
								onSelect: () => renameAgentPanel(panel),
							},
							{ id: 'rename-agent-separator', kind: 'separator' },
						]
					: [];
				const menuEntries: readonly WorkbenchActionMenuEntry[] = [
					...renameEntries,
					{
						destructive: true,
						icon: 'close',
						id: 'close-agent',
						kind: 'command',
						label: messages.agentMenuClose,
						onSelect: () => closeAgentPanel(panel),
					},
				];
				// Portaled tooltips are not inside the row, so only a click within it opens the Agent.
				const open = (event: MouseEvent<HTMLLIElement>) => {
					if (event.target instanceof Node && event.currentTarget.contains(event.target)) {
						navigateAgent(row.presentationId);
					}
				};
				return (
					<ContextMenu key={row.presentationId}>
						<ContextMenuTrigger
							render={
								<li
									className={styles['row']}
									data-current={row.current || undefined}
									data-first={index === 0 || undefined}
									data-last={index === overview.rows.length - 1 || undefined}
									data-navigation-reorder-excluded='true'
									onClick={open}
								/>
							}
						>
							<button
								aria-current={row.current ? 'true' : undefined}
								aria-label={
									row.unreadLabel === undefined
										? row.accessibleLabel
										: `${row.accessibleLabel} · ${row.unreadLabel}`
								}
								className={styles['rowMain']}
								data-sidebar-nav='agent'
								data-slot='workbench-sidebar-workspace-agent'
								tabIndex={-1}
								type='button'
							/>
							<Tooltip>
								<TooltipTrigger render={<span className={styles['glyph']} />}>
									<WorkbenchAgentStateGlyph status={row.status} />
								</TooltipTrigger>
								<TooltipContent side='bottom'>{row.status.status}</TooltipContent>
							</Tooltip>
							<SidebarTruncationTooltip
								content={provider === null ? title : `${title} · ${provider}`}
								render={<span className={styles['text']} />}
							>
								<span className={styles['title']}>{title}</span>
								{provider === null ? null : <span className={styles['provider']}>{provider}</span>}
							</SidebarTruncationTooltip>
							{row.unreadLabel === undefined ? null : (
								<Tooltip>
									<TooltipTrigger render={<span className={styles['unread']} />}>
										<span className={styles['dot']} />
									</TooltipTrigger>
									<TooltipContent side='bottom'>{row.unreadLabel}</TooltipContent>
								</Tooltip>
							)}
						</ContextMenuTrigger>
						<WorkbenchActionContextMenuContent entries={menuEntries} />
					</ContextMenu>
				);
			})}
		</ul>
	);
}
