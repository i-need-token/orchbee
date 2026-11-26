import type { DesktopSessionRefusal } from '@orchbee/desktop-client-protocol';
import {
	sessionConnectionMessages,
	sessionPathRefusalMessages,
	useOrchbeeIntl,
} from '@orchbee/i18n';
import { Button } from '@orchbee/ui/components';
import type { ReactNode } from 'react';

import styles from './SessionUnavailable.module.css';

/** The system a Desktop window and its Session run on. */
export type DesktopSystem = 'linux' | 'macos' | 'windows';

export interface SessionUnavailableProps {
	readonly onTryAgain: () => void;
	/** Why starting the Session was refused, or null when nothing says why the window cannot reach it. */
	readonly refusal: DesktopSessionRefusal | null;
	/** The system this window and its Session run on, whose own way to end a process the copy names. */
	readonly system: DesktopSystem;
}

/** A command the copy names stays on one line, so it reads and copies as written. */
function command(chunks: readonly ReactNode[]): ReactNode {
	return <code className={styles['command']}>{chunks}</code>;
}

/** A command that holds a path can outgrow a line, so it wraps instead of leaving the page. */
function pathCommand(chunks: readonly ReactNode[]): ReactNode {
	return <code className={styles['pathCommand']}>{chunks}</code>;
}

type ServerNotStarted = Extract<DesktopSessionRefusal, { reason: 'server-not-started' }>;

/**
 * Which sentence names what the system refused, as the TUI and CLI choose it: a missing or denied
 * program file by name, any other refusal by the system's error number when it stated one.
 */
function systemAnswer({ ioKind, osError }: ServerNotStarted): { cause: string; code: string } {
	if (ioKind === 'not-found') return { cause: 'missing', code: '' };
	if (ioKind === 'permission-denied') return { cause: 'denied', code: '' };
	return osError === null ? { cause: 'other', code: '' } : { cause: 'code', code: String(osError) };
}

/** Says what the TUI and CLI say for the same refusal, with Desktop naming itself. */
function describe(
	intl: ReturnType<typeof useOrchbeeIntl>,
	refusal: DesktopSessionRefusal | null,
	system: SessionUnavailableProps['system'],
): ReactNode {
	switch (refusal?.reason) {
		case undefined:
			return intl.formatMessage(sessionConnectionMessages.unavailableDescription);
		case 'recovery-required':
			// Process IDs stay as the system writes them, without digit grouping.
			return refusal.blockingProcess === undefined
				? intl.formatMessage(sessionConnectionMessages.unavailableRecoveryRequired, { command })
				: intl.formatMessage(sessionConnectionMessages.unavailableRecoveryBlocked, {
						command,
						processId: String(refusal.blockingProcess.processId),
						program: refusal.blockingProcess.program,
						role: refusal.blockingProcess.role,
						system,
					});
		case 'server-not-started':
			return intl.formatMessage(
				sessionConnectionMessages.unavailableServerNotStarted,
				systemAnswer(refusal),
			);
		case 'desktop-too-old':
			return intl.formatMessage(sessionConnectionMessages.unavailableDesktopTooOld, {
				desktopVersion: refusal.desktopVersion,
				hasSessionVersion: String(refusal.sessionVersion !== null),
				sessionVersion: refusal.sessionVersion ?? '',
			});
		case 'session-too-old':
			return intl.formatMessage(sessionConnectionMessages.unavailableSessionTooOld, {
				command,
				desktopVersion: refusal.desktopVersion,
				hasSessionVersion: String(refusal.sessionVersion !== null),
				sessionVersion: refusal.sessionVersion ?? '',
				system,
			});
		case 'other-environment':
			return intl.formatMessage(sessionConnectionMessages.unavailableOtherEnvironment);
		case 'development-build-mismatch':
			return intl.formatMessage(sessionConnectionMessages.unavailableDevelopmentBuildMismatch, {
				command,
			});
		case 'path-refused':
			return intl.formatMessage(sessionPathRefusalMessages[refusal.cause], {
				command: pathCommand,
				path: refusal.path,
				system: refusal.system,
			});
	}
}

/**
 * Takes the place of a window that never reached its Session, as a TUI that cannot start says why
 * and stops. A refusal names its own remedy; a Session too old to join was already restarted where
 * that keeps its terminals, so its remedy names only what the person can do on the system. Trying
 * again is the only effect here. The window keeps a title-bar strip to be moved by, since no
 * Workbench draws one.
 */
export function SessionUnavailable({
	onTryAgain,
	refusal,
	system,
}: SessionUnavailableProps): ReactNode {
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
				<p className={styles['description']}>{describe(intl, refusal, system)}</p>
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
