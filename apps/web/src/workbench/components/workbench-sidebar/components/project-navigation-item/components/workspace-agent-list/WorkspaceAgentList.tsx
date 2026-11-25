import {
	ContextMenu,
	ContextMenuTrigger,
	Tooltip,
	TooltipContent,
	TooltipTrigger,
} from '@orchbee/ui/components';
import type { TerminalPresentationId } from '@orchbee/workbench-client-protocol';
import { useId, type MouseEvent, type ReactNode } from 'react';

import type {
	WorkbenchAgentConfirmationChooseInput,
	WorkbenchPanelTargetInput,
} from '../../../../../../actions/workbench-action-ids';
import { hasAgentProductMark } from '../../../../../../../agent-provider-icon';
import { AgentProviderMark } from '../../../../../../../agent-provider-mark';
import { WorkbenchAgentStateGlyph } from '../../../../../workbench-agent-state-glyph/WorkbenchAgentStateGlyph';
import type {
	WorkbenchActionMenuChoice,
	WorkbenchActionMenuEntry,
} from '../../../../../../types/workbench-action-menu';
import type { WorkbenchWorkspaceAgentOverviewView } from '../../../../../../types/workbench-agent-overview';
import { WorkbenchActionContextMenuContent } from '../../../../../workbench-action-context-menu-content/WorkbenchActionContextMenuContent';
import type { WorkbenchProjectNavigationMessages } from '../../../../types/workbench-sidebar-types';
import { SidebarTruncationTooltip } from '../../../sidebar-truncation-tooltip/SidebarTruncationTooltip';
import styles from './WorkspaceAgentList.module.css';

interface WorkspaceAgentListProps {
	readonly chooseAgentConfirmation: (input: WorkbenchAgentConfirmationChooseInput) => void;
	readonly closeAgentPanel: (input: WorkbenchPanelTargetInput) => void;
	readonly messages: WorkbenchProjectNavigationMessages;
	readonly navigateAgent: (presentationId: TerminalPresentationId) => void;
	readonly overview: WorkbenchWorkspaceAgentOverviewView;
	readonly renameAgentPanel: (input: WorkbenchPanelTargetInput) => void;
}

/**
 * A Workspace's Agents, one line each as in the TUI: the state glyph, the title, the unread dot, and
 * the provider's product mark at the row's end, or its name where it has no mark. The current Agent's
 * glyph is solid and every other one hollow. The glyph names
 * its state under the pointer, with why a confirmation waits where one does, the mark names its
 * product, and the dot says Unread. A right-click opens the Agent's menu: Rename, which only a Panel's
 * primary terminal offers, Allow confirmations where the Server answers them for this Agent, then
 * Close, which asks first.
 */
export function WorkspaceAgentList({
	chooseAgentConfirmation,
	closeAgentPanel,
	messages,
	navigateAgent,
	overview,
	renameAgentPanel,
}: WorkspaceAgentListProps): ReactNode {
	const markPrefix = useId();
	return (
		<ul className={styles['list']} data-slot='workbench-sidebar-workspace-agents'>
			{overview.rows.map((row, index) => {
				const title = row.titleLabel === '' ? row.agentLabel : row.titleLabel;
				const provider = row.titleLabel === '' ? null : row.agentLabel;
				const marked = provider !== null && hasAgentProductMark(row.providerId);
				const panel = {
					panelId: row.panelId,
					tabId: row.tabId,
					terminalContextId: row.terminalContextId,
				};
				const choose = (answer: WorkbenchAgentConfirmationChooseInput['answer']) => () =>
					chooseAgentConfirmation({ answer, presentationId: row.presentationId });
				const agentEntries: readonly (WorkbenchActionMenuChoice | WorkbenchActionMenuEntry)[] = [
					...(row.primary
						? [
								{
									icon: 'edit',
									id: 'rename-agent',
									kind: 'command',
									label: messages.agentMenuRename,
									onSelect: () => renameAgentPanel(panel),
								} as const,
							]
						: []),
					...(row.confirmation === undefined
						? []
						: [
								{
									icon: 'keyboard',
									id: 'agent-confirmations',
									kind: 'choice',
									label: messages.agentMenuConfirmations,
									options: [
										{
											id: 'off',
											label: messages.agentMenuConfirmationsOff,
											onSelect: choose('off'),
										},
										{
											id: 'run',
											label: messages.agentMenuConfirmationsRun,
											onSelect: choose('run'),
										},
										{
											id: 'agent-type',
											label: row.confirmation.agentTypeLabel,
											onSelect: choose('agent-type'),
										},
									],
									value: row.confirmation.choice,
								} as const,
							]),
				];
				const menuEntries: readonly (WorkbenchActionMenuChoice | WorkbenchActionMenuEntry)[] = [
					...agentEntries,
					...(agentEntries.length === 0
						? []
						: [{ id: 'agent-separator', kind: 'separator' } as const]),
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
									<WorkbenchAgentStateGlyph selected={row.current} status={row.status} />
								</TooltipTrigger>
								<TooltipContent side='bottom'>
									{row.statusReason ?? row.status.status}
								</TooltipContent>
							</Tooltip>
							<SidebarTruncationTooltip
								content={provider === null ? title : `${title} · ${provider}`}
								render={<span className={styles['text']} />}
							>
								<span className={styles['title']}>{title}</span>
								{provider === null || marked ? null : (
									<span className={styles['provider']}>{provider}</span>
								)}
							</SidebarTruncationTooltip>
							<span className={styles['end']}>
								{row.unreadLabel === undefined ? null : (
									<Tooltip>
										<TooltipTrigger render={<span className={styles['unread']} />}>
											<span className={styles['dot']} />
										</TooltipTrigger>
										<TooltipContent side='bottom'>{row.unreadLabel}</TooltipContent>
									</Tooltip>
								)}
								{marked ? (
									<Tooltip>
										<TooltipTrigger render={<span className={styles['mark']} />}>
											<AgentProviderMark
												gradientPrefix={`${markPrefix}-${row.presentationId}`}
												providerId={row.providerId}
											/>
										</TooltipTrigger>
										<TooltipContent side='bottom'>{provider}</TooltipContent>
									</Tooltip>
								) : null}
							</span>
						</ContextMenuTrigger>
						<WorkbenchActionContextMenuContent entries={menuEntries} />
					</ContextMenu>
				);
			})}
		</ul>
	);
}
