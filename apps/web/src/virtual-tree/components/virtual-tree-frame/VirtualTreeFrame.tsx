import * as React from 'react';

import type { TreeViewport } from '../../layout/TreeViewport';
import { rowElementId, segmentElementId } from '../../utils/tree-element-ids';
import styles from './VirtualTreeFrame.module.css';

interface VirtualTreeFrameProps<K, T> {
	readonly ref: React.Ref<HTMLDivElement>;
	readonly scrollbarReference: React.Ref<HTMLDivElement>;
	readonly viewport: TreeViewport<K, T>;
	readonly treeId: string;
	readonly label: string;
	/** The content and sticky layers; they draw themselves, so a new active row redraws this alone. */
	readonly children: React.ReactNode;
}

/**
 * The tree's focusable element. It names the focused row through `aria-activedescendant`, or the
 * focused item of a compact row, as VS Code's Explorer names a compact folder's, so focus stays here
 * while the row elements are reused, and it holds the scrollbar over the rows.
 */
export function VirtualTreeFrame<K, T>({
	children,
	label,
	ref,
	scrollbarReference,
	treeId,
	viewport,
}: VirtualTreeFrameProps<K, T>): React.ReactNode {
	const focusIndex = React.useSyncExternalStore(viewport.subscribe, () => viewport.focusIndex);
	const focusedSegment = React.useSyncExternalStore(
		viewport.subscribe,
		() => viewport.focusedSegment,
	);
	const dropOnTree = React.useSyncExternalStore(viewport.subscribe, () => viewport.dropOnTree);
	let activeDescendant: string | undefined;
	if (focusIndex >= 0) {
		const rowId = rowElementId(treeId, focusIndex);
		activeDescendant = focusedSegment < 0 ? rowId : segmentElementId(rowId, focusedSegment);
	}
	return (
		<div
			aria-activedescendant={activeDescendant}
			aria-label={label}
			aria-multiselectable
			className={styles['frame']}
			data-drop-target={dropOnTree ? '' : undefined}
			ref={ref}
			role='tree'
			tabIndex={0}
		>
			{children}
			<div aria-hidden='true' className={styles['scrollbar']} ref={scrollbarReference}>
				<div className={styles['thumb']} />
			</div>
		</div>
	);
}
