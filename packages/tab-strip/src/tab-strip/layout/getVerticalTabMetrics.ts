import { LAYOUT_CONSTANTS } from './layout-constants';

/** Block sizes of a vertical strip's units: the pitch each one advances its column by. */
export interface VerticalTabMetrics {
	/** One ungrouped, unpinned Tab row; the layout leaves 2 px between rows. */
	readonly tabHeight: number;
	/** One Tab in an expanded group, whose rows stack without the layout's gap. */
	readonly groupTabHeight: number;
	/** One pinned tile. */
	readonly pinnedTabHeight: number;
	/** The narrowest pinned tile. */
	readonly pinnedTabMinWidth: number;
	/** An expanded group before its first member's pitch. */
	readonly groupHeaderBlockSize: number;
	/** A collapsed group: its header and padding. */
	readonly collapsedGroupBlockSize: number;
}

const DEFAULT_VERTICAL_TAB_METRICS: VerticalTabMetrics = Object.freeze({
	tabHeight: LAYOUT_CONSTANTS.verticalTabHeight,
	groupTabHeight: LAYOUT_CONSTANTS.verticalTabHeight,
	pinnedTabHeight: LAYOUT_CONSTANTS.verticalTabPinnedHeight,
	pinnedTabMinWidth: LAYOUT_CONSTANTS.verticalTabMinWidth,
	groupHeaderBlockSize: 36,
	collapsedGroupBlockSize: 34,
});

// Touch rows are 44 px targets with the 2 px the layout leaves between its units. A group's header
// is one such row, and its Tabs advance 46 px to carry that gap inside the group; a pinned tile is
// a 44 px square at least.
const TOUCH_TARGET = 44;
const TOUCH_ROW_GAP = 2;
const TOUCH_VERTICAL_TAB_METRICS: VerticalTabMetrics = Object.freeze({
	tabHeight: TOUCH_TARGET,
	groupTabHeight: TOUCH_TARGET + TOUCH_ROW_GAP,
	pinnedTabHeight: TOUCH_TARGET,
	pinnedTabMinWidth: TOUCH_TARGET,
	groupHeaderBlockSize: TOUCH_TARGET,
	collapsedGroupBlockSize: TOUCH_TARGET,
});

export function getVerticalTabMetrics(touchUi: boolean): VerticalTabMetrics {
	return touchUi ? TOUCH_VERTICAL_TAB_METRICS : DEFAULT_VERTICAL_TAB_METRICS;
}
