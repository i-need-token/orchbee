import type { PointerDrag, PointerDragCargo } from '../../pointer-drag';
import type { VirtualTreeRow } from '../model/virtual-tree-types';

/** How the tree takes a drag over a row that the view accepts, as VS Code's drag-over reaction says. */
export interface VirtualTreeDropReaction {
	/**
	 * Which rows show the drop: `down`, the default, shows the row and its visible descendants; `up`
	 * shows its parent and the parent's, or the whole tree for a top-level row, as a file hands VS
	 * Code's Explorer drops to its folder.
	 */
	readonly bubble?: 'down' | 'up' | undefined;
	/** Expands the row, while it is collapsed, once the pointer rests on it for 500 ms. */
	readonly autoExpand?: boolean | undefined;
}

/** A view's part in dragging rows and dropping on them; each part is optional. */
export interface VirtualTreeDragAndDrop<K, T> {
	/** What dragging `rows`, in tree order, carries, or null to leave them where they are. */
	readonly drag?: ((rows: readonly VirtualTreeRow<K, T>[]) => PointerDragCargo | null) | undefined;
	/**
	 * Whether `drag` may drop on `row`, or on the tree itself for null, below the rows or over its
	 * scrollbar, and how the tree shows it; null refuses. It runs again when the row under the pointer
	 * or the keys held change.
	 */
	readonly over?:
		| ((drag: PointerDrag, row: VirtualTreeRow<K, T> | null) => VirtualTreeDropReaction | null)
		| undefined;
	/** Drops `drag` on `row`, or on the tree for null, where `over` accepted it. */
	readonly drop?: ((drag: PointerDrag, row: VirtualTreeRow<K, T> | null) => void) | undefined;
}
