import type { VirtualTreeModel } from '../model/VirtualTreeModel';
import type { VirtualTreeRow } from '../model/virtual-tree-types';

/** How long VS Code's tree waits, with a drag resting on a collapsed row, before it expands it. */
export const HOVER_EXPAND_DELAY = 500;

/** The band along the top and bottom edges where a drag scrolls VS Code's lists, in CSS pixels. */
const SCROLL_EDGE = 35;

/** The farthest VS Code's lists scroll in one frame of a drag, in CSS pixels. */
const MAX_SCROLL_STEP = 14;

/**
 * How far a drag scrolls a list in one frame, as VS Code's list does, with the pointer `offset` from
 * the top of a view `height` tall: within 35 px of either edge, faster nearer the edge, by up to 14 px.
 */
export function dragScrollStep(offset: number, height: number): number {
	if (offset < SCROLL_EDGE) {
		return Math.max(-MAX_SCROLL_STEP, Math.floor(0.3 * (offset - SCROLL_EDGE)));
	}
	const lowerBand = height - SCROLL_EDGE;
	if (offset > lowerBand) return Math.min(MAX_SCROLL_STEP, Math.floor(0.3 * (offset - lowerBand)));
	return 0;
}

/**
 * The rows a drag from `row` carries, as VS Code's lists choose them: the selection in tree order when
 * it holds the row, or the row alone. Selected rows hidden in a collapsed row stay behind.
 */
export function rowsToDrag<K, T>(
	model: VirtualTreeModel<K, T>,
	row: VirtualTreeRow<K, T>,
): readonly VirtualTreeRow<K, T>[] {
	const { selection } = model;
	if (!selection.has(row.key)) return [row];
	const indexes = new Int32Array(selection.size);
	let count = 0;
	// A selection made in tree order, as Select All and Shift-click make it, needs no sort.
	let sorted = true;
	for (const key of selection) {
		const index = model.indexOf(key);
		if (index < 0) continue;
		if (count > 0 && index < (indexes[count - 1] ?? 0)) sorted = false;
		indexes[count] = index;
		count += 1;
	}
	const visible = indexes.subarray(0, count);
	if (!sorted) visible.sort();
	return Array.from(visible, (index) => model.rowAt(index));
}
