import type { ReactElement, ReactNode } from 'react';

import styles from './Tag.module.css';

// A small outlined label for a feature's stage; `honey` marks the one being built now.
export function Tag({
	honey = false,
	children,
}: {
	honey?: boolean;
	children: ReactNode;
}): ReactElement {
	return <span className={honey ? styles['honey'] : styles['tag']}>{children}</span>;
}
