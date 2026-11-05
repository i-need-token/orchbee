/**
 * The window width below which the sidebar steps aside: the default 250 px sidebar plus about 60
 * terminal columns, as the TUI hides its sidebar when fewer than 60 columns would remain.
 */
export const WORKBENCH_NARROW_WINDOW_QUERY = '(width < 731px)';

export interface WorkbenchSidebarPresentationState {
	/** The person's docked choice, kept and persisted while the window is wide. */
	readonly docked: boolean;
	readonly narrow: boolean;
	/** Whether the sidebar is open over the content; only a narrow window has the overlay. */
	readonly overlayOpen: boolean;
}

export type WorkbenchSidebarPresentationEvent =
	| { readonly narrow: boolean; readonly type: 'window-resized' }
	| { readonly type: 'overlay-dismissed' }
	| { readonly type: 'toggled' };

export type WorkbenchSidebarPresentation = 'docked' | 'hidden' | 'overlay';

/**
 * A wide window docks or hides the sidebar as the person chose. A narrow one hides it on its own
 * and the toggle opens it over the content instead; Esc, a click outside or choosing a row
 * dismisses that overlay, and widening docks the sidebar again as it was left.
 */
export function transitionWorkbenchSidebarPresentation(
	state: WorkbenchSidebarPresentationState,
	event: WorkbenchSidebarPresentationEvent,
): WorkbenchSidebarPresentationState {
	switch (event.type) {
		case 'toggled':
			return state.narrow
				? { ...state, overlayOpen: !state.overlayOpen }
				: { ...state, docked: !state.docked };
		case 'overlay-dismissed':
			return state.overlayOpen ? { ...state, overlayOpen: false } : state;
		case 'window-resized':
			return state.narrow === event.narrow
				? state
				: { ...state, narrow: event.narrow, overlayOpen: false };
	}
}

export function resolveWorkbenchSidebarPresentation(
	state: WorkbenchSidebarPresentationState,
): WorkbenchSidebarPresentation {
	if (state.narrow) return state.overlayOpen ? 'overlay' : 'hidden';
	return state.docked ? 'docked' : 'hidden';
}
