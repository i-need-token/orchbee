import type {
	ActionConfirmation,
	ActionAvailability,
	ActionExecution,
	ActionExecutionContext,
	ActionInvocationSource,
	ActionResult,
	ActionRuntime,
	ActionScope,
} from '@orchbee/actions';
import type { ProductIconName } from '@orchbee/ui/components';
import type { ReactNode } from 'react';

import type { TabSelectionInput } from '../actions/tab-strip-action-contracts';
import type { TabHoverCardController } from '../components/tab-strip-view/components/hover-card/model/tab-hover-card-controller';
import type { HoverCardThumbnailSource } from '../components/tab-strip-view/components/hover-card/model/TabHoverCardThumbnailObserver';
import type { TabData, TabGroupColor, TabGroupData, TabModel } from '../model/tab-model';
import type { TabCloseSource } from '../layout/tab-closing-mode';
import type {
	CrossWindowDragEndReason,
	CrossWindowDragSnapshot,
} from '../dragging/cross-window-drag-runtime';
import type { DragSessionData } from '../dragging/drag-session-data';
import type { DragPoint } from '../dragging/dragging-tabs-session';
import type { TabStripComboButtonController } from '../model/TabStripComboButtonController';
import type { TabStripOrientation } from './tab-strip-types';
import type { VerticalTabStripExpandOnHoverController } from '../model/vertical-tab-strip-expand-on-hover';

export interface TabStripCrossWindowDragRuntime {
	start(): void;
	update(pointInScreen: Readonly<DragPoint>): void;
	end(reason: CrossWindowDragEndReason): void;
	subscribe?(listener: () => void): () => void;
	getSnapshot?(): Pick<CrossWindowDragSnapshot, 'targetActive'> &
		Partial<
			Pick<CrossWindowDragSnapshot, 'sourceContextId' | 'attachedContextId' | 'state' | 'endReason'>
		>;
}

export type TabStripCrossWindowDragRuntimeFactory = (
	sessionData: DragSessionData<Readonly<TabGroupData>>,
) => TabStripCrossWindowDragRuntime | null;

export interface TabStripNewTabRequest {
	readonly groupId: string | null;
	readonly index?: number | undefined;
	readonly source: ActionInvocationSource;
}

export type TabStripTabCreationIntent =
	| {
			readonly groupId: string | null;
			readonly index?: number | undefined;
			readonly kind: 'new-tab';
	  }
	| { readonly kind: 'new-group' }
	| { readonly kind: 'duplicate'; readonly tabIds: readonly string[] };

interface TabStripCloseRequest {
	readonly closeSource: TabCloseSource;
	readonly tabIds: readonly string[];
}

export type TabStripCloseCallback = (
	context: ActionExecutionContext,
	request: TabStripCloseRequest,
) => ActionResult<void> | Promise<ActionResult<void>>;

export type TabStripCloseConfirmation = (
	request: TabStripCloseRequest,
) => ActionConfirmation | null;

export type TabStripSelectCallback = (
	context: ActionExecutionContext,
	input: TabSelectionInput,
) => ActionResult<void> | Promise<ActionResult<void>>;

/** How Tabs are arranged, in the words an authority that decides them for every Client can apply. */
export type TabStripArrangementChange =
	| { readonly direction: -1 | 1; readonly groupId: string; readonly kind: 'group-moved' }
	| { readonly groupId: string; readonly kind: 'group-ungrouped' }
	| {
			readonly color?: TabGroupColor | undefined;
			readonly groupId: string;
			readonly kind: 'group-updated';
			readonly title?: string | undefined;
	  }
	| {
			/** Moving onto a collapsed group goes past it, because none of its members are showing. */
			readonly adjacentGroupCollapsed: boolean;
			readonly direction: 'first' | 'last' | 'next' | 'previous';
			readonly kind: 'tab-moved';
			readonly tabId: string;
	  }
	| {
			/** A group that was dragged whole keeps its identity, so null never dissolves it. */
			readonly groupId: string | null;
			/** A boundary in `sourceOrder`, so it says nothing about any later order. */
			readonly insertIndex: number;
			readonly kind: 'tabs-dragged';
			readonly sourceOrder: readonly string[];
			readonly tabIds: readonly string[];
	  }
	| {
			readonly groupId: string | null;
			readonly kind: 'tabs-group-set';
			readonly tabIds: readonly string[];
	  }
	| {
			readonly color: TabGroupColor;
			readonly kind: 'tabs-grouped';
			readonly tabIds: readonly string[];
			/** Blank leaves the group unnamed. */
			readonly title: string;
	  }
	| {
			readonly kind: 'tabs-pinned-set';
			readonly pinned: boolean;
			readonly tabIds: readonly string[];
	  };

export type TabStripChange =
	| TabStripArrangementChange
	| { readonly kind: 'tab-created-in-new-group' }
	| { readonly kind: 'tabs-duplicated'; readonly tabIds: readonly string[] };

export type TabStripChangeCallback = (
	context: ActionExecutionContext,
	change: TabStripChange,
) => ActionResult<void> | Promise<ActionResult<void>>;

/** One way the New Tab button's menu offers to start a Tab, such as a saved launch profile. */
export interface TabStripNewTabChoice {
	readonly icon: ProductIconName;
	readonly id: string;
	readonly label: string;
	/** Trailing words, such as the mark on the choice the button's own click already makes. */
	readonly note?: string | undefined;
	readonly select: () => void;
}

/**
 * New group… and a group's Rename ask the composing application for a name, prefilled with `title`.
 * `commit` runs the Tab Strip Action with the name as typed, blank included; a cancelled request
 * never calls it.
 */
export interface TabStripGroupNameRequest {
	readonly commit: (title: string) => void;
	readonly kind: 'create' | 'rename';
	/** The group being renamed, or the Tabs a new group would hold, so a request whose target leaves can end. */
	readonly target:
		| { readonly groupId: string; readonly kind: 'group' }
		| { readonly kind: 'tabs'; readonly tabIds: readonly string[] };
	readonly title: string;
}

export type TabStatusAdornmentKind =
	| 'active'
	| 'attention'
	| 'success'
	| 'error'
	| 'degraded'
	| 'idle';

export interface TabStatusAdornment {
	/** Host-owned semantic symbol; when present it occupies the leading identity slot. */
	readonly glyph?: string | undefined;
	/** Independent trailing unread indicator; copy is supplied by the host. */
	readonly unreadLabel?: string | undefined;
	readonly accessibleLabel: string;
	readonly icon: ReactNode;
	readonly kind: TabStatusAdornmentKind;
}

export interface TabStripOptions {
	actionRuntime?: ActionRuntime | undefined;
	/** Monotonic host request to focus the current roving Tab Strip entry. */
	focusRequest?: number | undefined;
	/** Host-owned stable identity icon keyed by Tab identity. */
	identityIcons?: ReadonlyMap<string, ReactNode> | undefined;
	parentActionScope?: ActionScope | undefined;
	model: TabModel;
	/** Host-owned semantic status keyed by Tab identity. */
	statusAdornments?: ReadonlyMap<string, Readonly<TabStatusAdornment>> | undefined;
	orientation?: TabStripOrientation;
	onOrientationChange?: (orientation: TabStripOrientation) => void;
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
	verticalTabSearchOpen?: boolean;
	/** Host-owned controller; TabStrip never destroys an injected instance. */
	verticalComboButtonController?: TabStripComboButtonController;
	verticalToolbarHeight?: number;
	verticalCaptionButtonWidth?: number;
	verticalIsExitingExpandOnHover?: boolean;
	onVerticalCollapsedChange?: (collapsed: boolean) => void;
	onVerticalTabSearchClick?: () => void;
	onVerticalExpandOnHoverChange?: (enabled: boolean) => void;
	showNewTabButton?: boolean;
	horizontalFrameCondensed?: boolean;
	/** Mirrors ui::TouchUiController's touch-optimized tab metrics. */
	touchUi?: boolean;
	/** Mirrors current Chrome tab decluttering; pass false for legacy channels. */
	tabStripDeclutter?: boolean;
	/** Connects the DOM pointer lifecycle to a host-owned cross-window drag runtime. */
	createCrossWindowDragRuntime?: TabStripCrossWindowDragRuntimeFactory | undefined;
	/** Host bridge for Chromium's immersive-reveal drag lock lifecycle. */
	onDraggingChange?: (dragging: boolean) => void;
	/** Host bridge for BrowserCommandController::TabKeyboardFocusChangedTo. */
	onTabKeyboardFocusChanged?: (tabId: string | null) => void;
	/** Host-backed TabStripModel context commands. */
	onMoveTabsToNewWindow?: (tabIds: readonly string[]) => void;
	onMoveGroupToNewWindow?: (groupId: string) => void;
	ariaLabel?: string;
	onNewTab?: ((request: TabStripNewTabRequest) => ActionExecution) | undefined;
	getTabCreationAvailability?:
		| ((intent: TabStripTabCreationIntent) => ActionAvailability)
		| undefined;
	onCloseTabs?: TabStripCloseCallback | undefined;
	getCloseTabsConfirmation?: TabStripCloseConfirmation | undefined;
	onSelectTab?: TabStripSelectCallback | undefined;
	/**
	 * Present when another authority decides this collection. The strip then says what the person
	 * asked for instead of changing its model, and shows the answer as a new `model`. A dragged
	 * arrangement stays on screen until the result settles, and returns unless it completed.
	 */
	onTabsChange?: TabStripChangeCallback | undefined;
	onSetTabTitle?: ((tabId: string) => void) | undefined;
	/** Absent, New group… and a group's Rename are not offered. */
	onNameGroup?: ((request: TabStripGroupNameRequest) => void) | undefined;
	onNewTabButtonPressed?: () => void;
	/**
	 * The New Tab button's menu, in order. The button's own click already starts a Tab, so fewer than
	 * two choices open no menu.
	 */
	newTabChoices?: readonly TabStripNewTabChoice[] | undefined;
}
