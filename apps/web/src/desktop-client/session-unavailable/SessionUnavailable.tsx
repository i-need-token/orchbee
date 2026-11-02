import { sessionConnectionMessages, useOrchbeeIntl } from '@orchbee/i18n';
import { Button } from '@orchbee/ui/components';
import type { ReactNode } from 'react';

import styles from './SessionUnavailable.module.css';

export interface SessionUnavailableProps {
	readonly onTryAgain: () => void;
	/** `recovery-required` only when starting the Session was refused until it is recovered. */
	readonly reason: 'recovery-required' | 'unavailable';
}

/**
 * Takes the place of a window that never reached its Session, as a TUI that cannot start says why
 * and stops. Recovery stays an explicit command; trying again is the only effect here. The window
 * keeps a title-bar strip to be moved by, since no Workbench draws one.
 */
export function SessionUnavailable({ onTryAgain, reason }: SessionUnavailableProps): ReactNode {
	const intl = useOrchbeeIntl();
	return (
		<main className={styles['root']} data-reason={reason} data-slot='session-unavailable'>
			<div aria-hidden='true' className={styles['dragRegion']} />
			<div className={styles['content']}>
				<h1 className={styles['title']}>
					{intl.formatMessage(sessionConnectionMessages.unavailableTitle)}
				</h1>
				<p className={styles['description']}>
					{intl.formatMessage(
						reason === 'recovery-required'
							? sessionConnectionMessages.unavailableRecoveryRequired
							: sessionConnectionMessages.unavailableDescription,
					)}
				</p>
				<Button
					className={styles['action']}
					data-slot='session-try-again'
					onClick={onTryAgain}
					variant='outline'
				>
					{intl.formatMessage(sessionConnectionMessages.unavailableTryAgain)}
				</Button>
			</div>
		</main>
	);
}
