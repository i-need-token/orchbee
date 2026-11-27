import { useLayoutEffect, type RefObject } from 'react';

import { wheelEnvironment, wheelScrollDelta } from '../../../virtual-tree';
import type { MultiDiffController } from './MultiDiffController';

/**
 * Connects the multi-diff's scrolling element to its controller: the element's size, its offset,
 * and the wheel. The wheel scrolls as the Sidebar's lists do, VS Code's list scrolling, and moves
 * the element and the editors in sight together in one frame, so a file the view scrolls through
 * never shows its editor a frame behind.
 */
export function useMultiDiffScrolling(
	scrollerReference: RefObject<HTMLElement | null>,
	controller: MultiDiffController,
): void {
	useLayoutEffect(() => {
		const element = scrollerReference.current;
		const ownerWindow = element?.ownerDocument.defaultView;
		if (element === null || element === undefined || !ownerWindow) return undefined;
		// Monaco styles its widgets under `.monaco-editor`, wherever they are put.
		const overflowWidgets = element.ownerDocument.createElement('div');
		overflowWidgets.className = 'monaco-editor';
		element.append(overflowWidgets);
		controller.attach(element, overflowWidgets);
		const resizeObserver = new ownerWindow.ResizeObserver((records) => {
			const box = records.at(-1)?.contentRect;
			if (box !== undefined) controller.setViewport(box.width, box.height);
		});
		resizeObserver.observe(element);

		const environment = wheelEnvironment(ownerWindow.navigator.userAgent);
		let target: number | null = null;
		let request: number | null = null;
		const scroll = (): void => {
			request = null;
			if (target === null) return;
			const offset = target;
			target = null;
			element.scrollTop = offset;
			controller.scrollTo(offset);
		};
		const onWheel = (event: WheelEvent): void => {
			// Content that scrolls on its own, such as an editor's hover, took it.
			if (event.defaultPrevented) return;
			const delta = wheelScrollDelta(
				event,
				environment,
				event.view?.devicePixelRatio ?? ownerWindow.devicePixelRatio,
			);
			if (delta === 0) return;
			event.preventDefault();
			target = controller.clampScrollTop((target ?? controller.scrollTop) + delta);
			request ??= ownerWindow.requestAnimationFrame(scroll);
		};
		// The scrollbar, the keyboard and the controller's own moves arrive as scroll events.
		const onScroll = (): void => {
			if (target === null) controller.scrollTo(element.scrollTop);
		};
		element.addEventListener('wheel', onWheel, { passive: false });
		element.addEventListener('scroll', onScroll, { passive: true });
		return () => {
			resizeObserver.disconnect();
			element.removeEventListener('wheel', onWheel);
			element.removeEventListener('scroll', onScroll);
			if (request !== null) ownerWindow.cancelAnimationFrame(request);
			controller.attach(null, null);
			overflowWidgets.remove();
		};
	}, [controller, scrollerReference]);
}
