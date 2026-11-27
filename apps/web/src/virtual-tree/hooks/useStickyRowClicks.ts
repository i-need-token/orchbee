import * as React from 'react';

import type { TreeViewport } from '../layout/TreeViewport';

/** Controls inside a row's content handle their own clicks, as VS Code's lists leave them. */
const ROW_CONTROLS = 'a[href], button, input, select, textarea, [contenteditable]';

/**
 * Handles a click on a sticky row as VS Code's tree does: it scrolls the row's node to just below the
 * sticky rows of its ancestors, focuses and selects it, and a click on the twistie also collapses it,
 * with Alt every row below it too. A click with Shift, Ctrl or Cmd is left to the tree's selection.
 */
export function useStickyRowClicks<K, T>(
	frameReference: React.RefObject<HTMLElement | null>,
	viewport: TreeViewport<K, T>,
): void {
	React.useEffect(() => {
		const frame = frameReference.current;
		const ownerWindow = frame?.ownerDocument.defaultView;
		if (frame === null || !ownerWindow) return;
		const onClick = (event: MouseEvent) => {
			if (event.button !== 0 || event.shiftKey || event.ctrlKey || event.metaKey) return;
			const target = event.target;
			if (!(target instanceof ownerWindow.Element)) return;
			const element = target.closest('[data-sticky-index]');
			if (element === null || !frame.contains(element)) return;
			const control = target.closest(ROW_CONTROLS);
			if (control !== null && element.contains(control)) return;
			const depth = element.getAttribute('data-sticky-index');
			const sticky = depth === null ? null : viewport.stickyRow(Number(depth));
			if (sticky === null) return;
			const { model } = viewport;
			const { key } = sticky.row;
			const twistie = target.closest('[data-twistie]');
			viewport.scrollBelowStickyRows(key);
			model.setFocus(key);
			model.setAnchor(key);
			model.setSelection([key]);
			if (twistie !== null && element.contains(twistie) && !sticky.row.collapsed) {
				model.collapse(key, { recursive: event.altKey });
			}
		};
		frame.addEventListener('click', onClick);
		return () => {
			frame.removeEventListener('click', onClick);
		};
	}, [frameReference, viewport]);
}
