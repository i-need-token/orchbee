import {
	ContextMenuContent,
	ContextMenuItem,
	ContextMenuSeparator,
	ContextMenuShortcut,
} from '@orchbee/ui/components';
import type { ReactNode } from 'react';

import type {
	FileOverlayMenuCommand,
	FileOverlayMenuEntry,
} from '../../../../../../../../../../file-overlay-contract';
import styles from './FileTabEditorMenu.module.css';

interface FileTabEditorMenuProps {
	readonly entries: readonly FileOverlayMenuEntry[];
	readonly finalFocus: () => false;
	readonly onChoose: (entry: FileOverlayMenuCommand) => void;
}

/**
 * The text editor's menu, which shows VS Code's editor context menu: its items in VS Code's groups,
 * with VS Code's labels and keys. It shows above the overlay.
 */
export function FileTabEditorMenu({
	entries,
	finalFocus,
	onChoose,
}: FileTabEditorMenuProps): ReactNode {
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
