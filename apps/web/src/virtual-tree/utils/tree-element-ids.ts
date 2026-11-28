/** The DOM id of the element that shows a row, which the tree's `aria-activedescendant` names. */
export function rowElementId(treeId: string, index: number): string {
	return `${treeId}-row-${index}`;
}

/**
 * The DOM id of the element that shows one item of a compact row, which the tree's
 * `aria-activedescendant` names while that item has focus.
 */
export function segmentElementId(rowId: string, segment: number): string {
	return `${rowId}-${segment}`;
}
