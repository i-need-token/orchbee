import { sourceControlMessages, useOrchbeeIntl } from '@orchbee/i18n';
import type { ReactNode } from 'react';

import styles from './ScmProgress.module.css';

/**
 * VS Code's view progress bar: a 2px line at the top of Source Control, whose short bit runs across
 * while Git reads a status or runs an operation. It lies over the rows, so they never move for it.
 */
export function ScmProgress(): ReactNode {
	const intl = useOrchbeeIntl();
	return (
		<div
			aria-label={intl.formatMessage(sourceControlMessages.progress)}
			className={styles['root']}
			role='progressbar'
		>
			<span className={styles['bit']} />
		</div>
	);
}
