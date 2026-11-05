import * as React from 'react';

/**
 * Marks the edges where rows have scrolled out of sight, and places the overlay thumb that stands in
 * for the hidden native scrollbar. It writes the section's attributes and custom properties
 * directly, so scrolling never renders the list again.
 */
export function useSidebarScrollEdges(
	regionReference: React.RefObject<HTMLElement | null>,
): React.RefObject<HTMLDivElement | null> {
	const sectionReference = React.useRef<HTMLDivElement>(null);
	React.useEffect(() => {
		const region = regionReference.current;
		const section = sectionReference.current;
		if (region === null || section === null) return;
		const update = () => {
			const { clientHeight, scrollHeight, scrollTop } = region;
			const overflow = scrollHeight - clientHeight;
			section.toggleAttribute('data-scrolled-start', scrollTop > 0);
			section.toggleAttribute('data-scrolled-end', overflow - scrollTop > 1);
			section.toggleAttribute('data-overflowing', overflow > 1);
			if (overflow <= 1) return;
			section.style.setProperty(
				'--ob-comp-workbench-sidebar-thumb-size',
				`${(clientHeight / scrollHeight) * 100}%`,
			);
			section.style.setProperty(
				'--ob-comp-workbench-sidebar-thumb-start',
				`${(scrollTop / scrollHeight) * 100}%`,
			);
		};
		update();
		region.addEventListener('scroll', update, { passive: true });
		// Folding, new rows and a resized window change what overflows without a scroll.
		const observer = typeof ResizeObserver === 'function' ? new ResizeObserver(update) : null;
		observer?.observe(region);
		for (const child of region.children) observer?.observe(child);
		return () => {
			region.removeEventListener('scroll', update);
			observer?.disconnect();
		};
	}, [regionReference]);
	return sectionReference;
}
