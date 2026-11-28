import {
	ContextMenuContent,
	ContextMenuItem,
	ContextMenuSeparator,
	ContextMenuShortcut,
} from '@orchbee/ui/components';
import type { ReactNode } from 'react';

import type {
	FileTabMenuCommand,
	FileTabMenuEntry,
} from '../../../../../../../../file-overlay-contract';
import styles from './FileTabMenu.module.css';

interface FileTabMenuProps {
	readonly entries: readonly FileTabMenuEntry[];
	readonly finalFocus: () => HTMLElement | false;
	readonly onChoose: (entry: FileTabMenuCommand) => void;
}

/**
 * A file tab's menu, which shows VS Code's tab menu: its items in VS Code's groups, with VS Code's
 * labels and none of the icons VS Code does not show there. It shows above the overlay.
 */
export function FileTabMenu({ entries, finalFocus, onChoose }: FileTabMenuProps): ReactNode {
	return (
		<ContextMenuContent finalFocus={finalFocus} positionerClassName={styles['layer']}>
			{entries.map((entry) =>
				entry.kind === 'separator' ? (
					<ContextMenuSeparator key={entry.id} />
				) : (
					<ContextMenuItem key={entry.id} disabled={entry.disabled} onClick={() => onChoose(entry)}>
						{entry.label}
						{entry.shortcut === undefined ? null : (
							<ContextMenuShortcut>{entry.shortcut}</ContextMenuShortcut>
						)}
					</ContextMenuItem>
				),
			)}
		</ContextMenuContent>
	);
}
