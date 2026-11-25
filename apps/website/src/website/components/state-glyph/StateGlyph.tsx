import type { ReactElement } from 'react';

import { joinClassNames } from '../../joinClassNames';
import styles from './StateGlyph.module.css';

// The Agent states the product shows, in the product's fixed status colors.
export type AgentState = 'working' | 'attention' | 'done' | 'failed' | 'idle';

/**
 * The product's hexagon status glyph: its color names the state, and it is solid for the current
 * Agent, the selected Tab or the focused Panel (`selected`) and hollow everywhere else, legends
 * included. Working breathes slowly and attention quickly. `bind` names the scene state that
 * script writes into `data-state`. Inside a drawn desktop application, `product` draws it at the
 * product's icon size (`regular`, or `small` in Tabs and Panel headers) in the product's status
 * colors, which follow the window's appearance.
 */
export function StateGlyph({
	state,
	selected = false,
	bind,
	product,
}: {
	state: AgentState;
	selected?: boolean;
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
			data-fill={selected ? 'solid' : 'hollow'}
			data-scene-state={bind}
			aria-hidden='true'
		>
			<path className={styles['fill']} d='M7 1.5 11.76 4.25v5.5L7 12.5 2.24 9.75v-5.5Z' />
			<path className={styles['ring']} d='M7 2 11.33 4.5v5L7 12 2.67 9.5v-5Z' />
		</svg>
	);
}
