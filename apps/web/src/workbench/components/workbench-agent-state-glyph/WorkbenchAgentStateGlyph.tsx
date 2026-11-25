import type { ReactNode } from 'react';

import type { WorkbenchAgentStatusView } from '../../utils/formatWorkbenchAgentStatus';
import styles from './WorkbenchAgentStateGlyph.module.css';

interface WorkbenchAgentStateGlyphProps {
	/**
	 * Centered in its icon box by default, as an icon column needs; `start` sets it at the box's
	 * start, where a line of glyphs begins at a text edge.
	 */
	readonly alignment?: 'center' | 'start';
	/**
	 * Whether the glyph stands for the current Agent, or for the current Tab or Panel that holds it:
	 * drawn solid if so, hollow otherwise. A legend, which names no Agent, draws it hollow.
	 */
	readonly selected: boolean;
	readonly status: Pick<WorkbenchAgentStatusView, 'figure' | 'tone'>;
}

/**
 * Pointy-top hexagons in a 14-unit box (ORC-320), solid or as an outline, and the dot an unknown
 * state draws, solid or as a ring a little larger, so its outline still reads as one.
 */
const FIGURE_PATHS: Readonly<
	Record<WorkbenchAgentStatusView['figure'], Readonly<Record<'hollow' | 'solid', string>>>
> = {
	dot: {
		hollow: 'M4.5 7a2.5 2.5 0 1 0 5 0a2.5 2.5 0 1 0-5 0Z',
		solid: 'M5 7a2 2 0 1 0 4 0a2 2 0 1 0-4 0Z',
	},
	hexagon: {
		hollow: 'M7 2 11.33 4.5v5L7 12 2.67 9.5v-5Z',
		solid: 'M7 1.5 11.76 4.25v5.5L7 12.5 2.24 9.75v-5.5Z',
	},
};

/**
 * One Agent state's glyph in its tone, always at the status-glyph size (`icon-xs`). Its tone says
 * the state; solid or hollow says whether it is current. The state's words come from its owner.
 */
export function WorkbenchAgentStateGlyph({
	alignment = 'center',
	selected,
	status,
}: WorkbenchAgentStateGlyphProps): ReactNode {
	const fill = selected ? 'solid' : 'hollow';
	return (
		<span
			aria-hidden='true'
			className={styles['glyph']}
			data-agent-state-glyph=''
			data-alignment={alignment}
			data-figure={status.figure}
			data-fill={fill}
			data-tone={status.tone}
		>
			<svg className={styles['shape']} viewBox='0 0 14 14'>
				<path d={FIGURE_PATHS[status.figure][fill]} />
			</svg>
		</span>
	);
}
