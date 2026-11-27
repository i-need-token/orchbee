import * as React from 'react';

import type { TreeViewport } from '../layout/TreeViewport';
import { wheelEnvironment, wheelScrollDelta } from '../utils/wheel-delta';

/**
 * Scrolls the tree by the wheel as VS Code's lists do, and keeps every wheel event, as a list that
 * always consumes the wheel. Each event moves a target offset within the rows; the next animation
 * frame scrolls to it once, however many events arrived.
 */
export function useWheelScrolling<K, T>(
	frameReference: React.RefObject<HTMLElement | null>,
	viewport: TreeViewport<K, T>,
): void {
	React.useEffect(() => {
		const frame = frameReference.current;
		const ownerWindow = frame?.ownerDocument.defaultView;
		if (frame === null || !ownerWindow) return;
		const environment = wheelEnvironment(ownerWindow.navigator.userAgent);
		let target: number | null = null;
		let request: number | null = null;
		const scroll = () => {
			request = null;
			if (target === null) return;
			const offset = target;
			target = null;
			viewport.scrollTo(offset);
		};
		const onWheel = (event: WheelEvent) => {
			// A row's own scrolling content took it.
			if (event.defaultPrevented) return;
			const delta = wheelScrollDelta(
				event,
				environment,
				event.view?.devicePixelRatio ?? ownerWindow.devicePixelRatio,
			);
			if (delta !== 0) {
				// VS Code keeps each event's target within the rows, so a wheel past an edge turns back
				// from that edge.
				target = viewport.clampScrollTop((target ?? viewport.scrollTop) + delta);
				request ??= ownerWindow.requestAnimationFrame(scroll);
			}
			event.preventDefault();
			event.stopPropagation();
		};
		frame.addEventListener('wheel', onWheel, { passive: false });
		return () => {
			frame.removeEventListener('wheel', onWheel);
			if (request !== null) ownerWindow.cancelAnimationFrame(request);
		};
	}, [frameReference, viewport]);
}
