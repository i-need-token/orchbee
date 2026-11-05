/**
 * Scrolls the sidebar's own scroll owner just enough to show a row at its nearer edge. A row already
 * in view, a row taller than the view, and a row without geometry cause no scroll.
 */
export function revealSidebarRow(root: HTMLElement, target: HTMLElement): void {
	const scrollOwner = target.closest<HTMLElement>('[data-navigation-reorder-scroll]');
	if (scrollOwner === null || !root.contains(scrollOwner) || !scrollOwner.contains(target)) return;
	const viewportHeight = scrollOwner.clientHeight;
	const targetBounds = target.getBoundingClientRect();
	if (viewportHeight <= 0 || targetBounds.height <= 0) return;
	const viewportStart = scrollOwner.getBoundingClientRect().top + scrollOwner.clientTop;
	const startDelta = targetBounds.top - viewportStart;
	const endDelta = targetBounds.bottom - (viewportStart + viewportHeight);
	if ((startDelta >= 0 && endDelta <= 0) || (startDelta <= 0 && endDelta >= 0)) return;
	const delta = Math.abs(startDelta) < Math.abs(endDelta) ? startDelta : endDelta;
	scrollOwner.scrollTop = Math.max(
		0,
		Math.min(scrollOwner.scrollTop + delta, scrollOwner.scrollHeight - viewportHeight),
	);
}
