import { revealInScrollOwner } from '../../../utils/revealInScrollOwner';

/**
 * Scrolls the sidebar's own scroll owner just enough to show a row at its nearer edge. A row already
 * in view, a row taller than the view, and a row without geometry cause no scroll.
 */
export function revealSidebarRow(root: HTMLElement, target: HTMLElement): void {
	const scrollOwner = target.closest<HTMLElement>('[data-navigation-reorder-scroll]');
	if (scrollOwner === null || !root.contains(scrollOwner)) return;
	revealInScrollOwner(scrollOwner, target);
}
