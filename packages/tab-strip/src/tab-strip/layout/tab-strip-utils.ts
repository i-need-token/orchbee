import type { Rect } from '../utils/rectRight';

export type TabStripHierarchyNodeKind =
	| 'tab-strip'
	| 'pinned-container'
	| 'unpinned-container'
	| 'group-container'
	| 'view';

export interface TabStripHierarchyNode {
	kind: TabStripHierarchyNodeKind;
	bounds: Readonly<Rect>;
	targetBounds?: Readonly<Rect>;
	parent?: TabStripHierarchyNode | null;
}

const hasAnimatingLayoutManager = (kind: TabStripHierarchyNodeKind) =>
	kind === 'pinned-container' || kind === 'unpinned-container' || kind === 'group-container';

/** Ports GetTabStripViewTargetBounds(). */
export function getTabStripViewTargetBounds(node: Readonly<TabStripHierarchyNode>): Rect {
	if (node.parent && hasAnimatingLayoutManager(node.parent.kind) && node.targetBounds) {
		return { ...node.targetBounds };
	}
	return { ...node.bounds };
}

/** Ports GetTabStripView(), starting from the supplied view's parent. */
export function getTabStripView(
	node: Readonly<TabStripHierarchyNode>,
): TabStripHierarchyNode | null {
	let current = node.parent ?? null;
	while (current) {
		if (current.kind === 'tab-strip') return current;
		current = current.parent ?? null;
	}
	return null;
}
