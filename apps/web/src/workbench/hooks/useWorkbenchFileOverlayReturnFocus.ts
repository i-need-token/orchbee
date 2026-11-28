import { useMemoizedFn } from '@orchbee/foundation/react';
import * as React from 'react';

import type { FileOverlayStore } from '../../file-overlay';
import { resolveWorkbenchFocusRegionFromTarget } from '../utils/workbench-focus-dom';

/**
 * The focused element as the control that brought the overlay up, which takes the keys back as the
 * overlay closes with them. The overlay's own controls go with it, and the terminal takes them back
 * anyway.
 */
function invokingControl(element: Element | null): HTMLElement | null {
	if (!(element instanceof HTMLElement) || element === element.ownerDocument.body) return null;
	const region = resolveWorkbenchFocusRegionFromTarget(element);
	return region === 'file-overlay' || region === 'terminal' ? null : element;
}

/**
 * How the shown overlay gives the keys back as it closes with them: to the control that last brought
 * it up from closed, as a dialog returns focus to its invoker, while that control still shows and
 * takes them; else to the current terminal. Shrinking to picture-in-picture hands the terminal the
 * keys, so a later return goes there too.
 */
export function useWorkbenchFileOverlayReturnFocus(
	store: FileOverlayStore | null,
	focusTerminal: () => void,
): () => void {
	const invoker = React.useRef<HTMLElement | null>(null);
	React.useEffect(() => {
		invoker.current = null;
		if (store === null) return undefined;
		return store.state.subscribe((state, previous) => {
			if (state.mode.kind === previous.mode.kind) return;
			if (state.mode.kind === 'open') invoker.current = invokingControl(document.activeElement);
			else if (state.mode.kind === 'pip') invoker.current = null;
		});
	}, [store]);
	return useMemoizedFn(() => {
		const control = invoker.current;
		invoker.current = null;
		if (control?.isConnected === true && control.getClientRects().length > 0) {
			control.focus({ preventScroll: true });
			// A control that no longer takes the keys, as one gone inert, leaves them to the terminal.
			if (control.ownerDocument.activeElement === control) return;
		}
		focusTerminal();
	});
}
