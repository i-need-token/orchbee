/**
 * The TabStyle values used by Chromium's current horizontal tab strip.
 *
 * The C++ code reads these through the process-wide TabStyle singleton. They
 * are explicit here so alternate density/platform styles can use the same
 * layout algorithm without hiding a mutable global.
 */
export interface TabStyleMetrics {
	tabOverlap: number;
	standardHeight: number;
}

// Verified by tab_strip_layout_unittest.cc in the mirrored Chromium source.
export const DEFAULT_TAB_STYLE_METRICS: Readonly<TabStyleMetrics> = {
	tabOverlap: 18,
	standardHeight: 41,
};
