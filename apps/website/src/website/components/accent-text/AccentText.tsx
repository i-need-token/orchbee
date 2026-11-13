import type { ReactElement } from 'react';

import styles from './AccentText.module.css';

// The honey second line of the page's headlines.
export function AccentText({ children }: { children: string }): ReactElement {
	return <span className={styles['accent']}>{children}</span>;
}
