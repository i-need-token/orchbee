import type { VirtualTreeRow } from './virtual-tree-types';

export const NO_CHILDREN: readonly never[] = [];

/**
 * One loaded item, or the input at the root. The node itself is the public row object, so reading
 * rows allocates nothing; the model is the only writer of its fields.
 */
export class TreeNode<K, T> implements VirtualTreeRow<K, T> {
	readonly key: K;
	element: T;
	/** The node holding this one; null only for the root. A node never moves to another owner. */
	readonly owner: TreeNode<K, T> | null;
	children: readonly TreeNode<K, T>[] = NO_CHILDREN;
	readonly depth: number;
	collapsible: boolean;
	collapsed = true;
	/** False while a find filter hides the node together with its subtree. */
	visible = true;
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

	constructor(key: K, element: T, owner: TreeNode<K, T> | null, collapsible: boolean) {
		this.key = key;
		this.element = element;
		this.owner = owner;
		this.depth = owner === null ? 0 : owner.depth + 1;
		this.collapsible = collapsible;
		this.stale = collapsible;
	}

	get parent(): VirtualTreeRow<K, T> | null {
		const owner = this.owner;
		return owner === null || owner.owner === null ? null : owner;
	}

	/** The rows this node and its visible subtree occupy. */
	get renderNodeCount(): number {
		if (!this.visible) return 0;
		return this.collapsed ? 1 : 1 + this.childrenRenderCount;
	}

	get positionInSet(): number {
		return this.visibleChildIndex + 1;
	}

	get setSize(): number {
		return this.owner === null ? 1 : this.owner.visibleChildrenCount;
	}

	get visibleDescendantCount(): number {
		return this.collapsed || !this.visible ? 0 : this.childrenRenderCount;
	}
}

/** Appends the visible rows below `node`, in display order. */
export function collectRows<K, T>(node: TreeNode<K, T>, rows: TreeNode<K, T>[]): void {
	for (const child of node.children) {
		if (!child.visible) continue;
		rows.push(child);
		if (!child.collapsed && child.childrenRenderCount > 0) collectRows(child, rows);
	}
}
