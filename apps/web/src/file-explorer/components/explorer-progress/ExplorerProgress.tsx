import { fileExplorerMessages, useOrchbeeIntl } from '@orchbee/i18n';
import type { ReactNode } from 'react';

import styles from './ExplorerProgress.module.css';

/**
 * VS Code's view progress bar: a 2px line at the top of the Explorer's content, whose short bit runs
 * across while folders are slow to list. It lies over the rows, so they never move for it.
 */
export function ExplorerProgress(): ReactNode {
	const intl = useOrchbeeIntl();
	return (
		<div
			aria-label={intl.formatMessage(fileExplorerMessages.progress)}
			className={styles['root']}
			role='progressbar'
		>
			<span className={styles['bit']} />
		</div>
	);
}
