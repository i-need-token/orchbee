import type { TreeViewport } from '../layout/TreeViewport';
import type { VirtualTreeRow } from '../model/virtual-tree-types';

/** Controls inside a row's content handle their own clicks and drags, as VS Code's lists leave them. */
export const TREE_ROW_CONTROLS = 'a[href], button, input, select, textarea, [contenteditable]';

/** The row an element of the tree belongs to. */
export interface TreeRowHit<K, T> {
	readonly row: VirtualTreeRow<K, T>;
	readonly index: number;
	/** Whether the element belongs to the row's sticky copy. */
	readonly sticky: boolean;
	/** Whether the element is in the row's twistie. */
	readonly twistie: boolean;
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
	const { model } = viewport;
	const depth = element.getAttribute('data-sticky-index');
	if (depth !== null) {
		const sticky = viewport.stickyRow(Number(depth));
		if (sticky === null) return null;
		const index = model.indexOf(sticky.row.key);
		return index < 0 ? null : { index, row: sticky.row, sticky: true, twistie: onTwistie };
	}
	const index = Number(element.getAttribute('data-row-index'));
	if (!Number.isInteger(index) || index < 0 || index >= model.rowCount) return null;
	return { index, row: model.rowAt(index), sticky: false, twistie: onTwistie };
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
