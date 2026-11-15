import type { ReactElement, ReactNode } from 'react';

import styles from './Kicker.module.css';

// The small lowercase label above a heading or an index.
export function Kicker({
	children,
	reveal = false,
}: {
	children: ReactNode;
	/** Rises into view on its own, when it is not part of a revealed block. */
	reveal?: boolean;
}): ReactElement {
	return (
		<p className={styles['kicker']} data-reveal={reveal ? '' : undefined}>
			{children}
		</p>
	);
}
