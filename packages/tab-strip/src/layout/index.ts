export { TabLayoutState } from './TabLayoutState';
export {
	TabStripLayoutHelper,
	type TabSlotViewType,
	type TabSlotViewData,
	type TabStripLayoutController,
	type TabStripIdealBounds,
} from './TabStripLayoutHelper';
export { TabWidthConstraints } from './TabWidthConstraints';
export { type CollectionChildLayoutData } from './collection-layout-types';
export {
	getPinnedTabContainerDragAxes,
	calculatePinnedTabContainerLayout,
	calculatePinnedTabContainerMinimumSize,
	type PinnedChildLayoutData,
	type PinnedTabContainerLayoutInput,
} from './pinned-tab-container-layout';
export {
	EMPTY_PROPOSED_LAYOUT,
	type Size,
	type SizeBounds,
	type ChildLayout,
	type ProposedLayout,
} from './proposed-layout';
export {
	TAB_CLOSE_TOUCH_RESIZE_DELAY_MS,
	TAB_CLOSE_WATCH_VERTICAL_SLOP,
	TAB_CLOSE_WATCH_TRAILING_SLOP,
	calculateTabClosingModeWidth,
	getTabClosingModeWatchBounds,
	tabClosingModeContainsPoint,
	shouldExitTabClosingMode,
	type TabCloseSource,
	type TabClosingWidthInput,
} from './tab-closing-mode';
export {
	TabCollectionAnimationAxis,
	TabCollectionAnimationDirection,
	TabCollectionAnimatingLayoutManager,
	tabCollectionEaseInOut,
	type SourceLayoutInfo,
	type TabCollectionAnimatingLayoutDelegate,
	type AnimatedLayoutFrame,
} from './tab-collection-animating-layout-manager';
export {
	HORIZONTAL_TAB_MINIMUM_INACTIVE_WIDTH,
	HORIZONTAL_TAB_MINIMUM_CONTENTS_WIDTH_FOR_CLOSE,
	HORIZONTAL_TAB_TOUCH_MINIMUM_CONTENTS_WIDTH_FOR_CLOSE,
	HORIZONTAL_TAB_DECLUTTER_CLOSE_HIDE_WIDTH,
	HORIZONTAL_TAB_PINNED_NORMAL_RENDER_EXTRA_WIDTH,
	TAB_FAVICON_DESIGN_WIDTH,
	TAB_ALERT_DESIGN_WIDTH,
	TAB_CLOSE_DESIGN_WIDTH,
	TAB_CLOSE_TOUCH_DESIGN_WIDTH,
	TAB_AFTER_TITLE_PADDING,
	TAB_PRE_TITLE_PADDING,
	TAB_ALERT_CLOSE_PADDING_ADJUSTMENT,
	TAB_ALERT_CLOSE_PADDING_ADJUSTMENT_TOUCH,
	calculateHorizontalAlertCloseGap,
	calculateHorizontalTabChildVisibility,
	VERTICAL_TAB_HORIZONTAL_INSET,
	VERTICAL_TAB_CHILD_UNCOLLAPSED_MIN_WIDTH,
	VERTICAL_TAB_CLOSE_APPEARANCE_THRESHOLD,
	calculateVerticalTabChildVisibility,
	type TabChildVisibility,
	type HorizontalTabChildVisibilityInput,
	type VerticalTabChildVisibilityInput,
} from './tab-content-layout';
export {
	VERTICAL_TAB_GROUP_COLLAPSED_HEIGHT,
	GROUP_TAB_LEADING_PADDING,
	calculateTabGroupViewLayout,
	calculateTabGroupViewMinimumSize,
	type TabGroupViewLayoutInput,
} from './tab-group-view-layout';
export { LayoutDomain, type TabSizeInfo } from './tab-strip-layout-types';
export { TabSizer, calculateTabBounds, type TabBoundsResult } from './tab-strip-layout';
export {
	getTabStripViewTargetBounds,
	getTabStripView,
	type TabStripHierarchyNodeKind,
	type TabStripHierarchyNode,
} from './tab-strip-utils';
export {
	calculatePinnedContainerMainAxisSize,
	calculateTabStripViewLayout,
	type TabStripRegionLayout,
	type TabStripViewLayoutInput,
} from './tab-strip-view-layout';
export {
	calculateUnpinnedTabContainerLayout,
	calculateUnpinnedTabContainerMinimumSize,
	type UnpinnedTabContainerLayoutInput,
} from './unpinned-tab-container-view-layout';
export {
	TAB_STYLE_MAXIMUM_Z_VALUE,
	TAB_Z_VALUE_RESERVED_BITS,
	ACTIVE_TAB_Z_VALUE_BIT,
	GROUP_VIEW_Z_VALUE_BIT,
	getTabStyleZValue,
	calculateTabContainerZValue,
	compareTabContainerZValues,
	type TabContainerElementKind,
	type TabContainerZValueInput,
} from './z-orderable-tab-container-element';
