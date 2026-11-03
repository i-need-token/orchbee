import type { ReactNode } from 'react';

import type { WorkbenchAgentStatusView } from '../../utils/formatWorkbenchAgentStatus';
import { WorkbenchAgentProviderIcon } from '../workbench-agent-provider-icon/WorkbenchAgentProviderIcon';
import styles from './WorkbenchAgentIdentity.module.css';

interface WorkbenchAgentIdentityProps {
	readonly agents: readonly string[];
	readonly gradientPrefix: string;
	readonly status: Pick<WorkbenchAgentStatusView, 'glyph' | 'tone'>;
}

export function WorkbenchAgentIdentity({
	agents,
	gradientPrefix,
	status,
}: WorkbenchAgentIdentityProps): ReactNode {
	return (
		<span aria-hidden='true' className={styles['root']} data-tone={status.tone}>
			<span className={styles['status']} data-agent-state-glyph=''>
				{status.glyph}
			</span>
			<WorkbenchAgentProviderIcon agents={agents} gradientPrefix={gradientPrefix} />
		</span>
	);
}
