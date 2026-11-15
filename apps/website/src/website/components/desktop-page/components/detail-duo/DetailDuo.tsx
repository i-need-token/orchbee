import type { ReactElement, ReactNode } from 'react';

import styles from './DetailDuo.module.css';

/**
 * A section of two columns that rises into view as one: `first` beside `second`, the narrower
 * column first, or the wider one with `flip`. In one column the text leads, so a flipped section
 * moves its first column after the second.
 */
export function DetailDuo({
	first,
	second,
	flip = false,
}: {
	first: ReactNode;
	second: ReactNode;
	flip?: boolean;
}): ReactElement {
	return (
		<section className={styles['section']} data-reveal=''>
			<div className={flip ? styles['flip'] : styles['duo']}>
				<div>{first}</div>
				<div>{second}</div>
			</div>
		</section>
	);
}
