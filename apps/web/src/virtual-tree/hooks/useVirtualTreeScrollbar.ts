import {
	beginHistoryScrollbarDrag,
	HISTORY_SCROLLBAR_FADE_DELAY_MS,
	projectHistoryScrollbar,
	resolveHistoryScrollbarDrag,
	type HistoryScrollbarDrag,
	type HistoryScrollbarProjection,
} from '@orchbee/terminal';
import * as React from 'react';

import type { TreeViewport } from '../layout/TreeViewport';

/**
 * Gives the tree the Panels' scrollbar, as the Sidebar does, counted in pixels where a Panel counts
 * rows: it shows as the tree scrolls and fades once the tree has been still for the Panels' delay; a
 * thumb press drags it and a track press jumps there. It marks the frame while rows overflow and once
 * rows have scrolled out above. It writes the thumb's geometry on the scrollbar alone, so a scroll
 * restyles nothing else.
 */
export function useVirtualTreeScrollbar<K, T>(
	frameReference: React.RefObject<HTMLElement | null>,
	scrollbarReference: React.RefObject<HTMLElement | null>,
	viewport: TreeViewport<K, T>,
): void {
	React.useEffect(() => {
		const frame = frameReference.current;
		const scrollbar = scrollbarReference.current;
		const ownerWindow = frame?.ownerDocument.defaultView;
		if (frame === null || scrollbar === null || !ownerWindow) return;
		let opacity = 0;
		let fadeTimer: number | null = null;
		let gesture: {
			readonly drag: HistoryScrollbarDrag;
			readonly pointerId: number;
			/** The track's top edge, read once when the press starts so a drag reads no layout. */
			readonly trackTop: number;
		} | null = null;
		let shownOffset = viewport.scrollTop;
		let thumbStart = '';
		let thumbSize = '';
		let thumbOpacity = '';

		const project = (): HistoryScrollbarProjection | null =>
			projectHistoryScrollbar({
				opacity,
				totalRows: BigInt(viewport.content.height),
				viewportHeight: viewport.viewportHeight,
				viewportWidth: viewport.viewportWidth,
				visibleRows: viewport.viewportHeight,
				visibleStartRow: BigInt(viewport.scrollTop),
			});
		const update = () => {
			frame.toggleAttribute('data-scrolled-start', viewport.scrollTop > 0);
			const projection = project();
			frame.toggleAttribute('data-overflowing', projection !== null);
			if (projection === null) return;
			const start = `${projection.thumb.y}px`;
			const size = `${projection.thumb.height}px`;
			const thumb = String(projection.thumb.opacity);
			if (start !== thumbStart) {
				thumbStart = start;
				scrollbar.style.setProperty('--ob-comp-virtual-tree-thumb-start', start);
			}
			if (size !== thumbSize) {
				thumbSize = size;
				scrollbar.style.setProperty('--ob-comp-virtual-tree-thumb-size', size);
			}
			if (thumb !== thumbOpacity) {
				thumbOpacity = thumb;
				scrollbar.style.setProperty('--ob-comp-virtual-tree-thumb-opacity', thumb);
			}
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
		// Rows and size changes move the thumb; only a scroll shows it.
		const onViewportChange = () => {
			if (viewport.scrollTop === shownOffset) {
				update();
				return;
			}
			shownOffset = viewport.scrollTop;
			show();
		};
		const onPointerDown = (event: PointerEvent) => {
			if (event.button !== 0 || gesture !== null) return;
			const projection = project();
			if (projection === null) return;
			// The press neither moves focus nor selects text.
			event.preventDefault();
			const trackTop = scrollbar.getBoundingClientRect().top;
			const start = beginHistoryScrollbarDrag(projection, event.clientY - trackTop);
			gesture = { drag: start.drag, pointerId: event.pointerId, trackTop };
			try {
				scrollbar.setPointerCapture(event.pointerId);
			} catch {
				// Release and cancellation still end the drag when capture is unavailable.
			}
			viewport.scrollTo(Number(start.targetStartRow));
			show();
		};
		const onPointerMove = (event: PointerEvent) => {
			if (gesture === null || event.pointerId !== gesture.pointerId) return;
			const projection = project();
			if (projection === null) return;
			const offset = event.clientY - gesture.trackTop;
			viewport.scrollTo(Number(resolveHistoryScrollbarDrag(projection, gesture.drag, offset)));
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

		update();
		const unsubscribe = viewport.subscribe(onViewportChange);
		scrollbar.addEventListener('pointerdown', onPointerDown);
		scrollbar.addEventListener('pointermove', onPointerMove);
		scrollbar.addEventListener('pointerup', finish);
		scrollbar.addEventListener('pointercancel', finish);
		scrollbar.addEventListener('lostpointercapture', finish);
		return () => {
			cancelFade();
			unsubscribe();
			scrollbar.removeEventListener('pointerdown', onPointerDown);
			scrollbar.removeEventListener('pointermove', onPointerMove);
			scrollbar.removeEventListener('pointerup', finish);
			scrollbar.removeEventListener('pointercancel', finish);
			scrollbar.removeEventListener('lostpointercapture', finish);
		};
	}, [frameReference, scrollbarReference, viewport]);
}
