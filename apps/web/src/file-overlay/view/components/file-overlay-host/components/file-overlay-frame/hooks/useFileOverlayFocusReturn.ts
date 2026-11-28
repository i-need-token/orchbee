import { useMemoizedFn } from '@orchbee/foundation/react';
import { useLayoutEffect, type RefObject } from 'react';

import { useFileOverlayHostReturnFocus } from '../../../contexts/file-overlay-host-store';

/**
 * Gives the keys back when the frame goes away with the focus inside it, as when its last tab
 * closes: to the control that brought the overlay up while it still shows, else to the current
 * terminal, since the focused element leaves with the frame and the next keys would go nowhere. A
 * dismissal has given the terminal the keys already, so it finds no focus left to return.
 */
export function useFileOverlayFocusReturn(rootReference: RefObject<HTMLElement | null>): void {
	const giveBack = useFileOverlayHostReturnFocus();
	const returnFocus = useMemoizedFn(() => giveBack());
	useLayoutEffect(() => {
		const root = rootReference.current;
		// The frame's elements are still in the document while its layout Effects clean up.
		return () => {
			if (root !== null && root.contains(document.activeElement)) returnFocus();
		};
	}, [returnFocus, rootReference]);
}
