import { useMemoizedFn } from '@orchbee/foundation/react';
import * as React from 'react';

import { TreeFind } from '../find/TreeFind';
import type { TreeViewport } from '../layout/TreeViewport';
import type { VirtualTreeRow } from '../model/virtual-tree-types';
import type { VirtualTreeFindOptions } from '../types/virtual-tree-find';

/**
 * The tree's find while it has a label to find rows by, as VS Code's tree has one while it has a
 * keyboard navigation label provider. It follows the model while mounted, closes when the tree goes,
 * and takes the view's default mode and match type whenever they change, as VS Code's find takes its
 * settings'.
 */
export function useTreeFind<K, T>(
	viewport: TreeViewport<K, T>,
	label: ((row: VirtualTreeRow<K, T>) => string) | null,
	options: VirtualTreeFindOptions<K, T> | undefined,
	focusTree: () => void,
): TreeFind<K, T> | null {
	const provider = useMemoizedFn(() => options?.provider ?? null);
	const find = React.useMemo(
		() => (label === null ? null : new TreeFind(viewport, { focusTree, label, provider })),
		[focusTree, label, provider, viewport],
	);
	React.useEffect(() => find?.connect(), [find]);
	const mode = options?.mode ?? 'highlight';
	const matchType = options?.matchType ?? 'fuzzy';
	React.useLayoutEffect(() => {
		find?.setMode(mode);
	}, [find, mode]);
	React.useLayoutEffect(() => {
		find?.setMatchType(matchType);
	}, [find, matchType]);
	return find;
}
