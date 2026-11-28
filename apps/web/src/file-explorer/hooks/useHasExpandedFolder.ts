import * as React from 'react';

import type { VirtualTreeModel } from '../../virtual-tree';
import type { ExplorerItem } from '../model/explorer-item';
import { hasExpandedFolder } from '../model/hasExpandedFolder';

function subscribeToNothing(): () => void {
	return () => undefined;
}

/** Whether Collapse Folders in Explorer has a folder to collapse, kept current as rows expand. */
export function useHasExpandedFolder(
	model: VirtualTreeModel<string, ExplorerItem> | null,
): boolean {
	const subscribe = React.useMemo(
		() =>
			model === null
				? subscribeToNothing
				: (listener: () => void): (() => void) => model.subscribe(listener),
		[model],
	);
	return React.useSyncExternalStore(subscribe, () => model !== null && hasExpandedFolder(model));
}
