import type { ReactElement, ReactNode } from 'react';

import styles from './SectionHeading.module.css';

// A section's number and name, its two-line heading with the honey second line, and its lead.
export function SectionHeading({
	number,
	label,
	lead,
	accent,
	variant = 'scene',
	children,
}: {
	number?: string;
	label?: string;
	lead: string;
	accent: string;
	variant?: 'scene' | 'outro';
	children?: ReactNode;
}): ReactElement {
	return (
		<>
			{number === undefined ? null : (
				<p className={styles['label']}>
					<b>{number}</b>
					<i />
					<span>{label}</span>
				</p>
			)}
			<h2 className={styles[variant]}>
				<span className={styles['line']}>{lead}</span>
				<span className={styles['accent']}>{accent}</span>
			</h2>
			{children === undefined ? null : <p className={styles['body']}>{children}</p>}
		</>
	);
}
