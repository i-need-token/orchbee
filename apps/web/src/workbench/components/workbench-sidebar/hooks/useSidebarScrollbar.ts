import {
	beginHistoryScrollbarDrag,
	HISTORY_SCROLLBAR_FADE_DELAY_MS,
	projectHistoryScrollbar,
	resolveHistoryScrollbarDrag,
	type HistoryScrollbarDrag,
	type HistoryScrollbarProjection,
} from '@orchbee/terminal';
import * as React from 'react';

interface SidebarScrollbarReferences {
	/** The section around the list: it carries the edge and overflow attributes and thumb geometry. */
	readonly sectionReference: React.RefObject<HTMLDivElement | null>;
	/** The gutter that takes the scrollbar's presses, drags and wheel. */
	readonly scrollbarReference: React.RefObject<HTMLDivElement | null>;
}

/** Wheel deltas in lines or pages, as the browser scrolls a list by them. */
const WHEEL_LINE_PIXELS = 16;

/**
 * Gives the list the Panels' scrollbar, counted in pixels where a Panel counts rows: it shows as the
 * list scrolls and fades once the list has been still for the Panels' delay, a thumb press drags it
 * and a track press jumps there. It also marks the edges where rows have scrolled out of sight. It
 * writes the section's attributes and custom properties directly, so scrolling never renders the
 * list again.
 */
export function useSidebarScrollbar(
	regionReference: React.RefObject<HTMLElement | null>,
): SidebarScrollbarReferences {
	const sectionReference = React.useRef<HTMLDivElement>(null);
	const scrollbarReference = React.useRef<HTMLDivElement>(null);
	React.useEffect(() => {
		const region = regionReference.current;
		const section = sectionReference.current;
		const scrollbar = scrollbarReference.current;
		const ownerWindow = section?.ownerDocument.defaultView;
		if (region === null || section === null || scrollbar === null || !ownerWindow) return;
		let opacity = 0;
		let fadeTimer: number | null = null;
		let gesture: { readonly drag: HistoryScrollbarDrag; readonly pointerId: number } | null = null;

		const project = (): HistoryScrollbarProjection | null =>
			projectHistoryScrollbar({
				opacity,
				totalRows: BigInt(Math.round(region.scrollHeight)),
				viewportHeight: region.clientHeight,
				viewportWidth: region.clientWidth,
				visibleRows: region.clientHeight,
				visibleStartRow: BigInt(Math.max(0, Math.round(region.scrollTop))),
			});
		const update = () => {
			const { clientHeight, scrollHeight, scrollTop } = region;
			section.toggleAttribute('data-scrolled-start', scrollTop > 0);
			section.toggleAttribute('data-scrolled-end', scrollHeight - clientHeight - scrollTop > 1);
			const projection = project();
			section.toggleAttribute('data-overflowing', projection !== null);
			if (projection === null) return;
			section.style.setProperty(
				'--ob-comp-workbench-sidebar-thumb-start',
				`${projection.thumb.y}px`,
			);
			section.style.setProperty(
				'--ob-comp-workbench-sidebar-thumb-size',
				`${projection.thumb.height}px`,
			);
			section.style.setProperty(
				'--ob-comp-workbench-sidebar-thumb-opacity',
				String(projection.thumb.opacity),
			);
		};
		const cancelFade = () => {
			if (fadeTimer === null) return;
			ownerWindow.clearTimeout(fadeTimer);
			fadeTimer = null;
		};
		const show = () => {
			cancelFade();
			opacity = 1;
			update();
			if (gesture !== null) return;
			fadeTimer = ownerWindow.setTimeout(() => {
				fadeTimer = null;
				opacity = 0;
				update();
			}, HISTORY_SCROLLBAR_FADE_DELAY_MS);
		};
		const pointerOffset = (event: PointerEvent) =>
			event.clientY - scrollbar.getBoundingClientRect().top;
		const onPointerDown = (event: PointerEvent) => {
			if (event.button !== 0 || gesture !== null) return;
			const projection = project();
			if (projection === null) return;
			// The press neither moves focus nor selects text.
			event.preventDefault();
			const start = beginHistoryScrollbarDrag(projection, pointerOffset(event));
			gesture = { drag: start.drag, pointerId: event.pointerId };
			try {
				scrollbar.setPointerCapture(event.pointerId);
			} catch {
				// Release and cancellation still end the drag when capture is unavailable.
			}
			region.scrollTop = Number(start.targetStartRow);
			show();
		};
		const onPointerMove = (event: PointerEvent) => {
			if (gesture === null || event.pointerId !== gesture.pointerId) return;
			const projection = project();
			if (projection === null) return;
			region.scrollTop = Number(
				resolveHistoryScrollbarDrag(projection, gesture.drag, pointerOffset(event)),
			);
		};
		const finish = (event: PointerEvent) => {
			if (gesture === null || event.pointerId !== gesture.pointerId) return;
			gesture = null;
			try {
				if (scrollbar.hasPointerCapture(event.pointerId)) {
					scrollbar.releasePointerCapture(event.pointerId);
				}
			} catch {
				// The browser may already have released it.
			}
			show();
		};
		// The gutter lies over the list rather than in it, so its wheel scrolls the list here.
		const onWheel = (event: WheelEvent) => {
			let scale = 1;
			if (event.deltaMode === 1) scale = WHEEL_LINE_PIXELS;
			else if (event.deltaMode === 2) scale = region.clientHeight;
			region.scrollBy({ top: event.deltaY * scale });
		};

		update();
		region.addEventListener('scroll', show, { passive: true });
		scrollbar.addEventListener('pointerdown', onPointerDown);
		scrollbar.addEventListener('pointermove', onPointerMove);
		scrollbar.addEventListener('pointerup', finish);
		scrollbar.addEventListener('pointercancel', finish);
		scrollbar.addEventListener('lostpointercapture', finish);
		scrollbar.addEventListener('wheel', onWheel, { passive: true });
		// Folding, new rows and a resized window change what overflows without a scroll.
		const observer = typeof ResizeObserver === 'function' ? new ResizeObserver(update) : null;
		observer?.observe(region);
		for (const child of region.children) observer?.observe(child);
		return () => {
			cancelFade();
			region.removeEventListener('scroll', show);
			scrollbar.removeEventListener('pointerdown', onPointerDown);
			scrollbar.removeEventListener('pointermove', onPointerMove);
			scrollbar.removeEventListener('pointerup', finish);
			scrollbar.removeEventListener('pointercancel', finish);
			scrollbar.removeEventListener('lostpointercapture', finish);
			scrollbar.removeEventListener('wheel', onWheel);
			observer?.disconnect();
		};
	}, [regionReference]);
	return { scrollbarReference, sectionReference };
}
