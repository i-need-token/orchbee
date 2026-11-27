import * as React from 'react';

import { rowElementId } from '../../utils/rowElementId';
import type { TreeViewport } from '../../layout/TreeViewport';
import styles from './VirtualTreeFrame.module.css';

interface VirtualTreeFrameProps<K, T> {
	readonly ref: React.Ref<HTMLDivElement>;
	readonly scrollbarReference: React.Ref<HTMLDivElement>;
	readonly viewport: TreeViewport<K, T>;
	readonly treeId: string;
	readonly label: string;
	/** The content layer; it draws itself, so a change of the active row redraws only this frame. */
	readonly children: React.ReactNode;
}

/**
 * The tree's focusable element. It names the focused row through `aria-activedescendant`, so focus
 * stays here while the row elements are reused, and it holds the scrollbar over the rows.
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
	return (
		<div
			aria-activedescendant={focusIndex < 0 ? undefined : rowElementId(treeId, focusIndex)}
			aria-label={label}
			aria-multiselectable
			className={styles['frame']}
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
