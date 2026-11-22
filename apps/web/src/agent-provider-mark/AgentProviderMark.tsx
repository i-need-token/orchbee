import type { ReactNode } from 'react';

import { AgentProviderIcon } from '../agent-provider-icon';
import styles from './AgentProviderMark.module.css';

interface AgentProviderMarkProps {
	readonly gradientPrefix: string;
	readonly providerId: string;
}

/**
 * One Agent's product mark in a leading icon's place: the box a menu's leading ProductIcon takes,
 * so the labels after a mark and after an icon start in one column.
 */
export function AgentProviderMark({
	gradientPrefix,
	providerId,
}: AgentProviderMarkProps): ReactNode {
	return (
		<span aria-hidden='true' className={styles['mark']} data-agent-provider-mark={providerId}>
			<AgentProviderIcon agents={[providerId]} gradientPrefix={gradientPrefix} />
		</span>
	);
}
