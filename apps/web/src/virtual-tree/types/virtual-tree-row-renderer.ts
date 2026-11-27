import type * as React from 'react';

import type { VirtualTreeRow } from '../model/virtual-tree-types';

/** A row's state as its renderer draws it. */
interface VirtualTreeRowState {
	readonly focused: boolean;
	readonly selected: boolean;
}

/**
 * Draws a row's content; the tree draws the row around it, with its indentation and twistie. It runs
 * when the row's slot shows another row, or when the row's revision, position or state changes;
 * content that depends on other data subscribes to that data itself.
 */
export type VirtualTreeRowRenderer<K, T> = (
	row: VirtualTreeRow<K, T>,
	state: VirtualTreeRowState,
) => React.ReactNode;
