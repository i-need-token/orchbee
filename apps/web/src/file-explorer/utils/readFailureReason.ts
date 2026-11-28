import { fileExplorerMessages, type MessageDescriptor } from '@orchbee/i18n';

import type { FileError } from '../../file-session';

/**
 * Why a folder could not be listed, as the Explorer words it; null stands for a folder that is
 * gone. VS Code quotes the error itself; the backend sends a code, which Orchbee words.
 */
export function readFailureReason(error: FileError | null): MessageDescriptor {
	switch (error?.code) {
		case undefined:
		case 'not-found':
			return fileExplorerMessages.readFailureNotFound;
		case 'denied':
		case 'permission-denied':
			return fileExplorerMessages.readFailurePermissionDenied;
		case 'not-directory':
			return fileExplorerMessages.readFailureNotDirectory;
		case 'remote-disconnected':
			return fileExplorerMessages.readFailureDisconnected;
		case 'timeout':
			return fileExplorerMessages.readFailureTimeout;
		case 'unavailable':
			return fileExplorerMessages.readFailureUnavailable;
		case 'unsupported-location':
			return fileExplorerMessages.readFailureUnsupportedLocation;
		default:
			return fileExplorerMessages.readFailureFailed;
	}
}
