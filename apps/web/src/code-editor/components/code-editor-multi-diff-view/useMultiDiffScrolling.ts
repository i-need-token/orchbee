import { useLayoutEffect, type RefObject } from 'react';

import { wheelEnvironment, wheelScrollDeltas } from '../../../list-scrolling';
import type { MultiDiffController } from './MultiDiffController';

interface MultiDiffScrollingOptions {
	readonly controller: MultiDiffController;
	/** The sideways scrollbar under the list. */
	readonly horizontal: RefObject<HTMLElement | null>;
	/** The view, which takes the wheel over the list and the scrollbar alike. */
	readonly root: RefObject<HTMLElement | null>;
	/** The list, which scrolls up and down. */
	readonly scroller: RefObject<HTMLElement | null>;
}

/**
 * Connects the multi-diff's scrolling elements to its controller: the list's size and offset, the
 * sideways scrollbar's offset, and the wheel. The wheel scrolls as VS Code's multi-diff does, up and
 * down as the Sidebar's lists do and sideways with a horizontal wheel or Shift, and moves the list
 * and the editors in sight together in one frame, so a file the view scrolls through never shows
 * its editor a frame behind.
 */
export function useMultiDiffScrolling({
	controller,
	horizontal,
	root,
	scroller,
}: MultiDiffScrollingOptions): void {
	useLayoutEffect(() => {
		const rootElement = root.current;
		const element = scroller.current;
		const horizontalElement = horizontal.current;
		const ownerWindow = element?.ownerDocument.defaultView;
		if (rootElement === null || element === null || horizontalElement === null || !ownerWindow) {
			return undefined;
		}
		// Monaco styles its widgets under `.monaco-editor`, wherever they are put.
		const overflowWidgets = element.ownerDocument.createElement('div');
		overflowWidgets.className = 'monaco-editor';
		element.append(overflowWidgets);
		controller.attach(element, horizontalElement, overflowWidgets);
		const resizeObserver = new ownerWindow.ResizeObserver((records) => {
			const box = records.at(-1)?.contentRect;
			if (box !== undefined) controller.setViewport(box.width, box.height);
		});
		resizeObserver.observe(element);

		const environment = wheelEnvironment(ownerWindow.navigator.userAgent);
		let target: number | null = null;
		let targetLeft: number | null = null;
		let request: number | null = null;
		const scroll = (): void => {
			request = null;
			if (targetLeft !== null) {
				const offset = targetLeft;
				targetLeft = null;
				controller.scrollLeftTo(offset);
			}
			if (target === null) return;
			const offset = target;
			target = null;
			element.scrollTop = offset;
			controller.scrollTo(offset);
		};
		const onWheel = (event: WheelEvent): void => {
			// Content that scrolls on its own, such as an editor's hover, took it.
			if (event.defaultPrevented) return;
			const delta = wheelScrollDeltas(
				event,
				environment,
				event.view?.devicePixelRatio ?? ownerWindow.devicePixelRatio,
			);
			const sideways = delta.x !== 0 && controller.maxScrollLeft > 0;
			if (delta.y === 0 && !sideways) return;
			event.preventDefault();
			if (delta.y !== 0) {
				target = controller.clampScrollTop((target ?? controller.scrollTop) + delta.y);
			}
			if (sideways) {
				targetLeft = controller.clampScrollLeft((targetLeft ?? controller.scrollLeft) + delta.x);
			}
			request ??= ownerWindow.requestAnimationFrame(scroll);
		};
		// The scrollbars, the keyboard and the controller's own moves arrive as scroll events.
		const onScroll = (): void => {
			if (target === null) controller.scrollTo(element.scrollTop);
		};
		const onHorizontalScroll = (): void => {
			if (targetLeft === null) controller.scrollLeftTo(horizontalElement.scrollLeft);
		};
		rootElement.addEventListener('wheel', onWheel, { passive: false });
		element.addEventListener('scroll', onScroll, { passive: true });
		horizontalElement.addEventListener('scroll', onHorizontalScroll, { passive: true });
		return () => {
			resizeObserver.disconnect();
			rootElement.removeEventListener('wheel', onWheel);
			element.removeEventListener('scroll', onScroll);
			horizontalElement.removeEventListener('scroll', onHorizontalScroll);
			if (request !== null) ownerWindow.cancelAnimationFrame(request);
			controller.attach(null, null, null);
			overflowWidgets.remove();
		};
	}, [controller, horizontal, root, scroller]);
}
