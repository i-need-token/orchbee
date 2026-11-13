import type { ReactElement } from 'react';

import styles from './BeeMark.module.css';

// The bee mark at the size its container gives it; `tile` keeps the colors of the dark app tile.
export function BeeMark({ tile = false }: { tile?: boolean }): ReactElement {
	return (
		<svg className={styles['mark']} aria-hidden='true'>
			<use href={tile ? '#ob-bee-tile' : '#ob-bee'} />
		</svg>
	);
}
