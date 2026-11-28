import type * as React from 'react';

import type { VirtualTreeRow } from '../model/virtual-tree-types';

/** What `VirtualTreeSegments` draws the items of a compact row with, besides their names. */
interface VirtualTreeCompactRowState {
	/** The row element's id, which each item's element id extends; null for a sticky row. */
	readonly id: string | null;
	/** The row's own item's level; the items above it in the row are one level up each. */
	readonly level: number;
	/** Whether the row is expanded, which every item of it announces; undefined when it cannot be. */
	readonly expanded: boolean | undefined;
	readonly positionInSet: number;
	readonly setSize: number;
	readonly selected: boolean;
	/** The index of the focused item while the row has focus; -1 otherwise. */
	readonly focused: number;
	/** The index of the joined item a drag would drop on; -1 otherwise. */
	readonly drop: number;
}

/** A row's state as its renderer draws it. */
export interface VirtualTreeRowState {
	readonly focused: boolean;
	readonly selected: boolean;
	/**
	 * For a row that shows several items, what `VirtualTreeSegments` needs to draw them, so the
	 * renderer passes this state on to it whole; null for a row of one item.
	 */
	readonly compact: VirtualTreeCompactRowState | null;
}

/**
 * Draws a row's content; the tree draws the row around it, with its indentation and twistie. It runs
 * when the row's slot shows another row, or when the row's revision, position or state changes;
 * content that depends on other data subscribes to that data itself. A compact row's content draws
 * its items with `VirtualTreeSegments`.
 */
export type VirtualTreeRowRenderer<K, T> = (
	row: VirtualTreeRow<K, T>,
	state: VirtualTreeRowState,
) => React.ReactNode;
