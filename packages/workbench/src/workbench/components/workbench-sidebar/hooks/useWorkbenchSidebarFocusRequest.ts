import * as React from 'react';

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
			root.querySelector<HTMLElement>('[data-slot="workbench-sidebar-project"][data-active]') ??
			root.querySelector<HTMLElement>('[data-slot="workbench-sidebar-terminal"]');
		if (target === null) return;
		target.focus({ preventScroll: true });
		const scrollOwner = target.closest<HTMLElement>('[data-navigation-reorder-scroll]');
		if (scrollOwner === null || !root.contains(scrollOwner) || !scrollOwner.contains(target))
			return;
		const viewportHeight = scrollOwner.clientHeight;
		const targetBounds = target.getBoundingClientRect();
		if (viewportHeight <= 0 || targetBounds.height <= 0) return;
		const viewportStart = scrollOwner.getBoundingClientRect().top + scrollOwner.clientTop;
		const startDelta = targetBounds.top - viewportStart;
		const endDelta = targetBounds.bottom - (viewportStart + viewportHeight);
		if ((startDelta >= 0 && endDelta <= 0) || (startDelta <= 0 && endDelta >= 0)) return;
		const delta = Math.abs(startDelta) < Math.abs(endDelta) ? startDelta : endDelta;
		scrollOwner.scrollTop = Math.max(
			0,
			Math.min(scrollOwner.scrollTop + delta, scrollOwner.scrollHeight - viewportHeight),
		);
	}, [focusRequest]);
	return rootReference;
}
