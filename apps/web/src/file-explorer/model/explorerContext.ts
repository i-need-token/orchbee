import type { VirtualTreeModel, VirtualTreeRow } from '../../virtual-tree';
import type { ExplorerItem } from './explorer-item';

/** The tree an Explorer command acts in: its rows and the root they sit in. */
interface ExplorerContextTree {
	readonly model: VirtualTreeModel<string, ExplorerItem>;
	/** What VS Code titles the root, which no row shows. */
	readonly name: string;
	readonly root: string;
}

/** The selected rows in row order. A row a collapsed folder hides counts as no longer selected. */
function selectedRows(
	model: VirtualTreeModel<string, ExplorerItem>,
): readonly VirtualTreeRow<string, ExplorerItem>[] {
	const indexes: number[] = [];
	for (const key of model.selection) {
		const index = model.indexOf(key);
		if (index >= 0) indexes.push(index);
	}
	return indexes.sort((one, other) => one - other).map((index) => model.rowAt(index));
}

/**
 * The items an Explorer command acts on, by VS Code's Explorer rule with the multi-selection
 * respected. While more than one row is selected, every selected row acts, a compact row with each
 * folder it joins. Otherwise the focused item acts, a compact row's focused folder, or the selected
 * items when the focused item is one of them. Without focus or selection, as after a right click on
 * the empty space below the rows, the root acts. Unlike VS Code's, a command never acts on a selected
 * row a collapsed folder hides, so it touches only rows the person can see.
 */
export function explorerContext(tree: ExplorerContextTree): readonly ExplorerItem[] {
	const { model } = tree;
	const rows = selectedRows(model);
	const focusKey = rows.length > 1 ? null : model.focus;
	const focusIndex = focusKey === null ? -1 : model.indexOf(focusKey);
	const focusRow = focusIndex < 0 ? null : model.rowAt(focusIndex);
	const focused = focusRow?.segments.find((segment) => segment.key === focusKey) ?? null;
	const selected = rows.flatMap((row) => {
		// The other folders of the compact row that holds focus do not count, as in VS Code.
		if (focused !== null && row.key === focusRow?.key && row.segments.length > 1) {
			return focused.key === row.key ? [focused] : [];
		}
		return row.segments;
	});
	if (focused === null) {
		if (selected.length > 0) return selected.map((segment) => segment.element);
		return [{ directory: true, entry: null, excluded: false, name: tree.name, path: tree.root }];
	}
	return selected.some((segment) => segment.key === focused.key)
		? selected.map((segment) => segment.element)
		: [focused.element];
}
