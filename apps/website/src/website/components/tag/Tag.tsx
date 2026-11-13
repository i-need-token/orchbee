import type { ReactElement, ReactNode } from 'react';

import styles from './Tag.module.css';

// A small outlined label for a feature's stage.
export function Tag({ children }: { children: ReactNode }): ReactElement {
	return <span className={styles['tag']}>{children}</span>;
}
