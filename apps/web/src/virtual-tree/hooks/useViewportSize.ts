import * as React from 'react';
import { flushSync } from 'react-dom';

import type { TreeViewport } from '../layout/TreeViewport';

/**
 * Hands the frame's size to the viewport as the browser lays it out. A resize observer reports it
 * after layout and before paint, so nothing reads layout while the tree renders or scrolls, and the
 * rows for a new size render before that frame paints.
 */
export function useViewportSize<K, T>(
	frameReference: React.RefObject<HTMLElement | null>,
	viewport: TreeViewport<K, T>,
): void {
	React.useLayoutEffect(() => {
		const frame = frameReference.current;
		const ownerWindow = frame?.ownerDocument.defaultView;
		if (frame === null || !ownerWindow) return;
		const observer = new ownerWindow.ResizeObserver((entries) => {
			const size = entries.at(-1)?.contentRect;
			if (size === undefined) return;
			flushSync(() => {
				viewport.setSize(size.width, size.height);
			});
		});
		observer.observe(frame);
		return () => {
			observer.disconnect();
		};
	}, [frameReference, viewport]);
}
