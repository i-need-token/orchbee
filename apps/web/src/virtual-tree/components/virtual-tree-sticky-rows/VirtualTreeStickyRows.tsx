import * as React from 'react';

import type { TreeViewport } from '../../layout/TreeViewport';
import type { VirtualTreeRowRenderer } from '../../types/virtual-tree-row-renderer';
import { VirtualTreeRowView } from '../virtual-tree-row-view/VirtualTreeRowView';
import styles from './VirtualTreeStickyRows.module.css';

interface VirtualTreeStickyRowsProps<K, T> {
	readonly viewport: TreeViewport<K, T>;
	readonly renderRow: VirtualTreeRowRenderer<K, T>;
}

/**
 * The rows that stick at the top of the tree while their descendants scroll beneath them, drawn over
 * the rows. They repeat rows the tree already names, so assistive tools skip them, and they show
 * neither focus nor selection, as in VS Code. Each depth keeps its element, so a scroll redraws only
 * the sticky rows that changed.
 */
export function VirtualTreeStickyRows<K, T>({
	renderRow,
	viewport,
}: VirtualTreeStickyRowsProps<K, T>): React.ReactNode {
	const count = React.useSyncExternalStore(viewport.subscribe, () => viewport.stickyCount);
	const height = React.useSyncExternalStore(viewport.subscribe, () => viewport.stickyHeight);
	if (count === 0) return null;
	// A row pushed up slides beneath the rows above it, as in VS Code, so deeper rows paint first.
	const rows: React.ReactNode[] = [];
	for (let index = count - 1; index >= 0; index -= 1) {
		rows.push(<StickySlot index={index} key={index} renderRow={renderRow} viewport={viewport} />);
	}
	return (
		<div
			aria-hidden='true'
			className={styles['sticky']}
			style={{ '--ob-comp-virtual-tree-sticky-height': `${height}px` }}
		>
			{rows}
		</div>
	);
}

interface StickySlotProps<K, T> {
	readonly viewport: TreeViewport<K, T>;
	/** The depth among the sticky rows, outermost first. */
	readonly index: number;
	readonly renderRow: VirtualTreeRowRenderer<K, T>;
}

function StickySlot<K, T>({ index, renderRow, viewport }: StickySlotProps<K, T>): React.ReactNode {
	const sticky = React.useSyncExternalStore(viewport.subscribe, () => viewport.stickyRow(index));
	if (sticky === null) return null;
	return (
		<VirtualTreeRowView
			focused={false}
			height={sticky.height}
			place={{ index, kind: 'sticky' }}
			renderRow={renderRow}
			row={sticky.row}
			selected={false}
			top={sticky.top}
		/>
	);
}
