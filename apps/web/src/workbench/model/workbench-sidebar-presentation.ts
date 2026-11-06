/**
 * The window width below which the sidebar steps aside: the default 250 px sidebar plus about 60
 * terminal columns, as the TUI hides its sidebar when fewer than 60 columns would remain.
 */
export const WORKBENCH_NARROW_WINDOW_QUERY = '(width < 731px)';

/** The window width below which the mobile layout applies: the bar and the full-window lists. */
export const WORKBENCH_MOBILE_WINDOW_QUERY = '(width < 481px)';

/**
 * The window height below which the mobile layout keeps today's row and bar sizes instead of touch
 * sizes: 17 rows of 24 px, as the TUI keeps its spacing from 17 rows.
 */
export const WORKBENCH_SHORT_WINDOW_QUERY = '(height < 408px)';

/** Wide docks the sidebar, narrow lays it over the content, mobile fills the window with it. */
export type WorkbenchWindowTier = 'mobile' | 'narrow' | 'wide';

export interface WorkbenchSidebarPresentationState {
	/** The person's docked choice, kept and persisted while the window is wide. */
	readonly docked: boolean;
	/** Whether the sidebar is open over the content or, in the mobile layout, over the window. */
	readonly overlayOpen: boolean;
	/** Whether the mobile layout's Tab list fills the window. */
	readonly tabListOpen: boolean;
	readonly tier: WorkbenchWindowTier;
}

export type WorkbenchSidebarPresentationEvent =
	| { readonly tier: WorkbenchWindowTier; readonly type: 'window-resized' }
	| { readonly type: 'overlay-dismissed' }
	| { readonly type: 'tab-list-dismissed' }
	| { readonly type: 'tab-list-toggled' }
	| { readonly type: 'toggled' };

export type WorkbenchSidebarPresentation = 'docked' | 'fill' | 'hidden' | 'overlay';

/**
 * A wide window docks or hides the sidebar as the person chose. A narrow one hides it on its own
 * and the toggle opens it over the content instead; the mobile layout opens it over the whole
 * window. Esc, a click outside or choosing a row dismisses it. Crossing the mobile width keeps an
 * open sidebar open in the other form, and crossing the docking width closes it, so widening docks
 * the sidebar again as it was left. The Tab list belongs to the mobile layout: opening it or the
 * sidebar closes the other, and leaving the mobile layout closes it.
 */
export function transitionWorkbenchSidebarPresentation(
	state: WorkbenchSidebarPresentationState,
	event: WorkbenchSidebarPresentationEvent,
): WorkbenchSidebarPresentationState {
	switch (event.type) {
		case 'toggled':
			return state.tier === 'wide'
				? { ...state, docked: !state.docked }
				: { ...state, overlayOpen: !state.overlayOpen, tabListOpen: false };
		case 'overlay-dismissed':
			return state.overlayOpen ? { ...state, overlayOpen: false } : state;
		case 'tab-list-toggled':
			return state.tier === 'mobile'
				? { ...state, overlayOpen: false, tabListOpen: !state.tabListOpen }
				: state;
		case 'tab-list-dismissed':
			return state.tabListOpen ? { ...state, tabListOpen: false } : state;
		case 'window-resized': {
			if (state.tier === event.tier) return state;
			const crossedDocking = (state.tier === 'wide') !== (event.tier === 'wide');
			return {
				...state,
				overlayOpen: crossedDocking ? false : state.overlayOpen,
				tabListOpen: event.tier === 'mobile' && state.tabListOpen,
				tier: event.tier,
			};
		}
	}
}

export function resolveWorkbenchSidebarPresentation(
	state: WorkbenchSidebarPresentationState,
): WorkbenchSidebarPresentation {
	switch (state.tier) {
		case 'mobile':
			return state.overlayOpen && !state.tabListOpen ? 'fill' : 'hidden';
		case 'narrow':
			return state.overlayOpen ? 'overlay' : 'hidden';
		case 'wide':
			return state.docked ? 'docked' : 'hidden';
	}
}

export function resolveWorkbenchWindowTier(input: {
	readonly mobile: boolean;
	readonly narrow: boolean;
}): WorkbenchWindowTier {
	if (input.mobile) return 'mobile';
	return input.narrow ? 'narrow' : 'wide';
}
