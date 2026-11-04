import type { ReactNode } from 'react';

import type { WorkbenchAgentStatusView } from '../../utils/formatWorkbenchAgentStatus';
import styles from './WorkbenchAgentStateGlyph.module.css';

interface WorkbenchAgentStateGlyphProps {
	/**
	 * Centered in its icon box by default, as an icon column needs; `start` sets it at the box's
	 * start, where a line of glyphs begins at a text edge.
	 */
	readonly alignment?: 'center' | 'start';
	readonly status: Pick<WorkbenchAgentStatusView, 'glyph' | 'tone'>;
}

/** One Agent state's glyph in its tone; the state's words come from its owner. */
export function WorkbenchAgentStateGlyph({
	alignment = 'center',
	status,
}: WorkbenchAgentStateGlyphProps): ReactNode {
	return (
		<span
			aria-hidden='true'
			className={styles['glyph']}
			data-agent-state-glyph=''
			data-alignment={alignment}
			data-tone={status.tone}
		>
			{status.glyph}
		</span>
	);
}
