import { fileExplorerMessages, useOrchbeeIntl, type MessageDescriptor } from '@orchbee/i18n';
import { Tooltip, TooltipContent, TooltipTrigger } from '@orchbee/ui/components';
import type { ReactNode } from 'react';

import type { FileError } from '../../../file-session';
import styles from './ExplorerUnresolvedRow.module.css';

interface ExplorerUnresolvedRowProps {
	/** Why the root could not be listed; null when it is gone. */
	readonly error: FileError | null;
	/** The root's name. */
	readonly name: string;
}

/** VS Code quotes the error itself; Orchbee words the backend's code. */
function reasonOf(error: FileError | null): MessageDescriptor {
	switch (error?.code) {
		case undefined:
		case 'not-found':
			return fileExplorerMessages.unresolvedNotFound;
		case 'denied':
		case 'permission-denied':
			return fileExplorerMessages.unresolvedPermissionDenied;
		case 'not-directory':
			return fileExplorerMessages.unresolvedNotDirectory;
		case 'remote-disconnected':
			return fileExplorerMessages.unresolvedDisconnected;
		case 'timeout':
			return fileExplorerMessages.unresolvedTimeout;
		case 'unavailable':
			return fileExplorerMessages.unresolvedUnavailable;
		case 'unsupported-location':
			return fileExplorerMessages.unresolvedUnsupportedLocation;
		default:
			return fileExplorerMessages.unresolvedFailed;
	}
}

/**
 * VS Code's placeholder for a root it cannot resolve, in place of the tree: one row with the root's
 * name in the invalid-item color and a ! at the far end, whose tooltip says why.
 */
export function ExplorerUnresolvedRow({ error, name }: ExplorerUnresolvedRowProps): ReactNode {
	const intl = useOrchbeeIntl();
	const label = intl.formatMessage(fileExplorerMessages.unresolved, {
		reason: intl.formatMessage(reasonOf(error)),
	});
	return (
		<div className={styles['root']} data-slot='file-explorer-unresolved'>
			<span className={styles['name']}>{name}</span>
			<Tooltip>
				<TooltipTrigger render={<span aria-label={label} className={styles['mark']} role='img' />}>
					!
				</TooltipTrigger>
				<TooltipContent side='bottom'>{label}</TooltipContent>
			</Tooltip>
		</div>
	);
}
