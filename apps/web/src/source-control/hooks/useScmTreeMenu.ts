import { useMemoizedFn } from '@orchbee/foundation/react';
import * as React from 'react';

import type { SourceControlMenuGroup } from '../types/source-control-menu';

/**
 * A tree's context menu. The tree hears a request first, through its own listener, and sets the menu
 * for the row; the menu's trigger around the tree then hears the same request and opens it, unless
 * the menu is empty, as VS Code shows nothing where a row offers nothing.
 */
interface ScmTreeMenu {
	/** The menu the last request asked for. */
	readonly groups: readonly SourceControlMenuGroup[];
	/** The trigger's handler: keeps a request whose menu is empty from opening it. */
	readonly onTriggerContextMenu: (event: { readonly preventBaseUIHandler: () => void }) => void;
	/** Sets the menu for the request the tree just heard. */
	readonly show: (groups: readonly SourceControlMenuGroup[]) => void;
}

export function useScmTreeMenu(): ScmTreeMenu {
	const [groups, setGroups] = React.useState<readonly SourceControlMenuGroup[]>([]);
	const asked = React.useRef<readonly SourceControlMenuGroup[] | null>(null);
	const show = useMemoizedFn((next: readonly SourceControlMenuGroup[]) => {
		asked.current = next;
		setGroups(next);
	});
	const onTriggerContextMenu = useMemoizedFn(
		(event: { readonly preventBaseUIHandler: () => void }) => {
			const next = asked.current;
			asked.current = null;
			if (next === null || next.length === 0) event.preventBaseUIHandler();
		},
	);
	return { groups, onTriggerContextMenu, show };
}
