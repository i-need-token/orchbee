import { ProductIcon } from '@orchbee/ui/components';
import * as React from 'react';

import type { VirtualTreeRowRenderer } from '../../types/virtual-tree-row-renderer';
import { rowElementId } from '../../utils/rowElementId';
import type { TreeViewport } from '../../layout/TreeViewport';
import styles from './VirtualTreeSlot.module.css';

interface VirtualTreeSlotProps<K, T> {
	readonly viewport: TreeViewport<K, T>;
	/** Which slot this is, or `focused` for the focused row while it is outside the visible rows. */
	readonly slot: number | 'focused';
	readonly treeId: string;
	readonly renderRow: VirtualTreeRowRenderer<K, T>;
}

/**
 * One row element, kept for whichever row its slot shows, so a scroll draws only the slots whose row
 * changed. The tree draws the indentation and the twistie, which turns into a spinner while the row
 * shows that its children are loading; the view draws the content.
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
	const open = row.collapsible && !row.collapsed;
	const { loading } = row;
	return (
		<div
			aria-expanded={row.collapsible ? open : undefined}
			aria-level={row.depth}
			aria-posinset={shown.positionInSet}
			aria-selected={selected}
			aria-setsize={shown.setSize}
			className={styles['row']}
			data-focused={focused ? '' : undefined}
			id={rowElementId(treeId, shown.index)}
			role='treeitem'
			style={{
				'--ob-comp-virtual-tree-row-depth': row.depth,
				'--ob-comp-virtual-tree-row-height': `${shown.height}px`,
				'--ob-comp-virtual-tree-row-start': `${shown.top}px`,
			}}
		>
			<span
				className={styles['twistie']}
				data-loading={loading ? '' : undefined}
				data-open={open && !loading ? '' : undefined}
			>
				{row.collapsible ? (
					<ProductIcon name={loading ? 'loading' : 'chevron-right'} size='disclosure' />
				) : null}
			</span>
			<div className={styles['content']}>{renderRow(row, { focused, selected })}</div>
		</div>
	);
}
