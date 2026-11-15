import type { ReactElement, ReactNode } from 'react';

import styles from './PageHead.module.css';

// The head of a page other than home: its address, headline, lead, call to action and note.
// `children` follow the head in the same column, as the drawn window of a client page does.
export function PageHead({
	address,
	title,
	lead,
	action,
	note,
	children,
}: {
	address: string;
	title: ReactNode;
	lead: ReactNode;
	action: ReactNode;
	note?: ReactNode;
	children?: ReactNode;
}): ReactElement {
	return (
		<div className={styles['column']}>
			<div className={styles['head']}>
				<div>
					<span className={styles['address']}>{address}</span>
					<h1 className={styles['title']}>{title}</h1>
				</div>
				<div>
					<p className={styles['lead']}>{lead}</p>
					{action}
					{note === undefined ? null : <p className={styles['note']}>{note}</p>}
				</div>
			</div>
			{children}
		</div>
	);
}
