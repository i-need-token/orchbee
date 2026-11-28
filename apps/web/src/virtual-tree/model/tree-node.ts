import type { VirtualTreeRow, VirtualTreeSegment } from './virtual-tree-types';

export const NO_CHILDREN: readonly never[] = [];

/**
 * One loaded item, or the input at the root. The node itself is the public row object, so reading
 * rows allocates nothing; the model is the only writer of its fields.
 *
 * A merged node shows in its only child's row, as VS Code's compressed tree joins a folder with its
 * only subfolder: a chain of merged nodes and the node below the last of them make one row, which is
 * that last node's, the chain's tail.
 */
export class TreeNode<K, T> implements VirtualTreeRow<K, T> {
	readonly key: K;
	element: T;
	/** The node holding this one; null only for the root. A node never moves to another owner. */
	readonly owner: TreeNode<K, T> | null;
	children: readonly TreeNode<K, T>[] = NO_CHILDREN;
	readonly level: number;
	collapsible: boolean;
	collapsed = true;
	/** False while a find filter hides the node together with its subtree. */
	visible = true;
	/** Whether the source lets the item join its parent's row while it is the parent's only child. */
	compressible = false;
	/** Whether the node shows in its only child's row; only an expanded node can. */
	merged = false;
	/** The sum of the children's render counts, kept while collapsed so expanding needs no count. */
	childrenRenderCount = 0;
	/**
	 * Position in the owner's children; -1 for the root and for a new node until the owner's refresh
	 * commits it. Until then its rows and counts belong to no ancestor.
	 */
	childIndex = -1;
	visibleChildIndex = 0;
	visibleChildrenCount = 0;
	/** The row this node last occupied; trusted only while that row still holds this node. */
	rowIndex = -1;
	/** Bumped whenever the row changes in place, including a change of the chain it ends. */
	revision = 0;
	/** The refresh that last matched or created this node, so a repeated key is noticed. */
	claim = 0;
	/** The children must be fetched before they can show: never loaded, or dropped by a refresh. */
	stale: boolean;
	/** The children last arrived synchronously, so expanding recursively may load the stale ones. */
	loadedSynchronously = false;
	/** The request for the children has run for the loading indicator's delay. */
	loading = false;
	/** False once the node has left the tree. */
	attached = true;
	/** Whether the find the rows show matched the node's label. */
	findMatched = false;
	/** The find pass that last matched the node's label, which left its verdict in `findPassMatched`. */
	findPass = 0;
	findPassMatched = false;
	/** Scratch stamp of the latest row rebuild that kept the node's row. */
	mark = 0;
	/** The segments as last read, valid while the revision they were read at holds. */
	private segmentList: readonly TreeNode<K, T>[] = NO_CHILDREN;
	private segmentRevision = -1;

	constructor(key: K, element: T, owner: TreeNode<K, T> | null, collapsible: boolean) {
		this.key = key;
		this.element = element;
		this.owner = owner;
		this.level = owner === null ? 0 : owner.level + 1;
		this.collapsible = collapsible;
		this.stale = collapsible;
	}

	get parent(): VirtualTreeRow<K, T> | null {
		const owner = chainHead(this).owner;
		return owner === null || owner.owner === null ? null : owner;
	}

	get depth(): number {
		let depth = 0;
		for (let owner = this.owner; owner !== null; owner = owner.owner) {
			if (!owner.merged) depth += 1;
		}
		return depth;
	}

	/** The rows this node and its visible subtree occupy; a merged node has none of its own. */
	get renderNodeCount(): number {
		if (!this.visible) return 0;
		const own = this.merged ? 0 : 1;
		return this.collapsed ? own : own + this.childrenRenderCount;
	}

	get positionInSet(): number {
		return chainHead(this).visibleChildIndex + 1;
	}

	get setSize(): number {
		const owner = chainHead(this).owner;
		return owner === null ? 1 : owner.visibleChildrenCount;
	}

	get visibleDescendantCount(): number {
		return this.collapsed || !this.visible ? 0 : this.childrenRenderCount;
	}

	get segments(): readonly VirtualTreeSegment<K, T>[] {
		if (this.segmentRevision !== this.revision) {
			this.segmentList = chainOf(this);
			this.segmentRevision = this.revision;
		}
		return this.segmentList;
	}
}

/** The first node of the row a node shows in: the outermost merged node right above it, or itself. */
function chainHead<K, T>(node: TreeNode<K, T>): TreeNode<K, T> {
	let head = node;
	for (let owner = head.owner; owner?.merged === true; owner = head.owner) head = owner;
	return head;
}

/** The node whose row a node shows in: itself, or the tail of the chain it is merged into. */
export function chainTail<K, T>(node: TreeNode<K, T>): TreeNode<K, T> {
	let tail = node;
	while (tail.merged) {
		const only = tail.children[0];
		if (only === undefined) break;
		tail = only;
	}
	return tail;
}

/** The nodes a row shows, from its chain's head down to the row's own node. */
function chainOf<K, T>(node: TreeNode<K, T>): readonly TreeNode<K, T>[] {
	const chain = [node];
	for (let owner = node.owner; owner?.merged === true; owner = owner.owner) chain.push(owner);
	return chain.reverse();
}

/** Appends a node's own row, unless it is merged, and the visible rows below it, in display order. */
export function collectRow<K, T>(node: TreeNode<K, T>, rows: TreeNode<K, T>[]): void {
	if (!node.visible) return;
	if (!node.merged) rows.push(node);
	if (!node.collapsed && node.childrenRenderCount > 0) collectRows(node, rows);
}

/** Appends the visible rows below `node`, in display order. */
export function collectRows<K, T>(node: TreeNode<K, T>, rows: TreeNode<K, T>[]): void {
	for (const child of node.children) collectRow(child, rows);
}
