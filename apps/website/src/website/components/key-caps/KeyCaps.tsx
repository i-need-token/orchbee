import type { ReactElement } from 'react';

import styles from './KeyCaps.module.css';

// A cap's identity is its place in the sequence.
function byPosition(keys: string): { key: string; token: string }[] {
	return keys.split(' ').map((token, position) => ({ key: String(position), token }));
}

/**
 * Keys drawn as key caps. `keys` is written as the caps read: caps separated by spaces, a chord
 * inside one cap (`Ctrl+b Space` is two caps), and a lone `/` between alternatives (`n / p`).
 * `small` sets the caps inside a button.
 */
export function KeyCaps({
	keys,
	size = 'regular',
}: {
	keys: string;
	size?: 'regular' | 'small';
}): ReactElement {
	return (
		<span className={size === 'small' ? styles['smallKeys'] : styles['keys']}>
			{byPosition(keys).map(({ key, token }) =>
				token === '/' ? (
					<span key={key}>/</span>
				) : (
					<kbd key={key} className={styles['cap']}>
						{token}
					</kbd>
				),
			)}
		</span>
	);
}
