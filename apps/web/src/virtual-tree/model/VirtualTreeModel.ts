import { ChildrenRequest } from './ChildrenRequest';
import { longestIncreasingSubsequence } from './longestIncreasingSubsequence';
import { replaceRows, RowSpliceBuilder, SpliceList } from './row-splices';
import { chainTail, collectRow, collectRows, NO_CHILDREN, TreeNode } from './tree-node';
import type {
	VirtualTreeEvent,
	VirtualTreeModelOptions,
	VirtualTreeRevealOptions,
	VirtualTreeRow,
	VirtualTreeSource,
	VirtualTreeSplice,
	VirtualTreeViewState,
} from './virtual-tree-types';

/** Splices in one event beyond which a single replacement of every row describes it. */
const EVENT_SPLICE_LIMIT = 64;

/** How long a row's children load before the row shows it, as in VS Code's asynchronous tree. */
const LOADING_INDICATOR_DELAY_MS = 800;

const RESOLVED: Promise<void> = Promise.resolve();
const NO_ITEMS: readonly never[] = [];

type VirtualTreeListener<K> = (event: VirtualTreeEvent<K>) => void;

/** One finished operation's events and the listeners that were subscribed when it finished. */
interface Delivery<K> {
	readonly events: readonly VirtualTreeEvent<K>[];
	readonly listeners: readonly VirtualTreeListener<K>[];
}

interface RecursiveOption {
	readonly recursive?: boolean;
}

/**
 * A headless tree: loaded items, the flat array of visible rows, and focus, selection and anchor.
 * Every operation applies completely before its events go out, and changes the rows with as few
 * splices as it can: one for an expand or collapse, plus one for each row that the children an
 * expansion loads move away from elsewhere, and none for a refresh that changes nothing.
 *
 * A row's children are fetched by one request at a time. Callers share it while its answer is new
 * enough for them: any answer for an expansion, one requested after it was asked for a refresh.
 * Refreshes asked while an older request runs share one more request once it ends. Collapsing the
 * row, its leaving the tree, and `dispose` abort the request, and its late answer is dropped.
 *
 * An expanded item whose only child the source calls compressible shows in the child's row, as VS
 * Code's compact folders do: the row's segments name both, and it expands and collapses with its last
 * item. Selection and anchor hold rows, by their last item's key; focus may rest on any segment.
 */
export class VirtualTreeModel<K, T> {
	private readonly source: VirtualTreeSource<K, T>;
	private readonly expandedByDefault: ((item: T) => boolean) | undefined;
	private compression: boolean;
	private readonly root: TreeNode<K, T>;
	private readonly nodes = new Map<K, TreeNode<K, T>>();
	private rows: TreeNode<K, T>[] = [];
	/** Every row below this index holds its own index, so most lookups read it back directly. */
	private indexedRows = 0;
	private stamp = 0;
	private focusKey: K | null = null;
	private anchorKey: K | null = null;
	private selectionKeys: ReadonlySet<K> = new Set();
	private scrollOffset = 0;
	private readonly listeners = new Set<VirtualTreeListener<K>>();
	private readonly deliveries: Delivery<K>[] = [];
	private delivering = false;
	/** Keys a view-state restore expands as soon as they load. */
	private pendingExpanded: ReadonlySet<K> | null = null;
	/** The children requests that have not ended, one per node at most. */
	private readonly requests = new Map<TreeNode<K, T>, ChildrenRequest<T>>();
	/** How many refreshes have been asked for; a request answers those asked before it started. */
	private refreshCount = 0;
	private disposed = false;

	// The current operation's effects, delivered together when the outermost operation ends.
	private operationDepth = 0;
	private diffDepth = 0;
	private readonly splices = new SpliceList(EVENT_SPLICE_LIMIT);
	private rowCountBefore = 0;
	private rowsSpliced = false;
	private rowsChanged = false;
	private collapseEvents: VirtualTreeEvent<K>[] = [];
	private focusChanged = false;
	private anchorChanged = false;
	private selectionChanged = false;
	private scrollChanged = false;
	private revealRequest: { readonly key: K; readonly relativeTop: number | null } | null = null;
	private removedKeys: K[] = [];
	/** Nodes whose key a refresh gave to a new node elsewhere; they leave once the refresh ends. */
	private displaced: TreeNode<K, T>[] = [];
	/** Nodes that joined their child's row in this operation; their selection moves to that row. */
	private mergedNodes: TreeNode<K, T>[] = [];
	/** Whether this operation set focus itself, which keeps it where it was set. */
	private focusAssigned = false;
	/** Whether focus rested on a segment above its row's own when the last operation ended. */
	private focusMergedAtRest = false;

	constructor(options: VirtualTreeModelOptions<K, T>) {
		this.source = options.source;
		this.expandedByDefault = options.expandedByDefault;
		this.compression = options.compressed ?? true;
		this.root = new TreeNode(options.source.key(options.input), options.input, null, true);
		this.root.collapsed = false;
	}

	get rowCount(): number {
		return this.rows.length;
	}

	get focus(): K | null {
		return this.focusKey;
	}

	get anchor(): K | null {
		return this.anchorKey;
	}

	/** A snapshot in no particular order: each change replaces the set instead of editing it. */
	get selection(): ReadonlySet<K> {
		return this.selectionKeys;
	}

	/**
	 * How far the view has scrolled, in CSS pixels. It lives here, with focus and selection, so a tree
	 * shown again opens where it was left; the view keeps it within its rows.
	 */
	get scrollTop(): number {
		return this.scrollOffset;
	}

	/** Whether items the source calls compressible join their parent's row. */
	get compressed(): boolean {
		return this.compression;
	}

	rowAt(index: number): VirtualTreeRow<K, T> {
		const row = this.rows[index];
		if (row === undefined) throw new RangeError(`No row ${index} among ${this.rows.length} rows`);
		return row;
	}

	/**
	 * The row index of a key, which for an item a compact row joins is that row's, or -1 while it is
	 * not loaded or a collapsed parent hides it.
	 */
	indexOf(key: K): number {
		const node = this.nodes.get(key);
		return node === undefined ? -1 : this.indexOfNode(node);
	}

	/**
	 * Fetches the children of a row, or of the input for null, and merges them by key. A collapsed row
	 * only drops its children, to fetch them when it next expands. Expanded rows below the refreshed
	 * one fetch theirs too; `recursive` also drops the children of collapsed rows below it. A request
	 * that started before the refresh was asked is waited for and then made again, so the promise
	 * settles once every row the refresh reaches holds children fetched after it was asked.
	 */
	refresh(key: K | null, options?: RecursiveOption): Promise<void> {
		const node = key === null ? this.root : this.nodes.get(key);
		if (node === undefined) return RESOLVED;
		const recursive = options?.recursive === true;
		this.refreshCount += 1;
		const since = this.refreshCount;
		return this.settle(() => this.refreshNode(node, recursive, since));
	}

	/**
	 * Expands a row, fetching its children first when it has none loaded, and settles once they have
	 * arrived. `recursive` also expands every loaded row below it; rows whose children arrived
	 * asynchronously stay collapsed. A row that comes to join its only child expands the child too.
	 * An item a compact row joins expands the row.
	 */
	expand(key: K, options?: RecursiveOption): Promise<void> {
		const found = this.nodes.get(key);
		if (found === undefined) return RESOLVED;
		const node = chainTail(found);
		if (!node.collapsible) return RESOLVED;
		const recursive = options?.recursive === true;
		return this.settle(() => this.expandNode(node, recursive));
	}

	/**
	 * Collapses a row, aborting the request for its children; children a refresh was fetching again
	 * go, to load when the row next expands. `recursive` also collapses every row below it. An item a
	 * compact row joins collapses the row, which keeps its segments, as in VS Code.
	 */
	collapse(key: K, options?: RecursiveOption): void {
		const found = this.nodes.get(key);
		if (found === undefined) return;
		const node = chainTail(found);
		const recursive = options?.recursive === true;
		this.operate(() => {
			if (!recursive) {
				this.setCollapsed(node, true, true);
				return;
			}
			this.setCollapsed(node, true, false);
			this.collapseDescendants(node);
			this.collapseEvents.push({ collapsed: true, key: node.key, kind: 'collapse', recursive });
		});
	}

	/** Collapses every row, in one splice. */
	collapseAll(): void {
		this.operate(() => {
			const before = this.rows.length;
			this.collapseDescendants(this.root);
			const rows: TreeNode<K, T>[] = [];
			collectRows(this.root, rows);
			if (rows.length !== before) {
				this.writeRows(0, before, rows, [
					{ deleteCount: before, insertCount: rows.length, start: 0 },
				]);
			}
			this.collapseEvents.push({ collapsed: true, key: null, kind: 'collapse', recursive: true });
		});
	}

	/**
	 * Expands the collapsed rows above a loaded key, so its row shows, and asks the view to scroll it
	 * into sight.
	 */
	reveal(key: K, options?: VirtualTreeRevealOptions): void {
		const node = this.nodes.get(key);
		if (node === undefined) return;
		this.operate(() => {
			// Nearest first: only the outermost expansion joins the rows, in one splice.
			for (let owner = node.owner; owner !== null && owner.owner !== null; owner = owner.owner) {
				this.setCollapsed(owner, false, true);
			}
			this.revealRequest = { key, relativeTop: options?.relativeTop ?? null };
		});
	}

	/** Focuses an item: a row's, or one segment of a compact row. */
	setFocus(key: K | null): void {
		if (key === this.focusKey || (key !== null && !this.nodes.has(key))) return;
		this.operate(() => {
			this.focusKey = key;
			this.focusChanged = true;
			this.focusAssigned = true;
		});
	}

	/** Anchors a row; the key of an item a compact row joins anchors the row. */
	setAnchor(key: K | null): void {
		const node = key === null ? null : this.nodes.get(key);
		if (node === undefined) return;
		const next = node === null ? null : chainTail(node).key;
		if (next === this.anchorKey) return;
		this.operate(() => {
			this.anchorKey = next;
			this.anchorChanged = true;
		});
	}

	/** Scrolls to a whole, non-negative pixel offset, rounding the one given. */
	setScrollTop(offset: number): void {
		if (!Number.isFinite(offset)) throw new RangeError(`Invalid scroll offset ${offset}`);
		const next = Math.max(0, Math.round(offset));
		if (next === this.scrollOffset) return;
		this.operate(() => {
			this.scrollOffset = next;
			this.scrollChanged = true;
		});
	}

	/**
	 * Replaces the selection with the given keys that are loaded; the key of an item a compact row
	 * joins selects the row.
	 */
	setSelection(keys: Iterable<K>): void {
		const next = new Set<K>();
		for (const key of keys) {
			const node = this.nodes.get(key);
			if (node !== undefined) next.add(node.merged ? chainTail(node).key : key);
		}
		this.replaceSelection(next);
	}

	/** Selects every visible row. */
	selectAll(): void {
		const next = new Set<K>();
		for (const row of this.rows) next.add(row.key);
		this.replaceSelection(next);
	}

	/**
	 * Turns compact rows on or off, in one splice. Items keep their expansion, so the items a row
	 * joined show expanded in rows of their own once compact rows are off.
	 */
	setCompressed(compressed: boolean): void {
		if (compressed === this.compression) return;
		this.operate(() => {
			this.compression = compressed;
			const before = this.rows.length;
			if (!this.remerge(this.root)) return;
			const rows: TreeNode<K, T>[] = [];
			collectRows(this.root, rows);
			this.writeRows(0, before, rows, [
				{ deleteCount: before, insertCount: rows.length, start: 0 },
			]);
		});
	}

	viewState(): VirtualTreeViewState<K> {
		const expanded: K[] = [];
		const pending: TreeNode<K, T>[] = [...this.root.children];
		for (let node = pending.pop(); node !== undefined; node = pending.pop()) {
			// A compact row's items share its expansion, so they are named with the row's own item.
			if (!node.merged && node.collapsible && !node.collapsed) {
				expanded.push(node.key);
				for (let owner = node.owner; owner?.merged === true; owner = owner.owner) {
					expanded.push(owner.key);
				}
			}
			for (const child of node.children) pending.push(child);
		}
		return {
			anchor: this.anchorKey,
			expanded,
			focus: this.focusKey,
			scrollTop: this.scrollOffset,
			selection: [...this.selectionKeys],
		};
	}

	/**
	 * Expands the rows a saved state names as they load, loading the input first when it has never
	 * loaded, then restores focus, anchor and selection for the keys that exist once loading settles,
	 * and the scroll offset.
	 */
	restoreViewState(state: VirtualTreeViewState<K>): Promise<void> {
		const expanded = new Set(state.expanded);
		const finish = (): void => {
			if (this.pendingExpanded === expanded) this.pendingExpanded = null;
			this.restoreTraits(state);
		};
		let loading: Promise<void> | null;
		try {
			loading = this.operate(() => {
				this.pendingExpanded = expanded;
				const pending: Promise<void>[] = [];
				if (this.root.stale) collect(pending, this.loadChildren(this.root, false, 0));
				else this.expandSaved(this.root, expanded, pending);
				const settled = settleAll(pending);
				// A synchronous source restores everything in one batch of events.
				if (settled === null) finish();
				return settled;
			});
		} catch (error) {
			finish();
			return Promise.reject(error);
		}
		if (loading === null) return RESOLVED;
		return loading.then(finish, (error: unknown) => {
			finish();
			throw error;
		});
	}

	/**
	 * Hears every operation that finishes while the listener is subscribed, in the order operations
	 * finish. An operation a listener starts is heard by everyone after the current one.
	 */
	subscribe(listener: VirtualTreeListener<K>): () => void {
		this.listeners.add(listener);
		return () => {
			this.listeners.delete(listener);
		};
	}

	/**
	 * Aborts every request and stops delivering events; answers that arrive later change nothing,
	 * and the operations waiting for them settle. Nothing loads after it. The owner of the tree's view
	 * calls it once the tree goes away for good.
	 */
	dispose(): void {
		if (this.disposed) return;
		this.disposed = true;
		this.listeners.clear();
		const requests = [...this.requests.values()];
		this.requests.clear();
		for (const request of requests) request.abort();
	}

	// Loading

	private refreshNode(
		node: TreeNode<K, T>,
		recursive: boolean,
		since: number,
	): Promise<void> | null {
		if (node.owner !== null && node.collapsed) {
			// As in VS Code's asynchronous tree, a collapsed row forgets its children until it expands.
			this.abortRequest(node);
			this.setCollapsible(node, this.source.hasChildren(node.element));
			this.clearChildren(node);
			node.stale = node.collapsible;
			return null;
		}
		return this.refreshSubtree(node, recursive, since);
	}

	private refreshSubtree(
		node: TreeNode<K, T>,
		recursive: boolean,
		since: number,
	): Promise<void> | null {
		if (node.owner !== null) {
			const collapsible = this.source.hasChildren(node.element);
			if (!collapsible) {
				this.abortRequest(node);
				this.setChildren(node, NO_ITEMS, recursive, since, []);
				this.setCollapsible(node, false);
				node.collapsed = true;
				node.stale = false;
				return null;
			}
			this.setCollapsible(node, true);
		}
		return this.loadChildren(node, recursive, since);
	}

	/**
	 * Fetches a node's children and merges them in. A running request answers the caller when it
	 * started at or after refresh `since`; otherwise one more request follows it.
	 */
	private loadChildren(
		node: TreeNode<K, T>,
		recursive: boolean,
		since: number,
	): Promise<void> | null {
		if (this.disposed) return null;
		const running = this.requests.size > 0 ? this.requests.get(node) : undefined;
		if (running !== undefined) return this.joinRequest(node, running, recursive, since);
		const controller = new AbortController();
		const result = this.source.children(node.element, controller.signal);
		if (!('then' in result)) return this.applyChildren(node, result, recursive, true, since);
		const request = new ChildrenRequest<T>({
			answer: (answered, items) => this.answer(node, answered, items),
			controller,
			fail: (failed, error) => {
				this.answerFailed(node, failed, error);
			},
			recursive,
			result,
			startedAt: this.refreshCount,
		});
		this.requests.set(node, request);
		// The input has no row to show it on.
		if (node.owner !== null) {
			request.notifyAfter(LOADING_INDICATOR_DELAY_MS, () => {
				this.showLoading(node, request);
			});
		}
		return request.settled;
	}

	private joinRequest(
		node: TreeNode<K, T>,
		request: ChildrenRequest<T>,
		recursive: boolean,
		since: number,
	): Promise<void> {
		if (request.startedAt >= since) {
			if (recursive) request.recursive = true;
			return request.settled;
		}
		// The answer predates the refresh: one more request runs for every such caller once it ends.
		if (since > request.followUpSince) request.followUpSince = since;
		if (recursive) request.followUpRecursive = true;
		request.followUp ??= request.ended.then(() => this.refreshAgain(node, request));
		return request.followUp;
	}

	private refreshAgain(node: TreeNode<K, T>, request: ChildrenRequest<T>): Promise<void> {
		if (this.disposed || !node.attached) return RESOLVED;
		return this.settle(() =>
			this.refreshNode(node, request.followUpRecursive, request.followUpSince),
		);
	}

	/** Applies the answer of the node's current request; any other answer is no longer wanted. */
	private answer(
		node: TreeNode<K, T>,
		request: ChildrenRequest<T>,
		items: readonly T[],
	): Promise<void> | null {
		if (this.requests.get(node) !== request) return null;
		return this.operate(() => {
			this.endRequest(node, request);
			return this.applyChildren(node, items, request.recursive, false, request.startedAt);
		});
	}

	private answerFailed(node: TreeNode<K, T>, request: ChildrenRequest<T>, error: unknown): void {
		if (this.requests.get(node) !== request) return;
		this.operate(() => {
			this.endRequest(node, request);
			if (node.owner === null) return;
			// As in VS Code, a row whose children fail to load collapses; it asks again on expand.
			this.setCollapsed(node, true, true);
			this.clearChildren(node);
			node.stale = node.collapsible;
		});
		// VS Code's asynchronous tree takes a read its source cancelled as no children, not a failure.
		if (error instanceof Error && error.name === 'AbortError') return;
		throw error;
	}

	private showLoading(node: TreeNode<K, T>, request: ChildrenRequest<T>): void {
		if (this.requests.get(node) !== request) return;
		this.operate(() => {
			node.loading = true;
			this.touch(node);
		});
	}

	private endRequest(node: TreeNode<K, T>, request: ChildrenRequest<T>): void {
		this.requests.delete(node);
		request.close();
		if (node.loading) {
			node.loading = false;
			this.touch(node);
		}
	}

	/** Aborts a node's request; false when it had none. */
	private abortRequest(node: TreeNode<K, T>): boolean {
		if (this.requests.size === 0) return false;
		const request = this.requests.get(node);
		if (request === undefined) return false;
		this.requests.delete(node);
		request.abort();
		if (node.loading) {
			node.loading = false;
			this.touch(node);
		}
		return true;
	}

	/** Aborts the request of a row that collapsed; children a refresh was fetching again go. */
	private abortLoad(node: TreeNode<K, T>): void {
		if (!this.abortRequest(node) || node.stale) return;
		this.clearChildren(node);
		node.stale = true;
	}

	private applyChildren(
		node: TreeNode<K, T>,
		items: readonly T[],
		recursive: boolean,
		synchronous: boolean,
		since: number,
	): Promise<void> | null {
		const pending: Promise<void>[] = [];
		const joined = node.merged ? node.children[0] : undefined;
		const toRefresh = this.setChildren(node, items, recursive, since, pending);
		node.stale = false;
		node.loadedSynchronously = synchronous;
		for (const child of toRefresh) {
			if (!child.attached) continue;
			try {
				collect(pending, this.refreshSubtree(child, recursive, since));
			} catch (error) {
				pending.push(Promise.reject(error));
			}
		}
		// A row that comes to join its only child expands the child, as VS Code's compressed
		// asynchronous tree does, so the row shows what is inside it; a child it joined already keeps
		// the expansion it was given.
		const only = node.merged ? node.children[0] : undefined;
		if (only !== undefined && only !== joined && only.collapsible && only.collapsed) {
			const stale = only.stale;
			try {
				collect(pending, this.expandNode(only, false));
				// Children kept from an earlier expansion are fetched again, as the refresh does for every
				// expanded row it reaches.
				if (!stale) collect(pending, this.refreshSubtree(only, recursive, since));
			} catch (error) {
				pending.push(Promise.reject(error));
			}
		}
		return settleAll(pending);
	}

	private expandNode(node: TreeNode<K, T>, recursive: boolean): Promise<void> | null {
		const pending: Promise<void>[] = [];
		// Loading first lets a synchronous source show the children in the same splice.
		if (node.stale) collect(pending, this.loadChildren(node, false, 0));
		if (recursive) {
			if (!node.collapsed) this.setCollapsed(node, true, false);
			this.expandDescendants(node, pending);
			this.setCollapsed(node, false, false);
			this.collapseEvents.push({ collapsed: false, key: node.key, kind: 'collapse', recursive });
		} else {
			this.setCollapsed(node, false, true);
		}
		// A row that came to join its only child shows what is inside the child too.
		const only = node.merged ? node.children[0] : undefined;
		if (only?.collapsible === true && only.collapsed)
			collect(pending, this.expandNode(only, false));
		return settleAll(pending);
	}

	/**
	 * Expands every loaded row below a collapsed one without touching the rows, and loads the stale
	 * ones whose parent a synchronous source filled: a synchronous source then expands whole, like VS
	 * Code's object tree, and an asynchronous one leaves stale rows collapsed, like its async data
	 * tree. Recounts each subtree on the way back up.
	 */
	private expandDescendants(node: TreeNode<K, T>, pending: Promise<void>[]): void {
		for (const child of node.children) {
			// A load below can displace a sibling; a node that has left the tree stays as it is.
			if (!child.attached || !child.collapsible) continue;
			if (child.stale && node.loadedSynchronously) {
				try {
					collect(pending, this.loadChildren(child, false, 0));
				} catch (error) {
					pending.push(Promise.reject(error));
				}
			}
			if (child.stale) continue;
			if (child.collapsed) {
				child.collapsed = false;
				this.touch(child);
			}
			this.expandDescendants(child, pending);
			// The caller writes the rows once the subtree is expanded, so joining a row marks it alone.
			if (this.shouldMerge(child) !== child.merged) this.setMerged(child, !child.merged);
		}
		node.childrenRenderCount = renderCountOf(node.children);
	}

	/**
	 * Collapses every row below a collapsed one without touching the rows; recounts on the way up. An
	 * item a compact row joins stays expanded, since the row collapses with its last item.
	 */
	private collapseDescendants(node: TreeNode<K, T>): void {
		for (const child of node.children) {
			if (child.collapsible && !child.collapsed && !child.merged) {
				child.collapsed = true;
				this.touch(child);
				this.abortLoad(child);
			}
			if (child.children.length > 0) this.collapseDescendants(child);
		}
		node.childrenRenderCount = renderCountOf(node.children);
	}

	private expandSaved(
		node: TreeNode<K, T>,
		expanded: ReadonlySet<K>,
		pending: Promise<void>[],
	): void {
		for (const child of node.children) {
			if (!child.attached || !child.collapsible || !expanded.has(child.key)) continue;
			if (child.collapsed) collect(pending, this.expandNode(child, false));
			this.expandSaved(child, expanded, pending);
		}
	}

	// Structure

	/**
	 * Merges new children into a row by key and returns the reused expanded children whose own
	 * children the refresh fetches next. The rows change once for the whole range.
	 */
	private setChildren(
		parent: TreeNode<K, T>,
		items: readonly T[],
		recursive: boolean,
		since: number,
		pending: Promise<void>[],
	): readonly TreeNode<K, T>[] {
		if (parent.children.length === 0 && items.length === 0) return NO_CHILDREN;
		this.diffDepth += 1;
		try {
			return this.mergeChildren(parent, items, recursive, since, pending);
		} finally {
			this.diffDepth -= 1;
			if (this.diffDepth === 0 && this.displaced.length > 0) this.removeDisplaced();
		}
	}

	private mergeChildren(
		parent: TreeNode<K, T>,
		items: readonly T[],
		recursive: boolean,
		since: number,
		pending: Promise<void>[],
	): readonly TreeNode<K, T>[] {
		const source = this.source;
		const compressible = source.compressible;
		const nodes = this.nodes;
		this.stamp += 1;
		const stamp = this.stamp;
		const oldChildren = parent.children;
		const children: TreeNode<K, T>[] = [];
		const toRefresh: TreeNode<K, T>[] = [];
		// New rows that start expanded, reused rows that change kind, collapsed reused rows a
		// recursive refresh empties, and reused rows whose compressibility changes.
		let expandedNew: TreeNode<K, T>[] | null = null;
		let flipped: TreeNode<K, T>[] | null = null;
		let cleared: TreeNode<K, T>[] | null = null;
		let recompressed: TreeNode<K, T>[] | null = null;
		let ordered = true;
		let lastIndex = -1;
		let reused = 0;
		const displacedBefore = this.displaced.length;

		// Match by key. Most refreshes keep the old order, so the old child after the furthest match is
		// tried before the key index; only new nodes merge their children in the middle of this merge, so
		// every old child is still the node its key names. Reused nodes only take their new element
		// here, so a throwing callback leaves the tree as it was once the new nodes are unregistered.
		try {
			for (const item of items) {
				const key = source.key(item);
				const next = oldChildren[lastIndex + 1];
				const existing = next !== undefined && next.key === key ? next : nodes.get(key);
				if (existing !== undefined) {
					// A key repeated in one list keeps its first row.
					if (existing.claim === stamp) continue;
					if (existing.owner === parent) {
						existing.claim = stamp;
						reused += 1;
						if (existing.childIndex < lastIndex) ordered = false;
						else lastIndex = existing.childIndex;
						const collapsible = source.hasChildren(item);
						if (compressible !== undefined && compressible(item) !== existing.compressible) {
							(recompressed ??= []).push(existing);
						}
						if (existing.element !== item) {
							existing.element = item;
							this.touch(existing);
						}
						if (collapsible !== existing.collapsible) {
							(flipped ??= []).push(existing);
						} else if (collapsible) {
							if (!existing.collapsed) toRefresh.push(existing);
							else if (recursive && !existing.stale) (cleared ??= []).push(existing);
						}
						children.push(existing);
						continue;
					}
					// A row cannot hold its own ancestor.
					if (isAncestorOrSelf(existing, parent)) continue;
					this.displaced.push(existing);
				}
				const collapsible = source.hasChildren(item);
				const node = new TreeNode(key, item, parent, collapsible);
				node.claim = stamp;
				if (compressible !== undefined) node.compressible = compressible(item);
				if (collapsible && this.startsExpanded(key, item)) {
					node.collapsed = false;
					(expandedNew ??= []).push(node);
				}
				nodes.set(key, node);
				children.push(node);
			}
		} catch (error) {
			// New nodes are the ones no merge has placed yet.
			for (const node of children) {
				if (node.childIndex < 0 && nodes.get(node.key) === node) nodes.delete(node.key);
			}
			for (const node of this.displaced.splice(displacedBefore)) nodes.set(node.key, node);
			throw error;
		}

		if (recompressed !== null) {
			for (const node of recompressed) node.compressible = !node.compressible;
		}
		// A parent that stops joining its only child takes its own row back before the children's rows
		// change, so they change below it.
		if (parent.merged && !this.canMerge(parent, children)) this.unmerge(parent);

		// Reused rows that changed kind or went stale drop their children; the builder needs the rows
		// they held.
		let resized: Map<TreeNode<K, T>, number> | null = null;
		if (flipped !== null) {
			for (const node of flipped) {
				node.collapsible = !node.collapsible;
				this.touch(node);
			}
			cleared = cleared === null ? flipped : [...flipped, ...cleared];
		}
		if (cleared !== null) {
			for (const node of cleared) {
				// A row that joined the child it drops takes its own row back first.
				if (node.merged) this.unmerge(node);
				const before = node.renderNodeCount;
				// Its request, if any, answers for what the node was before.
				this.abortRequest(node);
				this.clearChildren(node);
				node.collapsed = true;
				node.stale = node.collapsible;
				if (node.renderNodeCount !== before) (resized ??= new Map()).set(node, before);
			}
		}

		// New rows that start expanded load now, so a synchronous source fills them before the rows
		// are written: a new subtree joins the rows in the parent's single splice.
		if (expandedNew !== null) {
			for (const node of expandedNew) {
				try {
					collect(pending, this.loadChildren(node, recursive, since));
				} catch (error) {
					node.collapsed = true;
					pending.push(Promise.reject(error));
				}
			}
		}

		// Reused children outside one longest run of increasing old positions have moved.
		let moved: Set<TreeNode<K, T>> | null = null;
		if (!ordered) {
			const kept = children.filter((child) => child.childIndex >= 0);
			moved = new Set(kept);
			for (const position of longestIncreasingSubsequence(kept.map((child) => child.childIndex))) {
				const node = kept[position];
				if (node !== undefined) moved.delete(node);
			}
		}

		if (parent.owner === null || (!parent.collapsed && this.isRevealed(parent))) {
			this.spliceChildRows(parent, oldChildren, children, moved, resized);
		}

		let total = 0;
		let index = 0;
		let visibleIndex = 0;
		for (const child of children) {
			child.childIndex = index;
			index += 1;
			if (child.visible) {
				child.visibleChildIndex = visibleIndex;
				visibleIndex += 1;
			}
			total += child.renderNodeCount;
		}
		if (reused < oldChildren.length) {
			for (const old of oldChildren) {
				if (old.claim !== stamp) this.discard(old);
			}
		}
		parent.children = children;
		parent.visibleChildrenCount = visibleIndex;
		const delta = total - parent.childrenRenderCount;
		parent.childrenRenderCount = total;
		// A new parent is not counted above yet; its own parent counts it whole when it commits.
		if (delta !== 0 && parent.childIndex >= 0 && parent.visible && !parent.collapsed) {
			this.propagate(parent, delta);
		}
		this.updateMerge(parent);
		return toRefresh;
	}

	/** Writes the rows of a refreshed, visible child range with the fewest splices it can name. */
	private spliceChildRows(
		parent: TreeNode<K, T>,
		oldChildren: readonly TreeNode<K, T>[],
		children: readonly TreeNode<K, T>[],
		moved: ReadonlySet<TreeNode<K, T>> | null,
		resized: ReadonlyMap<TreeNode<K, T>, number> | null,
	): void {
		// A merged parent's children start at the row it shares with them.
		let base = 0;
		if (parent.owner !== null) base = this.indexOfNode(parent) + (parent.merged ? 0 : 1);
		const oldLength = parent.childrenRenderCount;
		const builder = new RowSpliceBuilder(base);
		let oldPosition = 0;
		let newLength = 0;
		for (const child of children) {
			const count = child.renderNodeCount;
			newLength += count;
			if (child.childIndex < 0 || moved?.has(child) === true) {
				builder.insert(count);
				continue;
			}
			for (let old = oldChildren[oldPosition]; old !== undefined; old = oldChildren[oldPosition]) {
				oldPosition += 1;
				if (old === child) break;
				builder.remove(resized?.get(old) ?? old.renderNodeCount);
			}
			const before = resized?.get(child) ?? count;
			if (before === count) {
				builder.keep(count);
			} else {
				// The row stays; only the rows below it change.
				builder.keep(1);
				builder.remove(before - 1);
				builder.insert(count - 1);
			}
		}
		for (let old = oldChildren[oldPosition]; old !== undefined; old = oldChildren[oldPosition]) {
			oldPosition += 1;
			builder.remove(resized?.get(old) ?? old.renderNodeCount);
		}
		const splices = builder.finish(oldLength, newLength);
		if (splices.length === 0) return;
		const range: TreeNode<K, T>[] = [];
		for (const child of children) collectRow(child, range);
		this.writeRows(base, oldLength, range, splices);
	}

	/**
	 * Expands or collapses one row and splices the rows below it; false when nothing changed. A merged
	 * node takes its own row back to collapse, and a node that expands joins its only child's row when
	 * it may.
	 */
	private setCollapsed(node: TreeNode<K, T>, collapsed: boolean, notify: boolean): boolean {
		if (
			node.owner === null ||
			!node.attached ||
			!node.collapsible ||
			node.collapsed === collapsed
		) {
			return false;
		}
		if (node.merged) this.unmerge(node);
		const revealed = this.isRevealed(node);
		const start = revealed ? this.indexOfNode(node) + 1 : -1;
		node.collapsed = collapsed;
		this.touch(node);
		const childRows = node.childrenRenderCount;
		if (node.visible && childRows > 0) this.propagate(node, collapsed ? -childRows : childRows);
		if (revealed && childRows > 0) {
			if (collapsed) {
				this.writeRows(start, childRows, [], [{ deleteCount: childRows, insertCount: 0, start }]);
			} else {
				const inserted: TreeNode<K, T>[] = [];
				collectRows(node, inserted);
				this.writeRows(start, 0, inserted, [
					{ deleteCount: 0, insertCount: inserted.length, start },
				]);
			}
		}
		if (collapsed) this.abortLoad(node);
		else this.updateMerge(node);
		if (notify)
			this.collapseEvents.push({ collapsed, key: node.key, kind: 'collapse', recursive: false });
		return true;
	}

	// Compact rows

	/** Whether a node would join the row of its only child among `children`. */
	private canMerge(node: TreeNode<K, T>, children: readonly TreeNode<K, T>[]): boolean {
		return (
			this.compression &&
			node.owner !== null &&
			node.attached &&
			node.collapsible &&
			!node.collapsed &&
			children.length === 1 &&
			children[0]?.compressible === true
		);
	}

	private shouldMerge(node: TreeNode<K, T>): boolean {
		return this.canMerge(node, node.children);
	}

	/** Joins or leaves its only child's row as the node now should. */
	private updateMerge(node: TreeNode<K, T>): void {
		const merge = this.shouldMerge(node);
		if (merge === node.merged) return;
		if (merge) this.merge(node);
		else this.unmerge(node);
	}

	/** Joins a node's row with its only child's: its own row goes, and the child's shows both. */
	private merge(node: TreeNode<K, T>): void {
		const revealed = this.isRevealed(node);
		const start = revealed ? this.indexOfNode(node) : -1;
		this.setMerged(node, true);
		if (node.childIndex >= 0 && node.visible) this.propagate(node, -1);
		if (revealed) this.writeRows(start, 1, [], [{ deleteCount: 1, insertCount: 0, start }]);
	}

	/** Gives a merged node its own row back, right above the row it shared. */
	private unmerge(node: TreeNode<K, T>): void {
		const revealed = this.isRevealed(node);
		const start = revealed ? this.indexOfNode(node) : -1;
		this.setMerged(node, false);
		if (node.childIndex >= 0 && node.visible) this.propagate(node, 1);
		if (revealed) this.writeRows(start, 0, [node], [{ deleteCount: 0, insertCount: 1, start }]);
	}

	/** Sets the flag alone, marking the rows whose segments change; the caller owns rows and counts. */
	private setMerged(node: TreeNode<K, T>, merged: boolean): void {
		const tail = chainTail(node);
		node.merged = merged;
		if (merged) {
			this.touch(chainTail(node));
			this.mergedNodes.push(node);
		} else {
			this.touch(node);
			this.touch(tail);
		}
	}

	/** Brings every merged flag below a node in line with the setting and recounts; whether any changed. */
	private remerge(node: TreeNode<K, T>): boolean {
		let changed = false;
		for (const child of node.children) {
			if (child.children.length > 0 && this.remerge(child)) changed = true;
			if (this.shouldMerge(child) !== child.merged) {
				this.setMerged(child, !child.merged);
				changed = true;
			}
		}
		if (changed) node.childrenRenderCount = renderCountOf(node.children);
		return changed;
	}

	private setCollapsible(node: TreeNode<K, T>, collapsible: boolean): void {
		if (node.collapsible === collapsible) return;
		node.collapsible = collapsible;
		this.touch(node);
	}

	/** Drops a row's children; the caller owns the rows and counts above it. */
	private clearChildren(node: TreeNode<K, T>): void {
		if (node.children.length === 0) return;
		for (const child of node.children) this.discard(child);
		node.children = NO_CHILDREN;
		node.childrenRenderCount = 0;
		node.visibleChildrenCount = 0;
	}

	/** Removes an attached node from its owner, with its rows and counts. */
	private removeNode(node: TreeNode<K, T>): void {
		const owner = node.owner;
		if (owner === null) return;
		// An owner that showed in the node's row takes its own row back first.
		if (owner.merged) this.unmerge(owner);
		const count = node.renderNodeCount;
		if (count > 0 && this.isRevealed(node)) {
			const start = this.indexOfNode(node);
			this.writeRows(start, count, [], [{ deleteCount: count, insertCount: 0, start }]);
		}
		const remaining = owner.children.filter((sibling) => sibling !== node);
		let index = 0;
		let visibleIndex = 0;
		for (const sibling of remaining) {
			sibling.childIndex = index;
			index += 1;
			if (sibling.visible) {
				sibling.visibleChildIndex = visibleIndex;
				visibleIndex += 1;
			}
		}
		owner.children = remaining;
		owner.visibleChildrenCount = visibleIndex;
		owner.childrenRenderCount -= count;
		if (count > 0 && owner.childIndex >= 0 && owner.visible && !owner.collapsed) {
			this.propagate(owner, -count);
		}
		this.discard(node);
		this.rowsChanged = true;
		this.updateMerge(owner);
	}

	private removeDisplaced(): void {
		const displaced = this.displaced;
		this.displaced = [];
		for (const node of displaced) {
			if (node.attached) this.removeNode(node);
		}
	}

	/** Marks a subtree detached and forgets its keys, unless a newer node already owns one. */
	private discard(node: TreeNode<K, T>): void {
		node.attached = false;
		this.abortRequest(node);
		if (this.nodes.get(node.key) === node) {
			this.nodes.delete(node.key);
			this.removedKeys.push(node.key);
		}
		for (const child of node.children) this.discard(child);
	}

	/**
	 * Adds a change in a committed node's render count to every ancestor whose own count it changes:
	 * up to the root, or to the first ancestor that is collapsed, hidden, or not committed itself.
	 */
	private propagate(node: TreeNode<K, T>, delta: number): void {
		for (let owner = node.owner; owner !== null; owner = owner.owner) {
			owner.childrenRenderCount += delta;
			if (owner.childIndex < 0 || owner.collapsed || !owner.visible) return;
		}
	}

	private startsExpanded(key: K, item: T): boolean {
		return this.pendingExpanded?.has(key) === true || this.expandedByDefault?.(item) === true;
	}

	// Rows

	/** Whether the node's row shows: every node above it is committed, expanded and visible. */
	private isRevealed(node: TreeNode<K, T>): boolean {
		if (node.owner === null || !node.attached || !node.visible || node.childIndex < 0) return false;
		for (let owner = node.owner; owner.owner !== null; owner = owner.owner) {
			if (owner.collapsed || !owner.visible || owner.childIndex < 0) return false;
		}
		return true;
	}

	/** The index of the row a node shows in: its own, or for a merged node its chain's. */
	private indexOfNode(target: TreeNode<K, T>): number {
		const node = target.merged ? chainTail(target) : target;
		const rows = this.rows;
		const cached = node.rowIndex;
		if (cached >= 0 && rows[cached] === node) return cached;
		if (!this.isRevealed(node)) return -1;
		// Rows below `indexedRows` hold their index, so a missed node lies beyond it.
		for (let index = this.indexedRows; index < rows.length; index += 1) {
			const row = rows[index];
			if (row === undefined) break;
			row.rowIndex = index;
			if (row === node) {
				this.indexedRows = index + 1;
				return index;
			}
		}
		this.indexedRows = rows.length;
		throw new Error('A revealed tree node is missing from the rows');
	}

	/** Replaces rows with `insert`, which the model hands over, and records the splices it reports. */
	private writeRows(
		start: number,
		deleteCount: number,
		insert: TreeNode<K, T>[],
		splices: readonly VirtualTreeSplice[],
	): void {
		if (!this.rowsSpliced) {
			this.rowsSpliced = true;
			this.rowCountBefore = this.rows.length;
		}
		this.rows = replaceRows(this.rows, start, deleteCount, insert);
		if (start < this.indexedRows) this.indexedRows = start;
		for (const splice of splices)
			this.splices.add(splice.start, splice.deleteCount, splice.insertCount);
		this.rowsChanged = true;
	}

	private touch(node: TreeNode<K, T>): void {
		node.revision += 1;
		// A merged node draws in its chain's row, which is its tail's.
		if (node.merged) chainTail(node).revision += 1;
		this.rowsChanged = true;
	}

	// Traits and events

	private restoreTraits(state: VirtualTreeViewState<K>): void {
		this.operate(() => {
			this.setSelection(state.selection);
			this.setFocus(state.focus);
			this.setAnchor(state.anchor);
			this.setScrollTop(state.scrollTop);
		});
	}

	private replaceSelection(next: ReadonlySet<K>): void {
		if (sameKeys(next, this.selectionKeys)) return;
		this.operate(() => {
			this.selectionKeys = next;
			this.selectionChanged = true;
		});
	}

	/** Runs a loading operation, turning a synchronous throw into a rejection. */
	private settle(operation: () => Promise<void> | null): Promise<void> {
		try {
			return this.operate(operation) ?? RESOLVED;
		} catch (error) {
			return Promise.reject(error);
		}
	}

	private operate<R>(operation: () => R): R {
		this.operationDepth += 1;
		try {
			return operation();
		} finally {
			this.operationDepth -= 1;
			if (this.operationDepth === 0) this.flush();
		}
	}

	private flush(): void {
		this.retargetMergedTraits();
		this.forgetRemovedTraits();
		const events: VirtualTreeEvent<K>[] = [];
		if (this.rowsChanged) {
			const splices = this.splices.take(0, this.rowCountBefore, this.rows.length);
			events.push({ kind: 'rows', splices });
			this.rowsSpliced = false;
			this.rowsChanged = false;
		}
		if (this.collapseEvents.length > 0) {
			events.push(...this.collapseEvents);
			this.collapseEvents = [];
		}
		if (this.focusChanged) events.push({ kind: 'focus' });
		if (this.selectionChanged) events.push({ kind: 'selection' });
		if (this.anchorChanged) events.push({ kind: 'anchor' });
		if (this.scrollChanged) events.push({ kind: 'scroll' });
		// A reveal follows every change of its operation, so a view scrolls over the rows as they are.
		const reveal = this.revealRequest;
		if (reveal !== null) events.push({ kind: 'reveal', ...reveal });
		this.focusChanged = false;
		this.selectionChanged = false;
		this.anchorChanged = false;
		this.scrollChanged = false;
		this.revealRequest = null;
		this.focusAssigned = false;
		const focus = this.focusKey === null ? undefined : this.nodes.get(this.focusKey);
		this.focusMergedAtRest = focus?.merged === true;
		if (events.length === 0 || this.listeners.size === 0) return;
		this.deliveries.push({ events, listeners: [...this.listeners] });
		// Events of an operation a listener starts wait behind the ones being delivered, so a listener
		// that replays splices sees them in order.
		if (this.delivering) return;
		this.delivering = true;
		try {
			for (let next = this.deliveries.shift(); next !== undefined; next = this.deliveries.shift()) {
				this.deliver(next);
			}
		} finally {
			this.delivering = false;
		}
	}

	private deliver(delivery: Delivery<K>): void {
		for (const event of delivery.events) {
			for (const listener of delivery.listeners) {
				if (!this.listeners.has(listener)) continue;
				try {
					listener(event);
				} catch (error) {
					// One failing listener must not starve the others or leave the model mid-update.
					queueMicrotask(() => {
						throw error;
					});
				}
			}
		}
	}

	/**
	 * Moves the traits of items that joined their child's row in this operation to that row, as VS
	 * Code's compressed tree keeps them on rows: the selection and the anchor, and focus when it rested
	 * on the item's own row and the operation did not set it.
	 */
	private retargetMergedTraits(): void {
		const merged = this.mergedNodes;
		if (merged.length === 0) return;
		this.mergedNodes = [];
		const selection = this.selectionKeys;
		let next: Set<K> | null = null;
		for (const node of merged) {
			if (!node.attached || !node.merged || !selection.has(node.key)) continue;
			next ??= new Set(selection);
			next.delete(node.key);
			next.add(chainTail(node).key);
		}
		if (next !== null) {
			this.selectionKeys = next;
			this.selectionChanged = true;
		}
		const anchor = this.anchorKey === null ? undefined : this.nodes.get(this.anchorKey);
		if (anchor?.merged === true) {
			this.anchorKey = chainTail(anchor).key;
			this.anchorChanged = true;
		}
		const focus = this.focusKey === null ? undefined : this.nodes.get(this.focusKey);
		if (focus?.merged === true && !this.focusAssigned && !this.focusMergedAtRest) {
			this.focusKey = chainTail(focus).key;
			this.focusChanged = true;
		}
	}

	/** Drops focus, anchor and selection from keys that left the tree in this operation. */
	private forgetRemovedTraits(): void {
		const removed = this.removedKeys;
		if (removed.length === 0) return;
		this.removedKeys = [];
		const nodes = this.nodes;
		if (this.focusKey !== null && !nodes.has(this.focusKey)) {
			this.focusKey = null;
			this.focusChanged = true;
		}
		if (this.anchorKey !== null && !nodes.has(this.anchorKey)) {
			this.anchorKey = null;
			this.anchorChanged = true;
		}
		const selection = this.selectionKeys;
		if (selection.size === 0) return;
		const candidates: Iterable<K> = selection.size < removed.length ? selection : removed;
		let next: Set<K> | null = null;
		for (const key of candidates) {
			if (selection.has(key) && !nodes.has(key)) (next ??= new Set(selection)).delete(key);
		}
		if (next !== null) {
			this.selectionKeys = next;
			this.selectionChanged = true;
		}
	}
}

function renderCountOf<K, T>(nodes: readonly TreeNode<K, T>[]): number {
	let total = 0;
	for (const node of nodes) total += node.renderNodeCount;
	return total;
}

function isAncestorOrSelf<K, T>(node: TreeNode<K, T>, target: TreeNode<K, T>): boolean {
	for (let current: TreeNode<K, T> | null = target; current !== null; current = current.owner) {
		if (current === node) return true;
	}
	return false;
}

function sameKeys<K>(left: ReadonlySet<K>, right: ReadonlySet<K>): boolean {
	if (left.size !== right.size) return false;
	for (const key of left) {
		if (!right.has(key)) return false;
	}
	return true;
}

function collect(pending: Promise<void>[], promise: Promise<void> | null): void {
	if (promise !== null) pending.push(promise);
}

/** Waits for every load and then fails with the first failure, so one error never strands the rest. */
function settleAll(pending: readonly Promise<void>[]): Promise<void> | null {
	if (pending.length === 0) return null;
	return Promise.allSettled(pending).then((results) => {
		for (const result of results) {
			if (result.status === 'rejected') throw result.reason;
		}
	});
}
