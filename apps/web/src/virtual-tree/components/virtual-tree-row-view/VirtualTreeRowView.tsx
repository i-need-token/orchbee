import { ProductIcon } from '@orchbee/ui/components';
import type * as React from 'react';

import type { VirtualTreeRow } from '../../model/virtual-tree-types';
import type { VirtualTreeRowRenderer } from '../../types/virtual-tree-row-renderer';
import styles from './VirtualTreeRowView.module.css';

/** What a row element stands for: an item of the tree, or a sticky row repeating one. */
type RowPlace =
	| {
			readonly kind: 'item';
			readonly id: string;
			readonly positionInSet: number;
			readonly setSize: number;
	  }
	| {
			readonly kind: 'sticky';
			/** Its depth among the sticky rows, outermost first, which a click resolves. */
			readonly index: number;
	  };

interface VirtualTreeRowViewProps<K, T> {
	readonly row: VirtualTreeRow<K, T>;
	readonly place: RowPlace;
	/** The row's top edge in its layer, in CSS pixels. */
	readonly top: number;
	readonly height: number;
	readonly focused: boolean;
	readonly selected: boolean;
	readonly renderRow: VirtualTreeRowRenderer<K, T>;
}

/**
 * One row as the tree draws it: the indentation, the twistie, which turns into a spinner while the
 * row shows that its children are loading, and the view's content. An item row carries the tree
 * item's semantics; a sticky row repeats a row the tree already names, so it carries none.
 */
export function VirtualTreeRowView<K, T>({
	focused,
	height,
	place,
	renderRow,
	row,
	selected,
	top,
}: VirtualTreeRowViewProps<K, T>): React.ReactNode {
	const open = row.collapsible && !row.collapsed;
	const { loading } = row;
	const item = place.kind === 'item' ? place : null;
	return (
		<div
			aria-expanded={item !== null && row.collapsible ? open : undefined}
			aria-level={item === null ? undefined : row.depth}
			aria-posinset={item?.positionInSet}
			aria-selected={item === null ? undefined : selected}
			aria-setsize={item?.setSize}
			className={styles['row']}
			data-focused={focused ? '' : undefined}
			data-sticky-index={place.kind === 'sticky' ? place.index : undefined}
			id={item?.id}
			role={item === null ? undefined : 'treeitem'}
			style={{
				'--ob-comp-virtual-tree-row-depth': row.depth,
				'--ob-comp-virtual-tree-row-height': `${height}px`,
				'--ob-comp-virtual-tree-row-start': `${top}px`,
			}}
		>
			<span
				className={styles['twistie']}
				data-loading={loading ? '' : undefined}
				data-open={open && !loading ? '' : undefined}
				data-twistie=''
			>
				{row.collapsible ? (
					<ProductIcon name={loading ? 'loading' : 'chevron-right'} size='disclosure' />
				) : null}
			</span>
			<div className={styles['content']}>{renderRow(row, { focused, selected })}</div>
		</div>
	);
}
