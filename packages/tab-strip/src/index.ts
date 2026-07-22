export {
	GROUP_HOVER_CARD_MAX_TABS,
	getGroupContentString,
	getGroupTooltipText,
	getGroupAccessibleName,
	getAccessibleTabLabel,
	getTabMoveAnnouncement,
	getTabGroupMembershipAnnouncement,
	type AccessibleTabContext,
	type TabMoveDirection,
} from './accessibility';
export {
	TabStripOrientation,
	EndDragReason,
	CloseTabSource,
	ToggleTabGroupCollapsedStateOrigin,
	VerticalTabStripCollapseState,
} from './common';
export {
	CrossWindowDragState,
	CrossWindowTabDragRuntime,
	type CrossWindowDragEndReason,
	type CrossWindowDragHost,
	type CrossWindowDragSnapshot,
} from './dragging';
export {
	DragSlotViewType,
	DragSessionData,
	type GroupHeaderDragData,
	type TabDragGroupData,
	type TabDragData,
	type DragSessionDataInit,
} from './dragging';
export {
	DRAGGED_VIEW_VERTICAL_PADDING,
	DRAGGED_VIEW_HORIZONTAL_PADDING,
	DRAG_START_ANIMATION_DURATION_MS,
	getSourceViewOffsetFromPointer,
	buildDraggedTabsLayout,
	getDraggingViewsBoundsAtPoint,
	hasMinimumOverlap,
	shouldEnterVerticalTabGroup,
	shouldRemainInVerticalTabGroup,
	getTargetBeforeDraggedTabs,
	getAnimatedDraggedViewPosition,
	type DraggedTabsAxes,
	type DraggedTabsLayoutMode,
	type DraggedLayoutView,
	type DraggedViewVisualData,
	type DraggedTabsLayoutResult,
	type DragTargetLayoutChild,
} from './dragging';
export {
	areDraggedTabsConsecutive,
	calculateDragMouseOffset,
	calculateAttachedDragPoint,
	calculateDraggedViewBounds,
	calculateHorizontalMoveThreshold,
	shouldReorderDraggedTabs,
	calculateGroupForDraggedTabs,
	dragContainsGroupHeader,
	type DragPoint,
	type DragGroupCalculationInput,
} from './dragging';
export {
	getHorizontalGroupDragActiveSurface,
	createHorizontalGroupDragState,
	transitionHorizontalGroupDrag,
	getHorizontalGroupDragInvariantViolations,
	type HorizontalGroupDragTab,
	type HorizontalGroupDragGroup,
	type HorizontalGroupDragAuthoritativeState,
	type HorizontalGroupDragPhase,
	type HorizontalGroupDragState,
	type HorizontalGroupDragActiveSurface,
	type HorizontalGroupDragEvent,
	type HorizontalGroupDragCommand,
	type HorizontalGroupDragTransitionResult,
} from './dragging';
export {
	createArmedHorizontalGroupDrag,
	assertHorizontalGroupDragRefinement,
	applyHorizontalGroupDragCommands,
} from './dragging';
export {
	type TabDragPositioningDelegateBase,
	type TabDragPositioningDelegate,
	type TabDragContext,
} from './dragging';
export {
	TAB_DRAG_SCROLL_INCREMENT,
	TAB_DRAG_SCROLL_INTERVAL_MS,
	TabDragScrollHandler,
	type TabDragOverflowGradient,
	type TabDragScrollable,
	type TabDragScrollSnapshot,
} from './dragging';
export {
	type TabDragTargetController,
	type TabDragTarget,
	type TabDragPointResolver,
} from './dragging';
export {
	getHorizontalDragTarget,
	getHorizontalDragInsertionIndex,
	type DragPositionTab,
	type HorizontalDragInsertionInput,
} from './dragging';
export { rectRight, type Rect } from './geometry';
export {
	TAB_GROUP_EDITOR_IGNORE_SCROLL_MS,
	TabGroupEditorBubbleTracker,
	type TabGroupEditorCloseReason,
} from './groups';
export {
	TAB_GROUP_HEADER_CHIP_VERTICAL_INSET,
	TAB_GROUP_HEADER_TITLE_ADJUSTMENT,
	TAB_GROUP_HEADER_ATTENTION_WIDTH,
	TAB_GROUP_HEADER_EMPTY_CHIP_SIZE,
	TAB_GROUP_HEADER_CORNER_RADIUS,
	TAB_GROUP_OVERLAP_ADJUSTMENT,
	TAB_GROUP_UNDERLINE_STROKE_THICKNESS,
	getTabGroupTitleVisualCenteringOffset,
	calculateHorizontalTabGroupHeaderLayout,
	shouldHideTabGroupUnderline,
	calculateHorizontalTabGroupUnderlineBounds,
	mergeDraggedGroupUnderlineBounds,
	getTabGroupHighlightPath,
	type HorizontalTabGroupHeaderLayoutInput,
	type HorizontalTabGroupHeaderLayout,
	type HorizontalTabGroupUnderlineInput,
	type MergeDraggedGroupUnderlineInput,
} from './groups';
export {
	FIRST_STRONG_ISOLATE,
	POP_DIRECTIONAL_ISOLATE,
	ELLIPSIS,
	findImageDimensions,
	elideFilenameWithLineLengths,
	getFadeOutOpacity,
	interpolateFadeSize,
	type FilenameLineLengths,
	type FadeSize,
} from './hovercard';
export {
	HOVER_CARD_GROUP_MAX_TABS,
	formatHoverCardBytes,
	getHoverCardFooterRows,
	getHoverCardTitleData,
	getGroupHoverCardData,
	formatHoverCardDomain,
	type HoverCardTitleData,
	type GroupHoverCardData,
	type HoverCardFooterRow,
} from './hovercard';
export {
	HOVER_CARD_MINIMUM_TRIGGER_DELAY_MS,
	HOVER_CARD_MAXIMUM_TRIGGER_DELAY_MS,
	HOVER_CARD_MAX_WIDTH_ADDITIONAL_DELAY_MS,
	HOVER_CARD_IMMEDIATE_RESHOW_BUFFER_MS,
	HOVER_CARD_SLIDE_DURATION_MS,
	HoverCardUpdateType,
	getHoverCardShowDelay,
	ScopedHideHoverCardLock,
	TabHoverCardController,
	ThumbnailCaptureReadiness,
	ThumbnailWaitState,
	getThumbnailCaptureDelay,
	getThumbnailPlan,
	type HoverCardTarget,
	type TabHoverCardSnapshot,
	type ThumbnailPresentation,
	type ThumbnailPlanInput,
	type ThumbnailPlan,
} from './hovercard';
export {
	HOVER_CARD_PREVIEW_SIZE,
	TabHoverCardThumbnailObserver,
	type HoverCardThumbnailSize,
	type HoverCardThumbnailSubscription,
	type HoverCardThumbnailSource,
	type HoverCardThumbnailObserverCallback,
} from './hovercard';
export {
	HOVER_CARD_THUMBNAIL_TRANSITION_MS,
	HOVER_CARD_PREVIEW_CROSSFADE_START,
	TabHoverCardThumbnailRuntime,
	type HoverCardThumbnailVisual,
	type HoverCardThumbnailTarget,
	type HoverCardThumbnailSnapshot,
} from './hovercard';
export { TabLayoutState } from './layout';
export {
	TabStripLayoutHelper,
	type TabSlotViewType,
	type TabSlotViewData,
	type TabStripLayoutController,
	type TabStripIdealBounds,
} from './layout';
export { TabWidthConstraints } from './layout';
export { type CollectionChildLayoutData } from './layout';
export {
	getPinnedTabContainerDragAxes,
	calculatePinnedTabContainerLayout,
	calculatePinnedTabContainerMinimumSize,
	type PinnedChildLayoutData,
	type PinnedTabContainerLayoutInput,
} from './layout';
export {
	EMPTY_PROPOSED_LAYOUT,
	type Size,
	type SizeBounds,
	type ChildLayout,
	type ProposedLayout,
} from './layout';
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
} from './layout';
export {
	TabCollectionAnimationAxis,
	TabCollectionAnimationDirection,
	TabCollectionAnimatingLayoutManager,
	tabCollectionEaseInOut,
	type SourceLayoutInfo,
	type TabCollectionAnimatingLayoutDelegate,
	type AnimatedLayoutFrame,
} from './layout';
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
} from './layout';
export {
	VERTICAL_TAB_GROUP_COLLAPSED_HEIGHT,
	GROUP_TAB_LEADING_PADDING,
	calculateTabGroupViewLayout,
	calculateTabGroupViewMinimumSize,
	type TabGroupViewLayoutInput,
} from './layout';
export { LayoutDomain, type TabSizeInfo } from './layout';
export { TabSizer, calculateTabBounds, type TabBoundsResult } from './layout';
export {
	getTabStripViewTargetBounds,
	getTabStripView,
	type TabStripHierarchyNodeKind,
	type TabStripHierarchyNode,
} from './layout';
export {
	calculatePinnedContainerMainAxisSize,
	calculateTabStripViewLayout,
	type TabStripRegionLayout,
	type TabStripViewLayoutInput,
} from './layout';
export {
	calculateUnpinnedTabContainerLayout,
	calculateUnpinnedTabContainerMinimumSize,
	type UnpinnedTabContainerLayoutInput,
} from './layout';
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
} from './layout';
export { TabSlotAnimationDelegate, type TabSlotAnimationHost } from './model';
export { TabSlotViewState } from './model';
export { moveContiguousBlock, type ContiguousBlockMove } from './model';
export {
	TabCollectionNodeType,
	MemoryTabCollectionViewAdapter,
	TabCollectionNode,
	buildTabCollectionDescriptor,
	RootTabCollectionNode,
	type TabCollectionNodeDescriptor,
	type TabCollectionMemoryView,
	type TabCollectionViewAdapter,
} from './model';
export {
	calculateDraggedTabsInsertion,
	TabModel,
	type TabAlertState,
	type TabData,
	type TabGroupColor,
	type TabGroupData,
	type TabModelInit,
	type TabModelSnapshot,
	type TabDragRestoreState,
	type DetachedTabBundle,
	type DraggedTabsInsertionInput,
	type DraggedTabsInsertionResult,
} from './model';
export { TabOpen, TabPinned, TabActive, type TabGroupId } from './model';
export { TabStripObserverList, type TabStripObserver } from './shared';
export {
	ROUNDED_SCROLLBAR_THUMB_THICKNESS,
	ROUNDED_SCROLLBAR_TRAILING_PADDING,
	ROUNDED_SCROLLBAR_FADE_DELAY_MS,
	ROUNDED_SCROLLBAR_FADE_DURATION_MS,
	roundedScrollbarThickness,
	calculateRoundedScrollbarThumb,
	type RoundedScrollbarThumbGeometry,
} from './shared';
export {
	TAB_STRIP_COMBO_ANIMATION_DURATION_MS,
	TAB_SEARCH_EPHEMERAL_HIDE_DELAY_MS,
	getTabStripComboAnimationFactors,
	TabStripComboButtonController,
	type TabStripComboButtonId,
	type TabStripComboAnimationFactors,
	type TabStripComboButtonSnapshot,
	type TabStripComboButtonControllerOptions,
} from './shared';
export {
	TAB_STRIP_CONTROL_BUTTON_SIZE,
	TAB_STRIP_CONTROL_BUTTON_ICON_SIZE,
	TAB_STRIP_CONTROL_BUTTON_CORNER_RADIUS,
	TAB_STRIP_CONTROL_BUTTON_FLAT_CORNER_RADIUS,
	TabStripControlButtonEdge,
	getScaledTabStripControlButtonRadius,
	getTabStripControlButtonRadii,
	getTabStripControlButtonPreferredSize,
	type TabStripControlButtonRadii,
	type TabStripControlButtonInsets,
	type TabStripControlButtonShapeInput,
} from './shared';
export {
	TAB_STRIP_FLAT_EDGE_BUTTON_SIZE,
	TAB_STRIP_FLAT_EDGE_BUTTON_ICON_SIZE,
	TAB_STRIP_FLAT_EDGE_BUTTON_PADDING,
	TAB_STRIP_FLAT_EDGE_RADIUS,
	TAB_STRIP_ROUNDED_EDGE_RADIUS,
	TabStripFlatEdge,
	getFlatEdgeButtonCornerRadii,
	getFlatEdgeButtonPreferredSize,
	getComboButtonFlatEdges,
	type CornerRadii,
	type FlatEdgeButtonSize,
} from './shared';
export { LAYOUT_CONSTANTS } from './style';
export { DEFAULT_TAB_STYLE_METRICS, type TabStyleMetrics } from './style';
export {
	TAB_TOP_CORNER_RADIUS,
	TAB_BOTTOM_CORNER_RADIUS,
	TAB_SEPARATOR_WIDTH,
	TAB_SEPARATOR_HEIGHT,
	TAB_SEPARATOR_HORIZONTAL_MARGIN,
	TAB_HORIZONTAL_CONTENT_PADDING,
	TAB_VERTICAL_CONTENT_PADDING,
	TAB_CONTENT_HORIZONTAL_INSET,
	TAB_CONTENT_TOP_INSET,
	TAB_CONTENT_BOTTOM_INSET,
	TAB_SEPARATOR_OVERLAP,
	TAB_STRIP_DECLUTTER_MIN_TABS_FOR_SEPARATOR_HIDE,
	getHorizontalTabTopCornerRadius,
	getHorizontalTabPath,
	isPointInHorizontalTabHitTest,
	getHorizontalTabSeparatorBounds,
	getHorizontalTabSeparatorTargets,
	type HorizontalTabPathType,
	type TabSeparatorState,
} from './style';
export {
	ALERT_INDICATOR_FADE_IN_MS,
	ALERT_INDICATOR_FADE_OUT_MS,
	ALERT_INDICATOR_MINIMUM_HOLD_MS,
	ALERT_INDICATOR_FRAME_INTERVAL_MS,
	ALERT_INDICATOR_RECORDING_CYCLES,
	ALERT_INDICATOR_RECORDING_ANIMATION_MS,
	isAudioAlertState,
	isPulsingAlertState,
	requiresCameraMicMinimumHold,
	getTabAlertColorRole,
	getTabAlertStateText,
	getTabAlertAccessibleDescription,
	sampleAlertIndicatorOpacity,
	AlertIndicatorButtonController,
	type AlertIndicatorFadeKind,
	type AlertIndicatorSnapshot,
	type TabAlertColorRole,
} from './tab';
export {
	GLOW_HOVER_DEFAULT_DURATION_MS,
	VERTICAL_GLOW_HOVER_DURATION_MS,
	GLOW_HOVER_SUBTLE_OPACITY_SCALE,
	GLOW_HOVER_PRONOUNCED_OPACITY_SCALE,
	ShowHoverStyle,
	HideHoverStyle,
	sampleGlowHoverTween,
	GlowHoverController,
} from './tab';
export {
	TAB_CLOSE_ICON_SIZE,
	TAB_CLOSE_TOUCH_ICON_SIZE,
	TAB_CLOSE_BUTTON_PADDING,
	TAB_CLOSE_BUTTON_SIZE,
	TAB_CLOSE_TOUCH_BUTTON_SIZE,
	TAB_CLOSE_HOVER_OPACITY,
	TAB_CLOSE_PRESSED_OPACITY,
	getTabCloseButtonMetrics,
	isPointInTabCloseTarget,
	shouldTabCloseButtonHandlePointer,
	type TabCloseButtonMetrics,
	type TabCloseTargetInput,
} from './tab';
export {
	TAB_ICON_SIZE,
	TAB_ICON_ATTENTION_RADIUS,
	TAB_ICON_LOADING_STROKE_WIDTH,
	TAB_ICON_FAVICON_TRANSITION_MS,
	TAB_ICON_DISCARD_FADE_MS,
	TAB_ICON_CRASH_ANIMATION_MS,
	TAB_ICON_CRASH_FRAMES_PER_SECOND,
	TAB_ICON_THROBBER_ROTATION_MS,
	TAB_ICON_INCREASED_DISCARD_RADIUS,
	tabIconNetworkStateIsAnimated,
	resolveTabIconNetworkState,
	getIncreasedDiscardIndicatorRadius,
	getTabIconCrashFrame,
	getTabIconClipDiameter,
	getTabIconVisualState,
	type TabIconNetworkState,
	type TabIconVisualState,
	type TabIconCrashFrame,
} from './tab';
export { DEFAULT_TAB_TITLE, LOADING_TAB_TITLE, getVisibleTabTitle } from './tab';
export {
	getVerticalBottomContainerPresentation,
	isPositionInVerticalBottomContainerCaption,
	type VerticalBottomContainerPresentation,
} from './vertical';
export {
	ExpandOnHoverLockType,
	DEFAULT_VERTICAL_EXPAND_ON_HOVER_HEURISTIC,
	VerticalTabStripExpandOnHoverLock,
	VerticalTabStripExpandOnHoverController,
	type VerticalExpandOnHoverEnvironment,
	type VerticalExpandOnHoverSnapshot,
	type VerticalExpandOnHoverHeuristicOptions,
} from './vertical';
export {
	getVerticalTopContainerPreferredWidth,
	willVerticalTopContainerWrap,
	calculateVerticalTopContainerLayout,
	type VerticalTopContainerChildLayout,
	type VerticalTopContainerLayoutInput,
	type VerticalTopContainerLayout,
} from './vertical';
