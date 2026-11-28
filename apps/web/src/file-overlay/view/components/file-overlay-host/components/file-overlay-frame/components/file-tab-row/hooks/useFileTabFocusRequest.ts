import { useLayoutEffect, type RefObject } from 'react';

import { useFileOverlayHostFocusRequest } from '../../../../../contexts/file-overlay-host-store';
import { activeFileTab } from '../utils/activeFileTab';

/**
 * Gives the active tab the keys when asked, as VS Code focuses the editor it opens from the quick
 * input: once the row shows the tab the ask follows, or as the row first shows when the ask opened
 * the overlay. A row that `shown` says is hidden, as in picture-in-picture, takes no ask.
 */
export function useFileTabFocusRequest(
	rowReference: RefObject<HTMLElement | null>,
	shown: boolean,
): void {
	const focusRequest = useFileOverlayHostFocusRequest();
	useLayoutEffect(() => {
		const row = rowReference.current;
		if (!shown || focusRequest.count === 0 || row === null || !focusRequest.take()) return;
		activeFileTab(row)?.focus({ preventScroll: true });
	}, [focusRequest, rowReference, shown]);
}
