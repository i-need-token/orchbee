import * as React from 'react';

import { VirtualTreeContent } from './components/virtual-tree-content/VirtualTreeContent';
import { VirtualTreeFrame } from './components/virtual-tree-frame/VirtualTreeFrame';
import { VirtualTreeSlot } from './components/virtual-tree-slot/VirtualTreeSlot';
import { VirtualTreeStickyRows } from './components/virtual-tree-sticky-rows/VirtualTreeStickyRows';
import { useSlotCount } from './hooks/useSlotCount';
import { useStickyRowClicks } from './hooks/useStickyRowClicks';
import { useViewportSize } from './hooks/useViewportSize';
import { useVirtualTreeScrollbar } from './hooks/useVirtualTreeScrollbar';
import { useWheelScrolling } from './hooks/useWheelScrolling';
import { TreeViewport, type VirtualTreeRowHeight } from './layout/TreeViewport';
import type { VirtualTreeModel } from './model/VirtualTreeModel';
import type { VirtualTreeRowRenderer } from './types/virtual-tree-row-renderer';

export interface VirtualTreeProps<K, T> {
	readonly model: VirtualTreeModel<K, T>;
	/** The tree's accessible name. */
	readonly label: string;
	/**
	 * Every row's height in CSS pixels, VS Code's 22 by default, or a function that reads a row's.
	 * A function is read for every row whenever it changes identity, so keep it stable.
	 */
	readonly rowHeight?: VirtualTreeRowHeight<K, T>;
	/**
	 * How many expanded rows at most stick at the top while their descendants scroll beneath them,
	 * VS Code's 7 by default; 0 turns sticky scroll off. They never cover more than 40% of the tree.
	 */
	readonly maxStickyRows?: number;
	readonly renderRow: VirtualTreeRowRenderer<K, T>;
}

/**
 * A tree or list over a `VirtualTreeModel`, drawing only the rows in sight. It fills its container,
 * which must give it a definite size, scrolls by the wheel and its scrollbar as VS Code's lists do,
 * keeps the model's scroll offset, so a tree shown again opens where it was left, and keeps the rows
 * whose descendants are in sight stuck at the top, as VS Code's sticky scroll does.
 */
export function VirtualTree<K, T>({
	label,
	maxStickyRows,
	model,
	renderRow,
	rowHeight,
}: VirtualTreeProps<K, T>): React.ReactNode {
	const treeId = React.useId();
	const frameReference = React.useRef<HTMLDivElement>(null);
	const scrollbarReference = React.useRef<HTMLDivElement>(null);
	const viewport = React.useMemo(() => new TreeViewport(model), [model]);
	React.useLayoutEffect(() => {
		viewport.setRowHeight(rowHeight);
	}, [rowHeight, viewport]);
	React.useLayoutEffect(() => {
		viewport.setMaxStickyRows(maxStickyRows);
	}, [maxStickyRows, viewport]);
	const slotCount = useSlotCount(viewport);
	useViewportSize(frameReference, viewport);
	useWheelScrolling(frameReference, viewport);
	useVirtualTreeScrollbar(frameReference, scrollbarReference, viewport);
	useStickyRowClicks(frameReference, viewport);

	const slots: React.ReactNode[] = [];
	for (let slot = 0; slot < slotCount; slot += 1) {
		slots.push(
			<VirtualTreeSlot
				key={slot}
				renderRow={renderRow}
				slot={slot}
				treeId={treeId}
				viewport={viewport}
			/>,
		);
	}
	return (
		<VirtualTreeFrame
			label={label}
			ref={frameReference}
			scrollbarReference={scrollbarReference}
			treeId={treeId}
			viewport={viewport}
		>
			<VirtualTreeContent viewport={viewport}>
				{slots}
				<VirtualTreeSlot
					key='focused'
					renderRow={renderRow}
					slot='focused'
					treeId={treeId}
					viewport={viewport}
				/>
			</VirtualTreeContent>
			<VirtualTreeStickyRows renderRow={renderRow} viewport={viewport} />
		</VirtualTreeFrame>
	);
}
