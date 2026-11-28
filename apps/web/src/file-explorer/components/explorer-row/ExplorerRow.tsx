import { fileExplorerMessages, useOrchbeeIntl } from '@orchbee/i18n';
import { FileIcon, Tooltip, TooltipContent, TooltipTrigger } from '@orchbee/ui/components';
import type { ReactNode } from 'react';

import type { ExplorerItem } from '../../model/explorer-item';
import styles from './ExplorerRow.module.css';

interface ExplorerRowProps {
	readonly item: ExplorerItem;
}

interface ExplorerRowMarkProps {
	/** What the mark means, as its name and tooltip say it. */
	readonly label: string;
	readonly letter: string;
}

function ExplorerRowMark({ label, letter }: ExplorerRowMarkProps): ReactNode {
	return (
		<Tooltip>
			<TooltipTrigger render={<span aria-label={label} className={styles['mark']} role='img' />}>
				{letter}
			</TooltipTrigger>
			<TooltipContent side='bottom'>{label}</TooltipContent>
		</Tooltip>
	);
}

/**
 * One Explorer row's label: a file's Seti icon, since Seti has no folder icons, the entry's name,
 * and VS Code's mark at the far end, where a link's ⤷ wins over the ? of an entry that is neither a
 * file nor a folder.
 */
export function ExplorerRow({ item }: ExplorerRowProps): ReactNode {
	const intl = useOrchbeeIntl();
	let mark: ReactNode = null;
	if (item.entry?.symlink === true) {
		mark = (
			<ExplorerRowMark label={intl.formatMessage(fileExplorerMessages.symbolicLink)} letter='⤷' />
		);
	} else if (item.entry?.type === 'unknown') {
		mark = (
			<ExplorerRowMark label={intl.formatMessage(fileExplorerMessages.unknownType)} letter='?' />
		);
	}
	return (
		<span className={styles['root']}>
			{item.directory ? null : <FileIcon name={item.name} path={item.path} />}
			<span className={styles['name']}>{item.name}</span>
			{mark}
		</span>
	);
}
