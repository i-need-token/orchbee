import { MenuSearch } from '@orchbee/ui/components';
import { useState, type ReactNode } from 'react';

import type {
	WorkbenchActionMenuCommand,
	WorkbenchActionMenuSearchSubmenu,
} from '../../types/workbench-action-menu';

interface WorkbenchActionMenuSearchListProps {
	readonly entry: WorkbenchActionMenuSearchSubmenu;
	/** The surrounding menu's own row, so a choice closes that menu as any row does. */
	readonly renderCommand: (command: WorkbenchActionMenuCommand) => ReactNode;
}

/** A search submenu's body; it mounts with the submenu, so each opening starts unfiltered. */
export function WorkbenchActionMenuSearchList({
	entry,
	renderCommand,
}: WorkbenchActionMenuSearchListProps): ReactNode {
	const [query, setQuery] = useState('');
	const commands = entry.commands(query);
	return (
		<MenuSearch
			emptyLabel={commands.length === 0 ? entry.emptyLabel(query) : undefined}
			label={entry.searchLabel}
			query={query}
			onQueryChange={setQuery}
		>
			{commands.map(renderCommand)}
		</MenuSearch>
	);
}
