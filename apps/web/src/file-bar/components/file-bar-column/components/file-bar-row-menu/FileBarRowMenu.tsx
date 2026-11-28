import {
	ContextMenuCheckboxItem,
	ContextMenuContent,
	ContextMenuItem,
	ContextMenuSeparator,
	ContextMenuShortcut,
} from '@orchbee/ui/components';
import type { ReactNode } from 'react';

import type { FileBarRowMenuEntry } from '../../../../file-bar-contract';
import styles from './FileBarRowMenu.module.css';

interface FileBarRowMenuProps {
	readonly entries: readonly FileBarRowMenuEntry[];
	readonly finalFocus: () => HTMLElement | false;
	readonly onChoose: (entry: Exclude<FileBarRowMenuEntry, { readonly kind: 'separator' }>) => void;
}

/**
 * The toggles row's menu as VS Code draws its activity bar menu: rows of text without icons, a check
 * on each view whose toggle shows, and each item's shortcut. It shows above the file overlay, whose
 * tiers start below the row.
 */
export function FileBarRowMenu({ entries, finalFocus, onChoose }: FileBarRowMenuProps): ReactNode {
	return (
		<ContextMenuContent finalFocus={finalFocus} positionerClassName={styles['layer']}>
			{entries.map((entry) => {
				switch (entry.kind) {
					case 'separator':
						return <ContextMenuSeparator key={entry.id} />;
					case 'toggle':
						return (
							<ContextMenuCheckboxItem
								key={entry.id}
								checked={entry.checked}
								closeOnClick
								disabled={entry.disabled}
								onClick={() => onChoose(entry)}
							>
								{entry.label}
								{entry.shortcut === undefined ? null : (
									<ContextMenuShortcut>{entry.shortcut}</ContextMenuShortcut>
								)}
							</ContextMenuCheckboxItem>
						);
					case 'command':
						return (
							<ContextMenuItem
								key={entry.id}
								disabled={entry.disabled}
								onClick={() => onChoose(entry)}
							>
								{entry.label}
								{entry.shortcut === undefined ? null : (
									<ContextMenuShortcut>{entry.shortcut}</ContextMenuShortcut>
								)}
							</ContextMenuItem>
						);
				}
			})}
		</ContextMenuContent>
	);
}
