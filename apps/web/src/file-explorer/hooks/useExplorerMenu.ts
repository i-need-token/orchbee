import { useMemoizedFn } from '@orchbee/foundation/react';
import * as React from 'react';

import type { VirtualTreeContextMenuEvent, VirtualTreeModel } from '../../virtual-tree';
import type { ExplorerItem } from '../model/explorer-item';
import type { ExplorerMenuTarget } from '../model/explorer-menus';
import type { ExplorerMenuGroup, ExplorerMenuItem } from '../types/explorer-menu';

interface ExplorerMenu {
	/** Keeps a chosen row, which runs as the closed menu hands the keys back to the tree. */
	readonly choose: (item: ExplorerMenuItem) => void;
	readonly finalFocus: () => HTMLElement | false;
	/** The menu the last request asked for, kept while it closes. */
	readonly groups: readonly ExplorerMenuGroup[];
	/** The tree's handler, which hears a request before the menu's trigger does. */
	readonly onTreeContextMenu: (event: VirtualTreeContextMenuEvent<string, ExplorerItem>) => void;
	/** The trigger's handler: keeps a request whose menu is empty from opening it. */
	readonly onTriggerContextMenu: (event: { readonly preventBaseUIHandler: () => void }) => void;
}

/** Whether the tree can take the keys back: still in the document and not in a hidden view. */
function canTakeFocus(element: HTMLElement | null): element is HTMLElement {
	return element !== null && element.isConnected && element.closest('[hidden]') === null;
}

/** The menu a request asks for: its row's file or folder, or the root below the rows. */
function menuTarget({
	row,
}: VirtualTreeContextMenuEvent<string, ExplorerItem>): ExplorerMenuTarget {
	if (row === null) return 'root';
	return row.element.directory ? 'folder' : 'file';
}

/**
 * The Explorer's context menu. A request for a row selects the row unless the selection holds it,
 * and the menu is the one for the row's file or folder, or for the root on the empty space below
 * the rows, as in VS Code. VS Code hides its menu and gives the keys back to the tree before the
 * chosen command runs, so the command acts on the tree as the person left it and a command that
 * moves the keys, such as Open to the Side, keeps them where it moved them.
 */
export function useExplorerMenu(
	model: VirtualTreeModel<string, ExplorerItem> | null,
	treeReference: React.RefObject<HTMLElement | null>,
	build: (target: ExplorerMenuTarget) => readonly ExplorerMenuGroup[],
): ExplorerMenu {
	const [groups, setGroups] = React.useState<readonly ExplorerMenuGroup[]>([]);
	const asked = React.useRef<readonly ExplorerMenuGroup[] | null>(null);
	const chosen = React.useRef<ExplorerMenuItem | null>(null);
	const onTreeContextMenu = useMemoizedFn(
		(event: VirtualTreeContextMenuEvent<string, ExplorerItem>) => {
			const { row } = event;
			if (row !== null && model !== null && !model.selection.has(row.key)) {
				model.setSelection([row.key]);
			}
			const next = build(menuTarget(event));
			asked.current = next;
			chosen.current = null;
			setGroups(next);
		},
	);
	const onTriggerContextMenu = useMemoizedFn(
		(event: { readonly preventBaseUIHandler: () => void }) => {
			const next = asked.current;
			asked.current = null;
			if (next === null || next.length === 0) event.preventBaseUIHandler();
		},
	);
	const choose = useMemoizedFn((item: ExplorerMenuItem) => {
		chosen.current = item;
	});
	// Base UI asks where the keys go as the closed menu leaves the document, after the focus it holds
	// while open could take them back from a chosen command.
	const finalFocus = useMemoizedFn((): HTMLElement | false => {
		const item = chosen.current;
		chosen.current = null;
		const tree = treeReference.current;
		if (item === null) {
			if (!canTakeFocus(tree)) return false;
			const { activeElement, body } = tree.ownerDocument;
			// A press outside has placed the keys already; Base UI would take them back.
			const placed =
				activeElement !== null &&
				activeElement !== body &&
				activeElement.closest('[role="menu"]') === null;
			return placed ? false : tree;
		}
		if (canTakeFocus(tree)) tree.focus({ preventScroll: true });
		item.run();
		return false;
	});
	return { choose, finalFocus, groups, onTreeContextMenu, onTriggerContextMenu };
}
