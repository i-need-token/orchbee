import type { TabHoverCardController } from '../components/tab-strip-view/components/hover-card/model/tab-hover-card-controller';
import type { HoverCardThumbnailSource } from '../components/tab-strip-view/components/hover-card/model/tab-hover-card-thumbnail-observer';
import type { TabData, TabGroupData, TabModel } from '../model/tab-model';
import type {
	CrossWindowDragEndReason,
	CrossWindowDragSnapshot,
} from '../dragging/cross-window-drag-runtime';
import type { DragSessionData } from '../dragging/drag-session-data';
import type { DragPoint } from '../dragging/dragging-tabs-session';
import type { TabStripComboButtonController } from '../model/tab-strip-combo-button';
import type { TabStripOrientation } from './tab-strip-types';
import type { VerticalTabStripExpandOnHoverController } from '../model/vertical-tab-strip-expand-on-hover';

export interface TabStripCrossWindowDragRuntime {
	start(): void;
	update(pointInScreen: Readonly<DragPoint>): void;
	end(reason: CrossWindowDragEndReason): void;
	getSnapshot?(): Pick<CrossWindowDragSnapshot, 'targetActive'> &
		Partial<Pick<CrossWindowDragSnapshot, 'sourceContextId' | 'attachedContextId' | 'state'>>;
}

export interface TabStripOptions {
	model: TabModel;
	orientation?: TabStripOrientation;
	verticalCollapsed?: boolean;
	verticalExpandOnHover?: boolean;
	verticalExpandOnHoverController?: VerticalTabStripExpandOnHoverController;
	hoverCardController?: TabHoverCardController;
	getHoverCardThumbnailSource?: (tab: Readonly<TabData>) => HoverCardThumbnailSource<string> | null;
	showHoverCardPreviews?: boolean;
	showHoverCardMemoryUsage?: boolean;
	/** Mirrors the feature-gated tab-group hover card; false uses native title tooltips. */
	showGroupHoverCards?: boolean;
	onVerticalExpandedOnHoverChange?: (expanded: boolean) => void;
	showVerticalTopControls?: boolean;
	showVerticalTabSearchButton?: boolean;
	/** Compatibility override; model.focusedGroupId is the source of truth for focused-group layout. */
	verticalGroupFocused?: boolean;
	/** Mirrors kTabGroupsFocusingPinnedTabs. Chromium's default is false. */
	showPinnedTabsInFocusedGroups?: boolean;
	verticalTabSearchOpen?: boolean;
	verticalComboButtonController?: TabStripComboButtonController;
	verticalToolbarHeight?: number;
	verticalCaptionButtonWidth?: number;
	verticalIsExitingExpandOnHover?: boolean;
	onVerticalCollapsedChange?: (collapsed: boolean) => void;
	onVerticalUnfocusGroup?: () => void;
	onVerticalTabSearchClick?: () => void;
	onVerticalExpandOnHoverChange?: (enabled: boolean) => void;
	showNewTabButton?: boolean;
	horizontalFrameCondensed?: boolean;
	/** Mirrors ui::TouchUiController's touch-optimized tab metrics. */
	touchUi?: boolean;
	/** Mirrors current Chrome tab decluttering; pass false for legacy channels. */
	tabStripDeclutter?: boolean;
	/** Connects the DOM pointer lifecycle to a host-owned cross-window drag runtime. */
	createCrossWindowDragRuntime?: (
		sessionData: DragSessionData<Readonly<TabGroupData>>,
	) => TabStripCrossWindowDragRuntime | null;
	/** Host bridge for Chromium's immersive-reveal drag lock lifecycle. */
	onDraggingChange?: (dragging: boolean) => void;
	/** Host bridge for BrowserCommandController::TabKeyboardFocusChangedTo. */
	onTabKeyboardFocusChanged?: (tabId: string | null) => void;
	/** Host-backed TabStripModel context commands. */
	onMoveTabsToNewWindow?: (tabIds: readonly string[]) => void;
	onMoveGroupToNewWindow?: (groupId: string) => void;
	/** Mirrors features::kTabGroupsFocusing in the editor bubble. */
	enableTabGroupFocusing?: boolean;
	ariaLabel?: string;
	onNewTab?: (() => void) | undefined;
	onNewTabButtonPressed?: () => void;
}
