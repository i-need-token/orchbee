import { LAYOUT_CONSTANTS } from './layout-constants';

export const TAB_BOTTOM_CORNER_RADIUS = 10;
export const HORIZONTAL_TAB_TOP_INSET = 2;
export const TAB_SURFACE_HORIZONTAL_INSET = 10;
export const TAB_SEPARATOR_WIDTH = 2;
export const TAB_SEPARATOR_HEIGHT = 16;
export const TAB_SEPARATOR_HORIZONTAL_MARGIN = 2;
export const TAB_HORIZONTAL_CONTENT_PADDING = 8;
export const TAB_CONTENT_HORIZONTAL_INSET =
	TAB_BOTTOM_CORNER_RADIUS + TAB_HORIZONTAL_CONTENT_PADDING;
export const TAB_STRIP_DECLUTTER_MIN_TABS_FOR_SEPARATOR_HIDE = 20;

/** Port of TabStyleViewsImpl::GetSeparatorBounds at scale 1. */
export function getHorizontalTabSeparatorBounds(width: number): Readonly<{
	leading: Readonly<{ x: number; y: number; width: number; height: number }>;
	trailing: Readonly<{ x: number; y: number; width: number; height: number }>;
}> {
	const visibleHeight = LAYOUT_CONSTANTS.tabStripHeight - HORIZONTAL_TAB_TOP_INSET;
	const y = HORIZONTAL_TAB_TOP_INSET + (visibleHeight - TAB_SEPARATOR_HEIGHT) / 2;
	const leadingX = TAB_BOTTOM_CORNER_RADIUS - TAB_SEPARATOR_HORIZONTAL_MARGIN - TAB_SEPARATOR_WIDTH;
	return {
		leading: { x: leadingX, y, width: TAB_SEPARATOR_WIDTH, height: TAB_SEPARATOR_HEIGHT },
		trailing: {
			x: width - TAB_BOTTOM_CORNER_RADIUS + TAB_SEPARATOR_HORIZONTAL_MARGIN,
			y,
			width: TAB_SEPARATOR_WIDTH,
			height: TAB_SEPARATOR_HEIGHT,
		},
	};
}

export interface TabSeparatorState {
	active: boolean;
	selected: boolean;
	hovered: boolean;
	group: string | null;
}

function hasVisibleTabBackground(tab: Readonly<TabSeparatorState> | null): boolean {
	return Boolean(tab && (tab.active || tab.selected || tab.hovered));
}

/**
 * Steady-state form of TabStyleViewsImpl::GetSeparatorOpacity. CSS animates
 * between these targets using the same 200ms hover duration as Chromium.
 */
export function getHorizontalTabSeparatorTargets(input: {
	tab: Readonly<TabSeparatorState>;
	previous: Readonly<TabSeparatorState> | null;
	next: Readonly<TabSeparatorState> | null;
	tabCount: number;
	declutterEnabled?: boolean;
	trailingControlHasOpaqueBackground?: boolean;
}): Readonly<{ leading: number; trailing: number }> {
	if (
		(input.declutterEnabled ?? false) &&
		input.tabCount >= TAB_STRIP_DECLUTTER_MIN_TABS_FOR_SEPARATOR_HIDE
	) {
		return { leading: 0, trailing: 0 };
	}
	if (hasVisibleTabBackground(input.tab)) return { leading: 0, trailing: 0 };

	// Adjacent tab bounds overlap, so rendering both sides of one boundary
	// produces two visibly separated rules. The preceding tab owns the shared
	// boundary through its trailing separator.
	const next = input.next;
	if (!next) {
		return {
			leading: 0,
			trailing: input.trailingControlHasOpaqueBackground ? 0 : 1,
		};
	}
	if (hasVisibleTabBackground(next)) return { leading: 0, trailing: 0 };
	return { leading: 0, trailing: 1 };
}
