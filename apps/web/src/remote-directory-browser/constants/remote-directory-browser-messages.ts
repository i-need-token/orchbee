import { remoteDirectoryBrowserMessages, type MessageDescriptor } from '@orchbee/i18n';

import type { RemoteDirectoryNavigationFailureReason } from '../types/remote-directory-browser';

export const REMOTE_DIRECTORY_EMPTY_MESSAGES = {
	empty: remoteDirectoryBrowserMessages.empty,
	hidden: remoteDirectoryBrowserMessages.emptyHidden,
	'no-match': remoteDirectoryBrowserMessages.filterNoMatches,
} as const satisfies Readonly<Record<'empty' | 'hidden' | 'no-match', MessageDescriptor>>;

export const REMOTE_DIRECTORY_NAVIGATION_FAILURE_MESSAGES: Readonly<
	Record<RemoteDirectoryNavigationFailureReason, MessageDescriptor>
> = {
	'invalid-path': remoteDirectoryBrowserMessages.failureInvalidPath,
	'not-found': remoteDirectoryBrowserMessages.failureNotFound,
	'not-directory': remoteDirectoryBrowserMessages.failureNotDirectory,
	'permission-denied': remoteDirectoryBrowserMessages.failurePermission,
	'timed-out': remoteDirectoryBrowserMessages.failureTimedOut,
	busy: remoteDirectoryBrowserMessages.failureBusy,
	unavailable: remoteDirectoryBrowserMessages.failureUnavailable,
};
