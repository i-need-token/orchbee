import { Tooltip, TooltipContent, TooltipTrigger } from '@orchbee/ui/components';
import type { TerminalPresentationId } from '@orchbee/workbench-client-protocol';
import type { MouseEvent, ReactNode } from 'react';

import { WorkbenchAgentStateGlyph } from '../../../../../workbench-agent-state-glyph/WorkbenchAgentStateGlyph';
import type { WorkbenchWorkspaceAgentOverviewView } from '../../../../../../types/workbench-agent-overview';
import { SidebarTruncationTooltip } from '../../../sidebar-truncation-tooltip/SidebarTruncationTooltip';
import styles from './WorkspaceAgentList.module.css';

interface WorkspaceAgentListProps {
	readonly navigateAgent: (presentationId: TerminalPresentationId) => void;
	readonly overview: WorkbenchWorkspaceAgentOverviewView;
}

/**
 * A Workspace's Agents, one line each as in the TUI: the state glyph, title · provider, and the
 * unread dot. The glyph names its state under the pointer and the dot says Unread; Agent rows have
 * no menu.
 */
export function WorkspaceAgentList({
	navigateAgent,
	overview,
}: WorkspaceAgentListProps): ReactNode {
	return (
		<ul className={styles['list']} data-slot='workbench-sidebar-workspace-agents'>
			{overview.rows.map((row, index) => {
				const title = row.titleLabel === '' ? row.agentLabel : row.titleLabel;
				const provider = row.titleLabel === '' ? null : row.agentLabel;
				// Portaled tooltips are not inside the row, so only a click within it opens the Agent.
				const open = (event: MouseEvent<HTMLLIElement>) => {
					if (event.target instanceof Node && event.currentTarget.contains(event.target)) {
						navigateAgent(row.presentationId);
					}
				};
				return (
					<li
						key={row.presentationId}
						className={styles['row']}
						data-current={row.current || undefined}
						data-first={index === 0 || undefined}
						data-last={index === overview.rows.length - 1 || undefined}
						data-navigation-reorder-excluded='true'
						onClick={open}
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
					</li>
				);
			})}
		</ul>
	);
}
