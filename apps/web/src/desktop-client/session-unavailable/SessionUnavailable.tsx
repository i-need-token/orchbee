import type { DesktopSessionRefusal } from '@orchbee/desktop-client-protocol';
import { sessionConnectionMessages, useOrchbeeIntl } from '@orchbee/i18n';
import { Button } from '@orchbee/ui/components';
import type { ReactNode } from 'react';

import styles from './SessionUnavailable.module.css';

export interface SessionUnavailableProps {
	readonly onTryAgain: () => void;
	/** Why starting the Session was refused, or null when nothing says why the window cannot reach it. */
	readonly refusal: DesktopSessionRefusal | null;
}

/** Says what the TUI and CLI say for the same refusal, with Desktop naming itself. */
function describe(
	intl: ReturnType<typeof useOrchbeeIntl>,
	refusal: DesktopSessionRefusal | null,
): string {
	switch (refusal?.reason) {
		case undefined:
			return intl.formatMessage(sessionConnectionMessages.unavailableDescription);
		case 'recovery-required':
			return intl.formatMessage(sessionConnectionMessages.unavailableRecoveryRequired);
		case 'desktop-too-old':
		case 'session-too-old':
			return intl.formatMessage(
				refusal.reason === 'desktop-too-old'
					? sessionConnectionMessages.unavailableDesktopTooOld
					: sessionConnectionMessages.unavailableSessionTooOld,
				{
					desktopVersion: refusal.desktopVersion,
					hasSessionVersion: String(refusal.sessionVersion !== null),
					sessionVersion: refusal.sessionVersion ?? '',
				},
			);
		case 'other-environment':
			return intl.formatMessage(sessionConnectionMessages.unavailableOtherEnvironment);
		case 'development-build-mismatch':
			return intl.formatMessage(sessionConnectionMessages.unavailableDevelopmentBuildMismatch);
	}
}

/**
 * Takes the place of a window that never reached its Session, as a TUI that cannot start says why
 * and stops. A refusal names its own remedy, and stopping or recovering the Session stays an explicit
 * command; trying again is the only effect here. The window keeps a title-bar strip to be moved by,
 * since no Workbench draws one.
 */
export function SessionUnavailable({ onTryAgain, refusal }: SessionUnavailableProps): ReactNode {
	const intl = useOrchbeeIntl();
	return (
		<main
			className={styles['root']}
			data-reason={refusal?.reason ?? 'unavailable'}
			data-slot='session-unavailable'
		>
			<div aria-hidden='true' className={styles['dragRegion']} />
			<div className={styles['content']}>
				<h1 className={styles['title']}>
					{intl.formatMessage(sessionConnectionMessages.unavailableTitle)}
				</h1>
				<p className={styles['description']}>{describe(intl, refusal)}</p>
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
