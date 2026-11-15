// The Panels of one Tab as a tree of splits, and what the visitor can do with them. The model knows
// nothing about the window that draws it: an operation that depends on what the visitor sees takes
// the measurements it needs (the focused Panel's size, every Panel's box) from its caller.

/** A Panel, by the number it was created with. */
export interface PanelLeaf {
	readonly id: number;
}

/** Two parts side by side (`row`) or one above the other (`column`); `ratio` is the first part's share. */
export interface PanelSplit {
	readonly key: number;
	readonly axis: 'row' | 'column';
	readonly ratio: number;
	readonly first: PanelNode;
	readonly second: PanelNode;
}

export type PanelNode = PanelLeaf | PanelSplit;

export interface PanelLayout {
	readonly tree: PanelNode;
	readonly focus: number;
	/** The focused Panel fills the window; the others keep their places. */
	readonly zoomed: boolean;
	/** The last number given to a Panel or a split. */
	readonly serial: number;
}

export type PanelDirection = 'right' | 'down' | 'left' | 'up';

/** The actions a client offers as buttons and keys. */
export type PanelAction = PanelDirection | 'zoom' | 'equal' | 'close';

/** Why an operation was refused, as the notice names it. */
export type PanelRefusal = 'full' | 'small' | 'last' | 'one' | 'none';

/** What an operation did: the notice it leaves, and what of the drawing changed. */
export type PanelOutcome =
	| {
			readonly kind: 'layout';
			readonly layout: PanelLayout;
			readonly done: PanelAction;
			readonly fresh?: number;
	  }
	| { readonly kind: 'ratio'; readonly layout: PanelLayout; readonly split: number }
	| { readonly kind: 'focus'; readonly layout: PanelLayout }
	| { readonly kind: 'refused'; readonly reason: PanelRefusal }
	| { readonly kind: 'unchanged' };

export interface PanelSize {
	readonly width: number;
	readonly height: number;
}

export interface PanelBox {
	readonly id: number;
	readonly left: number;
	readonly top: number;
	readonly right: number;
	readonly bottom: number;
}

/** The most Panels the window shows. */
const MOST_PANELS = 6;
/** The room, in the window's own pixels, a Panel needs before it splits across or down. */
const SPLIT_ROOM: PanelSize = { width: 230, height: 150 };
/** How far a key moves a divider, and how far keys and drags may move it. */
const RESIZE_STEP = 0.05;
const KEY_RATIO = { least: 0.2, most: 0.8 } as const;
const DRAG_RATIO = { least: 0.15, most: 0.85 } as const;

export function isSplit(node: PanelNode): node is PanelSplit {
	return 'axis' in node;
}

/** The Panels in reading order. */
function panelLeaves(node: PanelNode): PanelLeaf[] {
	return isSplit(node) ? [...panelLeaves(node.first), ...panelLeaves(node.second)] : [node];
}

function parentOf(target: PanelNode, node: PanelNode): PanelSplit | undefined {
	if (!isSplit(node)) return undefined;
	if (node.first === target || node.second === target) return node;
	return parentOf(target, node.first) ?? parentOf(target, node.second);
}

function replaced(node: PanelNode, target: PanelNode, next: PanelNode): PanelNode {
	if (node === target) return next;
	if (!isSplit(node)) return node;
	const first = replaced(node.first, target, next);
	const second = replaced(node.second, target, next);
	return first === node.first && second === node.second ? node : { ...node, first, second };
}

function findLeaf(layout: PanelLayout, id: number): PanelLeaf | undefined {
	return panelLeaves(layout.tree).find((leaf) => leaf.id === id);
}

function clamp(value: number, { least, most }: { least: number; most: number }): number {
	return Math.min(most, Math.max(least, value));
}

/**
 * Splits the focused Panel toward `where` and focuses the new Panel. `room` is the focused Panel's
 * size in the window's own pixels; without it the split is not checked for room. A zoomed layout
 * must be restored and drawn before the caller measures.
 */
export function splitPanel(
	layout: PanelLayout,
	where: PanelDirection,
	room: PanelSize | undefined,
): PanelOutcome {
	const leaves = panelLeaves(layout.tree);
	if (leaves.length >= MOST_PANELS) return { kind: 'refused', reason: 'full' };
	const across = where === 'right' || where === 'left';
	if (
		room !== undefined &&
		(across ? room.width < SPLIT_ROOM.width : room.height < SPLIT_ROOM.height)
	)
		return { kind: 'refused', reason: 'small' };
	const leaf = leaves.find((candidate) => candidate.id === layout.focus);
	if (leaf === undefined) return { kind: 'unchanged' };
	const added: PanelLeaf = { id: layout.serial + 1 };
	const before = where === 'left' || where === 'up';
	const split: PanelSplit = {
		key: layout.serial + 2,
		axis: across ? 'row' : 'column',
		ratio: 0.5,
		first: before ? added : leaf,
		second: before ? leaf : added,
	};
	return {
		kind: 'layout',
		layout: {
			tree: replaced(layout.tree, leaf, split),
			focus: added.id,
			zoomed: false,
			serial: split.key,
		},
		done: where,
		fresh: added.id,
	};
}

/** Closes a Panel; its neighbor takes its room, and focus moves there if the Panel had it. */
export function closePanel(layout: PanelLayout, id: number): PanelOutcome {
	if (!isSplit(layout.tree)) return { kind: 'refused', reason: 'last' };
	const leaf = findLeaf(layout, id);
	const parent = leaf === undefined ? undefined : parentOf(leaf, layout.tree);
	if (parent === undefined) return { kind: 'unchanged' };
	const sibling = parent.first === leaf ? parent.second : parent.first;
	const focus = layout.focus === id ? (panelLeaves(sibling)[0]?.id ?? layout.focus) : layout.focus;
	return {
		kind: 'layout',
		layout: { ...layout, tree: replaced(layout.tree, parent, sibling), focus, zoomed: false },
		done: 'close',
	};
}

/** Maximizes the focused Panel, or restores the layout. */
export function toggleZoom(layout: PanelLayout): PanelOutcome {
	if (!isSplit(layout.tree)) return { kind: 'refused', reason: 'one' };
	return { kind: 'layout', layout: { ...layout, zoomed: !layout.zoomed }, done: 'zoom' };
}

function tracks(node: PanelNode, axis: PanelSplit['axis']): number {
	return isSplit(node) && node.axis === axis
		? tracks(node.first, axis) + tracks(node.second, axis)
		: 1;
}

function equalized(node: PanelNode): PanelNode {
	if (!isSplit(node)) return node;
	const first = tracks(node.first, node.axis);
	return {
		...node,
		ratio: first / (first + tracks(node.second, node.axis)),
		first: equalized(node.first),
		second: equalized(node.second),
	};
}

/** Gives every Panel along a row or a column the same share. */
export function equalizePanels(layout: PanelLayout): PanelOutcome {
	return {
		kind: 'layout',
		layout: { ...layout, tree: equalized(layout.tree), zoomed: false },
		done: 'equal',
	};
}

function withRatio(layout: PanelLayout, split: PanelSplit, ratio: number): PanelLayout {
	return { ...layout, tree: replaced(layout.tree, split, { ...split, ratio }) };
}

/** Moves the nearest divider on the focused Panel's way toward `where` by one step. */
export function resizePanel(layout: PanelLayout, where: PanelDirection): PanelOutcome {
	if (layout.zoomed) return { kind: 'unchanged' };
	const axis = where === 'left' || where === 'right' ? 'row' : 'column';
	let node: PanelNode | undefined = findLeaf(layout, layout.focus);
	let parent = node === undefined ? undefined : parentOf(node, layout.tree);
	while (parent !== undefined && parent.axis !== axis) {
		node = parent;
		parent = parentOf(node, layout.tree);
	}
	if (parent === undefined) return { kind: 'refused', reason: 'none' };
	const step = where === 'right' || where === 'down' ? RESIZE_STEP : -RESIZE_STEP;
	return {
		kind: 'ratio',
		layout: withRatio(layout, parent, clamp(parent.ratio + step, KEY_RATIO)),
		split: parent.key,
	};
}

function findSplit(node: PanelNode, key: number): PanelSplit | undefined {
	if (!isSplit(node)) return undefined;
	if (node.key === key) return node;
	return findSplit(node.first, key) ?? findSplit(node.second, key);
}

/** Puts a divider where the pointer drags it; `ratio` is the pointer's place across the split. */
export function dragDivider(layout: PanelLayout, key: number, ratio: number): PanelOutcome {
	const split = findSplit(layout.tree, key);
	if (split === undefined) return { kind: 'unchanged' };
	return { kind: 'ratio', layout: withRatio(layout, split, clamp(ratio, DRAG_RATIO)), split: key };
}

/** Focuses a Panel the visitor clicked. */
export function focusPanel(layout: PanelLayout, id: number): PanelOutcome {
	if (findLeaf(layout, id) === undefined) return { kind: 'unchanged' };
	return { kind: 'focus', layout: { ...layout, focus: id } };
}

/**
 * Moves focus to the Panel next to the focused one toward `where`, by what the visitor sees: of
 * the Panels ahead, the one most in line with the focused Panel's center, then the nearest.
 */
export function moveFocus(
	layout: PanelLayout,
	where: PanelDirection,
	boxes: readonly PanelBox[],
): PanelOutcome {
	if (layout.zoomed) return { kind: 'unchanged' };
	const current = boxes.find((box) => box.id === layout.focus);
	if (current === undefined) return { kind: 'unchanged' };
	const centerX = (current.left + current.right) / 2;
	const centerY = (current.top + current.bottom) / 2;
	const across = where === 'left' || where === 'right';
	const ahead = (box: PanelBox): number => {
		if (where === 'left') return current.left - box.right;
		if (where === 'right') return box.left - current.right;
		if (where === 'up') return current.top - box.bottom;
		return box.top - current.bottom;
	};
	const offset = (box: PanelBox): number =>
		across
			? Math.max(0, box.top - centerY, centerY - box.bottom)
			: Math.max(0, box.left - centerX, centerX - box.right);
	const candidates = boxes
		.filter((box) => box.id !== layout.focus)
		.map((box) => ({ id: box.id, ahead: ahead(box), offset: offset(box) }))
		.filter((candidate) => candidate.ahead > -2);
	candidates.sort((left, right) => left.offset - right.offset || left.ahead - right.ahead);
	const next = candidates[0];
	if (next === undefined) return { kind: 'unchanged' };
	return { kind: 'focus', layout: { ...layout, focus: next.id } };
}

/** The layout a window starts with: one Panel split to the right, focus on the new Panel. */
export function initialPanelLayout(): PanelLayout {
	const single: PanelLayout = { tree: { id: 1 }, focus: 1, zoomed: false, serial: 1 };
	const outcome = splitPanel(single, 'right', undefined);
	return outcome.kind === 'layout' ? outcome.layout : single;
}

/** The leaf a zoomed layout shows, or undefined when every Panel shows. */
export function zoomedLeaf(layout: PanelLayout): PanelLeaf | undefined {
	return layout.zoomed ? findLeaf(layout, layout.focus) : undefined;
}
