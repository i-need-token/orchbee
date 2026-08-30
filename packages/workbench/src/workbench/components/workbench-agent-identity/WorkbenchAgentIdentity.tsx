import type { AgentAggregatePresentation } from '@orchbee/agent';
import type { ReactNode } from 'react';

import type { WorkbenchAgentStatusView } from '../../utils/formatWorkbenchAgentStatus';
import { WorkbenchAgentProviderIcon } from '../workbench-agent-provider-icon/WorkbenchAgentProviderIcon';
import styles from './WorkbenchAgentIdentity.module.css';

interface WorkbenchAgentIdentityProps {
	readonly gradientPrefix: string;
	readonly providerIds: AgentAggregatePresentation['providerIds'];
	readonly status: Pick<WorkbenchAgentStatusView, 'glyph' | 'tone'>;
}

export function WorkbenchAgentIdentity({
	gradientPrefix,
	providerIds,
	status,
}: WorkbenchAgentIdentityProps): ReactNode {
	return (
		<span aria-hidden='true' className={styles['root']} data-tone={status.tone}>
			<WorkbenchAgentProviderIcon gradientPrefix={gradientPrefix} providerIds={providerIds} />
			<span className={styles['status']}>{status.glyph}</span>
		</span>
	);
}
