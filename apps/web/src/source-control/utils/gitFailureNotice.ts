import { sourceControlMessages, type IntlShape, type MessageDescriptor } from '@orchbee/i18n';

import type { FileError } from '../../file-session';

/** How a failed Git request is told: its words, and the severity of the card that tells them. */
export interface GitFailureNotice {
	readonly message: string;
	readonly severity: 'error' | 'info' | 'warning';
}

/** Git names the remote that refused the credentials in this line of its error output. */
const AUTHENTICATION_FAILED = /Authentication failed for '(.*)'/iu;

/**
 * What a failed Git request tells the person, as VS Code's Git command center tells it: why the
 * machine could not run it; VS Code's own words for the failures it explains, such as a rejected
 * push, merge conflicts or a missing identity; and otherwise Git's first line of error output
 * without its `error: ` prefix.
 */
export function gitFailureNotice(intl: IntlShape, error: FileError): GitFailureNotice {
	const notice = (
		message: MessageDescriptor,
		severity: GitFailureNotice['severity'] = 'error',
		values?: Record<string, string>,
	): GitFailureNotice => ({ message: intl.formatMessage(message, values), severity });
	switch (error.code) {
		case 'denied':
		case 'permission-denied':
			return notice(sourceControlMessages.errorPermissionDenied);
		case 'remote-disconnected':
			return notice(sourceControlMessages.errorDisconnected);
		case 'timeout':
			return notice(sourceControlMessages.errorTimeout);
		case 'unavailable':
			return notice(sourceControlMessages.errorUnavailable);
		default:
			break;
	}
	switch (error.gitErrorCode) {
		case 'authentication-failed': {
			const remote = AUTHENTICATION_FAILED.exec(error.detail ?? '')?.[1];
			return remote === undefined
				? notice(sourceControlMessages.errorAuthenticationFailedPlain)
				: notice(sourceControlMessages.errorAuthenticationFailed, 'error', { remote });
		}
		case 'conflict':
			return notice(sourceControlMessages.errorConflict, 'warning');
		case 'dirty-work-tree':
			return notice(sourceControlMessages.errorDirtyWorkTree);
		case 'empty-commit-message':
			return notice(sourceControlMessages.errorEmptyCommitMessage, 'info');
		case 'no-user-email-configured':
		case 'no-user-name-configured':
			return notice(sourceControlMessages.errorNoUserConfigured);
		case 'push-rejected':
			return notice(sourceControlMessages.errorPushRejected);
		default:
			break;
	}
	const hint = (error.detail ?? '')
		.replace(/^error: /imu, '')
		.replace(/^> husky.*$/imu, '')
		.split(/[\r\n]/u)
		.find((line) => line !== '');
	return hint === undefined
		? notice(sourceControlMessages.errorGeneric)
		: notice(sourceControlMessages.errorGit, 'error', { hint });
}
