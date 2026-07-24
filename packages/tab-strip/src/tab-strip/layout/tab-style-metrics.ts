/**
 * The TabStyle values used by Orchbee's horizontal tab strip.
 *
 * The overlap follows Chromium's layout algorithm while the standard height
 * follows Orchbee's even-pixel visual density.
 */
export interface TabStyleMetrics {
	tabOverlap: number;
	standardHeight: number;
}

// Verified by tab_strip_layout_unittest.cc in the mirrored Chromium source.
export const DEFAULT_TAB_STYLE_METRICS: Readonly<TabStyleMetrics> = {
	tabOverlap: 18,
	standardHeight: 40,
};
