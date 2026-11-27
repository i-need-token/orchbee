import type { ReactElement } from 'react';

import { joinClassNames } from '../../joinClassNames';
import styles from './StateGlyph.module.css';

// The Agent states the product shows, in the product's fixed status colors.
export type AgentState = 'working' | 'attention' | 'done' | 'failed' | 'idle';

/**
 * The product's hexagon status glyph: its color names the state and its shape whether the Agent is
 * at rest, hollow while idle and solid otherwise, wherever it stands and whatever is selected.
 * Working breathes slowly and attention quickly. `bind` names the scene state that script writes
 * into `data-state`, which sets the shape too. Inside a drawn desktop application, `product` draws
 * it at the product's icon size (`regular`, or `small` in Tabs and Panel headers) in the product's
 * status colors, which follow the window's appearance.
 */
export function StateGlyph({
	state,
	bind,
	product,
}: {
	state: AgentState;
	bind?: string | undefined;
	product?: 'regular' | 'small';
}): ReactElement {
	return (
		<svg
			className={joinClassNames(
				styles['glyph'],
				product !== undefined && styles['product'],
				product === 'small' && styles['productSmall'],
			)}
			viewBox='0 0 14 14'
			data-state={state}
			data-scene-state={bind}
			aria-hidden='true'
		>
			<path className={styles['fill']} d='M7 1.5 11.76 4.25v5.5L7 12.5 2.24 9.75v-5.5Z' />
			<path className={styles['ring']} d='M7 2 11.33 4.5v5L7 12 2.67 9.5v-5Z' />
		</svg>
	);
}
