export {
	SHARED_GROUP_AVATAR_CIRCLE_SIZE,
	SHARED_GROUP_AVATAR_ICON_SIZE,
	SHARED_GROUP_AVATAR_TEXT_SIZE,
	SHARED_GROUP_AVATAR_OVERLAP,
	SHARED_GROUP_AVATAR_MAX_VISIBLE,
	getSharedGroupAvatarSlots,
	getSharedGroupAvatarPileWidth,
	type SharedGroupAvatarSlot,
} from './avatar-container';
export {
	RECENT_ACTIVITY_MAX_ROWS,
	RECENT_ACTIVITY_BUBBLE_CONTENT_MARGIN,
	getRecentActivitySections,
	isRecentActivityActionEnabled,
	getRecentActivityDescription,
	type RecentActivitySections,
} from './recent-activity';
export {
	TAB_GROUP_EDITOR_IGNORE_SCROLL_MS,
	TabGroupEditorBubbleTracker,
	type TabGroupEditorCloseReason,
} from './tab-group-editor-bubble-tracker';
export {
	TAB_GROUP_HEADER_CHIP_VERTICAL_INSET,
	TAB_GROUP_HEADER_TITLE_ADJUSTMENT,
	TAB_GROUP_HEADER_SYNC_ICON_WIDTH,
	TAB_GROUP_HEADER_ATTENTION_WIDTH,
	TAB_GROUP_HEADER_EMPTY_CHIP_SIZE,
	TAB_GROUP_HEADER_CORNER_RADIUS,
	TAB_GROUP_OVERLAP_ADJUSTMENT,
	TAB_GROUP_SYNC_ICON_PADDING,
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
} from './tab-group-style';
