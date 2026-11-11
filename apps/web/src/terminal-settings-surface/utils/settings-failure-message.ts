import { terminalSettingsMessages, type MessageDescriptor } from '@orchbee/i18n';
import type { SettingsRefusal, SettingsRefusalReason } from '@orchbee/workbench-client-protocol';

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

/** The details an unusable file carries when the file exists but cannot be read. */
const UNREADABLE_DETAILS: ReadonlySet<string> = new Set(['not-file', 'io-error', 'too-large']);

/**
 * One short message per refusal, saying what the person can do about it. An unusable file whose
 * detail is a read failure is a file that cannot be read, not a location that cannot be used.
 */
export function settingsRefusalMessage(
	refusal: Pick<SettingsRefusal, 'reason' | 'detail'>,
): MessageDescriptor {
	if (
		refusal.reason === 'unusable-file' &&
		refusal.detail !== undefined &&
		UNREADABLE_DETAILS.has(refusal.detail)
	) {
		return terminalSettingsMessages.refusedUnreadable;
	}
	return REFUSAL_MESSAGES[refusal.reason];
}

/** The message a row shows under a change that did not land. */
export function settingsFailureMessage(failure: UserSettingsFailure): MessageDescriptor {
	switch (failure.kind) {
		case 'refused':
			return settingsRefusalMessage(failure.refusal);
		case 'outcome-unknown':
			return terminalSettingsMessages.sharedUnknown;
		case 'unavailable':
			return terminalSettingsMessages.sharedUnavailable;
	}
}
