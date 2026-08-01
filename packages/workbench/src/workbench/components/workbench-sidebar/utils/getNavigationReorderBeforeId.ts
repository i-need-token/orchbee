export function getNavigationReorderBeforeId(
	listElement: HTMLUListElement,
	sourceItemId: string,
	clientY: number,
): string | null {
	const listTop = listElement.getBoundingClientRect().top;
	const listLayoutTop = getDocumentLayoutTop(listElement);
	for (const child of listElement.children) {
		if (!(child instanceof HTMLElement)) continue;
		const itemId = child.dataset['navigationReorderId'];
		if (itemId === undefined || itemId === sourceItemId) continue;
		const row = child.querySelector<HTMLElement>('[data-navigation-reorder-row]');
		if (row === null) continue;
		let midpoint = listTop + (getDocumentLayoutTop(row) - listLayoutTop) + row.offsetHeight / 2;
		if (row.offsetHeight === 0) {
			const bounds = row.getBoundingClientRect();
			midpoint = bounds.top + bounds.height / 2;
		}
		if (clientY < midpoint) return itemId;
	}
	return null;
}

function getDocumentLayoutTop(element: HTMLElement): number {
	let top = 0;
	let current: HTMLElement | null = element;
	while (current !== null) {
		top += current.offsetTop;
		current = current.offsetParent instanceof HTMLElement ? current.offsetParent : null;
	}
	return top;
}
