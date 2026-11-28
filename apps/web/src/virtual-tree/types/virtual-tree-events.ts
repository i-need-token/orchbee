import type { VirtualTreeRow } from '../model/virtual-tree-types';

/**
 * A row to open, as VS Code's lists ask their view: on a click, a double click, a middle click, or
 * a key that selects one row. Rows that expand are asked for too; a view that opens only files
 * ignores them.
 */
export interface VirtualTreeOpenEvent<K, T> {
	readonly row: VirtualTreeRow<K, T>;
	/** Whether keyboard focus stays in the tree: false only for Enter and a double click. */
	readonly preserveFocus: boolean;
	/** Whether the row opens for good rather than as a preview: Enter, a double click or a middle click. */
	readonly pinned: boolean;
	/** Whether it opens beside the current editor: a click or double click with Ctrl, Cmd or Alt. */
	readonly sideBySide: boolean;
}

/**
 * The tree's context menu was asked for, by the pointer or by Shift+F10 or the context menu key. The
 * tree has focused the row by then; a view shows its menu from a `ContextMenu` trigger around the
 * tree, which opens at the event's position, beside the focused row for a key.
 */
export interface VirtualTreeContextMenuEvent<K, T> {
	/** The row the menu is for; null on the empty space below the rows. */
	readonly row: VirtualTreeRow<K, T> | null;
	readonly browserEvent: MouseEvent;
}
