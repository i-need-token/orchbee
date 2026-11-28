import { useMemoizedFn } from '@orchbee/foundation/react';
import { useLayoutEffect, useRef, type RefObject } from 'react';

/**
 * Gives the keys to the active tab when the focused tab leaves the row, as when its close button or
 * a middle click closes it: the focus would fall to the page and the next keys would go nowhere.
 * Returns what a tab calls as it leaves with the focus; the row focuses the active tab once the
 * change has rendered.
 */
export function useFileTabFocusHandoff(rowReference: RefObject<HTMLElement | null>): () => void {
	const handoff = useRef(false);
	// Without dependencies it runs after every render of the row, and a closed tab renders the row.
	useLayoutEffect(() => {
		if (!handoff.current) return;
		handoff.current = false;
		rowReference.current
			?.querySelector<HTMLElement>('[role="tab"][data-active]')
			?.focus({ preventScroll: true });
	});
	return useMemoizedFn(() => {
		handoff.current = true;
	});
}
