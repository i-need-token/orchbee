import { useEffect, type RefObject } from 'react';

import { wheelEnvironment, wheelScrollDelta } from '../../../../../../../../../list-scrolling';

/**
 * Scrolls the row sideways by the vertical wheel, as VS Code's tabs turn the wheel's vertical delta
 * into horizontal scrolling. A sideways wheel, a trackpad's sideways swipe and Shift with the wheel
 * scroll the row natively; Control with the wheel is a zoom, which the row leaves alone.
 */
export function useFileTabRowWheel(rowReference: RefObject<HTMLElement | null>): void {
	useEffect(() => {
		const row = rowReference.current;
		const ownerWindow = row?.ownerDocument.defaultView ?? null;
		if (row === null || ownerWindow === null) return undefined;
		const environment = wheelEnvironment(ownerWindow.navigator.userAgent);
		const onWheel = (event: WheelEvent) => {
			if (event.ctrlKey) return;
			const delta = wheelScrollDelta(
				event,
				environment,
				event.view?.devicePixelRatio ?? ownerWindow.devicePixelRatio,
			);
			if (delta === 0) return;
			event.preventDefault();
			row.scrollLeft += delta;
		};
		row.addEventListener('wheel', onWheel, { passive: false });
		return () => row.removeEventListener('wheel', onWheel);
	}, [rowReference]);
}
