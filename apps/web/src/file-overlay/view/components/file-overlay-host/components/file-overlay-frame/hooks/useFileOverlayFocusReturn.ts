import { useMemoizedFn } from '@orchbee/foundation/react';
import { useLayoutEffect, type RefObject } from 'react';

import { useFileOverlayHostFocusTerminal } from '../../../contexts/file-overlay-host-store';

/**
 * Gives the keys to the current terminal when the frame goes away with the focus inside it, as when
 * its last tab closes: the focused element would leave with it and the next keys would go nowhere.
 * A dismissal has given the terminal the keys already, so it finds no focus left to return.
 */
export function useFileOverlayFocusReturn(rootReference: RefObject<HTMLElement | null>): void {
	const focusTerminal = useFileOverlayHostFocusTerminal();
	const returnFocus = useMemoizedFn(() => focusTerminal());
	useLayoutEffect(() => {
		const root = rootReference.current;
		// The frame's elements are still in the document while its layout Effects clean up.
		return () => {
			if (root !== null && root.contains(document.activeElement)) returnFocus();
		};
	}, [returnFocus, rootReference]);
}
