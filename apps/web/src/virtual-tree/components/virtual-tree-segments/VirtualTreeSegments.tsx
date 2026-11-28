import { fileBarMessages, useOrchbeeIntl } from '@orchbee/i18n';
import type * as React from 'react';

import type { VirtualTreeRowState } from '../../types/virtual-tree-row-renderer';
import { segmentElementId } from '../../utils/tree-element-ids';
import styles from './VirtualTreeSegments.module.css';

export interface VirtualTreeSegmentsProps {
	/** Each item's name, outermost first: one for each of the row's `segments`. */
	readonly names: readonly string[];
	/** The row's state as the renderer received it. */
	readonly state: VirtualTreeRowState;
	/** What separates the names, `/` unless set, as VS Code's label service separates a path. */
	readonly separator?: string | undefined;
}

type CompactRowState = NonNullable<VirtualTreeRowState['compact']>;

/** One item's tree item semantics. */
interface ItemSemantics {
	readonly id: string;
	readonly label: string;
	readonly level: number;
	readonly positionInSet: number;
	readonly setSize: number;
}

/**
 * The names of the items a row shows, as VS Code's Explorer draws a compact folder: one name for a
 * row of one item, and for a compact row each item's name after the separator of the one above it.
 * Each item of a compact row is a tree item of its own, which the tree names as the active
 * descendant while the item has focus and announces with its parents in the row. Its name is
 * underlined under the pointer and while it has the focused row's focus, and lights up while a drag
 * would drop on it. A press on an item's name, or on the separator after it, is a press on that item.
 */
export function VirtualTreeSegments({
	names,
	separator = '/',
	state,
}: VirtualTreeSegmentsProps): React.ReactNode {
	const intl = useOrchbeeIntl();
	const { compact } = state;
	if (compact === null) return <span className={styles['segments']}>{names.join(separator)}</span>;
	const last = names.length - 1;
	const rowId = compact.id;
	return (
		<span className={styles['segments']}>
			{names.map((name, index) => {
				// A sticky row's items take presses but repeat items the tree already names.
				const item = rowId === null ? null : itemSemantics(intl, names, index, compact, rowId);
				// Each item keeps its element by its place in the row while the names change.
				return (
					<span
						aria-expanded={item === null ? undefined : compact.expanded}
						aria-label={item?.label}
						aria-level={item?.level}
						aria-posinset={item?.positionInSet}
						aria-selected={item === null ? undefined : compact.selected}
						aria-setsize={item?.setSize}
						className={styles['segment']}
						data-active={index === compact.focused ? '' : undefined}
						data-drop-target={index === compact.drop ? '' : undefined}
						data-tree-segment={index}
						id={item?.id}
						key={index}
						role={item === null ? undefined : 'treeitem'}
					>
						<span className={styles['name']}>{name}</span>
						{index < last ? (
							<span aria-hidden='true' className={styles['separator']}>
								{separator}
							</span>
						) : null}
					</span>
				);
			})}
		</span>
	);
}

/**
 * One item's tree item semantics, as VS Code's Explorer gives a compact folder's names: its name
 * with its parents in the row, and, beyond VS Code, its own level, as the items above it in the row
 * are its ancestors, and its place among its siblings: the first item takes the row's, and each item
 * below it is its parent's only child.
 */
function itemSemantics(
	intl: ReturnType<typeof useOrchbeeIntl>,
	names: readonly string[],
	index: number,
	compact: CompactRowState,
	rowId: string,
): ItemSemantics {
	const name = names[index] ?? '';
	// VS Code names an item's parents in the row nearest first.
	const parents = names.slice(0, index).reverse();
	return {
		id: segmentElementId(rowId, index),
		label:
			index === 0
				? name
				: intl.formatMessage(fileBarMessages.compactItem, {
						name,
						parents: intl.formatList(parents, { style: 'narrow', type: 'conjunction' }),
					}),
		level: compact.level - (names.length - 1 - index),
		positionInSet: index === 0 ? compact.positionInSet : 1,
		setSize: index === 0 ? compact.setSize : 1,
	};
}
