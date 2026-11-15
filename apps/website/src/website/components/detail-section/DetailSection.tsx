import type { ReactElement, ReactNode } from 'react';

import { Kicker } from '../kicker/Kicker';
import styles from './DetailSection.module.css';

/**
 * A section of a client page: its label, heading and optional paragraph stacked above its figure
 * (`children`). Each line rises into view on its own; a figure carries its own `data-reveal`.
 */
export function DetailSection({
	kicker,
	title,
	lead,
	children,
}: {
	kicker: ReactNode;
	title: ReactNode;
	lead?: ReactNode;
	children: ReactNode;
}): ReactElement {
	return (
		<section className={styles['section']}>
			<Kicker reveal>{kicker}</Kicker>
			<h2 className={styles['title']} data-reveal=''>
				{title}
			</h2>
			{lead === undefined ? null : (
				<p className={styles['lead']} data-reveal=''>
					{lead}
				</p>
			)}
			{children}
		</section>
	);
}
