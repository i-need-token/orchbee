import type {
	Dispatch,
	MouseEvent as ReactMouseEvent,
	PointerEvent as ReactPointerEvent,
	RefObject,
	SetStateAction,
} from 'react';
import type {
	HoverCardThumbnailSnapshot,
	TabHoverCardSnapshot,
	HoverCardUpdateType,
} from '../../hovercard';
import type { TabData, TabGroupData, TabModel, TabModelSnapshot } from '../../model';
import type { AnimatedLayoutFrame, TabBoundsResult, TabCloseSource } from '../../layout';
import type { Rect } from '../../geometry';
import type { TabStripOrientation } from '../../common';
import type { TabGroupEditorBubbleTracker } from '../../groups';
import type {
	VerticalExpandOnHoverSnapshot,
	VerticalTabStripExpandOnHoverController,
} from '../../vertical';
import type { TabStripComboButtonController, TabStripComboButtonSnapshot } from '../../shared';
import type { TabCollectionAnimationRuntime } from '../utils/TabCollectionAnimationRuntime';
import type {
	DragState,
	HoverState,
	MenuState,
	TabStripProps,
	VisualItem,
} from '../utils/tab-strip-internals';

export interface TabStripViewModel {
	readonly model: TabModel;
	readonly orientation: TabStripOrientation;
	readonly verticalCollapsed: boolean;
	readonly verticalExpandOnHover: boolean;
	readonly showHoverCardPreviews: boolean;
	readonly showHoverCardMemoryUsage: boolean;
	readonly showGroupHoverCards: boolean;
	readonly showVerticalTopControls: boolean;
	readonly verticalGroupFocused: boolean;
	readonly showPinnedTabsInFocusedGroups: boolean;
	readonly verticalToolbarHeight: number;
	readonly verticalCaptionButtonWidth: number;
	readonly verticalIsExitingExpandOnHover: boolean;
	readonly onVerticalCollapsedChange: TabStripProps['onVerticalCollapsedChange'];
	readonly onVerticalUnfocusGroup: TabStripProps['onVerticalUnfocusGroup'];
	readonly onVerticalTabSearchClick: TabStripProps['onVerticalTabSearchClick'];
	readonly onVerticalExpandOnHoverChange: TabStripProps['onVerticalExpandOnHoverChange'];
	readonly focusedGroupColor: string | undefined;
	readonly showNewTabButton: boolean;
	readonly horizontalFrameCondensed: boolean;
	readonly touchUi: boolean;
	readonly tabStripDeclutter: boolean;
	readonly onTabKeyboardFocusChanged: TabStripProps['onTabKeyboardFocusChanged'];
	readonly onMoveTabsToNewWindow: TabStripProps['onMoveTabsToNewWindow'];
	readonly onMoveGroupToNewWindow: TabStripProps['onMoveGroupToNewWindow'];
	readonly enableTabGroupFocusing: boolean;
	readonly ariaLabel: string;
	readonly snapshot: TabModelSnapshot;
	readonly horizontal: boolean;
	readonly focusedGroup: Readonly<TabGroupData> | null;
	readonly groupFocusActive: boolean;
	readonly tabVisibleWhileFocused: (tab: Readonly<TabData>) => boolean;
	readonly stripRef: RefObject<HTMLDivElement | null>;
	readonly size: Readonly<{ width: number; height: number }>;
	readonly items: VisualItem[];
	readonly horizontalAnimationRuntimeRef: RefObject<TabCollectionAnimationRuntime | null>;
	readonly animatedHorizontalFrame: AnimatedLayoutFrame | null;
	readonly menu: MenuState | null;
	readonly setMenu: Dispatch<SetStateAction<MenuState | null>>;
	readonly hover: HoverState | null;
	readonly drag: DragState | null;
	readonly closingTabIds: ReadonlySet<string>;
	readonly fullyCollapsedGroupIds: ReadonlySet<string>;
	readonly tabClosingMode: {
		width: number;
		source: Exclude<TabCloseSource, 'non-ui'>;
	} | null;
	readonly announcement: { text: string; sequence: number };
	readonly groupEditorTracker: TabGroupEditorBubbleTracker;
	readonly openGroupEditor: (id: string, x: number, y: number) => void;
	readonly hoverCardSnapshot: TabHoverCardSnapshot;
	readonly thumbnailSnapshot: HoverCardThumbnailSnapshot<string>;
	readonly comboButtonController: TabStripComboButtonController;
	readonly comboButtonSnapshot: TabStripComboButtonSnapshot;
	readonly expandOnHoverController: VerticalTabStripExpandOnHoverController;
	readonly expandOnHoverSnapshot: VerticalExpandOnHoverSnapshot;
	readonly expandedOnHover: boolean;
	readonly effectiveVerticalCollapsed: boolean;
	readonly requestCloseTabs: (tabIds: readonly string[], source?: TabCloseSource) => void;
	readonly expandedHeaderDragGroupId: string | null;
	readonly horizontalLayout: TabBoundsResult | null;
	readonly clearHover: (updateType?: HoverCardUpdateType) => void;
	readonly updateHoverForElement: (
		element: HTMLElement,
		id: string,
		updateType: HoverCardUpdateType,
		kind?: HoverState['kind'],
	) => void;
	readonly onTabMouseEnter: (event: ReactMouseEvent<HTMLDivElement>, tabId: string) => void;
	readonly onGroupMouseEnter: (event: ReactMouseEvent<HTMLDivElement>, groupId: string) => void;
	readonly onTabPointerDown: (
		event: ReactPointerEvent<HTMLDivElement>,
		tab: Readonly<TabData>,
	) => void;
	readonly onGroupPointerDown: (
		event: ReactPointerEvent<HTMLDivElement>,
		group: Readonly<TabGroupData>,
	) => void;
	readonly toggleGroupFromHeader: (
		groupId: string,
		source?: 'mouse' | 'touch' | 'keyboard',
	) => void;
	readonly onGroupKeyDown: (event: React.KeyboardEvent<HTMLDivElement>, groupId: string) => void;
	readonly onTabKeyDown: (event: React.KeyboardEvent, tabId: string) => void;
	readonly onTabKeyUp: (event: React.KeyboardEvent, tabId: string) => void;
	readonly addTab: () => void;
	readonly paintedHorizontalBounds: (index: number) => Rect | undefined;
	readonly settleTargetLeft: number | null;
}
