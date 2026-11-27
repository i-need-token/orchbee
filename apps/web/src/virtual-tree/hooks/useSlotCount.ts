import * as React from 'react';

import type { TreeViewport } from '../layout/TreeViewport';

/**
 * How many slots the tree renders, heard in the layout phase. The viewport learns its size just
 * before the first paint, and an external-store subscription would hear it only once passive Effects
 * run, after that paint; this one renders the rows into the first frame.
 */
export function useSlotCount<K, T>(viewport: TreeViewport<K, T>): number {
	const [slotCount, setSlotCount] = React.useState(() => viewport.slotCount);
	React.useLayoutEffect(() => {
		const read = () => {
			setSlotCount(viewport.slotCount);
		};
		const unsubscribe = viewport.subscribe(read);
		read();
		return unsubscribe;
	}, [viewport]);
	return slotCount;
}
