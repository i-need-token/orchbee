/**
 * Non-touch layout constants consumed by the tabs source tree.
 * Values mirror chrome/browser/ui/layout_constants.cc.
 */
export const LAYOUT_CONSTANTS = {
	tabHeight: 35,
	tabStripHeight: 36,
	tabStripPadding: 6,
	tabstripToolbarOverlap: 1,
	verticalTabCornerRadius: 8,
	verticalTabHeight: 30,
	verticalTabPinnedHeight: 32,
	verticalTabMinWidth: 32,
	verticalTabStripHorizontalPadding: 12,
	verticalTabStripUncollapsedVerticalPadding: 12,
	verticalTabStripCollapsedVerticalPadding: 8,
	verticalTabPinnedBorderThickness: 1,
	verticalTabStripButtonIconSize: 20,
	verticalTabStripNewTabButtonSize: 32,
	verticalTabStripCollapsedWidth: 56,
	verticalTabStripUncollapsedMinWidth: 126,
	verticalTabStripUncollapsedMaxWidth: 400,
} as const;

/** The horizontal strip's + alone. */
const NEW_TAB_BUTTON_WIDTH = 28;
/** The horizontal strip's + with its New Tab menu button after it. */
export const NEW_TAB_SPLIT_BUTTON_WIDTH = 36;
const NEW_TAB_MENU_BUTTON_WIDTH = 20;
/** What that pair takes beyond + alone, which the Tabs also leave room for. */
export const NEW_TAB_MENU_EXTRA_WIDTH =
	NEW_TAB_SPLIT_BUTTON_WIDTH + NEW_TAB_MENU_BUTTON_WIDTH - NEW_TAB_BUTTON_WIDTH;
