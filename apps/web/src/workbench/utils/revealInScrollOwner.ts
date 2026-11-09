/**
 * Scrolls one scroll owner just enough to show a descendant at its nearer edge, and no other
 * ancestor, unlike `scrollIntoView`. A target already in view, a target taller than the view, and a
 * target without geometry cause no scroll.
 */
export function revealInScrollOwner(scrollOwner: HTMLElement, target: HTMLElement): void {
	if (!scrollOwner.contains(target)) return;
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
