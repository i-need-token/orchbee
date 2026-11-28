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
	/**
	 * Whether the item may join its parent's row while it is the parent's only child and the parent is
	 * expanded, as VS Code's compact folders join a folder with its only subfolder; the joined row
	 * expands and collapses with its last item. The model counts the children `children` returns, so
	 * a source that hides items leaves them out there. Without it, every item keeps a row of its own.
	 * An item that cannot expand joins too, so answer false for files, as VS Code's Explorer keeps
	 * them out of compact folders. A view that makes rows compact draws their names with
	 * `VirtualTreeSegments`, which renders the element the tree names while one of the items has
	 * focus. It runs for every child on each refresh.
	 */
	readonly compressible?: ((item: T) => boolean) | undefined;
}

/** One item a row shows: the row's own, or, in a compact row, one of the items joined into it. */
export interface VirtualTreeSegment<K, T> {
	readonly key: K;
	/** The item the latest refresh supplied for this key. */
	readonly element: T;
	/** Whether the item can expand; an item joined into the row of its only child always can. */
	readonly collapsible: boolean;
}

/**
 * One visible row. A row object stays the same while its key stays in the tree. A compact row shows
 * several items, its `segments`; the row itself is its last item's, whose key, element, expansion and
 * loading state it carries.
 */
export interface VirtualTreeRow<K, T> extends VirtualTreeSegment<K, T> {
	/** The row this one is nested under; null for a top-level row. */
	readonly parent: VirtualTreeRow<K, T> | null;
	/** How far the row indents: one for a top-level row, and one more for each row it is nested under. */
	readonly depth: number;
	/**
	 * The item's level, which assistive technology announces: one for a top-level item, and one more
	 * for each item above it, counting the items compact rows join. It equals `depth` without them.
	 */
	readonly level: number;
	/**
	 * The items the row shows, outermost first: the items joined into it and then its own, or just its
	 * own. The array stays the same until the items change.
	 */
	readonly segments: readonly VirtualTreeSegment<K, T>[];
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
	 * an item joins or leaves the row or changes its element, or the characters find highlights in it
	 * change, so a renderer can skip a row whose revision it has drawn. Position, set size, depth and
	 * the descendant count are read directly.
	 */
	readonly revision: number;
}

/** Characters of a label a find matched, from `start` up to but not including `end`. */
export interface VirtualTreeHighlight {
	readonly start: number;
	readonly end: number;
}

/**
 * How find matches a label, ignoring case: `fuzzy` finds the pattern's characters in order anywhere
 * in it, preferring word starts and runs, and `contiguous` finds them together.
 */
export type VirtualTreeFindMatchType = 'fuzzy' | 'contiguous';

/** What a model finds among its rows, as VS Code's tree find widget asks for it. */
export interface VirtualTreeFindQuery<K, T> {
	/** The text to find, never empty: `find(null)` stops finding. */
	readonly pattern: string;
	readonly matchType: VirtualTreeFindMatchType;
	/**
	 * Whether rows whose label misses hide, unless a row below them matches, as VS Code's filter mode
	 * does; otherwise every row shows and the matched characters are highlighted.
	 */
	readonly filter: boolean;
	/**
	 * The text a row is found by, such as its type navigation label. It must depend only on the row's
	 * items, since a find that extends the pattern trusts earlier misses, and keep its identity while
	 * it means the same, since a new function starts the find over.
	 */
	readonly label: (row: VirtualTreeRow<K, T>) => string;
	/**
	 * While filtering, hides each row it answers false for, with its subtree, as an asynchronous find
	 * provider's `isVisible` does in VS Code. It is asked again whenever the rows settle.
	 */
	readonly visible?: ((row: VirtualTreeRow<K, T>) => boolean) | undefined;
}

/** The find the rows show and what it counted. */
export interface VirtualTreeFindStatus<K, T> {
	readonly query: VirtualTreeFindQuery<K, T>;
	/** Loaded items whose label matched, a compact row counting once. */
	readonly matchCount: number;
	/**
	 * Loaded items the find looked at: every one, collapsed or not, except those below a row that
	 * `visible` hid, as VS Code's find counts them.
	 */
	readonly totalCount: number;
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
	/** The find the rows show, or what it counted, changed; read `findStatus`. */
	| { readonly kind: 'find' }
	| ({ readonly kind: 'reveal' } & VirtualTreeRevealRequest<K>);

/** A row the model asks its view to scroll into sight once the operation that asked is through. */
export interface VirtualTreeRevealRequest<K> {
	/** A row that shows now. */
	readonly key: K;
	/** Leaves the view where it is while the row shows whole. */
	readonly onlyOutOfSight: boolean;
	/** Where the row goes, from 0 at the top of the view to 1 at the bottom; null for nearest. */
	readonly relativeTop: number | null;
}

export interface VirtualTreeRevealOptions {
	/**
	 * Places the row at this point of the view, from 0 at the top to 1 at the bottom. Without it the
	 * view scrolls as little as it takes to show the whole row.
	 */
	readonly relativeTop?: number;
	/**
	 * Leaves the view where it is while the row already shows whole, below the sticky rows, as VS
	 * Code's Explorer reveals the active file only where its list cannot place it. It matters with
	 * `relativeTop`, since scrolling as little as it takes never moves a row in sight.
	 */
	readonly onlyOutOfSight?: boolean;
}

/** Everything needed to bring a tree back after it is recreated, keyed by item identity. */
export interface VirtualTreeViewState<K> {
	/**
	 * Every loaded row that is expanded, including rows inside collapsed parents. A compact row names
	 * all its items while it is expanded and none while it is collapsed, as VS Code's compressed tree
	 * records it.
	 */
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
	/**
	 * Whether items the source calls compressible join their parent's row, true unless set; VS Code's
	 * Explorer turns its compact folders off for screen readers. `setCompressed` changes it later.
	 */
	readonly compressed?: boolean;
}
