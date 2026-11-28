import { sourceControlMessages, type IntlShape } from '@orchbee/i18n';

import type { FileError } from '../../file-session';

/**
 * What a failed Git request tells the person: why the machine could not run it, or, as VS Code's
 * Git says by default, Git's first line of error output without its `error: ` prefix.
 */
export function gitFailureMessage(intl: IntlShape, error: FileError): string {
	switch (error.code) {
		case 'denied':
		case 'permission-denied':
			return intl.formatMessage(sourceControlMessages.errorPermissionDenied);
		case 'remote-disconnected':
			return intl.formatMessage(sourceControlMessages.errorDisconnected);
		case 'timeout':
			return intl.formatMessage(sourceControlMessages.errorTimeout);
		case 'unavailable':
			return intl.formatMessage(sourceControlMessages.errorUnavailable);
		default:
			break;
	}
	const hint = (error.detail ?? '')
		.replace(/^error: /imu, '')
		.replace(/^> husky.*$/imu, '')
		.split(/[\r\n]/u)
		.find((line) => line !== '');
	return hint === undefined
		? intl.formatMessage(sourceControlMessages.errorGeneric)
		: intl.formatMessage(sourceControlMessages.errorGit, { hint });
}
