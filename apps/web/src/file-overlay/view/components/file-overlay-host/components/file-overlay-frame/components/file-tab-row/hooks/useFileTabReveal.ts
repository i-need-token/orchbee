import { useLayoutEffect, type RefObject } from 'react';

/**
 * Scrolls the row just enough that the active tab's slot shows whole, as VS Code reveals the tab.
 * It reads the slot from the layout, which leaves out the offset a drag along the row gives a tab.
 */
function revealActiveTab(row: HTMLElement, list: HTMLElement): void {
	const tab = list.querySelector<HTMLElement>(':scope > [role="tab"][data-active]');
	if (tab === null) return;
	const rowBox = row.getBoundingClientRect();
	const left = list.getBoundingClientRect().left + tab.offsetLeft;
	const right = left + tab.offsetWidth;
	if (left < rowBox.left) row.scrollLeft -= rowBox.left - left;
	else if (right > rowBox.right) row.scrollLeft += right - rowBox.right;
}

/**
 * Keeps the active tab in view whenever another tab becomes active, a tab opens, closes or moves,
 * the row changes width, or the tabs do, as when a label gains its folders or a font arrives. It
 * scrolls only the row: revealing through the document would also scroll the Workbench beneath
 * the overlay. `listReference` holds the tabs inside the row, as wide as they are together, and
 * `order` names the tabs in the row's order.
 */
export function useFileTabReveal(
	rowReference: RefObject<HTMLElement | null>,
	listReference: RefObject<HTMLElement | null>,
	activeId: string | null,
	order: string,
): void {
	useLayoutEffect(() => {
		const row = rowReference.current;
		const list = listReference.current;
		if (row !== null && list !== null) revealActiveTab(row, list);
	}, [activeId, listReference, order, rowReference]);
	useLayoutEffect(() => {
		const row = rowReference.current;
		const list = listReference.current;
		if (row === null || list === null) return undefined;
		const observer = new ResizeObserver(() => revealActiveTab(row, list));
		observer.observe(row);
		observer.observe(list);
		return () => observer.disconnect();
	}, [listReference, rowReference]);
}
