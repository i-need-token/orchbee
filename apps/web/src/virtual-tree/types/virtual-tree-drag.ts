import type { PointerDrag, PointerDragCargo } from '../../pointer-drag';
import type { VirtualTreeSegment } from '../model/virtual-tree-types';

/** How the tree takes a drag over a row that the view accepts, as VS Code's drag-over reaction says. */
export interface VirtualTreeDropReaction {
	/**
	 * Which rows show the drop: `down`, the default, shows the row and its visible descendants; `up`
	 * shows its parent and the parent's, or the whole tree for a top-level row, as a file hands VS
	 * Code's Explorer drops to its folder. An item a compact row joins shows a drop on it alone.
	 */
	readonly bubble?: 'down' | 'up' | undefined;
	/** Expands the row, while it is collapsed, once the pointer rests on it for 500 ms. */
	readonly autoExpand?: boolean | undefined;
}

/**
 * A view's part in dragging rows and dropping on them; each part is optional. The parts take items:
 * a row is its own item, and in a compact row each item it shows is one, as `segments` lists them.
 */
export interface VirtualTreeDragAndDrop<K, T> {
	/**
	 * What dragging `items` carries, or null to leave them where they are: the rows a drag takes, in
	 * tree order, or, for a drag that takes one compact row alone, the item whose name was pressed.
	 */
	readonly drag?:
		| ((items: readonly VirtualTreeSegment<K, T>[]) => PointerDragCargo | null)
		| undefined;
	/**
	 * Whether `drag` may drop on `target`, or on the tree itself for null, below the rows or over its
	 * scrollbar, and how the tree shows it; null refuses. The target is the row under the pointer, or
	 * the item of a compact row whose name is under it. It runs again when the target or the keys
	 * held change.
	 */
	readonly over?:
		| ((
				drag: PointerDrag,
				target: VirtualTreeSegment<K, T> | null,
		  ) => VirtualTreeDropReaction | null)
		| undefined;
	/** Drops `drag` on `target`, or on the tree for null, where `over` accepted it. */
	readonly drop?:
		| ((drag: PointerDrag, target: VirtualTreeSegment<K, T> | null) => void)
		| undefined;
}
