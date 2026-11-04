import type { ReactNode } from 'react';

import type { WorkbenchAgentStatusView } from '../../utils/formatWorkbenchAgentStatus';
import { WorkbenchAgentProviderIcon } from '../workbench-agent-provider-icon/WorkbenchAgentProviderIcon';
import { WorkbenchAgentStateGlyph } from '../workbench-agent-state-glyph/WorkbenchAgentStateGlyph';
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
		<span aria-hidden='true' className={styles['root']}>
			<WorkbenchAgentStateGlyph status={status} />
			<WorkbenchAgentProviderIcon agents={agents} gradientPrefix={gradientPrefix} />
		</span>
	);
}
