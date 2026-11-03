import type { ReactNode } from 'react';

import type { WorkbenchAgentStatusView } from '../../utils/formatWorkbenchAgentStatus';
import styles from './WorkbenchAgentStateGlyph.module.css';

interface WorkbenchAgentStateGlyphProps {
	readonly status: Pick<WorkbenchAgentStatusView, 'glyph' | 'tone'>;
}

/** One Agent state's glyph in its tone; the state's words come from its owner. */
export function WorkbenchAgentStateGlyph({ status }: WorkbenchAgentStateGlyphProps): ReactNode {
	return (
		<span
			aria-hidden='true'
			className={styles['glyph']}
			data-agent-state-glyph=''
			data-tone={status.tone}
		>
			{status.glyph}
		</span>
	);
}
