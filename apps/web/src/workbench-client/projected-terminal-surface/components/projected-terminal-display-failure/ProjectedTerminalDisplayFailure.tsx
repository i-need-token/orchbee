import { projectedTerminalMessages, useOrchbeeIntl } from '@orchbee/i18n';
import { Button } from '@orchbee/ui/components';
import { useId, type ReactNode } from 'react';

import styles from './ProjectedTerminalDisplayFailure.module.css';

interface ProjectedTerminalDisplayFailureProps {
	readonly onClose: () => void;
	readonly onRetry: () => void;
}

/**
 * Takes the place of a surface that failed to render. The terminal itself keeps running on the
 * Server, so Retry draws it again; nothing retries by itself, since a failure that repeats would
 * otherwise repeat forever.
 */
export function ProjectedTerminalDisplayFailure({
	onClose,
	onRetry,
}: ProjectedTerminalDisplayFailureProps): ReactNode {
	const intl = useOrchbeeIntl();
	const titleId = useId();
	return (
		<section
			aria-labelledby={titleId}
			className={styles['root']}
			data-slot='projected-terminal-display-failure'
		>
			<div className={styles['content']}>
				<h2 className={styles['title']} id={titleId}>
					{intl.formatMessage(projectedTerminalMessages.displayFailed)}
				</h2>
				<p className={styles['reason']}>
					{intl.formatMessage(projectedTerminalMessages.displayFailedDescription)}
				</p>
				<div className={styles['actions']}>
					<Button data-slot='projected-terminal-display-retry' onClick={onRetry}>
						{intl.formatMessage(projectedTerminalMessages.displayRetry)}
					</Button>
					<Button data-slot='projected-terminal-display-close' onClick={onClose} variant='outline'>
						{intl.formatMessage(projectedTerminalMessages.launchClose)}
					</Button>
				</div>
			</div>
		</section>
	);
}
