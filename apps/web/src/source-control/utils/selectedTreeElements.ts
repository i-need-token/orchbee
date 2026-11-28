import type { VirtualTreeModel } from '../../virtual-tree';

/** The selected rows' elements in the order they show, leaving out rows a collapsed parent hides. */
export function selectedTreeElements<K, T>(tree: VirtualTreeModel<K, T>): T[] {
	const indexes: number[] = [];
	for (const key of tree.selection) {
		const index = tree.indexOf(key);
		if (index >= 0) indexes.push(index);
	}
	return indexes.sort((one, other) => one - other).map((index) => tree.rowAt(index).element);
}
