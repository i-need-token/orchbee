import {
	ContextMenuContent,
	ContextMenuGroup,
	ContextMenuItem,
	ContextMenuRadioGroup,
	ContextMenuRadioItem,
	ContextMenuSeparator,
} from '@orchbee/ui/components';
import * as React from 'react';

import type { SourceControlMenuGroup } from '../../types/source-control-menu';

interface ScmContextMenuContentProps {
	/** Where the focus goes when the menu closes: the tree it opened over. */
	readonly finalFocus: React.RefObject<HTMLElement | null>;
	readonly groups: readonly SourceControlMenuGroup[];
}

/**
 * A Source Control context menu as VS Code draws it: rows of text without icons, in groups with a
 * separator between each two, and a choice group's choice in force checked.
 */
export function ScmContextMenuContent({
	finalFocus,
	groups,
}: ScmContextMenuContentProps): React.ReactNode {
	return (
		<ContextMenuContent finalFocus={finalFocus}>
			{groups.map((group, index) => (
				<React.Fragment key={group.commands[0]?.id ?? index}>
					{index > 0 ? <ContextMenuSeparator /> : null}
					{group.choice ? (
						<ContextMenuRadioGroup
							value={group.commands.find((command) => command.checked)?.id ?? null}
						>
							{group.commands.map((command) => (
								<ContextMenuRadioItem
									key={command.id}
									closeOnClick
									disabled={command.disabled}
									value={command.id}
									onClick={command.onSelect}
								>
									{command.label}
								</ContextMenuRadioItem>
							))}
						</ContextMenuRadioGroup>
					) : (
						<ContextMenuGroup>
							{group.commands.map((command) => (
								<ContextMenuItem
									key={command.id}
									disabled={command.disabled}
									onClick={command.onSelect}
								>
									{command.label}
								</ContextMenuItem>
							))}
						</ContextMenuGroup>
					)}
				</React.Fragment>
			))}
		</ContextMenuContent>
	);
}
