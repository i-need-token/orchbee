import * as React from 'react';

/**
 * Focuses the active view each time the request increases: `focusView` lets the view place focus
 * itself, as the Explorer puts it on its tree, and returns false for a view that does not, whose
 * panel takes focus instead. A request made before the column mounts is honored when it does, since
 * the File Bar's chunk may still be loading when a view is asked for.
 */
export function useFileBarViewFocusRequest(
	focusRequest: number,
	focusView: () => boolean,
): React.RefObject<HTMLDivElement | null> {
	const viewReference = React.useRef<HTMLDivElement>(null);
	const lastFocusRequestReference = React.useRef(0);
	React.useLayoutEffect(() => {
		if (focusRequest === lastFocusRequestReference.current) return;
		lastFocusRequestReference.current = focusRequest;
		if (focusRequest <= 0 || focusView()) return;
		viewReference.current?.focus({ preventScroll: true });
	}, [focusRequest, focusView]);
	return viewReference;
}
