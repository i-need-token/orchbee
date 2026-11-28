import { ProductIcon } from '@orchbee/ui/components';
import type * as React from 'react';

import styles from './VirtualTreeRowView.module.css';

/** What a row element stands for: an item of the tree, or a sticky row repeating one. */
type RowPlace =
	| {
			readonly kind: 'item';
			readonly id: string;
			/** The row's index, which a click resolves. */
			readonly index: number;
			readonly positionInSet: number;
			readonly setSize: number;
	  }
	| {
			readonly kind: 'sticky';
			/** Its depth among the sticky rows, outermost first, which a click resolves. */
			readonly index: number;
	  };

/**
 * The row's state as plain values, never the row itself: a row links to its parent's children, and
 * React's development build diffs a component's changed props a few levels deep on every render, so
 * a row prop would walk a large folder's children whenever a slot shows another row.
 */
interface VirtualTreeRowViewProps {
	readonly place: RowPlace;
	/** One for a top-level row; it sets the indentation. */
	readonly depth: number;
	readonly collapsible: boolean;
	/** Whether the row is expanded; false for a row that cannot be. */
	readonly expanded: boolean;
	/** Whether the row shows that its children are loading. */
	readonly loading: boolean;
	/** The row's top edge in its layer, in CSS pixels. */
	readonly top: number;
	readonly height: number;
	readonly focused: boolean;
	readonly selected: boolean;
	/** The view's content for the row. */
	readonly children: React.ReactNode;
}

/**
 * One row as the tree draws it: the indentation, the twistie, which turns into a spinner while the
 * row shows that its children are loading, and the view's content. An item row carries the tree
 * item's semantics; a sticky row repeats a row the tree already names, so it carries none.
 */
export function VirtualTreeRowView({
	children,
	collapsible,
	depth,
	expanded,
	focused,
	height,
	loading,
	place,
	selected,
	top,
}: VirtualTreeRowViewProps): React.ReactNode {
	const item = place.kind === 'item' ? place : null;
	return (
		<div
			aria-expanded={item !== null && collapsible ? expanded : undefined}
			aria-level={item === null ? undefined : depth}
			aria-posinset={item?.positionInSet}
			aria-selected={item === null ? undefined : selected}
			aria-setsize={item?.setSize}
			className={styles['row']}
			data-focused={focused ? '' : undefined}
			data-row-index={item?.index}
			data-sticky-index={place.kind === 'sticky' ? place.index : undefined}
			id={item?.id}
			role={item === null ? undefined : 'treeitem'}
			style={{
				'--ob-comp-virtual-tree-row-depth': depth,
				'--ob-comp-virtual-tree-row-height': `${height}px`,
				'--ob-comp-virtual-tree-row-start': `${top}px`,
			}}
		>
			<span
				className={styles['twistie']}
				data-loading={loading ? '' : undefined}
				data-open={expanded && !loading ? '' : undefined}
				data-twistie=''
			>
				{collapsible ? (
					<ProductIcon name={loading ? 'loading' : 'chevron-right'} size='disclosure' />
				) : null}
			</span>
			<div className={styles['content']}>{children}</div>
		</div>
	);
}
