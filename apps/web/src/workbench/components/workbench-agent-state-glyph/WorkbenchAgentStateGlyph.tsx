import type { ReactNode } from 'react';

import type { WorkbenchAgentStatusView } from '../../utils/formatWorkbenchAgentStatus';
import styles from './WorkbenchAgentStateGlyph.module.css';

interface WorkbenchAgentStateGlyphProps {
	/**
	 * Centered in its icon box by default, as an icon column needs; `start` sets it at the box's
	 * start, where a line of glyphs begins at a text edge.
	 */
	readonly alignment?: 'center' | 'start';
	/** `small` draws the 12 px glyph a folded Workspace's status line lists. */
	readonly size?: 'small';
	readonly status: Pick<WorkbenchAgentStatusView, 'shape' | 'tone'>;
}

/** Pointy-top hexagons in a 14-unit box (ORC-320): filled, its outline as a ring, or a dot. */
const SHAPE_PATHS: Readonly<Record<WorkbenchAgentStatusView['shape'], string>> = {
	dot: 'M5 7a2 2 0 1 0 4 0a2 2 0 1 0-4 0Z',
	hex: 'M7 1.5 11.76 4.25v5.5L7 12.5 2.24 9.75v-5.5Z',
	ring: 'M7 2 11.33 4.5v5L7 12 2.67 9.5v-5Z',
};

/** One Agent state's glyph in its tone; the state's words come from its owner. */
export function WorkbenchAgentStateGlyph({
	alignment = 'center',
	size,
	status,
}: WorkbenchAgentStateGlyphProps): ReactNode {
	return (
		<span
			aria-hidden='true'
			className={styles['glyph']}
			data-agent-state-glyph=''
			data-alignment={alignment}
			data-shape={status.shape}
			data-size={size}
			data-tone={status.tone}
		>
			<svg className={styles['shape']} viewBox='0 0 14 14'>
				<path d={SHAPE_PATHS[status.shape]} />
			</svg>
		</span>
	);
}
