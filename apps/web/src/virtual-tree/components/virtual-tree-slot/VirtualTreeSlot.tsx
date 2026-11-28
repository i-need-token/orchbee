import * as React from 'react';

import type { TreeViewport } from '../../layout/TreeViewport';
import type { VirtualTreeRowRenderer } from '../../types/virtual-tree-row-renderer';
import { rowElementId } from '../../utils/rowElementId';
import { VirtualTreeRowView } from '../virtual-tree-row-view/VirtualTreeRowView';

interface VirtualTreeSlotProps<K, T> {
	readonly viewport: TreeViewport<K, T>;
	/** Which slot this is, or `focused` for the focused row while it is outside the visible rows. */
	readonly slot: number | 'focused';
	readonly treeId: string;
	readonly renderRow: VirtualTreeRowRenderer<K, T>;
}

/**
 * One row element, kept for whichever row its slot shows, so a scroll draws only the slots whose row
 * changed.
 */
export function VirtualTreeSlot<K, T>({
	renderRow,
	slot,
	treeId,
	viewport,
}: VirtualTreeSlotProps<K, T>): React.ReactNode {
	const shown = React.useSyncExternalStore(viewport.subscribe, () =>
		slot === 'focused' ? viewport.focusedRow : viewport.slot(slot),
	);
	if (shown === null) return null;
	const { focused, row, selected } = shown;
	return (
		<VirtualTreeRowView
			collapsible={row.collapsible}
			depth={row.depth}
			expanded={row.collapsible && !row.collapsed}
			focused={focused}
			height={shown.height}
			loading={row.loading}
			place={{
				id: rowElementId(treeId, shown.index),
				index: shown.index,
				kind: 'item',
				positionInSet: shown.positionInSet,
				setSize: shown.setSize,
			}}
			selected={selected}
			top={shown.top}
		>
			{renderRow(row, { focused, selected })}
		</VirtualTreeRowView>
	);
}
