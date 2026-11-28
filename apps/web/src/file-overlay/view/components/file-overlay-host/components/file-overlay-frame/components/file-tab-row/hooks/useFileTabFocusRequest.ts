import { useLayoutEffect, type RefObject } from 'react';

import { useFileOverlayHostFocusRequest } from '../../../../../contexts/file-overlay-host-store';
import { activeFileTab } from '../utils/activeFileTab';

/**
 * Gives the active tab the keys when the Workbench asks, as VS Code focuses the editor it opens from
 * the quick input: once the row shows the tab the ask follows, or as the row first shows when the
 * ask opened the overlay.
 */
export function useFileTabFocusRequest(rowReference: RefObject<HTMLElement | null>): void {
	const focusRequest = useFileOverlayHostFocusRequest();
	useLayoutEffect(() => {
		const row = rowReference.current;
		if (focusRequest.count === 0 || row === null || !focusRequest.take()) return;
		activeFileTab(row)?.focus({ preventScroll: true });
	}, [focusRequest, rowReference]);
}
