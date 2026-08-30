import { Button } from '@orchbee/ui/components';
import type { ReactNode } from 'react';

import { WorkbenchAgentIdentity } from '../../../../../workbench-agent-identity/WorkbenchAgentIdentity';
import type { WorkbenchWorkspaceAgentOverviewView } from '../../../../../../types/workbench-agent-overview';
import styles from './WorkspaceAgentList.module.css';

interface WorkspaceAgentListProps {
	readonly gradientPrefix: string;
	readonly navigateAgent: (
		sessionId: WorkbenchWorkspaceAgentOverviewView['rows'][number]['sessionId'],
	) => void;
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
				<li key={row.sessionId}>
					<Button
						aria-label={row.accessibleLabel}
						className={styles['item']}
						data-navigation-reorder-excluded='true'
						data-slot='workbench-sidebar-workspace-agent'
						disabled={!row.navigable}
						title={row.locationLabel}
						variant='ghost'
						onClick={() => navigateAgent(row.sessionId)}
					>
						<span className={styles['identity']}>
							<WorkbenchAgentIdentity
								gradientPrefix={`${gradientPrefix}-${index}`}
								providerIds={row.providerIds}
								status={row.status}
							/>
						</span>
						<span className={styles['content']}>
							<span className={styles['primary']}>
								<span className={styles['provider']}>{row.providerLabel}</span>
								<span aria-hidden='true' className={styles['separator']}>
									·
								</span>
								<span className={styles['status']}>{row.status.status}</span>
							</span>
							<span className={styles['location']}>{row.locationLabel}</span>
						</span>
					</Button>
				</li>
			))}
		</ul>
	);
}
