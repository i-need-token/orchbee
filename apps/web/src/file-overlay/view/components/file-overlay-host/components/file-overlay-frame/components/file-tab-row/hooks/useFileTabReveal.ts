import { useLayoutEffect, type RefObject } from 'react';

/** Scrolls the row just enough that the active tab shows whole, as VS Code reveals it. */
function revealActiveTab(row: HTMLElement): void {
	const tab = row.querySelector<HTMLElement>('[role="tab"][data-active]');
	if (tab === null) return;
	const rowBox = row.getBoundingClientRect();
	const tabBox = tab.getBoundingClientRect();
	if (tabBox.left < rowBox.left) row.scrollLeft -= rowBox.left - tabBox.left;
	else if (tabBox.right > rowBox.right) row.scrollLeft += tabBox.right - rowBox.right;
}

/**
 * Keeps the active tab in view whenever another tab becomes active, a tab opens or closes, the row
 * changes width, or the tabs do, as when a label gains its folders or a font arrives. It scrolls
 * only the row: revealing through the document would also scroll the Workbench beneath the overlay.
 * `listReference` holds the tabs inside the row, as wide as they are together.
 */
export function useFileTabReveal(
	rowReference: RefObject<HTMLElement | null>,
	listReference: RefObject<HTMLElement | null>,
	activeId: string | null,
	tabCount: number,
): void {
	useLayoutEffect(() => {
		const row = rowReference.current;
		if (row !== null) revealActiveTab(row);
	}, [activeId, rowReference, tabCount]);
	useLayoutEffect(() => {
		const row = rowReference.current;
		const list = listReference.current;
		if (row === null || list === null) return undefined;
		const observer = new ResizeObserver(() => revealActiveTab(row));
		observer.observe(row);
		observer.observe(list);
		return () => observer.disconnect();
	}, [listReference, rowReference]);
}
