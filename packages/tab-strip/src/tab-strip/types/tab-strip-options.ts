import type {
	ActionConfirmation,
	ActionExecution,
	ActionExecutionContext,
	ActionInvocationSource,
	ActionResult,
	ActionRuntime,
	ActionScope,
} from '@orchbee/actions';

import type { TabSelectionInput } from '../actions/tab-strip-action-contracts';
import type { TabHoverCardController } from '../components/tab-strip-view/components/hover-card/model/tab-hover-card-controller';
import type { HoverCardThumbnailSource } from '../components/tab-strip-view/components/hover-card/model/tab-hover-card-thumbnail-observer';
import type { TabData, TabGroupData, TabModel } from '../model/tab-model';
import type { TabCloseSource } from '../layout/tab-closing-mode';
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

export interface TabStripCloseRequest {
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

export interface TabStripShortcutLabels {
	readonly closeTab?: string | undefined;
	readonly newTab?: string | undefined;
}

export interface TabStripOptions {
	actionRuntime?: ActionRuntime | undefined;
	parentActionScope?: ActionScope | undefined;
	model: TabModel;
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
	onCloseTabs?: TabStripCloseCallback | undefined;
	getCloseTabsConfirmation?: TabStripCloseConfirmation | undefined;
	onSelectTab?: TabStripSelectCallback | undefined;
	onNewTabButtonPressed?: () => void;
	/** Current platform-formatted bindings supplied by the composing application. */
	shortcutLabels?: TabStripShortcutLabels | undefined;
}
