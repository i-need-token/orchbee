import type { ReactElement, ReactNode } from 'react';

import styles from './DetailDuo.module.css';

/**
 * A detail section of two columns that rises into view as one: `text` in the narrower column
 * beside `figure`, or with `flip` the figure first in the wider column. In one column the text
 * always leads, and it comes first in the document in both arrangements.
 */
export function DetailDuo({
	text,
	figure,
	flip = false,
}: {
	text: ReactNode;
	figure: ReactNode;
	flip?: boolean;
}): ReactElement {
	return (
		<section className={styles['section']} data-reveal=''>
			<div className={flip ? styles['flip'] : styles['duo']}>
				<div className={styles['text']}>{text}</div>
				<div>{figure}</div>
			</div>
		</section>
	);
}
