import { useMemoizedFn } from '@orchbee/foundation/react';
import type { KeyboardEvent } from 'react';

import {
	useFileOverlayHostDispatch,
	useFileOverlayHostFocusTerminal,
} from '../../../contexts/file-overlay-host-store';

/**
 * Esc that nothing in the frame took: the focused widget, a field or a drag takes its own first, and
 * the menus and pickers the frame's controls open lie outside it. The open overlay shrinks to
 * picture-in-picture with the keys in the current terminal, as a press on the scrim does; in
 * picture-in-picture the terminal takes the keys. It is no shortcut binding, so the frame hears Esc
 * last, as it bubbles.
 */
export function useFileOverlayEscape(shrunk: boolean): (event: KeyboardEvent<HTMLElement>) => void {
	const dispatch = useFileOverlayHostDispatch();
	const focusTerminal = useFileOverlayHostFocusTerminal();
	return useMemoizedFn((event: KeyboardEvent<HTMLElement>) => {
		if (
			event.key !== 'Escape' ||
			event.defaultPrevented ||
			event.nativeEvent.isComposing ||
			event.altKey ||
			event.ctrlKey ||
			event.metaKey ||
			event.shiftKey ||
			!(event.target instanceof Node) ||
			// Esc in a menu a control opened reaches the frame only through React's tree.
			!event.currentTarget.contains(event.target)
		) {
			return;
		}
		event.preventDefault();
		if (shrunk) focusTerminal();
		else dispatch({ type: 'dismissed' });
	});
}
