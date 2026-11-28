import { fileExplorerMessages, useOrchbeeIntl } from '@orchbee/i18n';
import { Tooltip, TooltipContent, TooltipTrigger } from '@orchbee/ui/components';
import type { ReactNode } from 'react';

import type { FileError } from '../../../file-session';
import { readFailureReason } from '../../utils/readFailureReason';
import styles from './ExplorerUnresolvedRow.module.css';

interface ExplorerUnresolvedRowProps {
	/** Why the root could not be listed; null when it is gone. */
	readonly error: FileError | null;
	/** The root's name. */
	readonly name: string;
}

/**
 * VS Code's placeholder for a root it cannot resolve, in place of the tree: one row with the root's
 * name in the invalid-item color and a ! at the far end, whose tooltip says why.
 */
export function ExplorerUnresolvedRow({ error, name }: ExplorerUnresolvedRowProps): ReactNode {
	const intl = useOrchbeeIntl();
	const label = intl.formatMessage(fileExplorerMessages.unresolved, {
		reason: intl.formatMessage(readFailureReason(error)),
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
