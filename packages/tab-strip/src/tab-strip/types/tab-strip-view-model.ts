import type {
	Dispatch,
	MouseEvent as ReactMouseEvent,
	PointerEvent as ReactPointerEvent,
	RefObject,
	SetStateAction,
} from 'react';
import type { HoverCardThumbnailSnapshot } from '../components/tab-strip-view/components/hover-card/model/TabHoverCardThumbnailRuntime';
import type {
	TabHoverCardSnapshot,
	HoverCardUpdateType,
} from '../components/tab-strip-view/components/hover-card/model/tab-hover-card-controller';
import type { TabData, TabGroupData, TabModel, TabModelSnapshot } from '../model/tab-model';
import type { TabBoundsResult } from '../layout/calculateTabBounds';
import type { TabCloseSource } from '../layout/tab-closing-mode';
import type { Rect } from '../utils/rectRight';
import type { TabStripOrientation } from './tab-strip-types';
import type { TabGroupEditorBubbleTracker } from '../model/TabGroupEditorBubbleTracker';
import type { TabCollectionAnimationRuntime } from '../utils/TabCollectionAnimationRuntime';
import type { TabStripOptions } from './tab-strip-options';
import type { DragState, HoverState, MenuState, VisualItem } from '../utils/tab-strip-internals';
import type { TabStripActions } from './tab-strip-actions';
import type { TabStripRovingItem } from '../accessibility/tab-strip-roving-focus';

export interface TabStripViewModel {
	readonly actions: TabStripActions;
	readonly identityIcons: NonNullable<TabStripOptions['identityIcons']>;
	readonly model: TabModel;
	readonly statusAdornments: NonNullable<TabStripOptions['statusAdornments']>;
	readonly orientation: TabStripOrientation;
	readonly verticalCollapsed: boolean;
	readonly showHoverCardPreviews: boolean;
	readonly showHoverCardMemoryUsage: boolean;
	readonly showGroupHoverCards: boolean;
	readonly showNewTabButton: boolean;
	readonly horizontalFrameCondensed: boolean;
	readonly touchUi: boolean;
	readonly verticalFill: boolean;
	readonly tabStripDeclutter: boolean;
	readonly onTabKeyboardFocusChanged: TabStripOptions['onTabKeyboardFocusChanged'];
	readonly onRovingFocusChanged: (key: TabStripRovingItem['key'] | null) => void;
	readonly onRovingFocusExited: () => void;
	readonly rovingFocusKey: TabStripRovingItem['key'] | null;
	readonly onSetTabTitle: TabStripOptions['onSetTabTitle'];
	readonly onNameGroup: TabStripOptions['onNameGroup'];
	readonly newTabSections: TabStripOptions['newTabSections'];
	readonly newTabMenuAvailable: boolean;
	readonly ariaLabel: string;
	readonly snapshot: TabModelSnapshot;
	readonly verticalTabCountTransitionSuppressed: boolean;
	readonly horizontal: boolean;
	readonly stripRef: RefObject<HTMLDivElement | null>;
	readonly horizontalTrackRef: RefObject<HTMLDivElement | null>;
	readonly size: Readonly<{ width: number; height: number }>;
	readonly items: VisualItem[];
	readonly horizontalAnimationRuntimeRef: RefObject<TabCollectionAnimationRuntime | null>;
	readonly menu: MenuState | null;
	readonly setMenu: Dispatch<SetStateAction<MenuState | null>>;
	readonly hover: HoverState | null;
	readonly drag: DragState | null;
	readonly tabClosingMode: {
		width: number;
		source: Exclude<TabCloseSource, 'non-ui'>;
	} | null;
	readonly announcement: { text: string; sequence: number };
	readonly groupEditorTracker: TabGroupEditorBubbleTracker;
	readonly openGroupEditor: (id: string, x: number, y: number) => void;
	readonly hoverCardSnapshot: TabHoverCardSnapshot;
	readonly thumbnailSnapshot: HoverCardThumbnailSnapshot<string>;
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
	readonly repositionDragAfterScroll: () => void;
	readonly toggleGroupFromHeader: (
		groupId: string,
		source?: 'mouse' | 'touch' | 'keyboard',
	) => void;
	readonly onGroupKeyDown: (event: React.KeyboardEvent<HTMLDivElement>, groupId: string) => void;
	readonly onTabKeyDown: (event: React.KeyboardEvent, tabId: string) => void;
	readonly onTabKeyUp: (event: React.KeyboardEvent, tabId: string) => void;
	readonly addTab: () => void;
	readonly finishHorizontalDragSettle: () => void;
	readonly paintedHorizontalBounds: (index: number) => Rect | undefined;
	readonly settleTargetLeft: number | null;
}
