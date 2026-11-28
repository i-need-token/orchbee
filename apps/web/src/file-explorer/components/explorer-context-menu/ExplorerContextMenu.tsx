import {
	ContextMenuContent,
	ContextMenuGroup,
	ContextMenuItem,
	ContextMenuSeparator,
	ContextMenuShortcut,
} from '@orchbee/ui/components';
import * as React from 'react';

import type { ExplorerMenuGroup, ExplorerMenuItem } from '../../types/explorer-menu';
import styles from './ExplorerContextMenu.module.css';

interface ExplorerContextMenuProps {
	readonly finalFocus: () => HTMLElement | false;
	readonly groups: readonly ExplorerMenuGroup[];
	readonly onChoose: (item: ExplorerMenuItem) => void;
}

/**
 * The Explorer's context menu as VS Code draws it: rows of text without icons, each with its keys,
 * in groups with a separator between each two. It opens over the file overlay, which may lie under
 * where it opens.
 */
export function ExplorerContextMenu({
	finalFocus,
	groups,
	onChoose,
}: ExplorerContextMenuProps): React.ReactNode {
	return (
		<ContextMenuContent finalFocus={finalFocus} positionerClassName={styles['layer']}>
			{groups.map((group, index) => (
				<React.Fragment key={group[0]?.id ?? index}>
					{index > 0 ? <ContextMenuSeparator /> : null}
					<ContextMenuGroup>
						{group.map((item) => (
							<ContextMenuItem
								key={item.id}
								disabled={item.disabled}
								onClick={() => onChoose(item)}
							>
								{item.label}
								{item.shortcut === undefined ? null : (
									<ContextMenuShortcut>{item.shortcut}</ContextMenuShortcut>
								)}
							</ContextMenuItem>
						))}
					</ContextMenuGroup>
				</React.Fragment>
			))}
		</ContextMenuContent>
	);
}
