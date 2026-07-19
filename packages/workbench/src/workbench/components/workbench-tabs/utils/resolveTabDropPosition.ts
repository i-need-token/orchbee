export type TabDropEdge = 'after' | 'before';

export interface TabDropPosition {
	readonly boundaryIndex: number;
	readonly targetIndex: number;
}

export function resolveTabDropPosition(
	tabIds: readonly string[],
	draggedTabId: string | undefined,
	dropTargetTabId: string,
	dropEdge: TabDropEdge,
): TabDropPosition | undefined {
	const sourceIndex = draggedTabId === undefined ? -1 : tabIds.indexOf(draggedTabId);
	const hoveredIndex = tabIds.indexOf(dropTargetTabId);
	if (sourceIndex === -1 || hoveredIndex === -1) return undefined;

	const boundaryIndex = hoveredIndex + (dropEdge === 'after' ? 1 : 0);
	const targetIndex = boundaryIndex - (sourceIndex < boundaryIndex ? 1 : 0);
	if (sourceIndex === targetIndex) return undefined;

	return { boundaryIndex, targetIndex };
}
