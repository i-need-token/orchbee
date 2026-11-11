import { terminalSettingsMessages, type MessageDescriptor } from '@orchbee/i18n';
import type { SettingsRefusalReason } from '@orchbee/workbench-client-protocol';

import type { UserSettingsFailure } from '../../user-settings';

const REFUSAL_MESSAGES = {
	busy: terminalSettingsMessages.refusedBusy,
	changed: terminalSettingsMessages.refusedChanged,
	denied: terminalSettingsMessages.refusedDenied,
	'file-in-use': terminalSettingsMessages.refusedFileInUse,
	'invalid-value': terminalSettingsMessages.refusedInvalidValue,
	locked: terminalSettingsMessages.refusedLocked,
	'read-only': terminalSettingsMessages.refusedReadOnly,
	'session-mismatch': terminalSettingsMessages.refusedSessionMismatch,
	'syntax-error': terminalSettingsMessages.refusedSyntaxError,
	'too-large': terminalSettingsMessages.refusedTooLarge,
	unavailable: terminalSettingsMessages.refusedUnavailable,
	'unknown-key': terminalSettingsMessages.refusedUnknownKey,
	'unusable-file': terminalSettingsMessages.refusedUnusableFile,
	'write-failed': terminalSettingsMessages.refusedWriteFailed,
} as const satisfies Readonly<Record<SettingsRefusalReason, MessageDescriptor>>;

/** One short message per refusal reason, saying what the person can do about it. */
export function settingsRefusalMessage(reason: SettingsRefusalReason): MessageDescriptor {
	return REFUSAL_MESSAGES[reason];
}

/** The message a row shows under a change that did not land. */
export function settingsFailureMessage(failure: UserSettingsFailure): MessageDescriptor {
	switch (failure.kind) {
		case 'refused':
			return settingsRefusalMessage(failure.refusal.reason);
		case 'outcome-unknown':
			return terminalSettingsMessages.sharedUnknown;
		case 'unavailable':
			return terminalSettingsMessages.sharedUnavailable;
	}
}
