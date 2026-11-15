import type { ReactElement, ReactNode } from 'react';

import styles from './Kicker.module.css';

// The small lowercase label above a heading or an index; above an index it can be the section's
// heading itself.
export function Kicker({
	children,
	reveal = false,
	heading = false,
}: {
	children: ReactNode;
	/** Rises into view on its own, when it is not part of a revealed block. */
	reveal?: boolean;
	/** The label names a section that has no other heading. */
	heading?: boolean;
}): ReactElement {
	const Label = heading ? 'h2' : 'p';
	return (
		<Label className={styles['kicker']} data-reveal={reveal ? '' : undefined}>
			{children}
		</Label>
	);
}
