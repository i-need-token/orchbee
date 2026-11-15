import type { ReactElement, ReactNode } from 'react';

import styles from './WindowFit.module.css';

/**
 * The slot a drawn window fills. Windows keep the product's real pixel size and zoom to the width
 * of this slot; they read it as the `window-fit` container, so a narrow slot gets a narrower
 * layout of the window instead of a smaller picture of the wide one. A scene plays while one of
 * its window slots is on screen.
 */
export function WindowFit({ children }: { children: ReactNode }): ReactElement {
	return (
		<div className={styles['fit']} data-window-fit=''>
			{children}
		</div>
	);
}
