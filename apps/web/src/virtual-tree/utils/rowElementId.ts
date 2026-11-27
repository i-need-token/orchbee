/** The DOM id of the element that shows a row, which the tree's `aria-activedescendant` names. */
export function rowElementId(treeId: string, index: number): string {
	return `${treeId}-row-${index}`;
}
