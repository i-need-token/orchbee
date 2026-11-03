import { Button } from '@orchbee/ui/components';
import type { TerminalPresentationId } from '@orchbee/workbench-client-protocol';
import type { ReactNode } from 'react';

import { WorkbenchAgentIdentity } from '../../../../../workbench-agent-identity/WorkbenchAgentIdentity';
import type { WorkbenchWorkspaceAgentOverviewView } from '../../../../../../types/workbench-agent-overview';
import styles from './WorkspaceAgentList.module.css';

interface WorkspaceAgentListProps {
	readonly gradientPrefix: string;
	readonly navigateAgent: (presentationId: TerminalPresentationId) => void;
	readonly overview: WorkbenchWorkspaceAgentOverviewView;
}

export function WorkspaceAgentList({
	gradientPrefix,
	navigateAgent,
	overview,
}: WorkspaceAgentListProps): ReactNode {
	return (
		<ul className={styles['list']} data-slot='workbench-sidebar-workspace-agents'>
			{overview.rows.map((row, index) => (
				<li key={row.presentationId}>
					<Button
						aria-label={
							row.unreadLabel === undefined
								? row.accessibleLabel
								: `${row.accessibleLabel} · ${row.unreadLabel}`
						}
						className={styles['item']}
						data-navigation-reorder-excluded='true'
						data-slot='workbench-sidebar-workspace-agent'
						title={row.locationLabel}
						variant='ghost'
						onClick={() => navigateAgent(row.presentationId)}
					>
						<span className={styles['identity']}>
							<WorkbenchAgentIdentity
								agents={row.agents}
								gradientPrefix={`${gradientPrefix}-${index}`}
								status={row.status}
							/>
						</span>
						<span className={styles['content']}>
							<span className={styles['primary']}>
								<span className={styles['provider']}>{row.agentLabel}</span>
								<span aria-hidden='true' className={styles['separator']}>
									·
								</span>
								<span className={styles['status']}>{row.status.status}</span>
							</span>
							<span className={styles['location']}>{row.locationLabel}</span>
						</span>
						<span
							aria-hidden='true'
							className={styles['unread']}
							data-agent-unread={row.unreadLabel === undefined ? undefined : ''}
							title={row.unreadLabel}
						>
							●
						</span>
					</Button>
				</li>
			))}
		</ul>
	);
}
