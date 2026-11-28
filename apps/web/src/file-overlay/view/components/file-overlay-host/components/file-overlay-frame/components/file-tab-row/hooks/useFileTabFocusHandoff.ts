import { useMemoizedFn } from '@orchbee/foundation/react';
import { useLayoutEffect, useRef, type RefObject } from 'react';

function focusActiveTab(row: HTMLElement): void {
	row.querySelector<HTMLElement>('[role="tab"][data-active]')?.focus({ preventScroll: true });
}

/**
 * Keeps the keys on the active tab while they are in the row. When a key shows another tab, the
 * focus follows it; when the focused tab leaves the row, as when its close button or a middle click
 * closes it, the active tab takes the focus, which would otherwise fall to the page and leave the
 * next keys going nowhere. Returns what a tab calls as it leaves with the focus; the row focuses the
 * active tab once the change has rendered.
 */
export function useFileTabFocusHandoff(
	rowReference: RefObject<HTMLElement | null>,
	activeId: string | null,
): () => void {
	const handoff = useRef(false);
	// Without dependencies it runs after every render of the row, and a closed tab renders the row.
	useLayoutEffect(() => {
		if (!handoff.current) return;
		handoff.current = false;
		const row = rowReference.current;
		if (row !== null) focusActiveTab(row);
	});
	useLayoutEffect(() => {
		const row = rowReference.current;
		const focused = row?.ownerDocument.activeElement ?? null;
		if (row === null || focused === null || !row.contains(focused)) return;
		if (!focused.matches('[data-active]')) focusActiveTab(row);
	}, [activeId, rowReference]);
	return useMemoizedFn(() => {
		handoff.current = true;
	});
}
