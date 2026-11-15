import type { ReactElement, ReactNode } from 'react';

import styles from './FigureCaption.module.css';

/**
 * The line under a drawn figure: what it shows (`children`, whose `<b>` labels the figure) and,
 * across from it, an optional `aside` such as a legend.
 */
export function FigureCaption({
	children,
	aside,
}: {
	children: ReactNode;
	aside?: ReactNode;
}): ReactElement {
	return (
		<div className={styles['caption']}>
			<span>{children}</span>
			{aside}
		</div>
	);
}
