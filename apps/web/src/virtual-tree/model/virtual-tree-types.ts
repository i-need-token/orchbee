/**
 * Supplies the items of one tree. The model calls it with the input item for the top level and with
 * a row's item when that row first expands or refreshes. Its callbacks run in the middle of a model
 * operation, so they must not call back into the model.
 */
export interface VirtualTreeSource<K, T> {
	/**
	 * An item's identity: stable across refreshes and unique across the whole tree. Focus, selection,
	 * expansion and view state follow it, and a refresh reuses the row whose key matches. It runs for
	 * every child on each refresh, so it must be constant-time.
	 */
	readonly key: (item: T) => K;
	/** Whether the item can expand. It runs for every child on each refresh. */
	readonly hasChildren: (item: T) => boolean;
	/**
	 * The item's children in display order. Return an array when they are at hand, so the rows change
	 * in the same task; return a promise when they must be fetched. The model asks for one item's
	 * children once at a time, and the signal aborts when nobody wants the answer any more: the row
	 * collapsed or left the tree, or the model was disposed. A promise that rejects after the abort is
	 * ignored; one that rejects before it collapses the row and fails the operation that asked, unless
	 * the error is an `AbortError`, which counts as the source cancelling the read.
	 */
	readonly children: (item: T, signal: AbortSignal) => readonly T[] | Promise<readonly T[]>;
}

/** One visible row. A row object stays the same while its key stays in the tree. */
export interface VirtualTreeRow<K, T> {
	readonly key: K;
	/** The item the latest refresh supplied for this key. */
	readonly element: T;
	/** The row this one is nested under; null for a top-level row. */
	readonly parent: VirtualTreeRow<K, T> | null;
	/** One for a top-level row. */
	readonly depth: number;
	readonly collapsible: boolean;
	readonly collapsed: boolean;
	/** One-based position among the visible siblings. */
	readonly positionInSet: number;
	readonly setSize: number;
	/** The rows directly below this one that belong to its subtree. */
	readonly visibleDescendantCount: number;
	/**
	 * True once the row's children have been loading for 800 ms, when VS Code's asynchronous tree
	 * shows a folder as slow; the tree draws a spinner in place of the twistie. A quicker load never
	 * shows one.
	 */
	readonly loading: boolean;
	/**
	 * Changes whenever the row's element, collapsibility, expansion or loading state changes in place,
	 * so a renderer can skip a row whose revision it has drawn. Position, set size, depth and the
	 * descendant count are read directly.
	 */
	readonly revision: number;
}

/**
 * Rows removed and inserted at one position. Splices in one event are ordered and apply in order:
 * each starts after the rows the one before it inserted, so every inserted row reads back with
 * `rowAt(start + offset)` once the event arrives.
 */
export interface VirtualTreeSplice {
	readonly start: number;
	readonly deleteCount: number;
	readonly insertCount: number;
}

/**
 * A change the model has finished applying, or, for `reveal`, a request to the view. `rows` with no
 * splices means rows changed in place; re-read the visible rows and compare their revisions.
 */
export type VirtualTreeEvent<K> =
	| { readonly kind: 'rows'; readonly splices: readonly VirtualTreeSplice[] }
	| {
			readonly kind: 'collapse';
			/** The row that expanded or collapsed; null when every row collapsed at once. */
			readonly key: K | null;
			readonly collapsed: boolean;
			readonly recursive: boolean;
	  }
	| { readonly kind: 'focus' }
	| { readonly kind: 'selection' }
	| { readonly kind: 'anchor' }
	| { readonly kind: 'scroll' }
	| {
			readonly kind: 'reveal';
			/** A row that shows now, for a view to scroll into sight. */
			readonly key: K;
			/** Where the row goes, from 0 at the top of the view to 1 at the bottom; null for nearest. */
			readonly relativeTop: number | null;
	  };

export interface VirtualTreeRevealOptions {
	/**
	 * Places the row at this point of the view, from 0 at the top to 1 at the bottom. Without it the
	 * view scrolls as little as it takes to show the whole row.
	 */
	readonly relativeTop?: number;
}

/** Everything needed to bring a tree back after it is recreated, keyed by item identity. */
export interface VirtualTreeViewState<K> {
	/** Every loaded row that is expanded, including rows inside collapsed parents. */
	readonly expanded: readonly K[];
	readonly focus: K | null;
	readonly anchor: K | null;
	readonly selection: readonly K[];
	/** The view's scroll offset in CSS pixels. */
	readonly scrollTop: number;
}

export interface VirtualTreeModelOptions<K, T> {
	readonly source: VirtualTreeSource<K, T>;
	/**
	 * The item whose children are the top-level rows. It never has a row of its own; its key is only
	 * read so the model can hold it like any other item.
	 */
	readonly input: T;
	/** Whether a newly loaded item starts expanded; the model loads its children right away. */
	readonly expandedByDefault?: (item: T) => boolean;
}
