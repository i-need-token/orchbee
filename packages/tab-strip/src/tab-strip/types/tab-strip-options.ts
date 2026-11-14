import type {
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
import type { TabData, TabGroupColor, TabModel } from '../model/tab-model';
import type { TabCloseSource } from '../layout/tab-closing-mode';
import type { TabStripOrientation } from './tab-strip-types';

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

/** One group of the New Tab button's menu, drawn after a separator when another group precedes it. */
export interface TabStripNewTabSection {
	readonly choices: readonly TabStripNewTabChoice[];
	readonly id: string;
	/** A heading above the choices, which is neither selectable nor focusable. */
	readonly label?: string | undefined;
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
	/**
	 * Host-owned status mark drawn in the leading identity slot in place of `glyph`. A mark is a
	 * complete drawing: the strip adds no kind color or animation to it and draws no status dot beside
	 * it, so its own tone stands. A text symbol belongs in `glyph`, which the strip colors by `kind`.
	 * When both are set, `mark` wins.
	 */
	readonly mark?: ReactNode | undefined;
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
	/** Set by the host's layout; the strip offers no way to change it. */
	orientation?: TabStripOrientation;
	/** Draws a vertical strip at its collapsed width; the strip offers no control for it. */
	verticalCollapsed?: boolean;
	hoverCardController?: TabHoverCardController;
	getHoverCardThumbnailSource?: (tab: Readonly<TabData>) => HoverCardThumbnailSource<string> | null;
	showHoverCardPreviews?: boolean;
	showHoverCardMemoryUsage?: boolean;
	/** Mirrors the feature-gated tab-group hover card; false uses native title tooltips. */
	showGroupHoverCards?: boolean;
	showNewTabButton?: boolean;
	horizontalFrameCondensed?: boolean;
	/**
	 * Mirrors ui::TouchUiController's touch-optimized tab metrics. A vertical strip's rows, group
	 * headers, and pinned tiles then take a 44 px target with a 2 px gap between rows.
	 */
	touchUi?: boolean;
	/**
	 * A vertical strip that fills its host's inline size, as a full-window Tab list does, instead of
	 * taking its own expanded or collapsed width. The host keeps it expanded.
	 */
	verticalFill?: boolean;
	/** Mirrors current Chrome tab decluttering; pass false for legacy channels. */
	tabStripDeclutter?: boolean;
	/** Host bridge for Chromium's immersive-reveal drag lock lifecycle. */
	onDraggingChange?: ((dragging: boolean) => void) | undefined;
	/** Host bridge for BrowserCommandController::TabKeyboardFocusChangedTo. */
	onTabKeyboardFocusChanged?: (tabId: string | null) => void;
	ariaLabel?: string;
	onNewTab?: ((request: TabStripNewTabRequest) => ActionExecution) | undefined;
	getTabCreationAvailability?:
		| ((intent: TabStripTabCreationIntent) => ActionAvailability)
		| undefined;
	onCloseTabs?: TabStripCloseCallback | undefined;
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
	 * The New Tab button's menu, section by section. The button's own click already starts a Tab, so
	 * the menu opens only when its sections hold two choices or more together. A section without
	 * choices is not drawn. The menu keeps the choices it opened with until it closes.
	 */
	newTabSections?: readonly TabStripNewTabSection[] | undefined;
}
