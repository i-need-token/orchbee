import * as React from 'react';

import { revealSidebarRow } from '../utils/revealSidebarRow';

export function useWorkbenchSidebarFocusRequest(
	focusRequest: number,
): React.RefObject<HTMLElement | null> {
	const rootReference = React.useRef<HTMLElement>(null);
	const lastFocusRequestReference = React.useRef(0);
	React.useLayoutEffect(() => {
		if (focusRequest === lastFocusRequestReference.current) return;
		lastFocusRequestReference.current = focusRequest;
		if (focusRequest <= 0) return;
		const root = rootReference.current;
		if (root === null) return;
		const target =
			root.querySelector<HTMLElement>('[aria-current="page"]') ??
			root.querySelector<HTMLElement>('[data-holds-active-workspace]');
		if (target === null) return;
		target.focus({ preventScroll: true });
		revealSidebarRow(root, target);
	}, [focusRequest]);
	return rootReference;
}
