import * as React from 'react';

import type { TreeViewport } from '../../layout/TreeViewport';
import styles from './VirtualTreeContent.module.css';

interface VirtualTreeContentProps<K, T> {
	readonly viewport: TreeViewport<K, T>;
	/** The slots; they draw themselves, so a scroll redraws only this layer's offset. */
	readonly children: React.ReactNode;
}

/** The layer that holds every row at its offset and moves by the scroll offset. */
export function VirtualTreeContent<K, T>({
	children,
	viewport,
}: VirtualTreeContentProps<K, T>): React.ReactNode {
	const geometry = React.useSyncExternalStore(viewport.subscribe, () => viewport.content);
	return (
		<div
			className={styles['content']}
			style={{
				'--ob-comp-virtual-tree-content-height': `${geometry.height}px`,
				'--ob-comp-virtual-tree-content-offset': `${-geometry.offset}px`,
			}}
		>
			{children}
		</div>
	);
}
