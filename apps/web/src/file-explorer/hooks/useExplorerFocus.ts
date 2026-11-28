import * as React from 'react';

import type { FileExplorerFocus } from '../types/file-explorer-focus';

/**
 * The Explorer's side of a focus request: the tree takes focus, or, while it has not loaded, the
 * Explorer itself does and hands focus to the tree once the tree shows, unless focus has moved.
 */
export function useExplorerFocus(
	reference: React.Ref<FileExplorerFocus> | undefined,
	rootReference: React.RefObject<HTMLDivElement | null>,
	treeReference: React.RefObject<HTMLDivElement | null>,
	treeShown: boolean,
): void {
	const pendingReference = React.useRef(false);
	React.useImperativeHandle(
		reference,
		() => ({
			focus: () => {
				const tree = treeReference.current;
				pendingReference.current = tree === null;
				(tree ?? rootReference.current)?.focus({ preventScroll: true });
			},
		}),
		[rootReference, treeReference],
	);
	React.useLayoutEffect(() => {
		const root = rootReference.current;
		const tree = treeReference.current;
		if (!treeShown || !pendingReference.current || root === null || tree === null) return;
		pendingReference.current = false;
		if (root.ownerDocument.activeElement === root) tree.focus({ preventScroll: true });
	}, [rootReference, treeReference, treeShown]);
}
