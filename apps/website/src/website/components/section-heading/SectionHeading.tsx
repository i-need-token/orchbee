import type { ReactElement } from 'react';

import { AccentText } from '../accent-text/AccentText';
import styles from './SectionHeading.module.css';

export function SectionHeading({
	number,
	label,
	lead,
	accent,
	body,
}: {
	number: string;
	label: string;
	lead: string;
	accent: string;
	body?: string;
}): ReactElement {
	return (
		<div className={styles['heading']}>
			<p className={styles['label']}>
				<span className={styles['number']}>{number}</span>
				<span className={styles['rule']} aria-hidden='true' />
				{label}
			</p>
			<h2 className={styles['title']}>
				<span className={styles['line']}>{lead}</span>
				<AccentText>{accent}</AccentText>
			</h2>
			{body === undefined ? null : <p className={styles['body']}>{body}</p>}
		</div>
	);
}
