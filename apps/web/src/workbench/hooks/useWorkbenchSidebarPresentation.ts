import { useMemoizedFn } from '@orchbee/foundation/react';
import * as React from 'react';

import {
	readWorkbenchSidebarVisibility,
	writeWorkbenchSidebarVisibility,
	type WorkbenchLayoutStorage,
} from '../../workbench-layout-storage';
import {
	resolveWorkbenchSidebarPresentation,
	resolveWorkbenchWindowTier,
	transitionWorkbenchSidebarPresentation,
	WORKBENCH_MOBILE_WINDOW_QUERY,
	WORKBENCH_NARROW_WINDOW_QUERY,
	WORKBENCH_SHORT_WINDOW_QUERY,
	type WorkbenchSidebarPresentation,
} from '../model/workbench-sidebar-presentation';

export interface WorkbenchSidebarPresentationView {
	readonly dismissOverlay: () => void;
	readonly dismissTabList: () => void;
	/** The mobile layout: the bar replaces the Tab strip and the lists fill the window. */
	readonly mobile: boolean;
	readonly presentation: WorkbenchSidebarPresentation;
	readonly tabListOpen: boolean;
	readonly toggle: () => void;
	readonly toggleTabList: () => void;
	/** Touch sizes: the mobile layout with room for them, from the width alone, mouse or not. */
	readonly touch: boolean;
}

/** Window tiers come from the width and touch sizes from the height, through `matchMedia`. */
export function useWorkbenchSidebarPresentation(
	storage: WorkbenchLayoutStorage | undefined,
): WorkbenchSidebarPresentationView {
	const [state, dispatch] = React.useReducer(
		transitionWorkbenchSidebarPresentation,
		storage,
		(initialStorage) => ({
			docked: readWorkbenchSidebarVisibility(initialStorage),
			overlayOpen: false,
			tabListOpen: false,
			tier: resolveWorkbenchWindowTier({
				mobile: matchesWindowQuery(WORKBENCH_MOBILE_WINDOW_QUERY),
				narrow: matchesWindowQuery(WORKBENCH_NARROW_WINDOW_QUERY),
			}),
		}),
	);
	const [short, setShort] = React.useState(() => matchesWindowQuery(WORKBENCH_SHORT_WINDOW_QUERY));
	React.useEffect(() => {
		if (typeof window === 'undefined' || typeof window.matchMedia !== 'function') return;
		const narrow = window.matchMedia(WORKBENCH_NARROW_WINDOW_QUERY);
		const mobile = window.matchMedia(WORKBENCH_MOBILE_WINDOW_QUERY);
		const shortWindow = window.matchMedia(WORKBENCH_SHORT_WINDOW_QUERY);
		const update = () => {
			dispatch({
				tier: resolveWorkbenchWindowTier({ mobile: mobile.matches, narrow: narrow.matches }),
				type: 'window-resized',
			});
			setShort(shortWindow.matches);
		};
		update();
		const queries = [narrow, mobile, shortWindow];
		for (const query of queries) query.addEventListener('change', update);
		return () => {
			for (const query of queries) query.removeEventListener('change', update);
		};
	}, []);
	const toggle = useMemoizedFn(() => {
		// Only a wide window's choice is kept: a narrower one opens and closes the sidebar over it.
		if (state.tier === 'wide') writeWorkbenchSidebarVisibility(storage, !state.docked);
		dispatch({ type: 'toggled' });
	});
	const dismissOverlay = useMemoizedFn(() => dispatch({ type: 'overlay-dismissed' }));
	const toggleTabList = useMemoizedFn(() => dispatch({ type: 'tab-list-toggled' }));
	const dismissTabList = useMemoizedFn(() => dispatch({ type: 'tab-list-dismissed' }));
	const mobile = state.tier === 'mobile';
	return {
		dismissOverlay,
		dismissTabList,
		mobile,
		presentation: resolveWorkbenchSidebarPresentation(state),
		tabListOpen: mobile && state.tabListOpen,
		toggle,
		toggleTabList,
		touch: mobile && !short,
	};
}

function matchesWindowQuery(query: string): boolean {
	if (typeof window === 'undefined' || typeof window.matchMedia !== 'function') return false;
	return window.matchMedia(query).matches;
}
