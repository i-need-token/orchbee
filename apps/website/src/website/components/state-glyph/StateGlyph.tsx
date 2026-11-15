import type { ReactElement } from 'react';

import styles from './StateGlyph.module.css';

// The Agent states the product shows, in the product's fixed status colors.
export type AgentState = 'working' | 'attention' | 'done' | 'failed' | 'idle';

/**
 * The product's hexagon status glyph. Working breathes slowly and attention quickly; idle is an
 * outline. Script may change the state later through `data-state`.
 */
export function StateGlyph({ state }: { state: AgentState }): ReactElement {
	return (
		<svg className={styles['glyph']} viewBox='0 0 14 14' data-state={state} aria-hidden='true'>
			<path className={styles['fill']} d='M7 1.5 11.76 4.25v5.5L7 12.5 2.24 9.75v-5.5Z' />
			<path className={styles['ring']} d='M7 2 11.33 4.5v5L7 12 2.67 9.5v-5Z' />
		</svg>
	);
}
