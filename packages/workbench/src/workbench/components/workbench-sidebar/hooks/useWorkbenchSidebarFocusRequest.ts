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
		const target =
			rootReference.current?.querySelector<HTMLElement>('[aria-current="page"]') ??
			rootReference.current?.querySelector<HTMLElement>('[data-slot="workbench-sidebar-terminal"]');
		target?.focus({ preventScroll: true });
	}, [focusRequest]);
	return rootReference;
}
