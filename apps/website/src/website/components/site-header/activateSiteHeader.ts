import type { ActivationContext } from '../../activation';

// Past this scroll position the bar takes its background.
const STUCK_AFTER_SCROLL = 8;

// Gives the bar its background once the page scrolls, steps the bar's Download aside while the
// page's own Download button is on screen, and shows the menu button that opens the sheet of links
// at narrow widths. Choosing a link or pressing Escape closes the sheet.
export function activateSiteHeader({ document }: ActivationContext): void {
	const header = document.querySelector<HTMLElement>('[data-site-header]');
	const view = document.defaultView;
	if (header === null || view === null) return;
	const stick = (): void => {
		header.toggleAttribute('data-stuck', view.scrollY > STUCK_AFTER_SCROLL);
	};
	view.addEventListener('scroll', stick, { passive: true });
	stick();

	const pageDownload = document.querySelector('[data-page-download]');
	if (pageDownload !== null) {
		new view.IntersectionObserver(
			([entry]) => {
				header.toggleAttribute('data-page-download-shown', entry?.isIntersecting === true);
			},
			// A button under the bar is not on screen.
			{ rootMargin: `-${header.offsetHeight}px 0px 0px 0px` },
		).observe(pageDownload);
	}

	const menu = header.querySelector<HTMLButtonElement>('[data-site-menu]');
	if (menu === null) return;
	const setOpen = (open: boolean): void => {
		header.toggleAttribute('data-open', open);
		menu.setAttribute('aria-expanded', String(open));
	};
	menu.hidden = false;
	menu.addEventListener('click', () => {
		setOpen(!header.hasAttribute('data-open'));
	});
	header.addEventListener('click', (event) => {
		if (event.target instanceof Element && event.target.closest('a') !== null) setOpen(false);
	});
	document.addEventListener('click', (event) => {
		if (
			header.hasAttribute('data-open') &&
			event.target instanceof Node &&
			!header.contains(event.target)
		) {
			setOpen(false);
		}
	});
	document.addEventListener('keydown', (event) => {
		if (event.key !== 'Escape' || !header.hasAttribute('data-open')) return;
		setOpen(false);
		menu.focus();
	});
}
