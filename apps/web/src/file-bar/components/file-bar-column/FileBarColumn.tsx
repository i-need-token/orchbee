import type { ReactNode } from 'react';

import styles from './FileBarColumn.module.css';

/** The column's own 36px band, where the view toggles sit, above the active view. */
export function FileBarColumn(): ReactNode {
	return (
		<div className={styles['root']}>
			<div className={styles['togglesRow']} data-slot='file-bar-toggles-row' />
		</div>
	);
}
