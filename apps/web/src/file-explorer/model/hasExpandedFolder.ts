import type { VirtualTreeModel } from '../../virtual-tree';

/**
 * Whether a top-level folder is expanded, which VS Code requires before Collapse Folders in Explorer
 * runs. Only collapsed rows are stepped over, so the walk stops at the first expanded folder.
 */
export function hasExpandedFolder<K, T>(model: VirtualTreeModel<K, T>): boolean {
	for (let index = 0; index < model.rowCount; index += 1) {
		const row = model.rowAt(index);
		if (row.collapsible && !row.collapsed) return true;
	}
	return false;
}
