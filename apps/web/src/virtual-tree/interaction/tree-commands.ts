import type { TreeViewport } from '../layout/TreeViewport';
import type { VirtualTreeModel } from '../model/VirtualTreeModel';
import type { VirtualTreeRow, VirtualTreeSegment } from '../model/virtual-tree-types';
import type { VirtualTreeOpenEvent } from '../types/virtual-tree-events';
import type { TreeKeyCommand } from './tree-keys';

/** Asks the view to open a row. */
export type OpenTreeRow<K, T> = (event: VirtualTreeOpenEvent<K, T>) => void;

/** How far Ctrl+Up and Ctrl+Down scroll, in CSS pixels, as in VS Code. */
const SCROLL_STEP = 10;

/**
 * Runs a key's command as VS Code's list command of the same name does, and reports whether it
 * applied. A command applies whenever VS Code's would run; one that does not leaves the key to the
 * next command bound to it, or to the page. Moving focus starts from the top while a collapsed
 * parent hides the focused row, as in VS Code's list, and a row hidden that way neither expands nor
 * opens.
 */
export function runTreeKeyCommand<K, T>(
	viewport: TreeViewport<K, T>,
	command: TreeKeyCommand,
	open: OpenTreeRow<K, T>,
): boolean {
	const count = viewport.model.rowCount;
	switch (command) {
		case 'focusNext':
			return navigate(viewport, nextIndex(viewport));
		case 'focusPrevious':
			return navigate(viewport, previousIndex(viewport));
		case 'focusFirst':
			return navigate(viewport, count > 0 ? 0 : -1);
		case 'focusLast':
			return navigate(viewport, count - 1);
		case 'focusPageDown':
			return navigate(viewport, pageDownIndex(viewport));
		case 'focusPageUp':
			return navigate(viewport, pageUpIndex(viewport));
		case 'previousSegment':
			return focusSegment(viewport, (index) => index - 1);
		case 'nextSegment':
			return focusSegment(viewport, (index) => index + 1);
		case 'firstSegment':
			return focusSegment(viewport, () => 0);
		case 'lastSegment':
			return focusSegment(viewport, (_, segments) => segments - 1);
		case 'expandSelectionDown':
			return expandSelection(viewport, nextIndex(viewport), open);
		case 'expandSelectionUp':
			return expandSelection(viewport, previousIndex(viewport), open);
		case 'collapse':
			return collapseOrFocusParent(viewport);
		case 'collapseAll':
			viewport.model.collapseAll();
			return true;
		case 'expand':
			return expandOrFocusChild(viewport);
		case 'select':
			return select(viewport, false, open);
		case 'toggleExpand':
			return toggleExpand(viewport, open);
		case 'selectAll':
			return selectAll(viewport, open);
		case 'toggleSelection':
			return toggleSelection(viewport);
		case 'clear':
			return clear(viewport.model);
		case 'scrollUp':
			if (viewport.scrollTop === 0) return false;
			viewport.scrollTo(viewport.scrollTop - SCROLL_STEP);
			return true;
		case 'scrollDown':
			if (viewport.content.height - viewport.viewportHeight - viewport.scrollTop < 1) return false;
			viewport.scrollTo(viewport.scrollTop + SCROLL_STEP);
			return true;
	}
}

/** Expands a collapsed row or collapses an expanded one; `recursive` takes every row below it along. */
export function toggleCollapsed<K, T>(
	model: VirtualTreeModel<K, T>,
	row: VirtualTreeRow<K, T>,
	recursive: boolean,
): void {
	if (!row.collapsed) {
		model.collapse(row.key, { recursive });
		return;
	}
	// A failed read collapses the row again, and its source reports the failure.
	model.expand(row.key, { recursive }).catch(() => undefined);
}

function focusedRow<K, T>(viewport: TreeViewport<K, T>): VirtualTreeRow<K, T> | null {
	const index = viewport.focusIndex;
	return index < 0 ? null : viewport.model.rowAt(index);
}

/** The focused item: the focused row's own, or the focused item of a compact row; null without one. */
export function focusedSegment<K, T>(
	viewport: TreeViewport<K, T>,
): VirtualTreeSegment<K, T> | null {
	const row = focusedRow(viewport);
	if (row === null) return null;
	const segment = viewport.focusedSegment;
	return segment < 0 ? row : (row.segments[segment] ?? row);
}

/**
 * VS Code's compact folder navigation, its Left, Right, Home and End: focus moves among the items of
 * the focused compact row, to the one `target` picks from the focused one's index and their count.
 * It applies only while the row shows several items and focus moves, leaving the key to the list
 * command bound after it otherwise, as VS Code's Explorer does on the first or the last item.
 */
function focusSegment<K, T>(
	viewport: TreeViewport<K, T>,
	target: (index: number, segments: number) => number,
): boolean {
	const row = focusedRow(viewport);
	const index = viewport.focusedSegment;
	if (row === null || index < 0) return false;
	const next = target(index, row.segments.length);
	const segment = row.segments[next];
	if (next === index || segment === undefined) return false;
	viewport.model.setFocus(segment.key);
	return true;
}

/** VS Code's `navigate`: focuses the row, if any, then scrolls focus into sight and anchors it. */
function navigate<K, T>(viewport: TreeViewport<K, T>, index: number): true {
	const { model } = viewport;
	if (index >= 0) model.setFocus(model.rowAt(index).key);
	const focus = model.focus;
	if (focus !== null) model.reveal(focus);
	model.setAnchor(focus);
	return true;
}

/** The row below focus, the first row without focus, and -1 past the last row. */
function nextIndex<K, T>(viewport: TreeViewport<K, T>): number {
	const count = viewport.model.rowCount;
	const focus = viewport.focusIndex;
	const index = focus < 0 ? 0 : focus + 1;
	return index < count ? index : -1;
}

/** The row above focus, the first row without focus, and -1 above the first row. */
function previousIndex<K, T>(viewport: TreeViewport<K, T>): number {
	if (viewport.model.rowCount === 0) return -1;
	const focus = viewport.focusIndex;
	return focus < 0 ? 0 : focus - 1;
}

/** The last row the view shows whole, or the one a row cut by its bottom edge follows. */
function lastPageIndex<K, T>(viewport: TreeViewport<K, T>): number {
	const index = viewport.indexAtOffset(viewport.scrollTop + viewport.viewportHeight);
	return index === 0 ? 0 : index - 1;
}

/** The first row the view shows whole below the sticky rows. */
function firstPageIndex<K, T>(viewport: TreeViewport<K, T>): number {
	const count = viewport.model.rowCount;
	const top = viewport.scrollTop + viewport.stickyHeight;
	const index = top === 0 ? viewport.indexAtOffset(0) : viewport.indexAtOffset(top - 1) + 1;
	return Math.min(index, count - 1);
}

/**
 * VS Code's Page Down: focus goes to the last row in sight, or, when it is there already, the view
 * scrolls a page and focus goes to the last row in sight then.
 */
function pageDownIndex<K, T>(viewport: TreeViewport<K, T>): number {
	if (viewport.model.rowCount === 0) return -1;
	const focus = viewport.focusIndex;
	const last = lastPageIndex(viewport);
	if (focus !== last && (focus < 0 || last > focus)) return last;
	const before = viewport.scrollTop;
	viewport.scrollTo(before + viewport.viewportHeight);
	return viewport.scrollTop === before ? -1 : lastPageIndex(viewport);
}

/** VS Code's Page Up, the mirror of Page Down below the sticky rows. */
function pageUpIndex<K, T>(viewport: TreeViewport<K, T>): number {
	if (viewport.model.rowCount === 0) return -1;
	const focus = viewport.focusIndex;
	const first = firstPageIndex(viewport);
	if (focus !== first && (focus < 0 || focus >= first)) return first;
	const before = viewport.scrollTop + viewport.stickyHeight;
	viewport.scrollTo(viewport.scrollTop - viewport.viewportHeight);
	return viewport.scrollTop + viewport.stickyHeight === before ? -1 : firstPageIndex(viewport);
}

/**
 * VS Code's Shift+Down and Shift+Up: focus moves one row, and the row it reaches joins the
 * selection, or, when it is selected already, the row it left leaves it, so moving back shrinks the
 * selection. The anchor stays.
 */
function expandSelection<K, T>(
	viewport: TreeViewport<K, T>,
	index: number,
	open: OpenTreeRow<K, T>,
): true {
	const { model } = viewport;
	// Selection holds rows, and focus may rest on an item a compact row joins.
	const previous = focusedRow(viewport)?.key ?? model.focus;
	if (index >= 0) model.setFocus(model.rowAt(index).key);
	const focus = model.focus;
	if (focus !== null && focus !== previous) {
		const selection = model.selection;
		const next = new Set(selection);
		if (selection.has(focus)) {
			if (previous !== null) next.delete(previous);
		} else {
			next.add(focus);
		}
		model.setSelection(next);
		openSoleSelection(viewport, open);
	}
	if (focus !== null) model.reveal(focus);
	return true;
}

/**
 * VS Code opens the row when a key that sets the selection leaves exactly one row selected, as a
 * preview that keeps focus in the tree, whether or not the selection changed.
 */
function openSoleSelection<K, T>(viewport: TreeViewport<K, T>, open: OpenTreeRow<K, T>): void {
	const { model } = viewport;
	const selection = model.selection;
	if (selection.size !== 1) return;
	for (const key of selection) {
		const index = model.indexOf(key);
		if (index >= 0)
			open({ pinned: false, preserveFocus: true, row: model.rowAt(index), sideBySide: false });
	}
}

/** VS Code's Left: collapses the focused row, or focuses its parent when it cannot collapse. */
function collapseOrFocusParent<K, T>(viewport: TreeViewport<K, T>): boolean {
	const row = focusedRow(viewport);
	if (row === null) return false;
	if (row.collapsible && !row.collapsed) {
		viewport.model.collapse(row.key);
		return true;
	}
	if (row.parent === null) return false;
	return navigate(viewport, viewport.model.indexOf(row.parent.key));
}

/** VS Code's Right: expands the focused row, or focuses its first child when it is expanded. */
function expandOrFocusChild<K, T>(viewport: TreeViewport<K, T>): boolean {
	const row = focusedRow(viewport);
	if (row === null) return false;
	if (row.collapsible && row.collapsed) {
		toggleCollapsed(viewport.model, row, false);
		return true;
	}
	if (row.visibleDescendantCount === 0) return false;
	return navigate(viewport, viewport.focusIndex + 1);
}

/**
 * VS Code's Enter, and Space on a row that cannot expand: the focused row toggles its expansion,
 * becomes the anchor and the whole selection, and opens. Enter opens it for good and moves focus to
 * it; Space previews it and keeps focus in the tree.
 */
function select<K, T>(
	viewport: TreeViewport<K, T>,
	preserveFocus: boolean,
	open: OpenTreeRow<K, T>,
): true {
	const { model } = viewport;
	const row = focusedRow(viewport);
	if (row?.collapsible === true) toggleCollapsed(model, row, false);
	const focus = model.focus;
	model.setAnchor(focus);
	model.setSelection(focus === null ? [] : [focus]);
	if (row !== null) open({ pinned: !preserveFocus, preserveFocus, row, sideBySide: false });
	return true;
}

/** VS Code's Space: toggles the focused row's expansion, or selects it when it cannot expand. */
function toggleExpand<K, T>(viewport: TreeViewport<K, T>, open: OpenTreeRow<K, T>): true {
	const row = focusedRow(viewport);
	if (row?.collapsible !== true) return select(viewport, true, open);
	toggleCollapsed(viewport.model, row, false);
	return true;
}

/**
 * VS Code's Select All in a tree: the rows that show below the parent of the focused row, or of
 * the first selected row when focus is selected already. Once those are all selected, the parent
 * joins them, so pressing it again widens the selection a level at a time.
 */
function selectAll<K, T>(viewport: TreeViewport<K, T>, open: OpenTreeRow<K, T>): true {
	const { model } = viewport;
	const selection = model.selection;
	const focus = focusedRow(viewport);
	const start = focus !== null && !selection.has(focus.key) ? focus : firstSelectedRow(viewport);
	const scope = start?.parent ?? null;
	if (scope === null) {
		model.selectAll();
	} else {
		const first = model.indexOf(scope.key) + 1;
		const keys: K[] = [];
		for (let index = first; index < first + scope.visibleDescendantCount; index += 1) {
			keys.push(model.rowAt(index).key);
		}
		if (selection.size === keys.length) keys.push(scope.key);
		model.setSelection(keys);
	}
	openSoleSelection(viewport, open);
	return true;
}

/** The selected row that shows highest in the tree, or null when no selected row shows. */
function firstSelectedRow<K, T>(viewport: TreeViewport<K, T>): VirtualTreeRow<K, T> | null {
	const { model } = viewport;
	let first = -1;
	for (const key of model.selection) {
		const index = model.indexOf(key);
		if (index >= 0 && (first < 0 || index < first)) first = index;
	}
	return first < 0 ? null : model.rowAt(first);
}

/** VS Code's Ctrl+Shift+Enter: the focused row joins or leaves the selection. */
function toggleSelection<K, T>(viewport: TreeViewport<K, T>): true {
	const { model } = viewport;
	const focus = focusedRow(viewport)?.key ?? model.focus;
	if (focus === null) return true;
	const next = new Set(model.selection);
	if (!next.delete(focus)) next.add(focus);
	model.setSelection(next);
	return true;
}

/**
 * VS Code's Escape: clears a selection of several rows, or else the selection and focus, and the
 * anchor; with neither, it leaves the key alone.
 */
function clear<K, T>(model: VirtualTreeModel<K, T>): boolean {
	if (model.selection.size === 0 && model.focus === null) return false;
	const several = model.selection.size > 1;
	model.setSelection([]);
	if (!several) model.setFocus(null);
	model.setAnchor(null);
	return true;
}

/**
 * VS Code's Shift-click: selects from the anchor to the clicked row, in place of the run of selected
 * rows the anchor stands in, keeping the other selected rows, and focuses the row. Without an anchor
 * in sight, focus or the clicked row becomes one.
 */
export function selectRange<K, T>(viewport: TreeViewport<K, T>, index: number): void {
	const { model } = viewport;
	let anchor = model.anchor === null ? -1 : model.indexOf(model.anchor);
	if (anchor < 0) {
		anchor = viewport.focusIndex < 0 ? index : viewport.focusIndex;
		model.setAnchor(model.rowAt(anchor).key);
	}
	const selection = model.selection;
	let runStart = anchor;
	while (runStart > 0 && selection.has(model.rowAt(runStart - 1).key)) runStart -= 1;
	let runEnd = anchor;
	const last = model.rowCount - 1;
	while (runEnd < last && selection.has(model.rowAt(runEnd + 1).key)) runEnd += 1;
	const next = new Set<K>();
	for (const key of selection) {
		const position = model.indexOf(key);
		if (position < runStart || position > runEnd) next.add(key);
	}
	const end = Math.max(anchor, index);
	for (let position = Math.min(anchor, index); position <= end; position += 1) {
		next.add(model.rowAt(position).key);
	}
	model.setSelection(next);
	model.setFocus(model.rowAt(index).key);
}

/**
 * VS Code's Ctrl-click, Cmd-click on macOS: the row joins or leaves the selection, and takes the
 * anchor and focus, which lands on `focus`, an item of the row.
 */
export function toggleRow<K, T>(model: VirtualTreeModel<K, T>, key: K, focus: K): void {
	model.setFocus(focus);
	model.setAnchor(key);
	const next = new Set(model.selection);
	if (!next.delete(key)) next.add(key);
	model.setSelection(next);
}
