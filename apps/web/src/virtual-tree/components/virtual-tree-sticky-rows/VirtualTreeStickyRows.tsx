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
	const { row } = sticky;
	const expanded = row.collapsible && !row.collapsed;
	// A sticky compact row draws its items, which a press reaches, without the semantics of its row's.
	const compact =
		row.segments.length > 1
			? {
					drop: -1,
					expanded: row.collapsible ? expanded : undefined,
					focused: -1,
					id: null,
					level: row.level,
					positionInSet: row.positionInSet,
					selected: false,
					setSize: row.setSize,
				}
			: null;
	return (
		<VirtualTreeRowView
			collapsible={row.collapsible}
			depth={sticky.depth}
			dropTarget={false}
			expanded={expanded}
			focused={false}
			height={sticky.height}
			loading={row.loading}
			place={{ index, kind: 'sticky' }}
			selected={false}
			top={sticky.top}
		>
			{renderRow(row, {
				compact,
				focused: false,
				highlights: viewport.model.highlightsOf(row),
				selected: false,
			})}
		</VirtualTreeRowView>
	);
}
