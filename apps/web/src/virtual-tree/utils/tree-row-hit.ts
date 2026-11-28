import type { TreeViewport } from '../layout/TreeViewport';
import type { VirtualTreeRow, VirtualTreeSegment } from '../model/virtual-tree-types';

/**
 * Controls inside a row's content handle their own clicks and drags, as VS Code's lists leave them,
 * a code editor's included.
 */
export const TREE_ROW_CONTROLS =
	'a[href], button, input, select, textarea, [contenteditable], .monaco-editor';

/** The row an element of the tree belongs to. */
export interface TreeRowHit<K, T> {
	readonly row: VirtualTreeRow<K, T>;
	readonly index: number;
	/** Whether the element belongs to the row's sticky copy. */
	readonly sticky: boolean;
	/** Whether the element is in the row's twistie. */
	readonly twistie: boolean;
	/** The item of a compact row whose name, or the separator after it, holds the element; else null. */
	readonly segment: VirtualTreeSegment<K, T> | null;
}

/** The row, or the sticky copy of a row, that holds `target` inside `frame`; null elsewhere. */
export function hitTreeRow<K, T>(
	frame: Element,
	viewport: TreeViewport<K, T>,
	target: Element,
): TreeRowHit<K, T> | null {
	const element = target.closest('[data-row-index], [data-sticky-index]');
	if (element === null || !frame.contains(element)) return null;
	const twistie = target.closest('[data-twistie]');
	const onTwistie = twistie !== null && element.contains(twistie);
	const segmentElement = target.closest('[data-tree-segment]');
	const segmentIndex =
		segmentElement !== null && element.contains(segmentElement)
			? Number(segmentElement.getAttribute('data-tree-segment'))
			: -1;
	const { model } = viewport;
	const depth = element.getAttribute('data-sticky-index');
	let row: VirtualTreeRow<K, T>;
	let index: number;
	if (depth === null) {
		index = Number(element.getAttribute('data-row-index'));
		if (!Number.isInteger(index) || index < 0 || index >= model.rowCount) return null;
		row = model.rowAt(index);
	} else {
		const sticky = viewport.stickyRow(Number(depth));
		if (sticky === null) return null;
		({ row } = sticky);
		index = model.indexOf(row.key);
		if (index < 0) return null;
	}
	const segment = row.segments.length > 1 ? (row.segments[segmentIndex] ?? null) : null;
	return { index, row, segment, sticky: depth !== null, twistie: onTwistie };
}

/**
 * The element of the tree an event targets, unless it belongs to the scrollbar or to one of
 * `controls` inside a row.
 */
export function treePointerTarget(
	frame: Element,
	scrollbar: Element | null,
	target: EventTarget | null,
	controls: string,
): Element | null {
	const view = frame.ownerDocument.defaultView;
	if (view === null || !(target instanceof view.Element)) return null;
	if (scrollbar?.contains(target) === true) return null;
	const control = target.closest(controls);
	return control !== null && frame.contains(control) ? null : target;
}
