import type { AgentLocationPresentation } from '@orchbee/agent';
import { ProductIcon } from '@orchbee/ui/components';
import type { ReactNode } from 'react';

import { useWorkbenchAgentStatus } from './hooks/useWorkbenchAgentStatus';
import styles from './WorkbenchAgentStatus.module.css';

interface WorkbenchAgentStatusProps {
	readonly presentation: AgentLocationPresentation;
}

export function WorkbenchAgentStatus({ presentation }: WorkbenchAgentStatusProps): ReactNode {
	const view = useWorkbenchAgentStatus(presentation);

	return (
		<span
			aria-label={`${view.provider}: ${view.status}`}
			className={styles['root']}
			data-freshness={presentation.freshness}
			data-slot='workbench-agent-status'
			data-tone={view.tone}
		>
			<span className={styles['provider']}>
				<ProductIcon name='agent' />
				<span>{view.provider}</span>
			</span>
			<span aria-hidden='true' className={styles['separator']}>
				·
			</span>
			<span className={styles['status']}>{view.status}</span>
		</span>
	);
}
