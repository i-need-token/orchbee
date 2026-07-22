import {
	requireDefined,
	TabStripOrientation,
	VerticalTabStripCollapseState,
} from '../../../../common';
import {
	type ChangeEvent as ReactChangeEvent,
	type CSSProperties,
	type DragEvent as ReactDragEvent,
	type FocusEvent as ReactFocusEvent,
	type KeyboardEvent as ReactKeyboardEvent,
	type MouseEvent as ReactMouseEvent,
	type PointerEvent as ReactPointerEvent,
	type ReactNode,
	useCallback,
	useEffect,
	useMemo,
	useRef,
	useState,
	useSyncExternalStore,
} from 'react';
import { createPortal } from 'react-dom';
import {
	HoverCardUpdateType,
	TabHoverCardController,
	ThumbnailWaitState,
	type HoverCardThumbnailSource,
	TabHoverCardThumbnailRuntime,
	type HoverCardThumbnailSnapshot,
	type HoverCardThumbnailVisual,
	elideFilenameWithLineLengths,
	formatHoverCardDomain,
	getGroupHoverCardData,
	getHoverCardFooterRows,
	getHoverCardTitleData,
} from '../../../../hovercard';
import {
	TabActive,
	TabOpen,
	TabPinned,
	type TabSlotViewState,
	type TabModel,
	type TabGroupActivity,
	type TabData,
	type TabDragRestoreState,
	type TabGroupData,
	type TabModelSnapshot,
	type TabGroupColor,
	HoverTabSelector,
} from '../../../../model';
import {
	calculateGroupForDraggedTabs,
	calculateHorizontalMoveThreshold,
	type DragPoint,
	transitionHorizontalGroupDrag,
	type HorizontalGroupDragState,
	applyHorizontalGroupDragCommands,
	assertHorizontalGroupDragRefinement,
	createArmedHorizontalGroupDrag,
	getHorizontalDragTarget,
	DragSessionData,
	DragSlotViewType,
	type TabDragData,
	type CrossWindowDragEndReason,
	type CrossWindowDragSnapshot,
	buildDraggedTabsLayout,
	getDraggingViewsBoundsAtPoint,
	getTargetBeforeDraggedTabs,
	shouldEnterVerticalTabGroup,
	shouldRemainInVerticalTabGroup,
	DragPositionHint,
	getHorizontalLinkDropIndexForTab,
	getLinkDropIndexForNode,
	type TabLinkDropNode,
	createDomTabDragScrollable,
	TabDragScrollHandler,
} from '../../../../dragging';
import {
	TabLayoutState,
	calculateTabBounds,
	VERTICAL_TAB_GROUP_COLLAPSED_HEIGHT,
	tabCollectionEaseInOut,
	type AnimatedLayoutFrame,
	calculateSplitTabViewLayout,
	calculatePinnedTabContainerLayout,
	calculatePinnedTabContainerMinimumSize,
	getPinnedTabContainerDragAxes,
	calculateTabStripViewLayout,
	calculateUnpinnedTabContainerLayout,
	calculateUnpinnedTabContainerMinimumSize,
	calculateTabContainerZValue,
	calculateHorizontalAlertCloseGap,
	calculateHorizontalTabChildVisibility,
	calculateVerticalTabChildVisibility,
	TAB_AFTER_TITLE_PADDING,
	TAB_ALERT_DESIGN_WIDTH,
	TAB_CLOSE_DESIGN_WIDTH,
	calculateTabClosingModeWidth,
	getTabClosingModeWatchBounds,
	shouldExitTabClosingMode,
	TAB_CLOSE_TOUCH_RESIZE_DELAY_MS,
	tabClosingModeContainsPoint,
	type TabCloseSource,
	type TabSizeInfo,
	TabWidthConstraints,
} from '../../../../layout';
import {
	LAYOUT_CONSTANTS,
	DEFAULT_TAB_STYLE_METRICS,
	getHorizontalTabPath,
	getHorizontalTabSeparatorBounds,
	getHorizontalTabSeparatorTargets,
	TAB_CONTENT_HORIZONTAL_INSET,
	type SplitTabPhysicalPosition,
} from '../../../../style';
import {
	calculateHorizontalTabGroupHeaderLayout,
	calculateHorizontalTabGroupUnderlineBounds,
	getTabGroupHighlightPath,
	getTabGroupTitleVisualCenteringOffset,
	mergeDraggedGroupUnderlineBounds,
	TabGroupEditorBubbleTracker,
} from '../../../../groups';
import {
	ExpandOnHoverLockType,
	VerticalTabStripExpandOnHoverController,
} from '../../../../vertical';
import { AlertIndicatorButton } from '../alert-indicator-button/AlertIndicatorButton';
import { DropArrow } from '../drop-arrow/DropArrow';
import { ManageSharingRow } from '../manage-sharing-row/ManageSharingRow';
import { RecentActivityBubble } from '../recent-activity-bubble/RecentActivityBubble';
import { RoundedScrollbar } from '../rounded-scrollbar/RoundedScrollbar';
import { TabAlertIndicatorGlyph } from '../tab-alert-indicator-glyph/TabAlertIndicatorGlyph';
import { TabCloseButton } from '../tab-close-button/TabCloseButton';
import { TabIcon } from '../tab-icon/TabIcon';
import { TabStripControlButton } from '../tab-strip-control-button/TabStripControlButton';
import { TabStripIcon } from '../tab-strip-icon/TabStripIcon';
import { VerticalTabStripBottomContainer } from '../vertical-tab-strip-bottom-container/VerticalTabStripBottomContainer';
import { VerticalTabStripTopContainer } from '../vertical-tab-strip-top-container/VerticalTabStripTopContainer';
import { TabCollectionAnimationRuntime } from '../../utils/TabCollectionAnimationRuntime';
import {
	DropArrowDirection,
	DropGroupInclusion,
	DropRelativeToIndex,
	type DropArrowPlacement,
	type TabDropIndex,
	TabStripComboButtonController,
	TabStripComboStartKind,
} from '../../../../shared';
import {
	getAccessibleTabLabel,
	getGroupAccessibleName,
	getGroupTooltipText,
	getTabGroupMembershipAnnouncement,
	getTabMoveAnnouncement,
} from '../../../../accessibility';
import { getVisibleTabTitle } from '../../../../tab';
const TAB_SIZE: Readonly<TabSizeInfo> = {
	pinnedTabWidth: 64,
	minActiveWidth: 56,
	minInactiveWidth: 32,
	standardWidth: 256,
};
const SPLIT_TAB_SIZE: Readonly<TabSizeInfo> = {
	pinnedTabWidth: 55,
	minActiveWidth: 52,
	minInactiveWidth: 32,
	standardWidth: 137,
};

// TabContainerImpl::AnimateViewTo uses RichAnimationDuration(200ms).
const TAB_BOUNDS_ANIMATION_DURATION_MS = 200;
function requestManagedAnimationFrame(callback: FrameRequestCallback): number {
	if (typeof window.requestAnimationFrame === 'function') {
		return window.requestAnimationFrame(callback);
	}
	return window.setTimeout(() => callback(performance.now()), 16);
}
function cancelManagedAnimationFrame(handle: number): void {
	if (typeof window.cancelAnimationFrame === 'function') window.cancelAnimationFrame(handle);
	else window.clearTimeout(handle);
}
const GROUP_COLORS: Record<TabGroupColor, string> = {
	grey: '#5f6368',
	blue: '#1a73e8',
	red: '#d93025',
	yellow: '#f9ab00',
	green: '#188038',
	pink: '#d01884',
	purple: '#9334e6',
	cyan: '#007b83',
	orange: '#fa7b17',
};
function getMaxContrastForeground(background: string): '#202124' | '#fff' {
	const channels =
		background
			.slice(1)
			.match(/.{2}/g)
			?.map((channel) => Number.parseInt(channel, 16) / 255) ?? [];
	const luminance = channels.reduce((sum, channel, index) => {
		const linear = channel <= 0.04045 ? channel / 12.92 : ((channel + 0.055) / 1.055) ** 2.4;
		return sum + linear * requireDefined([0.2126, 0.7152, 0.0722][index]);
	}, 0);
	// color_utils::GetColorWithMaxContrast uses GoogleGrey900 rather than
	// absolute black and switches at its precomputed equal-contrast midpoint.
	return luminance < 0.211692036 ? '#fff' : '#202124';
}
const GROUP_COLOR_NAMES: readonly TabGroupColor[] = [
	'grey',
	'blue',
	'red',
	'yellow',
	'green',
	'pink',
	'purple',
	'cyan',
	'orange',
];
type VisualItem =
	| {
			type: 'tab';
			id: string;
			tab: Readonly<TabData>;
	  }
	| {
			type: 'group';
			id: string;
			group: Readonly<TabGroupData>;
	  };
interface DragState {
	sourceId: string;
	tabIds: string[];
	pointerId: number;
	pointerType: string;
	touchDragReady: boolean;
	startX: number;
	startY: number;
	shiftPressedOnPointerDown: boolean;
	x: number;
	y: number;
	started: boolean;
	targetIndex: number | null;
	targetGroup: TabData['group'];
	targetIdealLeft: number | null;
	lastReorderCoordinate: number;
	restoreState: TabDragRestoreState;
	groupHeaderId: string | null;
	groupWasCollapsedBeforeDrag: boolean;
	horizontalGroupDrag: HorizontalGroupDragState | null;
	label: string;
	visualViews: readonly DragVisualView[];
	pointerOffsetRatios: Readonly<{
		x: number;
		y: number;
	}>;
	pointerOffsetFromVisualOrigin: Readonly<{
		x: number;
		y: number;
	}>;
	sourceViewOffsetFromSlot: Readonly<{
		x: number;
		y: number;
	}>;
	sourceSlotSize: Readonly<{
		width: number;
		height: number;
	}>;
	groupUnderlineBounds: Readonly<
		Record<
			string,
			Readonly<{
				x: number;
				y: number;
				width: number;
				height: number;
			}>
		>
	>;
	dragAreaBounds: Readonly<{
		left: number;
		top: number;
		width: number;
		height: number;
	}>;
	crossWindowRuntime: TabStripImplementationCrossWindowDragRuntime | null;
	settling: boolean;
}

/** Pointer-lifecycle surface implemented by CrossWindowTabDragRuntime. */
export interface TabStripImplementationCrossWindowDragRuntime {
	start(): void;
	update(pointInScreen: Readonly<DragPoint>): void;
	end(reason: CrossWindowDragEndReason): void;
	getSnapshot?(): Pick<CrossWindowDragSnapshot, 'targetActive'> &
		Partial<Pick<CrossWindowDragSnapshot, 'sourceContextId' | 'attachedContextId' | 'state'>>;
}
interface DragVisualView {
	id: string;
	kind: 'tab' | 'group';
	tabIds?: readonly string[];
	x: number;
	y: number;
	width: number;
	height: number;
}
interface PackedHorizontalDragView extends DragVisualView {
	initialOffsetX: number;
}
function packHorizontalDragViews(input: {
	views: readonly DragVisualView[];
	sourceVisualId: string;
	pointerRatioX: number;
	sourceWidthOverride?: number;
}): Readonly<{
	views: readonly PackedHorizontalDragView[];
	width: number;
	mouseOffset: number;
}> {
	let cursor = 0;
	const positioned = input.views.map((view) => {
		const width =
			view.id === input.sourceVisualId && input.sourceWidthOverride !== undefined
				? input.sourceWidthOverride
				: view.width;
		const packed = {
			...view,
			x: cursor,
			y: 0,
			width,
		};
		cursor += width - DEFAULT_TAB_STYLE_METRICS.tabOverlap;
		return packed;
	});
	const sourceOriginalX = input.views.find((view) => view.id === input.sourceVisualId)?.x ?? 0;
	const sourcePacked = positioned.find((view) => view.id === input.sourceVisualId);
	const sourcePackedX = sourcePacked?.x ?? 0;
	const views = positioned.map((view, index) =>
		Object.freeze({
			...view,
			initialOffsetX:
				requireDefined(input.views[index]).x - sourceOriginalX - (view.x - sourcePackedX),
		}),
	);
	return Object.freeze({
		views: Object.freeze(views),
		width: Math.max(0, ...views.map((view) => view.x + view.width)),
		mouseOffset: sourcePacked
			? sourcePacked.x + Math.round(input.pointerRatioX * sourcePacked.width)
			: 0,
	});
}
function getSourceVisualId(
	drag: Pick<DragState, 'sourceId' | 'groupHeaderId' | 'visualViews'>,
): string {
	if (drag.groupHeaderId !== null) return `group:${drag.groupHeaderId}`;
	return drag.visualViews.find((view) => view.tabIds?.includes(drag.sourceId))?.id ?? drag.sourceId;
}
function getVerticalDragVisualViews(
	drag: Pick<DragState, 'groupHeaderId' | 'visualViews'>,
): readonly DragVisualView[] {
	if (drag.groupHeaderId === null) return drag.visualViews;
	const draggedGroupId = `group:${drag.groupHeaderId}`;
	return drag.visualViews.map((view) =>
		view.id === draggedGroupId
			? Object.freeze({
					...view,
					height: VERTICAL_TAB_GROUP_COLLAPSED_HEIGHT,
				})
			: view,
	);
}
function captureDragVisuals(input: {
	sourceElement: HTMLElement;
	sourceId: string;
	tabIds: readonly string[];
	sourceGroupId: string | null;
	groupIds: readonly string[];
	underlineGroupIds: readonly string[];
	orientation: TabStripOrientation;
	pointer: Readonly<{
		x: number;
		y: number;
	}>;
}): Pick<
	DragState,
	| 'visualViews'
	| 'pointerOffsetRatios'
	| 'pointerOffsetFromVisualOrigin'
	| 'sourceViewOffsetFromSlot'
	| 'sourceSlotSize'
	| 'groupUnderlineBounds'
	| 'dragAreaBounds'
> {
	const root = input.sourceElement.closest<HTMLElement>('[data-ct-root]');
	const rootRect = root?.getBoundingClientRect();
	const verticalDragViewport =
		input.orientation === TabStripOrientation.Vertical
			? input.sourceElement.closest<HTMLElement>('.ct-vertical-scroll, .ct-vertical-pinned-scroll')
			: null;
	const dragAreaRect = verticalDragViewport?.getBoundingClientRect() ?? rootRect;
	const sourceRect = input.sourceElement.getBoundingClientRect();
	let fallbackSourceWidth = 52;
	if (input.orientation === TabStripOrientation.Vertical) fallbackSourceWidth = 216;
	else if (input.sourceGroupId === null) fallbackSourceWidth = TAB_SIZE.standardWidth;
	const fallbackSourceHeight =
		input.orientation === TabStripOrientation.Vertical
			? LAYOUT_CONSTANTS.verticalTabHeight
			: LAYOUT_CONSTANTS.tabStripHeight;
	const sourceWidth = sourceRect.width || fallbackSourceWidth;
	const sourceHeight = sourceRect.height || fallbackSourceHeight;
	const underlineGroupIds = new Set(input.underlineGroupIds);
	const groupUnderlineBounds: Record<
		string,
		Readonly<{
			x: number;
			y: number;
			width: number;
			height: number;
		}>
	> = {};
	for (const underlineElement of root?.querySelectorAll<HTMLElement>('[data-group-underline-id]') ??
		[]) {
		const groupId = underlineElement.dataset['groupUnderlineId'];
		if (!groupId || !underlineGroupIds.has(groupId)) continue;
		const rect = underlineElement.getBoundingClientRect();
		if (rect.width <= 0) continue;
		groupUnderlineBounds[groupId] = Object.freeze({
			x: rect.left,
			y: rect.top,
			width: rect.width,
			height: rect.height,
		});
	}
	const rawViews: Array<
		DragVisualView & {
			left: number;
			top: number;
		}
	> = [];
	const selectedTabs = new Set(input.tabIds);
	const selectedGroups = new Set(input.groupIds);
	const tabsRepresentedByGroups = new Set<string>();
	const representedSplits = new Set<string>();
	if (input.orientation === TabStripOrientation.Vertical) {
		for (const element of root?.querySelectorAll<HTMLElement>(
			'.ct-vertical-group[data-group-id]',
		) ?? []) {
			const groupId = element.dataset['groupId'];
			if (!groupId || !selectedGroups.has(groupId)) continue;
			const representedTabIds = [...element.querySelectorAll<HTMLElement>('[data-tab-id]')].flatMap(
				(candidate) => (candidate.dataset['tabId'] ? [candidate.dataset['tabId']] : []),
			);
			representedTabIds.forEach((id) => tabsRepresentedByGroups.add(id));
			const rect = element.getBoundingClientRect();
			rawViews.push({
				id: `group:${groupId}`,
				kind: 'group',
				tabIds: Object.freeze(representedTabIds),
				x: 0,
				y: 0,
				left: rect.left,
				top: rect.top,
				width: rect.width || 216,
				height: rect.height || 34,
			});
		}
	}
	const slotSelector =
		input.orientation === TabStripOrientation.Horizontal
			? '.ct-group-header[data-group-id], .ct-tab[data-tab-id]'
			: '.ct-tab[data-tab-id]';
	const slotElements = [...(root?.querySelectorAll<HTMLElement>(slotSelector) ?? [])];
	for (const element of slotElements) {
		const groupId = element.dataset['groupId'];
		if (groupId !== undefined) {
			if (!selectedGroups.has(groupId)) continue;
			const rect = element.getBoundingClientRect();
			rawViews.push({
				id: `group:${groupId}`,
				kind: 'group',
				x: 0,
				y: 0,
				left: rect.left,
				top: rect.top,
				width: rect.width || 52,
				height: rect.height || fallbackSourceHeight,
			});
			continue;
		}
		const id = element.dataset['tabId'];
		if (id === undefined || !selectedTabs.has(id) || tabsRepresentedByGroups.has(id)) continue;
		const splitContainer = element.closest<HTMLElement>('[data-split-id]');
		const splitId = splitContainer?.dataset['splitId'];
		if (splitId && representedSplits.has(splitId)) continue;
		if (splitId) representedSplits.add(splitId);
		const visualElement = splitContainer ?? element;
		const rect = visualElement.getBoundingClientRect();
		const representedTabIds = splitContainer
			? [...splitContainer.querySelectorAll<HTMLElement>('[data-tab-id]')].flatMap((candidate) =>
					candidate.dataset['tabId'] ? [candidate.dataset['tabId']] : [],
				)
			: [id];
		const visualId =
			splitContainer &&
			[...splitContainer.querySelectorAll<HTMLElement>('[data-tab-id]')].some(
				(candidate) => candidate.dataset['tabId'] === input.sourceId,
			)
				? input.sourceId
				: id;
		rawViews.push({
			id: visualId,
			kind: 'tab',
			tabIds: Object.freeze(representedTabIds),
			x: 0,
			y: 0,
			left: rect.left,
			top: rect.top,
			width:
				rect.width ||
				(input.orientation === TabStripOrientation.Vertical ? 216 : TAB_SIZE.standardWidth),
			height:
				rect.height ||
				(input.orientation === TabStripOrientation.Vertical
					? LAYOUT_CONSTANTS.verticalTabHeight
					: LAYOUT_CONSTANTS.tabStripHeight),
		});
	}
	if (input.orientation === TabStripOrientation.Vertical) {
		// GetDragInitDataForTabDrag walks selected tabs in model order and inserts
		// a completely selected group at its first member. The DOM collection
		// bounds are the equivalent ordering source after group/split deduping.
		rawViews.sort((a, b) => a.top - b.top || a.left - b.left);
	}
	if (rawViews.length === 0) {
		rawViews.push({
			id:
				input.sourceGroupId === null
					? (input.tabIds[0] ?? 'drag-source')
					: `group:${input.sourceGroupId}`,
			kind: input.sourceGroupId === null ? 'tab' : 'group',
			...(input.sourceGroupId === null
				? {
						tabIds: Object.freeze([...input.tabIds]),
					}
				: {}),
			x: 0,
			y: 0,
			left: sourceRect.left,
			top: sourceRect.top,
			width: sourceWidth,
			height: sourceHeight,
		});
	}
	const hasMeasuredPositions = rawViews.some((view) => view.left !== 0 || view.top !== 0);
	if (!hasMeasuredPositions) {
		let cursor = 0;
		for (const view of rawViews) {
			view.left = input.orientation === TabStripOrientation.Horizontal ? cursor : 0;
			view.top = input.orientation === TabStripOrientation.Vertical ? cursor : 0;
			cursor +=
				(input.orientation === TabStripOrientation.Horizontal ? view.width : view.height) -
				(input.orientation === TabStripOrientation.Horizontal
					? DEFAULT_TAB_STYLE_METRICS.tabOverlap
					: -2);
		}
	}
	const originLeft = Math.min(...rawViews.map((view) => view.left));
	const originTop = Math.min(...rawViews.map((view) => view.top));
	const visualViews = rawViews.map(({ left, top, ...view }) =>
		Object.freeze({
			...view,
			x: left - originLeft,
			y: top - originTop,
		}),
	);
	const sourceLeft =
		sourceRect.width || sourceRect.height
			? sourceRect.left
			: (rawViews.find(
					(view) =>
						view.id ===
						(input.sourceGroupId === null ? input.sourceId : `group:${input.sourceGroupId}`),
				)?.left ?? originLeft);
	const sourceTop =
		sourceRect.width || sourceRect.height
			? sourceRect.top
			: (rawViews.find(
					(view) =>
						view.id ===
						(input.sourceGroupId === null ? input.sourceId : `group:${input.sourceGroupId}`),
				)?.top ?? originTop);
	const sourceVisual = rawViews.find((view) =>
		input.sourceGroupId !== null
			? view.id === `group:${input.sourceGroupId}`
			: view.id === input.sourceId || view.tabIds?.includes(input.sourceId),
	);
	return {
		visualViews: Object.freeze(visualViews),
		pointerOffsetRatios: Object.freeze({
			x: Math.min(1, Math.max(0, (input.pointer.x - sourceLeft) / sourceWidth)),
			y: Math.min(1, Math.max(0, (input.pointer.y - sourceTop) / sourceHeight)),
		}),
		pointerOffsetFromVisualOrigin: Object.freeze({
			x: input.pointer.x - originLeft,
			y: input.pointer.y - originTop,
		}),
		sourceViewOffsetFromSlot: Object.freeze({
			x: sourceLeft - (sourceVisual?.left ?? sourceLeft),
			y: sourceTop - (sourceVisual?.top ?? sourceTop),
		}),
		sourceSlotSize: Object.freeze({
			width: sourceWidth,
			height: sourceHeight,
		}),
		groupUnderlineBounds: Object.freeze(groupUnderlineBounds),
		dragAreaBounds: Object.freeze(
			dragAreaRect && dragAreaRect.width > 0
				? {
						left: dragAreaRect.left,
						top: dragAreaRect.top,
						width: dragAreaRect.width,
						height: dragAreaRect.height,
					}
				: {
						// DOM test environments do not perform layout. An effectively
						// unbounded area preserves pointer-relative positioning there; real
						// browser roots always take the measured branch above.
						left: -Number.MAX_SAFE_INTEGER / 4,
						top:
							input.orientation === TabStripOrientation.Vertical
								? -Number.MAX_SAFE_INTEGER / 4
								: originTop,
						width: Number.MAX_SAFE_INTEGER / 2,
						height:
							input.orientation === TabStripOrientation.Vertical
								? Number.MAX_SAFE_INTEGER / 2
								: sourceHeight,
					},
		),
	};
}
function buildDragSessionData(input: {
	snapshot: TabModelSnapshot;
	tabIds: readonly string[];
	sourceId: string;
	groupHeaderId: string | null;
	groupWasCollapsedBeforeDrag: boolean;
	pointerOffsetRatios: Readonly<{
		x: number;
		y: number;
	}>;
}): DragSessionData<Readonly<TabGroupData>> {
	const tabIdSet = new Set(input.tabIds);
	const groupTabs =
		input.groupHeaderId === null
			? []
			: input.snapshot.tabs.filter((tab) => tab.group === input.groupHeaderId);
	const tabDragData: TabDragData<Readonly<TabGroupData>>[] = input.snapshot.tabs.flatMap(
		(tab, sourceModelIndex) => {
			if (!tabIdSet.has(tab.id)) return [];
			const group =
				tab.group === null
					? null
					: (input.snapshot.groups.find((candidate) => candidate.id === tab.group) ?? null);
			return [
				{
					sourceModelIndex,
					viewType: DragSlotViewType.Tab,
					contentsId: tab.id,
					attachedViewId: tab.id,
					pinned: tab.pinned,
					tabGroupData: group
						? {
								groupId: group.id,
								groupVisualData: group,
								indexInGroup: input.snapshot.tabs
									.filter((candidate) => candidate.group === group.id)
									.findIndex((candidate) => candidate.id === tab.id),
							}
						: null,
				},
			];
		},
	);
	const group =
		input.groupHeaderId === null
			? null
			: (input.snapshot.groups.find((candidate) => candidate.id === input.groupHeaderId) ?? null);
	if (group) {
		tabDragData.unshift({
			sourceModelIndex: null,
			viewType: DragSlotViewType.GroupHeader,
			contentsId: null,
			attachedViewId: `group:${group.id}`,
			pinned: false,
			tabGroupData: {
				groupId: group.id,
				groupVisualData: group,
				indexInGroup: 0,
			},
		});
	}
	const sourceViewIndex = group
		? 0
		: Math.max(
				0,
				tabDragData.findIndex((datum) => datum.contentsId === input.sourceId),
			);
	return new DragSessionData({
		tabDragData,
		groupHeaderDragData: group
			? {
					group: group.id,
					activeTabIndexWithinGroup: Math.max(
						0,
						groupTabs.findIndex((tab) => tab.id === input.sourceId),
					),
					wasCollapsedFromDrag: !input.groupWasCollapsedBeforeDrag,
				}
			: null,
		sourceViewIndex,
		mouseOffsetToSizeRatios: input.pointerOffsetRatios,
		...(group
			? {
					draggingGroups: new Set([group.id]),
				}
			: {}),
	});
}
function areDraggedTabIdsConsecutive(
	tabs: readonly Readonly<{
		id: string;
	}>[],
	tabIds: readonly string[],
): boolean {
	const draggedIds = new Set(tabIds);
	const indices = tabs
		.map((tab, index) => (draggedIds.has(tab.id) ? index : -1))
		.filter((index) => index !== -1);
	return indices.every(
		(index, position) => position === 0 || index === requireDefined(indices[position - 1]) + 1,
	);
}
type MenuState =
	| {
			kind: 'tab';
			id: string;
			x: number;
			y: number;
	  }
	| {
			kind: 'group';
			id: string;
			x: number;
			y: number;
	  }
	| {
			kind: 'new-tab';
			x: number;
			y: number;
	  }
	| {
			kind: 'collapse';
			x: number;
			y: number;
	  }
	| {
			kind: 'combo-start';
			x: number;
			y: number;
	  }
	| {
			kind: 'combo-end';
			x: number;
			y: number;
	  };
type HoverState =
	| {
			kind: 'tab';
			id: string;
			x: number;
			y: number;
	  }
	| {
			kind: 'group';
			id: string;
			x: number;
			y: number;
	  };
function urlFromDataTransfer(dataTransfer: DataTransfer): string | null {
	const uriListUrl = dataTransfer
		.getData('text/uri-list')
		.split(/\r?\n/)
		.find((line) => line.length > 0 && !line.startsWith('#'));
	const rawUrl = uriListUrl ?? dataTransfer.getData('text/plain').trim();
	if (!rawUrl) return null;
	try {
		const url = new URL(rawUrl);
		return url.protocol === 'http:' || url.protocol === 'https:' ? url.href : null;
	} catch {
		return null;
	}
}
function titleForDroppedUrl(url: string): string {
	try {
		return new URL(url).hostname || url;
	} catch {
		return url;
	}
}
function nextAvailableEntityId(prefix: string, existingIds: ReadonlySet<string>): string {
	let sequence = 1;
	while (existingIds.has(`${prefix}-${sequence}`)) sequence += 1;
	return `${prefix}-${sequence}`;
}
function createNewTabGroup(model: TabModel): void {
	const snapshot = model.getSnapshot();
	const groupId = nextAvailableEntityId('group', new Set(snapshot.groups.map((group) => group.id)));
	const tabId = model.addTab({
		title: 'New Tab',
		pinned: false,
		group: null,
		split: null,
	});
	model.createGroup(
		{
			id: groupId,
			title: '',
			color: 'grey',
			collapsed: false,
		},
		[tabId],
	);
}
function createGroupForTabs(model: TabModel, tabIds: readonly string[]): string | null {
	const snapshot = model.getSnapshot();
	const eligible = snapshot.tabs.filter((tab) => tabIds.includes(tab.id)).map((tab) => tab.id);
	if (eligible.length === 0) return null;
	const groupId = nextAvailableEntityId('group', new Set(snapshot.groups.map((group) => group.id)));
	model.createGroup(
		{
			id: groupId,
			title: '',
			color: 'grey',
			collapsed: false,
		},
		eligible,
	);
	return groupId;
}
function createSplitTabWithTab(model: TabModel, tabId?: string): void {
	const snapshot = model.getSnapshot();
	const activeIndex = snapshot.tabs.findIndex((tab) => tab.id === (tabId ?? snapshot.activeTabId));
	const active = snapshot.tabs[activeIndex];
	if (!active || active.split !== null) return;
	const splitIds = new Set(
		snapshot.tabs.map((tab) => tab.split).filter((id): id is string => id !== null),
	);
	const splitId = nextAvailableEntityId('split', splitIds);
	model.updateTab(active.id, {
		split: splitId,
	});
	model.addTab(
		{
			title: 'New Tab',
			url: 'chrome://newtab',
			pinned: active.pinned,
			group: active.group,
			split: splitId,
		},
		{
			index: activeIndex + 1,
			activate: true,
		},
	);
}

/** Port of TabStripModel::ExecuteAddToNewSplitCommand. */
function createSplitFromTabContext(model: TabModel, contextTabId: string): void {
	const snapshot = model.getSnapshot();
	const active = snapshot.tabs.find((tab) => tab.id === snapshot.activeTabId);
	const context = snapshot.tabs.find((tab) => tab.id === contextTabId);
	if (!active || !context || active.split !== null || context.split !== null) return;
	const commandTabs = snapshot.selectedTabIds.has(context.id)
		? snapshot.tabs.filter((tab) => snapshot.selectedTabIds.has(tab.id))
		: [context];
	if (commandTabs.length > 2) return;
	const candidates = commandTabs.filter((tab) => tab.id !== active.id);
	if (candidates.length > 1) return;
	const otherId =
		candidates[0]?.id ??
		model.addTab(
			{
				title: 'New Tab',
				url: 'chrome://newtab',
				pinned: active.pinned,
				group: active.group,
				split: null,
			},
			{
				index: (model.getIndexOfTab(active.id) ?? 0) + 1,
				activate: false,
			},
		);
	const current = model.getSnapshot();
	const splitIds = new Set(
		current.tabs.map((tab) => tab.split).filter((id): id is string => id !== null),
	);
	model.createSplit([active.id, otherId], active.id, nextAvailableEntityId('split', splitIds));
}
function contextMenuStyle(state: MenuState): CSSProperties {
	const margin = 8;
	const width = 260;
	let estimatedHeight = 176;
	if (state.kind === 'group') estimatedHeight = 360;
	else if (state.kind === 'tab') estimatedHeight = 340;
	else if (state.kind === 'collapse' || state.kind === 'combo-start' || state.kind === 'combo-end')
		estimatedHeight = 52;
	const viewportWidth =
		typeof window === 'undefined' ? state.x + width + margin : window.innerWidth;
	const viewportHeight =
		typeof window === 'undefined' ? state.y + estimatedHeight + margin : window.innerHeight;
	return {
		left: Math.max(margin, Math.min(state.x, viewportWidth - width - margin)),
		top: Math.max(margin, Math.min(state.y, viewportHeight - estimatedHeight - margin)),
	};
}
export interface TabStripImplementationOptions {
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
	showVerticalProjectsButton?: boolean;
	showVerticalTabSearchButton?: boolean;
	/** Compatibility override; model.focusedGroupId is the source of truth for focused-group layout. */
	verticalGroupFocused?: boolean;
	/** Mirrors kTabGroupsFocusingPinnedTabs. Chromium's default is false. */
	showPinnedTabsInFocusedGroups?: boolean;
	verticalProjectsOpen?: boolean;
	verticalTabSearchOpen?: boolean;
	verticalComboButtonController?: TabStripComboButtonController;
	verticalToolbarHeight?: number;
	verticalCaptionButtonWidth?: number;
	verticalIsExitingExpandOnHover?: boolean;
	onVerticalCollapsedChange?: (collapsed: boolean) => void;
	onVerticalUnfocusGroup?: () => void;
	onVerticalProjectsClick?: () => void;
	onVerticalTabSearchClick?: () => void;
	onVerticalExpandOnHoverChange?: (enabled: boolean) => void;
	className?: string;
	style?: CSSProperties;
	showNewTabButton?: boolean;
	horizontalFrameCondensed?: boolean;
	/** Mirrors ui::TouchUiController's touch-optimized tab metrics. */
	touchUi?: boolean;
	enableTabMuting?: boolean;
	/** Mirrors current Chrome tab decluttering; pass false for legacy channels. */
	tabStripDeclutter?: boolean;
	/** Mirrors ChromeOS OnTask locking: tabs stay selectable but cannot be closed or dragged. */
	lockedForOnTask?: boolean;
	/** Connects the DOM pointer lifecycle to a host-owned cross-window drag runtime. */
	createCrossWindowDragRuntime?: (
		sessionData: DragSessionData<Readonly<TabGroupData>>,
	) => TabStripImplementationCrossWindowDragRuntime | null;
	/** Host bridge for Chromium's immersive-reveal drag lock lifecycle. */
	onDraggingChange?: (dragging: boolean) => void;
	/** Host bridge for BrowserCommandController::TabKeyboardFocusChangedTo. */
	onTabKeyboardFocusChanged?: (tabId: string | null) => void;
	onTabAudioMuteToggle?: (tabId: string, muted: boolean) => void;
	/** Host-backed TabStripModel context commands. */
	onMoveTabsToNewWindow?: (tabIds: readonly string[]) => void;
	onReloadTabs?: (tabIds: readonly string[]) => void;
	onAddTabsToReadingList?: (tabs: readonly Readonly<TabData>[]) => void;
	onManageGroupSharing?: (groupId: string) => void;
	onShareGroup?: (groupId: string) => void;
	onMoveGroupToNewWindow?: (groupId: string) => void;
	/** Mirrors features::kTabGroupsFocusing in the editor bubble. */
	enableTabGroupFocusing?: boolean;
	onDeleteSavedGroup?: (groupId: string) => void;
	showSavedGroupFooter?: boolean;
	onOpenTabGroupsLearnMore?: () => void;
	/** Enables Chromium's feature-gated Tab Group Home command. */
	enableTabGroupHome?: boolean;
	onOpenTabGroupHome?: (groupId: string) => void;
	/** Host bookmark editor. Resolve/return false when the conversion is canceled. */
	onConvertGroupToBookmark?: (
		groupId: string,
		tabs: readonly Readonly<TabData>[],
	) => boolean | void | Promise<boolean | void>;
	bookmarksEditable?: boolean;
	/** Mirrors the saved-group deletion-dialog preference. Defaults to confirmation enabled. */
	confirmSavedGroupDestructiveActions?: boolean;
	onAskGeminiAboutGroup?: (groupId: string) => void;
	onGroupActivityAction?: (groupId: string, item: Readonly<TabGroupActivity>) => void;
	onSeeAllGroupActivity?: (groupId: string) => void;
	ariaLabel?: string;
	onNewTab?: () => void;
	onNewTabButtonPressed?: () => void;
	onNewTabFromClipboard?: () => void;
	onUrlDrop?: (url: string, index: Readonly<TabDropIndex>) => void;
}
function useElementSize<T extends HTMLElement>() {
	const ref = useRef<T>(null);
	const [size, setSize] = useState({
		width: 0,
		height: 0,
	});
	useEffect(() => {
		const element = ref.current;
		if (!element) return;
		const update = () => {
			const rect = element.getBoundingClientRect();
			setSize({
				width: Math.round(rect.width),
				height: Math.round(rect.height),
			});
		};
		update();
		const observer = new ResizeObserver(update);
		observer.observe(element);
		return () => observer.disconnect();
	}, []);
	return [ref, size] as const;
}
function buildVisualItems(snapshot: TabModelSnapshot): VisualItem[] {
	const groups = new Map(snapshot.groups.map((group) => [group.id, group]));
	const seen = new Set<string>();
	const items: VisualItem[] = [];
	for (const tab of snapshot.tabs) {
		if (tab.group !== null && !seen.has(tab.group)) {
			const group = groups.get(tab.group);
			if (group) {
				items.push({
					type: 'group',
					id: `group-${group.id}`,
					group,
				});
				seen.add(tab.group);
			}
		}
		items.push({
			type: 'tab',
			id: tab.id,
			tab,
		});
	}
	return items;
}
let groupTitleMeasurementContext: OffscreenCanvasRenderingContext2D | null | undefined;
function measuredGroupTitleWidth(title: string): number {
	if (groupTitleMeasurementContext === undefined) {
		groupTitleMeasurementContext =
			typeof OffscreenCanvas === 'undefined' ? null : new OffscreenCanvas(1, 1).getContext('2d');
		if (groupTitleMeasurementContext) {
			groupTitleMeasurementContext.font =
				'600 12px system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif';
		}
	}
	return groupTitleMeasurementContext?.measureText(title).width ?? title.length * 7.2;
}
function groupHeaderWidth(group: Readonly<TabGroupData>, tabCount = 1): number {
	return Math.round(
		calculateHorizontalTabGroupHeaderLayout({
			title: group.title,
			measuredTitleWidth: measuredGroupTitleWidth(group.title),
			collapsed: group.collapsed,
			tabCount,
			...(group.shared === undefined
				? {}
				: {
						shared: group.shared,
					}),
			...(group.attention === undefined
				? {}
				: {
						attention: group.attention,
					}),
		}).desiredWidth,
	);
}
function sizeInfoForItem(item: VisualItem): TabSizeInfo {
	if (item.type === 'tab') {
		return item.tab.split === null ? TAB_SIZE : SPLIT_TAB_SIZE;
	}
	const width = groupHeaderWidth(item.group);
	return {
		pinnedTabWidth: width,
		minActiveWidth: width,
		minInactiveWidth: width,
		standardWidth: width,
	};
}
function tabIsApparentlyActive(tab: Readonly<TabData>, snapshot: TabModelSnapshot): boolean {
	if (tab.id === snapshot.activeTabId) return true;
	if (tab.split === null) return false;
	const active = snapshot.tabs.find((candidate) => candidate.id === snapshot.activeTabId);
	return active?.split === tab.split;
}
function TabButton({
	tab,
	snapshot,
	width,
	orientation,
	collapsed,
	closing = false,
	onPointerDown,
	onClose,
	onContextMenu,
	onMouseEnter,
	onMouseLeave,
	onKeyDown,
	onKeyUp,
	enableTabMuting,
	touchUi,
	closeButtonAllowed,
	declutterEnabled = false,
	splitLayout,
	hoveredTabId,
	verticalExpandedOnHover = false,
	frameCondensed = false,
	onAudioMuteToggle,
	slotState,
	dragging = false,
}: {
	tab: Readonly<TabData>;
	snapshot: TabModelSnapshot;
	width: number;
	orientation: TabStripOrientation;
	collapsed: boolean;
	closing?: boolean;
	onPointerDown: (event: ReactPointerEvent<HTMLDivElement>, tab: Readonly<TabData>) => void;
	onClose: (event: ReactMouseEvent, tabId: string, source: TabCloseSource) => void;
	onContextMenu: (event: ReactMouseEvent<HTMLDivElement>, tabId: string) => void;
	onMouseEnter: (event: ReactMouseEvent<HTMLDivElement>, tabId: string) => void;
	onMouseLeave: () => void;
	onKeyDown: (event: React.KeyboardEvent, tabId: string) => void;
	onKeyUp: (event: React.KeyboardEvent, tabId: string) => void;
	enableTabMuting: boolean;
	touchUi: boolean;
	closeButtonAllowed: boolean;
	declutterEnabled?: boolean;
	splitLayout?: 'side-by-side' | 'stacked';
	hoveredTabId?: string | null;
	verticalExpandedOnHover?: boolean;
	frameCondensed?: boolean;
	onAudioMuteToggle: (tabId: string, muted: boolean) => void;
	slotState?: Readonly<TabSlotViewState>;
	dragging?: boolean;
}) {
	// Closing views remain mounted for the shrink animation, but Chromium no
	// longer paints them as active during that interval.
	const [tabViewRef, measuredTabSize] = useElementSize<HTMLDivElement>();
	// Chromium's BoundsAnimator lays out a Tab at each animation frame. Observe
	// the transitioning wrapper so paths and child visibility use that current
	// width instead of jumping directly to the ideal target width.
	const renderedWidth =
		orientation === TabStripOrientation.Horizontal && measuredTabSize.width > 0
			? measuredTabSize.width
			: width;
	const active = !closing && tabIsApparentlyActive(tab, snapshot);
	const selected = snapshot.selectedTabIds.has(tab.id);
	const [hovered, setHovered] = useState(false);
	const [focused, setFocused] = useState(false);
	const visualHovered = hovered || hoveredTabId === tab.id;
	const alertState = tab.alertState ?? 'none';
	const hasAlert = alertState !== 'none';
	const hasFavicon = tab.shouldDisplayFavicon !== false;
	const horizontalContentsWidth = Math.max(0, renderedWidth - TAB_CONTENT_HORIZONTAL_INSET * 2);
	const childVisibility =
		orientation === TabStripOrientation.Horizontal
			? calculateHorizontalTabChildVisibility({
					width: renderedWidth,
					contentsWidth: horizontalContentsWidth,
					active,
					pinned: tab.pinned,
					hovered: visualHovered,
					focused,
					touchUi,
					declutterEnabled,
					hasFavicon,
					hasAlert,
					closeAllowed: closeButtonAllowed,
					pinnedWidth: tab.split === null ? TAB_SIZE.pinnedTabWidth : SPLIT_TAB_SIZE.pinnedTabWidth,
				})
			: calculateVerticalTabChildVisibility({
					width,
					active,
					pinned: tab.pinned,
					collapsed,
					expandingOnHover: verticalExpandedOnHover,
					hovered: visualHovered,
					focused,
					hasFavicon,
					hasAlert,
					closeAllowed: closeButtonAllowed,
				});
	const group =
		tab.group === null
			? null
			: (snapshot.groups.find((candidate) => candidate.id === tab.group) ?? null);
	const splitTabs =
		tab.split === null ? [] : snapshot.tabs.filter((candidate) => candidate.split === tab.split);
	let splitIndex: 0 | 1 | undefined;
	if (tab.split !== null) {
		splitIndex = splitTabs.findIndex((candidate) => candidate.id === tab.id) <= 0 ? 0 : 1;
	}
	let splitPosition: SplitTabPhysicalPosition = 'none';
	if (tab.split !== null) splitPosition = splitIndex === 0 ? 'left' : 'right';
	const separatorBounds = getHorizontalTabSeparatorBounds(renderedWidth);
	const tabIndex = snapshot.tabs.findIndex((candidate) => candidate.id === tab.id);
	const separatorStateFor = (candidate: Readonly<TabData> | undefined) =>
		candidate
			? {
					active: tabIsApparentlyActive(candidate, snapshot),
					selected: snapshot.selectedTabIds.has(candidate.id),
					hovered: hoveredTabId === candidate.id,
					group: candidate.group,
					split: candidate.split,
				}
			: null;
	const separatorTargets = getHorizontalTabSeparatorTargets({
		tab: {
			active,
			selected,
			hovered: visualHovered,
			group: tab.group,
			split: tab.split,
		},
		previous: separatorStateFor(snapshot.tabs[tabIndex - 1]),
		next: separatorStateFor(snapshot.tabs[tabIndex + 1]),
		tabCount: snapshot.tabs.length,
		declutterEnabled,
	});
	return (
		<div
			ref={tabViewRef}
			role='tab'
			tabIndex={-1}
			aria-selected={selected}
			aria-label={getAccessibleTabLabel(tab, {
				group,
				...(splitIndex === undefined
					? {}
					: {
							splitIndex,
						}),
				...(splitLayout === undefined
					? {}
					: {
							splitLayout,
						}),
			})}
			title=''
			className='ct-tab'
			data-tab-id={tab.id}
			data-active={active || undefined}
			data-selected={selected || undefined}
			data-hovered={visualHovered || undefined}
			data-pinned={tab.pinned || undefined}
			data-render-normal={
				(orientation === TabStripOrientation.Horizontal &&
					tab.pinned &&
					childVisibility.showTitle) ||
				undefined
			}
			data-split={tab.split !== null || undefined}
			data-center-control={childVisibility.centerSingleControl || undefined}
			data-horizontal-close={
				(orientation === TabStripOrientation.Horizontal && childVisibility.showClose) || undefined
			}
			data-visible-alert={childVisibility.showAlert || undefined}
			data-compact-alert-close-gap={
				(orientation === TabStripOrientation.Horizontal &&
					childVisibility.showAlert &&
					childVisibility.showClose &&
					calculateHorizontalAlertCloseGap(horizontalContentsWidth, touchUi) ===
						TAB_AFTER_TITLE_PADDING) ||
				undefined
			}
			data-layout-closed={
				(!childVisibility.showFavicon &&
					!childVisibility.showAlert &&
					!childVisibility.showClose &&
					!childVisibility.showTitle) ||
				undefined
			}
			data-orientation={orientation}
			data-collapsed={collapsed || undefined}
			data-slot-group={slotState?.group ?? undefined}
			data-slot-split={slotState?.split ?? undefined}
			data-slot-dragging={slotState?.dragging || dragging || undefined}
			data-slot-animating={slotState?.animating || undefined}
			onPointerDown={(event: ReactPointerEvent<HTMLDivElement>) => onPointerDown(event, tab)}
			onAuxClick={(event: ReactMouseEvent<HTMLDivElement>) => {
				if (event.button === 1) onClose(event, tab.id, 'mouse');
			}}
			onContextMenu={(event: ReactMouseEvent<HTMLDivElement>) => onContextMenu(event, tab.id)}
			onMouseEnter={(event: ReactMouseEvent<HTMLDivElement>) => {
				setHovered(true);
				onMouseEnter(event, tab.id);
			}}
			onMouseLeave={() => {
				setHovered(false);
				onMouseLeave();
			}}
			onFocusCapture={() => setFocused(true)}
			onBlurCapture={(event: ReactFocusEvent<HTMLDivElement>) => {
				if (
					!(event.relatedTarget instanceof Node) ||
					!event.currentTarget.contains(event.relatedTarget)
				)
					setFocused(false);
			}}
			onKeyDown={(event: ReactKeyboardEvent<HTMLDivElement>) => onKeyDown(event, tab.id)}
			onKeyUp={(event: ReactKeyboardEvent<HTMLDivElement>) => onKeyUp(event, tab.id)}
		>
			{orientation === TabStripOrientation.Horizontal ? (
				<>
					<svg
						className='ct-tab__background'
						viewBox={`0 0 ${Math.max(1, renderedWidth)} ${LAYOUT_CONSTANTS.tabStripHeight}`}
						preserveAspectRatio='none'
						aria-hidden='true'
					>
						<path
							className='ct-tab__hit-target'
							d={getHorizontalTabPath({
								width: renderedWidth,
								pathType: 'hit-test',
								splitPosition,
								closing,
								frameCondensed,
							})}
						/>

						<path
							className='ct-tab__background-fill'
							d={getHorizontalTabPath({
								width: renderedWidth,
								pathType: active ? 'active' : 'highlight',
								splitPosition,
								closing,
								strokeThickness: active && group ? 2 : 0,
							})}
						/>
						{active && group ? (
							<path
								className='ct-tab__background-stroke'
								d={getHorizontalTabPath({
									width: renderedWidth,
									pathType: 'border',
									splitPosition,
									closing,
									strokeThickness: 2,
								})}
								style={{
									stroke: GROUP_COLORS[group.color],
								}}
							/>
						) : null}
					</svg>

					<span
						className='ct-tab__separator ct-tab__separator--leading'
						style={{
							left: separatorBounds.leading.x,
							top: separatorBounds.leading.y,
							width: separatorBounds.leading.width,
							height: separatorBounds.leading.height,
							opacity: separatorTargets.leading,
						}}
					/>

					<span
						className='ct-tab__separator ct-tab__separator--trailing'
						style={{
							left: separatorBounds.trailing.x,
							top: separatorBounds.trailing.y,
							width: separatorBounds.trailing.width,
							height: separatorBounds.trailing.height,
							opacity: separatorTargets.trailing,
						}}
					/>
				</>
			) : (
				<span className='ct-tab__background' />
			)}

			<span className='ct-tab__content'>
				{childVisibility.showFavicon ? (
					<TabIcon tab={tab} availableWidth={Math.max(0, renderedWidth - 18)} />
				) : null}
				{childVisibility.showTitle ? (
					<span className='ct-tab__title'>{getVisibleTabTitle(tab)}</span>
				) : null}
				{childVisibility.showAlert ? (
					<AlertIndicatorButton
						alertState={alertState}
						active={active}
						largestSelectableRegionWidth={Math.max(
							0,
							renderedWidth -
								(orientation === TabStripOrientation.Horizontal ? 18 : 8) -
								TAB_ALERT_DESIGN_WIDTH -
								(childVisibility.showClose ? TAB_CLOSE_DESIGN_WIDTH + TAB_AFTER_TITLE_PADDING : 0),
						)}
						enableTabMuting={enableTabMuting}
						onToggleAudioMute={(muted: boolean) => onAudioMuteToggle(tab.id, muted)}
					/>
				) : null}
				{childVisibility.showClose ? (
					<TabCloseButton
						tabTitle={tab.title}
						touchUi={touchUi}
						onClose={(event: ReactMouseEvent<HTMLButtonElement>, pointerType: string) =>
							onClose(event, tab.id, pointerType === 'touch' ? 'touch' : 'mouse')
						}
					/>
				) : null}
			</span>
		</div>
	);
}
function GroupHeader({
	group,
	tabs,
	activeTabId,
	orientation,
	collapsedStrip,
	onToggle,
	onPointerDown,
	onKeyDown,
	onContextMenu,
	onOpenEditor,
	onMouseEnter,
	onMouseLeave,
	showHoverCard,
	frameCondensed = false,
}: {
	group: Readonly<TabGroupData>;
	tabs: readonly Readonly<TabData>[];
	activeTabId: string | null;
	orientation: TabStripOrientation;
	collapsedStrip: boolean;
	onToggle: (source: 'mouse' | 'touch' | 'keyboard') => void;
	onPointerDown: (event: ReactPointerEvent<HTMLDivElement>) => void;
	onKeyDown: (event: React.KeyboardEvent<HTMLDivElement>) => void;
	onContextMenu: (event: ReactMouseEvent<HTMLDivElement>) => void;
	onOpenEditor?: (x: number, y: number) => void;
	onMouseEnter: (event: ReactMouseEvent<HTMLDivElement>) => void;
	onMouseLeave: () => void;
	showHoverCard: boolean;
	frameCondensed?: boolean;
}) {
	const [headerRef, measuredHeaderSize] = useElementSize<HTMLDivElement>();
	const lastPointerType = useRef<string>('mouse');
	const editorOpenedOnPress = useRef(false);
	const applyMacEmojiCentering =
		orientation === TabStripOrientation.Horizontal &&
		getTabGroupTitleVisualCenteringOffset(group.title, {
			isMac: typeof navigator !== 'undefined' && /Mac/.test(navigator.platform),
		}) !== 0;
	const horizontalLayout = calculateHorizontalTabGroupHeaderLayout({
		title: group.title,
		measuredTitleWidth: measuredGroupTitleWidth(group.title),
		collapsed: group.collapsed,
		tabCount: tabs.length,
		...(group.shared === undefined
			? {}
			: {
					shared: group.shared,
				}),
		...(group.attention === undefined
			? {}
			: {
					attention: group.attention,
				}),
	});
	const groupColor = GROUP_COLORS[group.color];
	const groupForeground = getMaxContrastForeground(groupColor);
	const representedActiveTabId =
		group.collapsed && activeTabId !== null && tabs.some((tab) => tab.id === activeTabId)
			? activeTabId
			: null;
	const renderedHorizontalWidth =
		measuredHeaderSize.width > 0 ? measuredHeaderSize.width : horizontalLayout.desiredWidth;
	return (
		<div
			ref={headerRef}
			role='tablist'
			tabIndex={-1}
			className='ct-group-header'
			data-group-id={group.id}
			data-active={representedActiveTabId !== null || undefined}
			data-active-tab-id={representedActiveTabId ?? undefined}
			data-orientation={orientation}
			data-frame-condensed={
				(orientation === TabStripOrientation.Horizontal && frameCondensed) || undefined
			}
			data-strip-collapsed={collapsedStrip || undefined}
			aria-expanded={!group.collapsed}
			aria-label={getGroupAccessibleName(group, tabs)}
			title={showHoverCard ? '' : getGroupTooltipText(group, tabs)}
			style={
				{
					'--ct-group-color': groupColor,
					'--ct-group-foreground': groupForeground,
				} satisfies CSSProperties & Record<'--ct-group-color' | '--ct-group-foreground', string>
			}
			onClick={(event: ReactMouseEvent<HTMLDivElement>) => {
				let origin: 'keyboard' | 'mouse' | 'touch' = 'mouse';
				if (event.detail === 0) origin = 'keyboard';
				else if (lastPointerType.current === 'touch') origin = 'touch';
				onToggle(origin);
				event.currentTarget.focus({
					preventScroll: true,
				});
			}}
			onPointerDown={(event: ReactPointerEvent<HTMLDivElement>) => {
				lastPointerType.current = event.pointerType || 'mouse';
				onPointerDown(event);
			}}
			onKeyDown={(event: ReactKeyboardEvent<HTMLDivElement>) => {
				if (event.key === ' ' || event.key === 'Enter') {
					event.preventDefault();
					onToggle('keyboard');
					return;
				}
				onKeyDown(event);
			}}
			onContextMenu={onContextMenu}
			onMouseEnter={onMouseEnter}
			onMouseLeave={() => onMouseLeave()}
		>
			{orientation === TabStripOrientation.Horizontal ? (
				<>
					{representedActiveTabId !== null ? (
						<svg
							className='ct-group-header__active-background'
							viewBox={`0 0 ${Math.max(1, renderedHorizontalWidth)} ${LAYOUT_CONSTANTS.tabStripHeight}`}
							preserveAspectRatio='none'
							aria-hidden='true'
						>
							<path
								className='ct-group-header__active-background-fill'
								d={getHorizontalTabPath({
									width: renderedHorizontalWidth,
									pathType: 'active',
								})}
							/>
						</svg>
					) : null}

					<span
						className='ct-group-header__chip'
						data-shared={horizontalLayout.showHeaderIcon || undefined}
						data-attention={horizontalLayout.showAttentionIndicator || undefined}
						data-has-title={Boolean(group.title) || undefined}
						style={{
							width: horizontalLayout.chipWidth,
						}}
					>
						{horizontalLayout.showHeaderIcon ? (
							<TabStripIcon name='shared-group' className='ct-group-header__sync' />
						) : null}
						{group.title ? (
							<span
								className='ct-group-header__title'
								data-mac-emoji-center={applyMacEmojiCentering || undefined}
							>
								{group.title}
							</span>
						) : null}
						{horizontalLayout.showAttentionIndicator ? (
							<span className='ct-group-header__attention'>
								<TabStripIcon name='group-attention' />
							</span>
						) : null}
					</span>
				</>
			) : (
				<>
					{group.shared ? (
						<TabStripIcon name='shared-group' className='ct-group-header__sync' />
					) : null}
					<span className='ct-group-header__title'>{group.title}</span>
					{group.shared && group.collapsed && group.attention && tabs.length > 0 ? (
						<span className='ct-group-header__attention'>
							<TabStripIcon name='group-attention' />
						</span>
					) : null}
					{!collapsedStrip && onOpenEditor ? (
						<button
							type='button'
							className='ct-group-header__editor'
							aria-label='More options'
							tabIndex={-1}
							onPointerDown={(event: ReactPointerEvent<HTMLButtonElement>) => {
								if (event.button !== 0) return;
								event.preventDefault();
								event.stopPropagation();
								editorOpenedOnPress.current = true;
								const rect = event.currentTarget.getBoundingClientRect();
								onOpenEditor(rect.right, rect.bottom);
							}}
							onClick={(event: ReactMouseEvent<HTMLButtonElement>) => {
								event.preventDefault();
								event.stopPropagation();
								if (editorOpenedOnPress.current) {
									editorOpenedOnPress.current = false;
									return;
								}
								const rect = event.currentTarget.getBoundingClientRect();
								onOpenEditor(rect.right, rect.bottom);
							}}
						>
							<TabStripIcon name='more-vertical' />
						</button>
					) : null}

					<TabStripIcon name='chevron' className='ct-group-header__chevron' />
				</>
			)}
		</div>
	);
}
function GroupEditorMenuIcon({
	kind,
}: {
	kind:
		| 'new-tab'
		| 'move-window'
		| 'home'
		| 'focus'
		| 'share'
		| 'activity'
		| 'close'
		| 'gemini'
		| 'ungroup'
		| 'bookmark'
		| 'delete';
}) {
	const paths: Record<typeof kind, ReactNode> = {
		'new-tab': (
			<>
				<rect x='4' y='4' width='14' height='14' rx='1' />
				<path d='M11 7v8M7 11h8' />
			</>
		),
		'move-window': (
			<>
				<rect x='4' y='6' width='14' height='13' rx='1' />
				<path d='M10 3h10v10m0-10-8 8' />
			</>
		),
		home: (
			<>
				<path d='m4 11 8-7 8 7' />
				<path d='M6 10v10h12V10M10 20v-6h4v6' />
			</>
		),
		focus: (
			<>
				<path d='M9 4H4v5m11-5h5v5M9 20H4v-5m11 5h5v-5' />
			</>
		),
		share: (
			<>
				<circle cx='9' cy='8' r='3' />
				<circle cx='17' cy='9' r='2.5' />
				<path d='M3.5 19c.5-4 2.3-6 5.5-6s5 2 5.5 6M14 14c3.5-.5 5.6 1.2 6.5 4' />
			</>
		),
		activity: (
			<>
				<circle cx='12' cy='12' r='8' />
				<path d='M12 7v5l3 2' />
			</>
		),
		close: (
			<>
				<rect x='4' y='4' width='14' height='14' rx='1' />
				<path d='m8 8 6 6m0-6-6 6' />
			</>
		),
		gemini: (
			<>
				<path d='M12 3c.7 4.7 2.3 6.3 7 7-4.7.7-6.3 2.3-7 7-.7-4.7-2.3-6.3-7-7 4.7-.7 6.3-2.3 7-7Z' />
			</>
		),
		ungroup: (
			<>
				<path d='M8 7H5a2 2 0 0 0-2 2v8a2 2 0 0 0 2 2h8a2 2 0 0 0 2-2v-3' />
				<path d='M10 10 20 4m-5 0h5v5' />
			</>
		),
		bookmark: (
			<>
				<path d='M6 4h12v17l-6-4-6 4Z' />
			</>
		),
		delete: (
			<>
				<path d='M5 7h14M9 7V4h6v3m-8 0 1 13h8l1-13M10 10v7m4-7v7' />
			</>
		),
	};
	return (
		<svg className='ct-group-editor__menu-icon' viewBox='0 0 24 24' aria-hidden='true'>
			{paths[kind]}
		</svg>
	);
}
function ContextMenu({
	state,
	snapshot,
	model,
	orientation,
	requestCloseTabs,
	closeAllowed,
	verticalExpandOnHover,
	onVerticalExpandOnHoverChange,
	comboButtonController,
	enableTabMuting,
	onTabAudioMuteToggle,
	onMoveTabsToNewWindow,
	onReloadTabs,
	onAddTabsToReadingList,
	onManageGroupSharing,
	onShareGroup,
	onMoveGroupToNewWindow,
	enableTabGroupFocusing = false,
	onDeleteSavedGroup,
	showSavedGroupFooter = false,
	onOpenTabGroupsLearnMore,
	enableTabGroupHome,
	onOpenTabGroupHome,
	onConvertGroupToBookmark,
	bookmarksEditable,
	confirmSavedGroupDestructiveActions,
	onAskGeminiAboutGroup,
	onGroupActivityAction,
	onSeeAllGroupActivity,
	onClose,
}: {
	state: MenuState;
	snapshot: TabModelSnapshot;
	model: TabModel;
	orientation: TabStripOrientation;
	requestCloseTabs: (
		tabIds: readonly string[],
		source?: TabCloseSource,
		skipSavedGroupConfirmation?: boolean,
	) => void;
	closeAllowed: boolean;
	verticalExpandOnHover: boolean;
	onVerticalExpandOnHoverChange?: (enabled: boolean) => void;
	comboButtonController: TabStripComboButtonController;
	enableTabMuting: boolean;
	onTabAudioMuteToggle?: (tabId: string, muted: boolean) => void;
	onMoveTabsToNewWindow?: (tabIds: readonly string[]) => void;
	onReloadTabs?: (tabIds: readonly string[]) => void;
	onAddTabsToReadingList?: (tabs: readonly Readonly<TabData>[]) => void;
	onManageGroupSharing?: (groupId: string) => void;
	onShareGroup?: (groupId: string) => void;
	onMoveGroupToNewWindow?: (groupId: string) => void;
	enableTabGroupFocusing: boolean;
	onDeleteSavedGroup?: (groupId: string) => void;
	showSavedGroupFooter: boolean;
	onOpenTabGroupsLearnMore?: () => void;
	enableTabGroupHome: boolean;
	onOpenTabGroupHome?: (groupId: string) => void;
	onConvertGroupToBookmark?: (
		groupId: string,
		tabs: readonly Readonly<TabData>[],
	) => boolean | void | Promise<boolean | void>;
	bookmarksEditable: boolean;
	confirmSavedGroupDestructiveActions: boolean;
	onAskGeminiAboutGroup?: (groupId: string) => void;
	onGroupActivityAction?: (groupId: string, item: Readonly<TabGroupActivity>) => void;
	onSeeAllGroupActivity?: (groupId: string) => void;
	onClose: () => void;
}) {
	const [activityOpen, setActivityOpen] = useState(false);
	const [tabGroupSubmenuOpen, setTabGroupSubmenuOpen] = useState(false);
	const [splitSubmenuOpen, setSplitSubmenuOpen] = useState(false);
	const [pendingSavedGroupAction, setPendingSavedGroupAction] = useState<
		'close' | 'ungroup' | null
	>(null);
	const tabMenuRef = useRef<HTMLDivElement>(null);
	const tabGroupTriggerRef = useRef<HTMLButtonElement>(null);
	const tabGroupSubmenuRef = useRef<HTMLDivElement>(null);
	const enterTabGroupSubmenuFromKeyboard = useRef(false);
	const splitTriggerRef = useRef<HTMLButtonElement>(null);
	const splitSubmenuRef = useRef<HTMLDivElement>(null);
	const enterSplitSubmenuFromKeyboard = useRef(false);
	useEffect(() => {
		if (state.kind === 'tab') tabMenuRef.current?.focus();
	}, [state]);
	useEffect(() => {
		if (!tabGroupSubmenuOpen || !enterTabGroupSubmenuFromKeyboard.current) return;
		enterTabGroupSubmenuFromKeyboard.current = false;
		tabGroupSubmenuRef.current?.querySelector<HTMLButtonElement>('button:not(:disabled)')?.focus();
	}, [tabGroupSubmenuOpen]);
	useEffect(() => {
		if (!splitSubmenuOpen || !enterSplitSubmenuFromKeyboard.current) return;
		enterSplitSubmenuFromKeyboard.current = false;
		splitSubmenuRef.current?.querySelector<HTMLButtonElement>('button:not(:disabled)')?.focus();
	}, [splitSubmenuOpen]);
	useEffect(() => {
		const close = () => onClose();
		const closeForEscape = (event: KeyboardEvent) => {
			if (event.key !== 'Escape') return;
			event.preventDefault();
			close();
		};
		window.addEventListener('pointerdown', close);
		window.addEventListener('blur', close);
		window.addEventListener('keydown', closeForEscape);
		return () => {
			window.removeEventListener('pointerdown', close);
			window.removeEventListener('blur', close);
			window.removeEventListener('keydown', closeForEscape);
		};
	}, [onClose]);
	const run = (action: () => void) => () => {
		action();
		onClose();
	};
	const moveMenuFocus = (event: React.KeyboardEvent<HTMLDivElement>, container: HTMLDivElement) => {
		const items = Array.from(container.children).filter(
			(child): child is HTMLButtonElement => child instanceof HTMLButtonElement && !child.disabled,
		);
		if (items.length === 0) return;
		const currentIndex =
			document.activeElement instanceof HTMLButtonElement
				? items.indexOf(document.activeElement)
				: -1;
		let nextIndex: number | null = null;
		if (event.key === 'ArrowDown')
			nextIndex = currentIndex < 0 ? 0 : (currentIndex + 1) % items.length;
		else if (event.key === 'ArrowUp')
			nextIndex =
				currentIndex < 0 ? items.length - 1 : (currentIndex - 1 + items.length) % items.length;
		else if (event.key === 'Home') nextIndex = 0;
		else if (event.key === 'End') nextIndex = items.length - 1;
		if (nextIndex === null) return;
		event.preventDefault();
		event.stopPropagation();
		items[nextIndex]?.focus();
	};
	const vertical = orientation === TabStripOrientation.Vertical;
	const style = contextMenuStyle(state);
	if (state.kind === 'combo-start' || state.kind === 'combo-end') {
		const combo = comboButtonController.getSnapshot();
		const isStart = state.kind === 'combo-start';
		const pinned = isStart ? combo.startPinned : combo.tabSearchPinned;
		let noun = 'Search tabs';
		if (isStart) {
			noun = combo.startKind === TabStripComboStartKind.EverythingMenu ? 'Tab groups' : 'Projects';
		}
		return createPortal(
			<div
				className='ct-menu'
				role='menu'
				aria-label={`${noun} menu`}
				style={style}
				onPointerDown={(event: ReactPointerEvent<HTMLDivElement>) => event.stopPropagation()}
			>
				<button
					role='menuitemcheckbox'
					aria-checked={pinned}
					onClick={run(() =>
						comboButtonController.executeTogglePinned(isStart ? 'start' : 'tab-search'),
					)}
				>
					{pinned ? `Unpin ${noun}` : `Pin ${noun}`}
				</button>
			</div>,
			document.body,
		);
	}
	if (state.kind === 'collapse') {
		return createPortal(
			<div
				className='ct-menu'
				role='menu'
				aria-label='Vertical tab strip menu'
				style={style}
				onPointerDown={(event: ReactPointerEvent<HTMLDivElement>) => event.stopPropagation()}
			>
				<button
					role='menuitemcheckbox'
					aria-checked={verticalExpandOnHover}
					onClick={run(() => onVerticalExpandOnHoverChange?.(!verticalExpandOnHover))}
				>
					{'Expand tab strip on hover'}
				</button>
			</div>,
			document.body,
		);
	}
	if (state.kind === 'new-tab') {
		const recentGroup =
			snapshot.mostRecentGroupId === null
				? null
				: (snapshot.groups.find((group) => group.id === snapshot.mostRecentGroupId) ?? null);
		const recentGroupTabCount = recentGroup
			? snapshot.tabs.filter((tab) => tab.group === recentGroup.id).length
			: 0;
		let recentGroupLabel = 'New tab in group (no groups)';
		if (recentGroup) {
			recentGroupLabel = recentGroup.title
				? `New tab in ${recentGroup.title}`
				: `New tab in ${recentGroupTabCount} ${recentGroupTabCount === 1 ? 'tab' : 'tabs'}`;
		}
		const active = snapshot.tabs.find((tab) => tab.id === snapshot.activeTabId) ?? null;
		return createPortal(
			<div
				className='ct-menu'
				role='menu'
				aria-label='New tab menu'
				style={style}
				onPointerDown={(event: ReactPointerEvent<HTMLDivElement>) => event.stopPropagation()}
			>
				<button
					role='menuitem'
					onClick={run(() =>
						model.addTab({
							title: 'New Tab',
							pinned: false,
							group: null,
							split: null,
						}),
					)}
				>
					{'New tab'}
				</button>

				<button
					role='menuitem'
					disabled={!recentGroup}
					onClick={run(() => {
						if (recentGroup)
							model.addTab({
								title: 'New Tab',
								pinned: false,
								group: recentGroup.id,
								split: null,
							});
					})}
				>
					{recentGroupLabel}
				</button>

				<div className='ct-menu__separator' />
				<button role='menuitem' onClick={run(() => createNewTabGroup(model))}>
					{'New group'}
				</button>

				<div className='ct-menu__separator' />
				<button
					role='menuitem'
					disabled={!active || active.split !== null}
					onClick={run(() => createSplitTabWithTab(model))}
				>
					{'New split tab with current'}
				</button>
			</div>,
			document.body,
		);
	}
	if (state.kind === 'group') {
		const group = snapshot.groups.find((candidate) => candidate.id === state.id);
		if (!group) return null;
		// Chromium's shared-group commands live inside the saved-group branch.
		// Treat externally supplied shared data as saved even when a host omitted
		// the redundant `saved` bit.
		const savedGroup = group.saved || group.shared;
		const groupTabs = snapshot.tabs.filter((tab) => tab.group === group.id);
		const members = group.members ?? [];
		const activity = group.recentActivity ?? [];
		const activityStyle: CSSProperties = {
			left:
				typeof window === 'undefined'
					? state.x + 268
					: Math.max(8, Math.min(state.x + 268, window.innerWidth - 368)),
			top:
				typeof window === 'undefined'
					? state.y
					: Math.max(8, Math.min(state.y, window.innerHeight - 420)),
		};
		const executeActivity = (item: Readonly<TabGroupActivity>) => {
			if (
				item.action === 'focus-tab' &&
				item.tabId &&
				snapshot.tabs.some((tab) => tab.id === item.tabId)
			) {
				model.selectTab(item.tabId);
			} else if (item.action === 'reopen-tab' && item.url) {
				model.addTab(
					{
						title: item.title,
						url: item.url,
						pinned: false,
						group: group.id,
						split: null,
					},
					{
						activate: false,
					},
				);
			} else if (item.action === 'manage-sharing') {
				onManageGroupSharing?.(group.id);
			}
			onGroupActivityAction?.(group.id, item);
			setActivityOpen(false);
		};
		const commitGroupAction = (action: 'close' | 'ungroup') => {
			if (action === 'close')
				requestCloseTabs(
					groupTabs.map((tab) => tab.id),
					'non-ui',
					true,
				);
			else model.ungroup(group.id);
			onClose();
		};
		if (pendingSavedGroupAction) {
			return createPortal(
				<div
					className='ct-saved-group-confirmation'
					role='alertdialog'
					aria-modal='true'
					aria-labelledby='ct-saved-group-confirmation-title'
					style={style}
					onPointerDown={(event: ReactPointerEvent<HTMLDivElement>) => event.stopPropagation()}
				>
					<strong id='ct-saved-group-confirmation-title'>
						{pendingSavedGroupAction === 'close' ? 'Close saved group?' : 'Ungroup saved group?'}
					</strong>
					<span>{'This group is saved. You can cancel to keep it unchanged.'}</span>

					<div className='ct-saved-group-confirmation__actions'>
						<button type='button' onClick={() => setPendingSavedGroupAction(null)}>
							{'Cancel'}
						</button>

						<button type='button' onClick={() => commitGroupAction(pendingSavedGroupAction)}>
							{pendingSavedGroupAction === 'close' ? 'Close group' : 'Ungroup'}
						</button>
					</div>
				</div>,
				document.body,
			);
		}
		return createPortal(
			<>
				<div
					className='ct-menu ct-group-editor'
					role='menu'
					style={style}
					onPointerDown={(event: ReactPointerEvent<HTMLDivElement>) => event.stopPropagation()}
				>
					<input
						className='ct-group-editor__title'
						aria-label='Group name'
						autoFocus={true}
						value={group.title}
						placeholder='Name this group'
						onChange={(event: ReactChangeEvent<HTMLInputElement>) =>
							model.updateGroup(group.id, {
								title: event.target.value,
							})
						}
						onKeyDown={(event: ReactKeyboardEvent<HTMLInputElement>) => {
							if (event.key !== 'Enter' && event.key !== 'Escape') return;
							event.preventDefault();
							event.stopPropagation();
							onClose();
						}}
					/>

					<div className='ct-group-editor__colors' role='radiogroup' aria-label='Group color'>
						{GROUP_COLOR_NAMES.map((color) => (
							<button
								key={color}
								role='radio'
								aria-checked={group.color === color}
								aria-label={color}
								tabIndex={group.color === color ? 0 : -1}
								className='ct-color'
								data-selected={group.color === color || undefined}
								style={
									{
										'--ct-color': GROUP_COLORS[color],
									} satisfies CSSProperties & Record<'--ct-color', string>
								}
								onClick={() =>
									model.updateGroup(group.id, {
										color,
									})
								}
								onKeyDown={(event: ReactKeyboardEvent<HTMLButtonElement>) => {
									const currentIndex = GROUP_COLOR_NAMES.indexOf(color);
									let offset = 0;
									if (event.key === 'ArrowLeft' || event.key === 'ArrowUp') offset = -1;
									else if (event.key === 'ArrowRight' || event.key === 'ArrowDown') offset = 1;
									let nextIndex =
										offset === 0
											? currentIndex
											: (currentIndex + offset + GROUP_COLOR_NAMES.length) %
												GROUP_COLOR_NAMES.length;
									if (event.key === 'Home') nextIndex = 0;
									else if (event.key === 'End') nextIndex = GROUP_COLOR_NAMES.length - 1;
									else if (offset === 0) return;
									event.preventDefault();
									const nextColor = requireDefined(GROUP_COLOR_NAMES[nextIndex]);
									model.updateGroup(group.id, {
										color: nextColor,
									});
									const radios =
										event.currentTarget.parentElement?.querySelectorAll<HTMLButtonElement>(
											'[role="radio"]',
										);
									radios?.[nextIndex]?.focus();
								}}
							/>
						))}
					</div>

					<div className='ct-menu__separator' />
					<button
						role='menuitem'
						onClick={run(() =>
							model.addTab({
								title: 'New Tab',
								pinned: false,
								group: group.id,
								split: null,
							}),
						)}
					>
						<GroupEditorMenuIcon kind='new-tab' /> <span>{'New tab in group'}</span>
					</button>
					{enableTabGroupHome ? (
						<button
							role='menuitem'
							onClick={run(() => {
								if (onOpenTabGroupHome) onOpenTabGroupHome(group.id);
								else
									model.addTab({
										title: 'Tab Group Home',
										url: 'chrome://tab-group-home/',
										pinned: false,
										group: null,
										split: null,
									});
							})}
						>
							<GroupEditorMenuIcon kind='home' /> <span>{'Open tab group home'}</span>
						</button>
					) : null}
					{onMoveGroupToNewWindow && snapshot.tabs.some((tab) => tab.group !== group.id) ? (
						<button role='menuitem' onClick={run(() => onMoveGroupToNewWindow(group.id))}>
							<GroupEditorMenuIcon kind='move-window' />
							<span>{'Move group to new window'}</span>
						</button>
					) : null}
					{savedGroup && enableTabGroupFocusing ? (
						<button
							role='menuitem'
							onClick={run(() =>
								model.setFocusedGroup(snapshot.focusedGroupId === group.id ? null : group.id),
							)}
						>
							<GroupEditorMenuIcon kind='focus' />
							<span>{snapshot.focusedGroupId === group.id ? 'Unfocus group' : 'Focus group'}</span>
						</button>
					) : null}
					{group.shared ? (
						<>
							<ManageSharingRow
								members={members}
								onClick={run(() => onManageGroupSharing?.(group.id))}
							/>

							<button
								role='menuitem'
								onClick={() => {
									setActivityOpen(true);
									if (group.attention)
										model.updateGroup(group.id, {
											attention: false,
										});
								}}
							>
								<GroupEditorMenuIcon kind='activity' />
								<span>{'Recent activity'}</span>
							</button>
						</>
					) : null}
					{savedGroup && !group.shared && onShareGroup ? (
						<button role='menuitem' onClick={run(() => onShareGroup(group.id))}>
							<GroupEditorMenuIcon kind='share' /> <span>{'Share group'}</span>
						</button>
					) : null}
					{savedGroup ? (
						<button
							role='menuitem'
							disabled={!closeAllowed}
							onClick={() => {
								if (confirmSavedGroupDestructiveActions) setPendingSavedGroupAction('close');
								else commitGroupAction('close');
							}}
						>
							<GroupEditorMenuIcon kind='close' /> <span>{'Close group'}</span>
						</button>
					) : null}
					{onAskGeminiAboutGroup ? (
						<>
							<div className='ct-menu__separator' />
							<button role='menuitem' onClick={run(() => onAskGeminiAboutGroup(group.id))}>
								<GroupEditorMenuIcon kind='gemini' /> <span>{'Ask Gemini'}</span>
							</button>
						</>
					) : null}
					{savedGroup ? (
						<>
							<div className='ct-menu__separator' />
							{!group.shared ? (
								<button
									role='menuitem'
									onClick={() => {
										if (confirmSavedGroupDestructiveActions) setPendingSavedGroupAction('ungroup');
										else commitGroupAction('ungroup');
									}}
								>
									<GroupEditorMenuIcon kind='ungroup' /> <span>{'Ungroup'}</span>
								</button>
							) : null}
							{!group.shared && onConvertGroupToBookmark && bookmarksEditable ? (
								<button
									role='menuitem'
									onClick={() => {
										void Promise.resolve(onConvertGroupToBookmark(group.id, groupTabs))
											.then((converted) => {
												if (converted !== false && model.getGroup(group.id))
													model.ungroup(group.id);
											})
											.catch(() => undefined)
											.finally(onClose);
									}}
								>
									<GroupEditorMenuIcon kind='bookmark' />
									<span>{'Convert group to bookmark'}</span>
								</button>
							) : null}

							<button
								role='menuitem'
								onClick={run(() => {
									if (onDeleteSavedGroup) onDeleteSavedGroup(group.id);
									else model.closeGroup(group.id);
								})}
							>
								<GroupEditorMenuIcon kind='delete' />
								<span>{group.shared ? 'Leave group' : 'Delete group'}</span>
							</button>
						</>
					) : (
						<>
							<div className='ct-menu__separator' />
							<button role='menuitem' onClick={() => commitGroupAction('ungroup')}>
								<GroupEditorMenuIcon kind='ungroup' /> <span>{'Ungroup'}</span>
							</button>

							<button
								role='menuitem'
								disabled={!closeAllowed}
								onClick={() => commitGroupAction('close')}
							>
								<GroupEditorMenuIcon kind='close' /> <span>{'Close group'}</span>
							</button>
						</>
					)}
					{savedGroup && showSavedGroupFooter ? (
						<div className='ct-group-editor__footer'>
							<span>
								{'Your tab groups are saved automatically and updated on your signed-in devices.'}
							</span>

							<button type='button' onClick={() => onOpenTabGroupsLearnMore?.()}>
								{'Learn more about tab groups'}
							</button>
						</div>
					) : null}
				</div>
				{activityOpen ? (
					<RecentActivityBubble
						activity={activity}
						style={activityStyle}
						onAction={executeActivity}
						onSeeAll={() => onSeeAllGroupActivity?.(group.id)}
						onClose={() => setActivityOpen(false)}
					/>
				) : null}
			</>,
			document.body,
		);
	}
	const tab = snapshot.tabs.find((candidate) => candidate.id === state.id);
	if (!tab) return null;
	const index = snapshot.tabs.findIndex((candidate) => candidate.id === tab.id);
	// TabStripModel::GetTabsForCommand applies a context-menu command to the
	// complete selection, or to the complete split when an unselected split tab
	// was invoked. Keep this separate from TabMenuModel's display count: Chrome
	// deliberately still uses a singular menu label for an unselected split.
	const invokedTabIsSelected = snapshot.selectedTabIds.has(tab.id);
	let commandTabs: TabData[];
	if (invokedTabIsSelected) {
		commandTabs = snapshot.tabs.filter((candidate) => snapshot.selectedTabIds.has(candidate.id));
	} else if (tab.split !== null) {
		commandTabs = snapshot.tabs.filter((candidate) => candidate.split === tab.split);
	} else commandTabs = [tab];
	const commandTabIds = commandTabs.map((candidate) => candidate.id);
	const groupableTabIds = commandTabIds;
	// TabMenuModel calculates plural labels and the initial mute direction from
	// the selected indices, but an unselected split still displays as one item.
	const menuDisplayTabs = invokedTabIsSelected
		? snapshot.tabs.filter((candidate) => snapshot.selectedTabIds.has(candidate.id))
		: [tab];
	const menuDisplayTabCount = menuDisplayTabs.length;
	const willPin = commandTabs.some((candidate) => !candidate.pinned);
	const willMuteSites = menuDisplayTabs.some((candidate) => !candidate.siteMuted);
	const canToggleAudio = enableTabMuting && commandTabs.some((candidate) => Boolean(candidate.url));
	// Port of TabStripModel::GetTabsClosedByCommand. Selected context tabs are
	// all preserved by "close other", and selected "close right" starts after
	// the selection's trailing edge. An unselected split is one exclusion unit.
	const selectedIndices = snapshot.tabs.flatMap((candidate, candidateIndex) =>
		snapshot.selectedTabIds.has(candidate.id) ? [candidateIndex] : [],
	);
	const lastUnclosedForTrailing = invokedTabIsSelected ? (selectedIndices.at(-1) ?? index) : index;
	const invokedSplitId = !invokedTabIsSelected ? tab.split : null;
	const getRelativeCloseTabIds = (lastUnclosedTab: number) =>
		snapshot.tabs.flatMap((candidate, candidateIndex) => {
			if (candidateIndex <= lastUnclosedTab || candidate.pinned) return [];
			if (candidate.id === tab.id) return [];
			if (invokedSplitId !== null && candidate.split === invokedSplitId) return [];
			if (invokedTabIsSelected && snapshot.selectedTabIds.has(candidate.id)) return [];
			return [candidate.id];
		});
	const closeOtherTabIds = getRelativeCloseTabIds(-1);
	const closeTrailingTabIds = getRelativeCloseTabIds(lastUnclosedForTrailing);
	const activeTab =
		snapshot.tabs.find((candidate) => candidate.id === snapshot.activeTabId) ?? null;
	const invokedSplitTabs =
		tab.split === null ? [] : snapshot.tabs.filter((candidate) => candidate.split === tab.split);
	const activeSplitTabs =
		activeTab?.split === null || activeTab?.split === undefined
			? []
			: snapshot.tabs.filter((candidate) => candidate.split === activeTab.split);
	const canCreateSplit =
		menuDisplayTabCount <= 2 && tab.split === null && activeTab?.split === null;
	const canMoveToNewWindow = snapshot.tabs.length > commandTabs.length;
	const readingListTabs = commandTabs.filter((candidate) => {
		try {
			const protocol = candidate.url ? new URL(candidate.url).protocol : '';
			return protocol === 'http:' || protocol === 'https:';
		} catch {
			return false;
		}
	});
	const submenuStyle: CSSProperties = {
		left:
			typeof window === 'undefined'
				? state.x + 252
				: Math.max(8, Math.min(state.x + 252, window.innerWidth - 268)),
		top:
			typeof window === 'undefined'
				? state.y + 64
				: Math.max(8, Math.min(state.y + 64, window.innerHeight - 300)),
	};
	const splitSubmenuStyle: CSSProperties = {
		...submenuStyle,
		top:
			typeof window === 'undefined'
				? state.y + 32
				: Math.max(8, Math.min(state.y + 32, window.innerHeight - 220)),
	};
	return createPortal(
		<div
			ref={tabMenuRef}
			className='ct-menu'
			role='menu'
			tabIndex={-1}
			style={style}
			onPointerDown={(event: ReactPointerEvent<HTMLDivElement>) => event.stopPropagation()}
			onKeyDown={(event: ReactKeyboardEvent<HTMLDivElement>) => {
				if (event.key === 'ArrowRight' && event.target === splitTriggerRef.current) {
					event.preventDefault();
					event.stopPropagation();
					enterSplitSubmenuFromKeyboard.current = true;
					setSplitSubmenuOpen(true);
					setTabGroupSubmenuOpen(false);
					return;
				}
				if (event.key === 'ArrowRight' && event.target === tabGroupTriggerRef.current) {
					event.preventDefault();
					event.stopPropagation();
					enterTabGroupSubmenuFromKeyboard.current = true;
					setTabGroupSubmenuOpen(true);
					setSplitSubmenuOpen(false);
					return;
				}
				moveMenuFocus(event, event.currentTarget);
			}}
		>
			<button
				role='menuitem'
				onClick={run(() =>
					model.addTab(
						{
							title: 'New Tab',
							group: tab.group,
							pinned: false,
							split: null,
						},
						{
							index: index + 1,
						},
					),
				)}
			>
				{vertical ? 'New tab below' : 'New tab to the right'}
			</button>
			{(() => {
				if (tab.split !== null)
					return (
						<>
							<button
								ref={splitTriggerRef}
								role='menuitem'
								aria-haspopup='menu'
								aria-expanded={splitSubmenuOpen}
								className='ct-menu__submenu-trigger'
								onMouseEnter={() => {
									setSplitSubmenuOpen(true);
									setTabGroupSubmenuOpen(false);
								}}
								onClick={() => {
									setSplitSubmenuOpen(true);
									setTabGroupSubmenuOpen(false);
								}}
							>
								{'Arrange split view'}
							</button>
							{splitSubmenuOpen ? (
								<div
									ref={splitSubmenuRef}
									className='ct-menu ct-menu--submenu'
									role='menu'
									aria-label='Arrange split view'
									style={splitSubmenuStyle}
									onPointerDown={(event: ReactPointerEvent<HTMLDivElement>) =>
										event.stopPropagation()
									}
									onKeyDown={(event: ReactKeyboardEvent<HTMLDivElement>) => {
										if (event.key === 'ArrowLeft' || event.key === 'Escape') {
											event.preventDefault();
											event.stopPropagation();
											setSplitSubmenuOpen(false);
											splitTriggerRef.current?.focus();
											return;
										}
										moveMenuFocus(event, event.currentTarget);
									}}
								>
									<button
										role='menuitem'
										onClick={run(() => model.reverseSplit(requireDefined(tab.split)))}
									>
										{'Reverse views'}
									</button>

									<div className='ct-menu__separator' />
									<button
										role='menuitem'
										disabled={!closeAllowed || invokedSplitTabs.length !== 2}
										onClick={run(() =>
											requestCloseTabs(invokedSplitTabs[0] ? [invokedSplitTabs[0].id] : []),
										)}
									>
										{'Close left view'}
									</button>

									<button
										role='menuitem'
										disabled={!closeAllowed || invokedSplitTabs.length !== 2}
										onClick={run(() =>
											requestCloseTabs(invokedSplitTabs[1] ? [invokedSplitTabs[1].id] : []),
										)}
									>
										{'Close right view'}
									</button>

									<div className='ct-menu__separator' />
									<button
										role='menuitem'
										onClick={run(() => model.removeSplit(requireDefined(tab.split)))}
									>
										{'Separate views'}
									</button>
								</div>
							) : null}
						</>
					);
				if (activeSplitTabs.length === 2)
					return (
						<>
							<button
								ref={splitTriggerRef}
								role='menuitem'
								disabled={menuDisplayTabCount !== 1}
								aria-haspopup='menu'
								aria-expanded={splitSubmenuOpen}
								className='ct-menu__submenu-trigger'
								onMouseEnter={() => {
									if (menuDisplayTabCount !== 1) return;
									setSplitSubmenuOpen(true);
									setTabGroupSubmenuOpen(false);
								}}
								onClick={() => {
									if (menuDisplayTabCount !== 1) return;
									setSplitSubmenuOpen(true);
									setTabGroupSubmenuOpen(false);
								}}
							>
								{'Swap with active split'}
							</button>
							{splitSubmenuOpen ? (
								<div
									ref={splitSubmenuRef}
									className='ct-menu ct-menu--submenu'
									role='menu'
									aria-label='Swap with active split'
									style={splitSubmenuStyle}
									onPointerDown={(event: ReactPointerEvent<HTMLDivElement>) =>
										event.stopPropagation()
									}
									onKeyDown={(event: ReactKeyboardEvent<HTMLDivElement>) => {
										if (event.key === 'ArrowLeft' || event.key === 'Escape') {
											event.preventDefault();
											event.stopPropagation();
											setSplitSubmenuOpen(false);
											splitTriggerRef.current?.focus();
											return;
										}
										moveMenuFocus(event, event.currentTarget);
									}}
								>
									<button
										role='menuitem'
										onClick={run(() =>
											model.swapTabIntoSplit(
												tab.id,
												requireDefined(requireDefined(activeTab).split),
												0,
											),
										)}
									>
										{'Swap left view'}
									</button>

									<button
										role='menuitem'
										onClick={run(() =>
											model.swapTabIntoSplit(
												tab.id,
												requireDefined(requireDefined(activeTab).split),
												1,
											),
										)}
									>
										{'Swap right view'}
									</button>
								</div>
							) : null}
						</>
					);
				return (
					<button
						role='menuitem'
						disabled={!canCreateSplit}
						onClick={run(() => createSplitFromTabContext(model, tab.id))}
					>
						{tab.id === snapshot.activeTabId
							? 'Add tab to new split view'
							: 'New split view with current tab'}
					</button>
				);
			})()}
			{groupableTabIds.length > 0 ? (
				<>
					<button
						ref={tabGroupTriggerRef}
						role='menuitem'
						aria-haspopup='menu'
						aria-expanded={tabGroupSubmenuOpen}
						className='ct-menu__submenu-trigger'
						onMouseEnter={() => {
							setTabGroupSubmenuOpen(true);
							setSplitSubmenuOpen(false);
						}}
						onClick={() => {
							setTabGroupSubmenuOpen(true);
							setSplitSubmenuOpen(false);
						}}
					>
						{menuDisplayTabCount === 1 ? 'Add tab to group' : 'Add tabs to group'}
					</button>
					{tabGroupSubmenuOpen ? (
						<div
							ref={tabGroupSubmenuRef}
							className='ct-menu ct-menu--submenu'
							role='menu'
							aria-label='Tab groups'
							style={submenuStyle}
							onPointerDown={(event: ReactPointerEvent<HTMLDivElement>) => event.stopPropagation()}
							onKeyDown={(event: ReactKeyboardEvent<HTMLDivElement>) => {
								if (event.key === 'ArrowLeft' || event.key === 'Escape') {
									event.preventDefault();
									event.stopPropagation();
									setTabGroupSubmenuOpen(false);
									tabGroupTriggerRef.current?.focus();
									return;
								}
								moveMenuFocus(event, event.currentTarget);
							}}
						>
							<button
								role='menuitem'
								onClick={run(() => {
									createGroupForTabs(model, groupableTabIds);
								})}
							>
								{'New group'}
							</button>
							{snapshot.groups
								.filter((group) => group.id !== tab.group)
								.map((group) => (
									<button
										key={group.id}
										role='menuitem'
										onClick={run(() => model.moveTabsToGroup(groupableTabIds, group.id))}
									>
										{group.title || 'Unnamed group'}
									</button>
								))}
						</div>
					) : null}
					{commandTabs.some((candidate) => candidate.group !== null) ? (
						<button
							role='menuitem'
							onClick={run(() => model.moveTabsToGroup(groupableTabIds, null))}
						>
							{'Remove from group'}
						</button>
					) : null}
				</>
			) : null}

			<button
				disabled={!canMoveToNewWindow || !onMoveTabsToNewWindow}
				role='menuitem'
				onClick={run(() => onMoveTabsToNewWindow?.(commandTabIds))}
			>
				{menuDisplayTabCount === 1 ? 'Move tab to new window' : 'Move tabs to new window'}
			</button>

			<div className='ct-menu__separator' />
			<button
				disabled={!onReloadTabs}
				role='menuitem'
				onClick={run(() => onReloadTabs?.(commandTabIds))}
			>
				{'Reload'}
			</button>

			<button
				role='menuitem'
				onClick={run(() => commandTabIds.forEach((id) => model.duplicateTab(id)))}
			>
				{'Duplicate'}
			</button>

			<button role='menuitem' onClick={run(() => model.setTabsPinned(commandTabIds, willPin))}>
				{willPin ? 'Pin' : 'Unpin'}
			</button>
			{canToggleAudio ? (
				<button
					role='menuitem'
					onClick={run(() => {
						commandTabs.forEach((candidate) => {
							const wasAudible =
								candidate.alertState === 'audio-playing' ||
								candidate.alertState === 'audio-muting' ||
								candidate.alertState === 'audio-muted';
							model.updateTab(candidate.id, {
								siteMuted: willMuteSites,
								...(wasAudible
									? {
											alertState: willMuteSites ? 'audio-muting' : 'audio-playing',
										}
									: {}),
							});
							onTabAudioMuteToggle?.(candidate.id, willMuteSites);
						});
					})}
				>{`${willMuteSites ? 'Mute' : 'Unmute'} ${menuDisplayTabCount === 1 ? 'site' : 'sites'}`}</button>
			) : null}

			<button
				disabled={readingListTabs.length === 0 || !onAddTabsToReadingList}
				role='menuitem'
				onClick={run(() => onAddTabsToReadingList?.(readingListTabs))}
			>{`Add ${menuDisplayTabCount} ${menuDisplayTabCount === 1 ? 'tab' : 'tabs'} to reading list`}</button>

			<div className='ct-menu__separator' />
			<button
				disabled={!closeAllowed}
				role='menuitem'
				onClick={run(() => requestCloseTabs(commandTabIds))}
			>
				{'Close'}
			</button>

			<button
				disabled={!closeAllowed || closeOtherTabIds.length === 0}
				role='menuitem'
				onClick={run(() => requestCloseTabs(closeOtherTabIds))}
			>
				{'Close other tabs'}
			</button>

			<button
				disabled={!closeAllowed || closeTrailingTabIds.length === 0}
				role='menuitem'
				onClick={run(() => requestCloseTabs(closeTrailingTabIds))}
			>
				{vertical ? 'Close tabs below' : 'Close tabs to the right'}
			</button>
		</div>,
		document.body,
	);
}
function VerticalDragTabVisual({
	tab,
	snapshot,
	width,
	collapsed,
	enableTabMuting,
	touchUi,
	closeButtonAllowed,
}: {
	tab: Readonly<TabData>;
	snapshot: TabModelSnapshot;
	width: number;
	collapsed: boolean;
	enableTabMuting: boolean;
	touchUi: boolean;
	closeButtonAllowed: boolean;
}) {
	return (
		<TabButton
			tab={tab}
			snapshot={snapshot}
			width={width}
			orientation={TabStripOrientation.Vertical}
			collapsed={collapsed}
			onPointerDown={() => undefined}
			onClose={() => undefined}
			onContextMenu={() => undefined}
			onMouseEnter={() => undefined}
			onMouseLeave={() => undefined}
			onKeyDown={() => undefined}
			onKeyUp={() => undefined}
			enableTabMuting={enableTabMuting}
			touchUi={touchUi}
			closeButtonAllowed={closeButtonAllowed}
			hoveredTabId={null}
			onAudioMuteToggle={() => undefined}
			dragging={true}
		/>
	);
}
function VerticalDragSplitVisual({
	tabs,
	snapshot,
	width,
	collapsed,
	enableTabMuting,
	touchUi,
	closeButtonAllowed,
}: {
	tabs: readonly [Readonly<TabData>, Readonly<TabData>];
	snapshot: TabModelSnapshot;
	width: number;
	collapsed: boolean;
	enableTabMuting: boolean;
	touchUi: boolean;
	closeButtonAllowed: boolean;
}) {
	const pinned = tabs[0].pinned;
	const childHeight = pinned
		? LAYOUT_CONSTANTS.verticalTabPinnedHeight
		: LAYOUT_CONSTANTS.verticalTabHeight;
	const layout = calculateSplitTabViewLayout({
		orientation: TabStripOrientation.Vertical,
		sizeBounds: {
			width,
			height: null,
		},
		children: [
			{
				width: Math.floor(width / 2),
				height: childHeight,
			},
			{
				width: Math.ceil(width / 2),
				height: childHeight,
			},
		],
		collapsed,
		pinned,
	});
	const splitLayout =
		layout.children[0]?.bounds.y === layout.children[1]?.bounds.y ? 'side-by-side' : 'stacked';
	return (
		<div
			className='ct-vertical-split'
			data-layout={splitLayout}
			data-pinned={pinned || undefined}
			data-split-id={tabs[0].split ?? undefined}
			style={{
				height: layout.hostSize.height,
			}}
		>
			{layout.children.map((child, index) => {
				const tab = requireDefined(tabs[index]);
				return (
					<div
						key={tab.id}
						className='ct-vertical-tab-wrap'
						data-split-child=''
						style={{
							left: child.bounds.x,
							top: child.bounds.y,
							width: child.bounds.width,
							height: child.bounds.height,
						}}
					>
						<TabButton
							tab={tab}
							snapshot={snapshot}
							width={child.bounds.width}
							orientation={TabStripOrientation.Vertical}
							collapsed={collapsed}
							splitLayout={splitLayout}
							onPointerDown={() => undefined}
							onClose={() => undefined}
							onContextMenu={() => undefined}
							onMouseEnter={() => undefined}
							onMouseLeave={() => undefined}
							onKeyDown={() => undefined}
							onKeyUp={() => undefined}
							enableTabMuting={enableTabMuting}
							touchUi={touchUi}
							closeButtonAllowed={closeButtonAllowed}
							hoveredTabId={null}
							onAudioMuteToggle={() => undefined}
							dragging={true}
						/>
					</div>
				);
			})}
		</div>
	);
}
function VerticalDragGroupVisual({
	group,
	tabs,
	snapshot,
	width,
	collapsedStrip,
	enableTabMuting,
	touchUi,
	closeButtonAllowed,
}: {
	group: Readonly<TabGroupData>;
	tabs: readonly Readonly<TabData>[];
	snapshot: TabModelSnapshot;
	width: number;
	collapsedStrip: boolean;
	enableTabMuting: boolean;
	touchUi: boolean;
	closeButtonAllowed: boolean;
}) {
	const renderTab = (tab: Readonly<TabData>) => (
		<div key={tab.id} className='ct-vertical-tab-wrap'>
			<TabButton
				tab={tab}
				snapshot={snapshot}
				width={Math.max(0, width - (collapsedStrip ? 12 : 10))}
				orientation={TabStripOrientation.Vertical}
				collapsed={collapsedStrip}
				onPointerDown={() => undefined}
				onClose={() => undefined}
				onContextMenu={() => undefined}
				onMouseEnter={() => undefined}
				onMouseLeave={() => undefined}
				onKeyDown={() => undefined}
				onKeyUp={() => undefined}
				enableTabMuting={enableTabMuting}
				touchUi={touchUi}
				closeButtonAllowed={closeButtonAllowed}
				hoveredTabId={null}
				onAudioMuteToggle={() => undefined}
				dragging={true}
			/>
		</div>
	);
	const contents: ReactNode[] = [];
	for (let index = 0; index < tabs.length; index += 1) {
		const tab = requireDefined(tabs[index]);
		const next = tabs[index + 1];
		if (tab.split !== null && next?.split === tab.split) {
			contents.push(
				<VerticalDragSplitVisual
					key={`split-${tab.split}`}
					tabs={[tab, next]}
					snapshot={snapshot}
					width={Math.max(0, width - (collapsedStrip ? 12 : 10))}
					collapsed={collapsedStrip}
					enableTabMuting={enableTabMuting}
					touchUi={touchUi}
					closeButtonAllowed={closeButtonAllowed}
				/>,
			);
			index += 1;
		} else {
			contents.push(renderTab(tab));
		}
	}
	return (
		<section
			className='ct-vertical-group'
			data-group-id={group.id}
			data-group-collapsed={group.collapsed || undefined}
			style={
				{
					width,
					'--ct-group-color': GROUP_COLORS[group.color],
				} satisfies CSSProperties & Record<'--ct-group-color', string>
			}
		>
			<GroupHeader
				group={group}
				tabs={tabs}
				activeTabId={snapshot.activeTabId}
				orientation={TabStripOrientation.Vertical}
				collapsedStrip={collapsedStrip}
				onToggle={() => undefined}
				onPointerDown={() => undefined}
				onKeyDown={() => undefined}
				onContextMenu={() => undefined}
				onOpenEditor={() => undefined}
				onMouseEnter={() => undefined}
				onMouseLeave={() => undefined}
				showHoverCard={false}
			/>

			<div className='ct-vertical-group__tabs' aria-hidden={group.collapsed || undefined}>
				<div className='ct-vertical-group__tabs-inner'>{contents}</div>
			</div>
		</section>
	);
}
function DragGhost({
	drag,
	snapshot,
	orientation,
	collapsed,
	settleTargetLeft,
	enableTabMuting,
	touchUi,
	closeButtonAllowed,
	declutterEnabled,
}: {
	drag: DragState;
	snapshot: TabModelSnapshot;
	orientation: TabStripOrientation;
	collapsed: boolean;
	settleTargetLeft?: number | null;
	enableTabMuting: boolean;
	touchUi: boolean;
	closeButtonAllowed: boolean;
	declutterEnabled: boolean;
}) {
	const tabs = drag.tabIds.flatMap((id) => {
		const tab = snapshot.tabs.find((candidate) => candidate.id === id);
		return tab ? [tab] : [];
	});
	const vertical = orientation === TabStripOrientation.Vertical;
	const sourceTab = snapshot.tabs.find((tab) => tab.id === drag.sourceId) ?? null;
	const squash = vertical && (collapsed || sourceTab?.pinned === true);
	const sourceVisualId = getSourceVisualId(drag);
	const group =
		drag.groupHeaderId === null
			? null
			: (snapshot.groups.find((candidate) => candidate.id === drag.groupHeaderId) ?? null);
	const tabById = new Map(tabs.map((tab) => [tab.id, tab]));
	if (vertical) {
		const verticalVisualViews = getVerticalDragVisualViews(drag);
		const layout = buildDraggedTabsLayout({
			layout: squash ? 'squash' : 'vertical',
			pointerOffsetRatios: drag.pointerOffsetRatios,
			sourceSlotSize: drag.sourceSlotSize,
			sourceViewOffsetFromSlot: drag.sourceViewOffsetFromSlot,
			views: verticalVisualViews.map((view) => ({
				id: view.id,
				bounds: {
					x: view.x,
					y: view.y,
					width: view.width,
					height: view.height,
				},
				source: view.id === sourceVisualId,
			})),
		});
		const visualData = new Map(layout.views.map((view) => [view.id, view]));
		const shownViews = verticalVisualViews.filter((view) => !visualData.get(view.id)?.shouldHide);
		const sourceView =
			verticalVisualViews.find((view) => view.id === sourceVisualId) ?? verticalVisualViews[0];
		const sourceVisual = sourceView ? visualData.get(sourceView.id) : undefined;
		const canMoveHorizontally = sourceTab?.pinned === true && !collapsed;
		const unclampedLeft = canMoveHorizontally
			? drag.x + layout.boundsRelativeToPointer.x
			: drag.startX - drag.pointerOffsetFromVisualOrigin.x;
		const unclampedTop = drag.y + layout.boundsRelativeToPointer.y;
		const clampingRightPadding = LAYOUT_CONSTANTS.verticalTabStripHorizontalPadding;
		const ghostLeft = Math.min(
			Math.max(
				drag.dragAreaBounds.left,
				drag.dragAreaBounds.left +
					drag.dragAreaBounds.width -
					clampingRightPadding -
					layout.boundsRelativeToPointer.width,
			),
			Math.max(drag.dragAreaBounds.left, unclampedLeft),
		);
		const ghostTop = Math.min(
			Math.max(
				drag.dragAreaBounds.top,
				drag.dragAreaBounds.top +
					drag.dragAreaBounds.height -
					layout.boundsRelativeToPointer.height,
			),
			Math.max(drag.dragAreaBounds.top, unclampedTop),
		);
		return (
			<div
				className='ct-drag-ghost'
				data-layout={squash ? 'squash' : 'vertical'}
				style={
					{
						left: ghostLeft,
						top: ghostTop,
						width: layout.boundsRelativeToPointer.width,
						height: layout.boundsRelativeToPointer.height,
						'--ct-group-color': group ? GROUP_COLORS[group.color] : undefined,
					} satisfies CSSProperties & Record<'--ct-group-color', string | undefined>
				}
				aria-hidden='true'
			>
				{shownViews.map((view) => {
					const visual = visualData.get(view.id);
					if (!visual) return null;
					const dragStartOffsetX =
						sourceView && sourceVisual
							? view.x - sourceView.x - (visual.offset.x - sourceVisual.offset.x)
							: 0;
					const dragStartOffsetY =
						sourceView && sourceVisual
							? view.y - sourceView.y - (visual.offset.y - sourceVisual.offset.y)
							: 0;
					const compacting = dragStartOffsetX !== 0 || dragStartOffsetY !== 0;
					const tab =
						view.kind === 'tab'
							? (tabById.get(view.id) ?? tabById.get(view.tabIds?.[0] ?? '') ?? null)
							: null;
					const draggedGroup =
						view.kind === 'group'
							? (snapshot.groups.find((candidate) => `group:${candidate.id}` === view.id) ?? null)
							: null;
					const draggedGroupTabs = draggedGroup
						? (view.tabIds ?? []).flatMap((id) => {
								const candidate = tabById.get(id);
								return candidate ? [candidate] : [];
							})
						: [];
					const splitTabs =
						view.kind === 'tab'
							? (view.tabIds ?? []).flatMap((id) => {
									const candidate = tabById.get(id);
									return candidate ? [candidate] : [];
								})
							: [];
					let visualContent: ReactNode;
					if (view.kind === 'group' && draggedGroup) {
						visualContent = (
							<VerticalDragGroupVisual
								group={draggedGroup}
								tabs={draggedGroupTabs}
								snapshot={snapshot}
								width={view.width}
								collapsedStrip={collapsed}
								enableTabMuting={enableTabMuting}
								touchUi={touchUi}
								closeButtonAllowed={closeButtonAllowed}
							/>
						);
					} else if (view.kind === 'group') {
						visualContent = <span>{drag.label}</span>;
					} else if (splitTabs.length === 2) {
						visualContent = (
							<VerticalDragSplitVisual
								tabs={[requireDefined(splitTabs[0]), requireDefined(splitTabs[1])]}
								snapshot={snapshot}
								width={view.width}
								collapsed={collapsed}
								enableTabMuting={enableTabMuting}
								touchUi={touchUi}
								closeButtonAllowed={closeButtonAllowed}
							/>
						);
					} else if (tab) {
						visualContent = (
							<VerticalDragTabVisual
								tab={tab}
								snapshot={snapshot}
								width={view.width}
								collapsed={collapsed}
								enableTabMuting={enableTabMuting}
								touchUi={touchUi}
								closeButtonAllowed={closeButtonAllowed}
							/>
						);
					} else {
						visualContent = <span>{drag.label}</span>;
					}
					let viewClassName = 'ct-drag-ghost__tab-view';
					if (view.kind === 'group') {
						viewClassName = draggedGroup ? 'ct-drag-ghost__group-view' : 'ct-drag-ghost__group';
					} else if (splitTabs.length === 2) viewClassName = 'ct-drag-ghost__split';
					return (
						<div
							key={view.id}
							className={viewClassName}
							data-source={view.id === sourceVisualId || undefined}
							data-compacting={compacting || undefined}
							style={
								{
									left: visual.offset.x,
									top: visual.offset.y,
									width: view.width,
									height: view.height,
									'--ct-drag-initial-offset-x': `${dragStartOffsetX}px`,
									'--ct-drag-initial-offset-y': `${dragStartOffsetY}px`,
								} satisfies CSSProperties &
									Record<'--ct-drag-initial-offset-x' | '--ct-drag-initial-offset-y', string>
							}
						>
							{visualContent}
						</div>
					);
				})}
				{squash && tabs.length > 1 ? (
					<span className='ct-drag-ghost__count'>{tabs.length}</span>
				) : null}
			</div>
		);
	}

	// TabStrip::MaybeStartDrag always places the header and every group tab in
	// the drag session. kCollapseTabGroupDuringDrag only collapses the backing
	// model while an originally-expanded group is in flight; those captured tab
	// views remain visible and move as one unit. A group that was already
	// collapsed is the only header drag whose painted surface stays compact.
	const compactGroupHeaderDrag = drag.groupHeaderId !== null && drag.groupWasCollapsedBeforeDrag;
	const horizontalSourceViews = compactGroupHeaderDrag
		? drag.visualViews.filter((view) => view.kind === 'group')
		: drag.visualViews;
	const packedHorizontalDrag = packHorizontalDragViews({
		views: horizontalSourceViews,
		sourceVisualId,
		pointerRatioX: drag.pointerOffsetRatios.x,
		...(compactGroupHeaderDrag && group
			? {
					sourceWidthOverride: groupHeaderWidth(group, tabs.length),
				}
			: {}),
	});
	const horizontalViews = packedHorizontalDrag.views;
	const width = packedHorizontalDrag.width;
	const height = LAYOUT_CONSTANTS.tabStripHeight;
	const horizontalMouseOffset =
		packedHorizontalDrag.mouseOffset || drag.pointerOffsetFromVisualOrigin.x;
	const unclampedGhostLeft =
		drag.settling && settleTargetLeft !== null && settleTargetLeft !== undefined
			? settleTargetLeft
			: drag.x - horizontalMouseOffset;
	const ghostLeft = Math.min(
		Math.max(
			drag.dragAreaBounds.left,
			drag.dragAreaBounds.left + drag.dragAreaBounds.width - width,
		),
		Math.max(drag.dragAreaBounds.left, unclampedGhostLeft),
	);
	const draggedGroupGeometry = snapshot.groups.flatMap((draggedGroup) => {
		const groupViews = horizontalViews.filter((view) =>
			view.kind === 'group'
				? view.id === `group:${draggedGroup.id}`
				: tabById.get(view.id)?.group === draggedGroup.id,
		);
		if (groupViews.length === 0) return [];
		const hasDraggedTab = groupViews.some((view) => view.kind === 'tab');
		const leading = groupViews.reduce((result, view) => (view.x < result.x ? view : result));
		const trailing = groupViews.reduce((result, view) =>
			view.x + view.width > result.x + result.width ? view : result,
		);
		const leadingTab = leading.kind === 'tab' ? tabById.get(leading.id) : null;
		const trailingTab = trailing.kind === 'tab' ? tabById.get(trailing.id) : null;
		const naturalUnderlineLeft =
			leading.x + (leadingTab && tabIsApparentlyActive(leadingTab, snapshot) ? -2 : 18);
		const naturalUnderlineRight =
			trailing.x +
			trailing.width +
			(trailingTab && tabIsApparentlyActive(trailingTab, snapshot) ? 2 : -18);
		const draggedUnderline = {
			x: naturalUnderlineLeft,
			width: Math.max(0, naturalUnderlineRight - naturalUnderlineLeft),
		};
		const normalUnderline = drag.groupUnderlineBounds[draggedGroup.id];
		const underline = normalUnderline
			? mergeDraggedGroupUnderlineBounds({
					normal: {
						x: normalUnderline.x - ghostLeft,
						width: normalUnderline.width,
					},
					dragged: draggedUnderline,
					leadingDraggedView: leading,
					trailingDraggedView: trailing,
				})
			: draggedUnderline;
		return [
			{
				group: draggedGroup,
				highlight:
					!drag.settling && groupViews.some((view) => view.id === `group:${draggedGroup.id}`)
						? {
								x: leading.x,
								width: trailing.x + trailing.width - leading.x,
							}
						: null,
				underline: hasDraggedTab
					? underline
					: {
							x: 0,
							width: 0,
						},
			},
		];
	});
	return (
		<div
			className='ct-drag-ghost'
			data-layout='horizontal'
			data-settling={drag.settling || undefined}
			style={{
				left: ghostLeft,
				top: drag.dragAreaBounds.top,
				width,
				height,
			}}
			aria-hidden='true'
		>
			{draggedGroupGeometry.map(({ group: draggedGroup, highlight }) =>
				highlight && highlight.width > 0 ? (
					<svg
						key={`highlight-${draggedGroup.id}`}
						className='ct-drag-ghost__highlight'
						viewBox={`0 0 ${highlight.width} ${height}`}
						width={highlight.width}
						height={height}
						style={{
							left: highlight.x,
						}}
					>
						<path d={getTabGroupHighlightPath(highlight.width, height)} />
					</svg>
				) : null,
			)}
			{horizontalViews.map((view) => {
				const tab = view.kind === 'tab' ? tabById.get(view.id) : null;
				const draggedGroupId = view.kind === 'group' ? view.id.slice('group:'.length) : null;
				const draggedGroup =
					draggedGroupId === null
						? null
						: (snapshot.groups.find((candidate) => candidate.id === draggedGroupId) ?? null);
				return (
					<div
						key={view.id}
						className={view.kind === 'group' ? 'ct-drag-ghost__group' : 'ct-drag-ghost__item'}
						data-source={view.id === sourceVisualId || undefined}
						data-compacting={view.initialOffsetX !== 0 || undefined}
						style={
							{
								left: view.x,
								top: view.y,
								width: view.width,
								height: view.height,
								'--ct-drag-initial-offset-x': `${view.initialOffsetX}px`,
							} satisfies CSSProperties & Record<'--ct-drag-initial-offset-x', string>
						}
					>
						{(() => {
							if (view.kind === 'group' && draggedGroup)
								return (
									<GroupHeader
										group={draggedGroup}
										tabs={snapshot.tabs.filter((candidate) => candidate.group === draggedGroup.id)}
										activeTabId={compactGroupHeaderDrag ? snapshot.activeTabId : null}
										orientation={TabStripOrientation.Horizontal}
										collapsedStrip={false}
										onToggle={() => undefined}
										onPointerDown={() => undefined}
										onKeyDown={() => undefined}
										onContextMenu={() => undefined}
										onMouseEnter={() => undefined}
										onMouseLeave={() => undefined}
										showHoverCard={false}
									/>
								);
							if (tab)
								return (
									<TabButton
										tab={tab}
										snapshot={snapshot}
										width={view.width}
										orientation={TabStripOrientation.Horizontal}
										collapsed={false}
										onPointerDown={() => undefined}
										onClose={() => undefined}
										onContextMenu={() => undefined}
										onMouseEnter={() => undefined}
										onMouseLeave={() => undefined}
										onKeyDown={() => undefined}
										onKeyUp={() => undefined}
										enableTabMuting={enableTabMuting}
										touchUi={touchUi}
										closeButtonAllowed={closeButtonAllowed}
										declutterEnabled={declutterEnabled}
										hoveredTabId={null}
										onAudioMuteToggle={() => undefined}
									/>
								);
							return null;
						})()}
					</div>
				);
			})}
			{draggedGroupGeometry.map(({ group: draggedGroup, underline }) =>
				underline.width > 0 ? (
					<span
						key={`underline-${draggedGroup.id}`}
						className='ct-drag-ghost__underline'
						data-group-underline-id={draggedGroup.id}
						style={
							{
								left: underline.x,
								width: underline.width,
								'--ct-group-color': GROUP_COLORS[draggedGroup.color],
							} satisfies CSSProperties & Record<'--ct-group-color', string>
						}
					/>
				) : null,
			)}
		</div>
	);
}
function HoverCard({
	hover,
	snapshot,
	showPreviews,
	showMemoryUsage,
	transitioning,
	thumbnailSnapshot,
}: {
	hover: HoverState;
	snapshot: TabModelSnapshot;
	showPreviews: boolean;
	showMemoryUsage: boolean;
	transitioning: boolean;
	thumbnailSnapshot: Readonly<HoverCardThumbnailSnapshot<string>>;
}) {
	if (hover.kind === 'group') {
		const group = snapshot.groups.find((candidate) => candidate.id === hover.id);
		if (!group) return null;
		const tabs = snapshot.tabs.filter((tab) => tab.group === group.id);
		const data = getGroupHoverCardData(group, tabs);
		return createPortal(
			<div
				className='ct-hover-card ct-hover-card--group'
				style={{
					left: hover.x,
					top: hover.y,
				}}
				data-transitioning={transitioning || undefined}
			>
				<div className='ct-hover-card__body'>
					<strong>{data.title.text}</strong>
					<ul className='ct-hover-card__group-tabs'>
						{data.tabTitles.map((title, index) => (
							<li key={tabs[index]?.id ?? index} data-filename={title.isFilename || undefined}>
								{title.text}
							</li>
						))}
					</ul>
					{data.excessText ? (
						<span className='ct-hover-card__group-excess'>{data.excessText}</span>
					) : null}
				</div>
			</div>,
			document.body,
		);
	}
	const tab = snapshot.tabs.find((candidate) => candidate.id === hover.id);
	if (!tab) return null;
	const runtimePreview = thumbnailSnapshot.targetId === tab.id ? thumbnailSnapshot : null;
	const showRuntimePreview =
		runtimePreview !== null &&
		(runtimePreview.current.presentation !== 'none' ||
			runtimePreview.fadingOut !== null ||
			runtimePreview.waitState !== ThumbnailWaitState.NotWaiting);
	const showPreview =
		showPreviews &&
		tab.id !== snapshot.activeTabId &&
		(runtimePreview ? showRuntimePreview : !tab.discarded || Boolean(tab.thumbnailUrl));
	const title = tab.title || 'New Tab';
	const titleData = getHoverCardTitleData(title, tab.url);
	const displayTitle = titleData.isFilename
		? elideFilenameWithLineLengths(title, {
				first: Math.min(title.length, 34),
				second: Math.min(title.length, 26),
			})
		: title;
	const domain = formatHoverCardDomain(
		tab.url ?? 'chrome://newtab',
		tab.shouldDisplayUrl !== false,
	);
	const footerRows = getHoverCardFooterRows(tab, {
		showMemoryUsage,
	});
	let previewContent: ReactNode;
	if (runtimePreview) {
		previewContent = (
			<>
				<ThumbnailVisualView visual={runtimePreview.current} />
				{runtimePreview.fadingOut ? (
					<ThumbnailVisualView
						key={`${runtimePreview.revision}:${runtimePreview.fadingOut.presentation}`}
						visual={runtimePreview.fadingOut}
						fadingOut={true}
					/>
				) : null}
			</>
		);
	} else if (tab.crashed) {
		previewContent = <span>{'This tab has crashed'}</span>;
	} else if (tab.thumbnailUrl) {
		previewContent = <img src={tab.thumbnailUrl} alt='' />;
	} else {
		previewContent = <span>{'Preview unavailable'}</span>;
	}
	return createPortal(
		<div
			className='ct-hover-card'
			style={{
				left: hover.x,
				top: hover.y,
			}}
			data-transitioning={transitioning || undefined}
		>
			{showPreview ? (
				<div
					className='ct-hover-card__preview'
					data-crashed={
						runtimePreview?.current.presentation === 'crashed' ||
						(!runtimePreview && tab.crashed) ||
						undefined
					}
					data-placeholder={
						runtimePreview?.current.presentation === 'placeholder' ||
						(!runtimePreview && !tab.crashed && !tab.thumbnailUrl) ||
						undefined
					}
					data-thumbnail-wait-state={runtimePreview?.waitState}
				>
					{previewContent}
				</div>
			) : null}

			<div className='ct-hover-card__body'>
				<strong data-filename={titleData.isFilename || undefined}>{displayTitle}</strong>
				{domain ? <span data-hover-card-domain=''>{domain}</span> : null}
			</div>
			{footerRows.length > 0 ? (
				<div className='ct-hover-card__footer'>
					{footerRows.map((row) => (
						<div
							key={row.kind}
							className={`ct-hover-card__footer-row ct-hover-card__${row.kind}`}
							data-alert-color={row.kind === 'alert' ? row.colorRole : undefined}
							data-high={(row.kind === 'performance' && row.high) || undefined}
						>
							{row.kind === 'alert' ? <TabAlertIndicatorGlyph state={row.alertState} /> : null}
							{row.kind === 'discard' || row.kind === 'performance' ? (
								<span className='ct-hover-card__resource-icon' aria-hidden='true'>
									{'\u25F4'}
								</span>
							) : null}
							{row.kind === 'collaboration'
								? (() => {
										if (row.avatarUrl)
											return <img className='ct-hover-card__avatar' src={row.avatarUrl} alt='' />;
										return (
											<span className='ct-hover-card__avatar' aria-hidden='true'>
												{tab.collaborationMessage?.givenName.slice(0, 1).toUpperCase()}
											</span>
										);
									})()
								: null}
							<span>{row.text}</span>
						</div>
					))}
				</div>
			) : null}
		</div>,
		document.body,
	);
}
function ThumbnailVisualView({
	visual,
	fadingOut = false,
}: {
	visual: Readonly<HoverCardThumbnailVisual<string>>;
	fadingOut?: boolean;
}) {
	return (
		<div className='ct-hover-card__preview-layer' data-fading-out={fadingOut || undefined}>
			{visual.presentation === 'thumbnail' && visual.image ? (
				<img src={visual.image} alt='' />
			) : null}
			{visual.presentation === 'crashed' ? <span>{'This tab has crashed'}</span> : null}
			{visual.presentation === 'placeholder' ? <span>{'Preview unavailable'}</span> : null}
		</div>
	);
}
export function TabStripImplementation({
	model,
	orientation = TabStripOrientation.Horizontal,
	verticalCollapsed = false,
	verticalExpandOnHover = true,
	verticalExpandOnHoverController,
	hoverCardController: externalHoverCardController,
	getHoverCardThumbnailSource,
	showHoverCardPreviews = true,
	showHoverCardMemoryUsage = true,
	showGroupHoverCards = true,
	onVerticalExpandedOnHoverChange,
	showVerticalTopControls = true,
	showVerticalProjectsButton = false,
	showVerticalTabSearchButton = false,
	verticalGroupFocused = false,
	showPinnedTabsInFocusedGroups = false,
	verticalProjectsOpen = false,
	verticalTabSearchOpen = false,
	verticalComboButtonController: externalComboButtonController,
	verticalToolbarHeight = 0,
	verticalCaptionButtonWidth = 0,
	verticalIsExitingExpandOnHover = false,
	onVerticalCollapsedChange,
	onVerticalUnfocusGroup,
	onVerticalProjectsClick,
	onVerticalTabSearchClick,
	onVerticalExpandOnHoverChange,
	className = '',
	style,
	showNewTabButton = true,
	horizontalFrameCondensed = false,
	touchUi = false,
	enableTabMuting = true,
	tabStripDeclutter = true,
	lockedForOnTask = false,
	createCrossWindowDragRuntime,
	onDraggingChange,
	onTabKeyboardFocusChanged,
	onTabAudioMuteToggle,
	onMoveTabsToNewWindow,
	onReloadTabs,
	onAddTabsToReadingList,
	onManageGroupSharing,
	onShareGroup,
	onMoveGroupToNewWindow,
	enableTabGroupFocusing = false,
	onDeleteSavedGroup,
	showSavedGroupFooter = false,
	onOpenTabGroupsLearnMore,
	enableTabGroupHome = false,
	onOpenTabGroupHome,
	onConvertGroupToBookmark,
	bookmarksEditable = true,
	confirmSavedGroupDestructiveActions = true,
	onAskGeminiAboutGroup,
	onGroupActivityAction,
	onSeeAllGroupActivity,
	ariaLabel = 'Browser tabs',
	onNewTab,
	onNewTabButtonPressed,
	onNewTabFromClipboard,
	onUrlDrop,
}: TabStripImplementationOptions): ReactNode {
	const snapshot = useSyncExternalStore(model.subscribe, model.getSnapshot, model.getSnapshot);
	const horizontal = orientation === TabStripOrientation.Horizontal;
	const hoverTabSelector = useMemo(() => new HoverTabSelector(model), [model]);
	const focusedGroup =
		snapshot.focusedGroupId === null
			? null
			: (snapshot.groups.find((group) => group.id === snapshot.focusedGroupId) ?? null);
	const groupFocusActive = focusedGroup !== null;
	const tabVisibleWhileFocused = useCallback(
		(tab: Readonly<TabData>) =>
			!groupFocusActive ||
			tab.group === focusedGroup?.id ||
			(showPinnedTabsInFocusedGroups && tab.pinned),
		[focusedGroup?.id, groupFocusActive, showPinnedTabsInFocusedGroups],
	);
	const [stripRef, size] = useElementSize<HTMLDivElement>();
	const items = useMemo(() => buildVisualItems(snapshot), [snapshot]);
	const horizontalAnimationRuntimeRef = useRef<TabCollectionAnimationRuntime | null>(null);
	if (horizontalAnimationRuntimeRef.current === null) {
		horizontalAnimationRuntimeRef.current = new TabCollectionAnimationRuntime();
	}
	const [animatedHorizontalFrame, setAnimatedHorizontalFrame] =
		useState<AnimatedLayoutFrame | null>(null);
	const horizontalAnimationRequestRef = useRef<number | null>(null);
	const [menu, setMenu] = useState<MenuState | null>(null);
	const [hover, setHover] = useState<HoverState | null>(null);
	// Kept as a compatibility hook so a pre-controller Fast Refresh instance
	// can still run its legacy cleanup closure without referencing a removed hook.
	const hoverTimer = useRef<number | null>(null);
	const [drag, setDrag] = useState<DragState | null>(null);
	const [closingTabIds, setClosingTabIds] = useState<ReadonlySet<string>>(() => new Set());
	const initialFullyCollapsedGroups = snapshot.groups
		.filter((group) => group.collapsed)
		.map((group) => group.id);
	const [fullyCollapsedGroupIds, setFullyCollapsedGroupIds] = useState<ReadonlySet<string>>(
		() => new Set(initialFullyCollapsedGroups),
	);
	const [pendingSavedClose, setPendingSavedClose] = useState<{
		tabIds: readonly string[];
		source: TabCloseSource;
	} | null>(null);
	const [tabClosingMode, setTabClosingMode] = useState<{
		width: number;
		source: Exclude<TabCloseSource, 'non-ui'>;
	} | null>(null);
	const [urlDrop, setUrlDrop] = useState<{
		index: TabDropIndex;
		placement: DropArrowPlacement;
	} | null>(null);
	const [announcement, setAnnouncement] = useState({
		text: '',
		sequence: 0,
	});
	const closingTimers = useRef(new Map<string, number>());
	const collapsedGroupTimers = useRef(new Map<string, number>());
	const fullyCollapsedGroupIdsRef = useRef(new Set(initialFullyCollapsedGroups));
	const middlePressRef = useRef<{
		tabId: string;
		pointerId: number;
	} | null>(null);
	const touchDragTimer = useRef<number | null>(null);
	const dragSettleTimer = useRef<number | null>(null);
	const dragRef = useRef<DragState | null>(null);
	const previousTabCountRef = useRef(snapshot.tabs.length);
	const suppressGroupToggleRef = useRef<string | null>(null);
	const previousDraggingRef = useRef(false);
	const onDraggingChangeRef = useRef(onDraggingChange);
	const groupEditorTrackerRef = useRef<TabGroupEditorBubbleTracker | null>(null);
	const previousOrientationRef = useRef(orientation);
	if (groupEditorTrackerRef.current === null) {
		groupEditorTrackerRef.current = new TabGroupEditorBubbleTracker();
	}
	const groupEditorTracker = groupEditorTrackerRef.current;
	const openGroupEditor = useCallback(
		(id: string, x: number, y: number) => {
			groupEditorTracker.open();
			setMenu({
				kind: 'group',
				id,
				x,
				y,
			});
		},
		[groupEditorTracker],
	);
	dragRef.current = drag;
	onDraggingChangeRef.current = onDraggingChange;
	useEffect(() => {
		const started = drag?.started === true && !drag.settling;
		if (started === previousDraggingRef.current) return;
		previousDraggingRef.current = started;
		onDraggingChangeRef.current?.(started);
	}, [drag?.settling, drag?.started]);
	useEffect(
		() => () => {
			if (touchDragTimer.current !== null) {
				window.clearTimeout(touchDragTimer.current);
				touchDragTimer.current = null;
			}
			if (dragSettleTimer.current !== null) {
				window.clearTimeout(dragSettleTimer.current);
				dragSettleTimer.current = null;
			}
			if (!previousDraggingRef.current) return;
			previousDraggingRef.current = false;
			onDraggingChangeRef.current?.(false);
		},
		[],
	);
	useEffect(() => {
		if (menu?.kind === 'group') return;
		groupEditorTracker.close('explicit');
	}, [groupEditorTracker, menu?.kind]);
	useEffect(() => {
		if (!menu) return;
		let targetStillExists = true;
		if (menu.kind === 'group') {
			targetStillExists = snapshot.groups.some((group) => group.id === menu.id);
		} else if (menu.kind === 'tab') {
			targetStillExists = snapshot.tabs.some((tab) => tab.id === menu.id);
		}
		if (targetStillExists) return;
		setMenu(null);
		groupEditorTracker.close('explicit');
	}, [groupEditorTracker, menu, snapshot.groups, snapshot.tabs]);
	useEffect(() => {
		if (!menu) return;
		const dismissOnOutsidePointer = (event: PointerEvent) => {
			const target = event.target;
			if (target instanceof Element && target.closest('.ct-menu, .ct-recent-activity')) return;
			const header =
				target instanceof Element ? target.closest<HTMLElement>('[data-group-id]') : null;
			if (menu.kind === 'group' && header?.dataset['groupId'] === menu.id) {
				suppressGroupToggleRef.current = menu.id;
			}
			setMenu(null);
		};
		window.addEventListener('pointerdown', dismissOnOutsidePointer, true);
		return () => window.removeEventListener('pointerdown', dismissOnOutsidePointer, true);
	}, [menu]);
	useEffect(() => {
		if (previousOrientationRef.current === orientation) return;
		previousOrientationRef.current = orientation;
		if (!groupEditorTracker.onVerticalTabStripModeWillChange()) return;
		setMenu((current) => (current?.kind === 'group' ? null : current));
	}, [groupEditorTracker, orientation]);
	useEffect(
		() => () => {
			const current = dragRef.current;
			if (!current?.started || current.settling) return;
			current.crossWindowRuntime?.end('cancel');
			if (horizontal && current.horizontalGroupDrag !== null) {
				const canceled = transitionHorizontalGroupDrag(current.horizontalGroupDrag, {
					type: 'cancel',
				});
				applyHorizontalGroupDragCommands(model, current.restoreState, canceled.commands);
				assertHorizontalGroupDragRefinement(model, canceled.state);
			} else {
				model.restoreCanceledDrag(current.restoreState);
			}
		},
		[horizontal, model],
	);
	useEffect(() => {
		const current = dragRef.current;
		if (!current?.started || current.settling) return;
		const liveIds = new Set(snapshot.tabs.map((tab) => tab.id));
		if (current.tabIds.every((id) => liveIds.has(id))) return;
		const runtimeSnapshot = current.crossWindowRuntime?.getSnapshot?.();
		const transferredToAnotherContext =
			runtimeSnapshot?.sourceContextId !== undefined &&
			runtimeSnapshot.attachedContextId !== undefined &&
			runtimeSnapshot.attachedContextId !== runtimeSnapshot.sourceContextId;
		if (transferredToAnotherContext) return;
		current.crossWindowRuntime?.end('tab-destroyed');
		announce('Tab move canceled');
		setDrag(null);
	}, [snapshot.revision, snapshot.tabs]);
	const announce = (text: string) => {
		setAnnouncement((current) => ({
			text,
			sequence: current.sequence + 1,
		}));
	};
	useEffect(
		() =>
			model.subscribeBeforeTabAdded(() => {
				const current = dragRef.current;
				if (!current?.started || current.settling || current.groupHeaderId === null) return;
				const runtimeSnapshot = current.crossWindowRuntime?.getSnapshot?.();
				const transferredToAnotherContext =
					runtimeSnapshot?.sourceContextId !== undefined &&
					runtimeSnapshot.attachedContextId !== undefined &&
					runtimeSnapshot.attachedContextId !== runtimeSnapshot.sourceContextId;
				current.crossWindowRuntime?.end(transferredToAnotherContext ? 'complete' : 'cancel');
				if (!transferredToAnotherContext) {
					if (horizontal && current.horizontalGroupDrag !== null) {
						const canceled = transitionHorizontalGroupDrag(current.horizontalGroupDrag, {
							type: 'cancel',
						});
						applyHorizontalGroupDragCommands(model, current.restoreState, canceled.commands);
						assertHorizontalGroupDragRefinement(model, canceled.state);
					} else {
						model.restoreCanceledDrag(current.restoreState);
					}
					announce('Group move canceled');
				}
				dragRef.current = null;
				setDrag(null);
			}),
		[horizontal, model],
	);
	const internalHoverCardController = useRef<TabHoverCardController | null>(null);
	if (internalHoverCardController.current === null) {
		internalHoverCardController.current = new TabHoverCardController();
	}
	const hoverCardController = externalHoverCardController ?? internalHoverCardController.current;
	const hoverCardSnapshot = useSyncExternalStore(
		hoverCardController.subscribe,
		hoverCardController.getSnapshot,
		hoverCardController.getSnapshot,
	);
	const thumbnailRuntimeRef = useRef<TabHoverCardThumbnailRuntime<string> | null>(null);
	if (thumbnailRuntimeRef.current === null)
		thumbnailRuntimeRef.current = new TabHoverCardThumbnailRuntime<string>();
	const thumbnailRuntime = thumbnailRuntimeRef.current;
	const thumbnailSnapshot = useSyncExternalStore(
		thumbnailRuntime.subscribe,
		thumbnailRuntime.getSnapshot,
		thumbnailRuntime.getSnapshot,
	);
	const internalComboButtonController = useRef<TabStripComboButtonController | null>(null);
	if (internalComboButtonController.current === null) {
		internalComboButtonController.current = new TabStripComboButtonController({
			startPinned: showVerticalProjectsButton,
			tabSearchPinned: showVerticalTabSearchButton,
		});
	}
	const comboButtonController =
		externalComboButtonController ?? internalComboButtonController.current;
	const comboButtonSnapshot = useSyncExternalStore(
		comboButtonController.subscribe,
		comboButtonController.getSnapshot,
		comboButtonController.getSnapshot,
	);
	const previousTabSearchOpen = useRef(false);
	const internalExpandOnHoverController = useRef<VerticalTabStripExpandOnHoverController | null>(
		null,
	);
	if (internalExpandOnHoverController.current === null) {
		internalExpandOnHoverController.current = new VerticalTabStripExpandOnHoverController();
	}
	const expandOnHoverController =
		verticalExpandOnHoverController ?? internalExpandOnHoverController.current;
	const expandOnHoverSnapshot = useSyncExternalStore(
		expandOnHoverController.subscribe,
		expandOnHoverController.getSnapshot,
		expandOnHoverController.getSnapshot,
	);
	const expandedOnHover = !horizontal && verticalCollapsed && expandOnHoverSnapshot.expanded;
	const effectiveVerticalCollapsed = verticalCollapsed && !expandedOnHover;
	useEffect(() => () => internalExpandOnHoverController.current?.destroy(), []);
	useEffect(() => () => internalHoverCardController.current?.destroy(), []);
	useEffect(() => () => hoverTabSelector.destroy(), [hoverTabSelector]);
	const tabOrderKey = snapshot.tabs.map((tab) => tab.id).join('\u0000');
	useEffect(() => {
		hoverTabSelector.cancelTabTransition();
	}, [hoverTabSelector, tabOrderKey]);
	// React StrictMode replays effects without recreating refs. Reset observation
	// here instead of permanently destroying the reusable runtime instance.
	useEffect(() => () => thumbnailRuntime.updateTarget(null), [thumbnailRuntime]);
	useEffect(() => {
		if (externalComboButtonController) return;
		comboButtonController.setPinned('start', showVerticalProjectsButton);
		comboButtonController.setPinned('tab-search', showVerticalTabSearchButton);
	}, [
		comboButtonController,
		externalComboButtonController,
		showVerticalProjectsButton,
		showVerticalTabSearchButton,
	]);
	useEffect(() => {
		comboButtonController.setOrientation(orientation);
	}, [comboButtonController, orientation]);
	useEffect(() => {
		if (previousTabSearchOpen.current === verticalTabSearchOpen) return;
		previousTabSearchOpen.current = verticalTabSearchOpen;
		if (verticalTabSearchOpen) comboButtonController.onBubbleInitializing();
		else comboButtonController.onBubbleDestroying();
	}, [comboButtonController, verticalTabSearchOpen]);
	useEffect(() => {
		const targetTab =
			hover?.kind === 'tab' ? (snapshot.tabs.find((tab) => tab.id === hover.id) ?? null) : null;
		const targetMatchesVisibleCard =
			targetTab !== null &&
			hoverCardSnapshot.visible &&
			hoverCardSnapshot.targetId === `tab:${targetTab.id}`;
		if (!getHoverCardThumbnailSource || !targetMatchesVisibleCard) {
			thumbnailRuntime.updateTarget(null);
			return;
		}
		thumbnailRuntime.updateTarget({
			id: targetTab.id,
			source: getHoverCardThumbnailSource(targetTab),
			previewsEnabled: showHoverCardPreviews,
			needsThumbnail: targetTab.id !== snapshot.activeTabId,
			isInitialShow: hoverCardSnapshot.cardsSeen <= 1,
			useAnimations: true,
			...(targetTab.discarded === undefined
				? {}
				: {
						discarded: targetTab.discarded,
					}),
			...(targetTab.crashed === undefined
				? {}
				: {
						crashed: targetTab.crashed,
					}),
		});
	}, [
		getHoverCardThumbnailSource,
		hover?.id,
		hover?.kind,
		hoverCardSnapshot.cardsSeen,
		hoverCardSnapshot.targetId,
		hoverCardSnapshot.visible,
		showHoverCardPreviews,
		snapshot.activeTabId,
		snapshot.revision,
		thumbnailRuntime,
		snapshot.tabs,
	]);
	useEffect(
		() => () => {
			new Set(closingTimers.current.values()).forEach((timer) => window.clearTimeout(timer));
			closingTimers.current.clear();
		},
		[],
	);
	const collapsedGroupStateKey = snapshot.groups
		.map((group) => `${group.id}:${group.collapsed ? 1 : 0}`)
		.join('|');
	const collapsedGroupStateRef = useRef(snapshot.groups);
	const previousCollapsedGroupStateKeyRef = useRef(collapsedGroupStateKey);
	if (previousCollapsedGroupStateKeyRef.current !== collapsedGroupStateKey) {
		previousCollapsedGroupStateKeyRef.current = collapsedGroupStateKey;
		collapsedGroupStateRef.current = snapshot.groups;
	}
	useEffect(() => {
		const collapsedGroupState = collapsedGroupStateRef.current;
		const liveGroups = new Set(collapsedGroupState.map((group) => group.id));
		collapsedGroupState.forEach((group) => {
			const pending = collapsedGroupTimers.current.get(group.id);
			if (!group.collapsed) {
				if (pending !== undefined) window.clearTimeout(pending);
				collapsedGroupTimers.current.delete(group.id);
				if (!fullyCollapsedGroupIdsRef.current.delete(group.id)) return;
				setFullyCollapsedGroupIds(new Set(fullyCollapsedGroupIdsRef.current));
				return;
			}
			if (fullyCollapsedGroupIdsRef.current.has(group.id) || pending !== undefined) return;
			const timer = window.setTimeout(() => {
				collapsedGroupTimers.current.delete(group.id);
				const stillCollapsed = model
					.getSnapshot()
					.groups.some((candidate) => candidate.id === group.id && candidate.collapsed);
				if (!stillCollapsed) return;
				fullyCollapsedGroupIdsRef.current.add(group.id);
				setFullyCollapsedGroupIds(new Set(fullyCollapsedGroupIdsRef.current));
			}, TAB_BOUNDS_ANIMATION_DURATION_MS);
			collapsedGroupTimers.current.set(group.id, timer);
		});
		[...collapsedGroupTimers.current].forEach(([groupId, timer]) => {
			if (liveGroups.has(groupId)) return;
			window.clearTimeout(timer);
			collapsedGroupTimers.current.delete(groupId);
			if (fullyCollapsedGroupIdsRef.current.delete(groupId)) {
				setFullyCollapsedGroupIds(new Set(fullyCollapsedGroupIdsRef.current));
			}
		});
		// The serialized key intentionally limits this effect to collapse/removal
		// transitions; unrelated model revisions must not restart the 200ms timer.
	}, [collapsedGroupStateKey, model]);
	useEffect(
		() => () => {
			collapsedGroupTimers.current.forEach((timer) => window.clearTimeout(timer));
			collapsedGroupTimers.current.clear();
		},
		[],
	);
	const requestCloseTabs = (
		tabIds: readonly string[],
		source: TabCloseSource = 'non-ui',
		skipSavedGroupConfirmation = false,
	) => {
		if (lockedForOnTask) return;
		const beforeClose = model.getSnapshot();
		const existingIds = new Set(beforeClose.tabs.map((tab) => tab.id));
		const ids = [...new Set(tabIds)].filter(
			(id) => existingIds.has(id) && !closingTimers.current.has(id),
		);
		if (ids.length === 0) return;
		if (!skipSavedGroupConfirmation && confirmSavedGroupDestructiveActions) {
			const closingIdSet = new Set(ids);
			const removesSavedGroup = beforeClose.groups.some(
				(group) =>
					group.saved &&
					beforeClose.tabs.some((tab) => tab.group === group.id) &&
					beforeClose.tabs
						.filter((tab) => tab.group === group.id)
						.every((tab) => closingIdSet.has(tab.id)),
			);
			if (removesSavedGroup) {
				setMenu(null);
				setPendingSavedClose({
					tabIds: ids,
					source,
				});
				return;
			}
		}
		const closingTitles = beforeClose.tabs
			.filter((tab) => ids.includes(tab.id))
			.map((tab) => tab.title || 'New Tab');
		if (horizontal && source !== 'non-ui' && ids.length === 1 && horizontalLayout) {
			const removedId = requireDefined(ids[0]);
			const removedModelIndex = beforeClose.tabs.findIndex((tab) => tab.id === removedId);
			const removedTab = beforeClose.tabs[removedModelIndex];
			const removedVisualIndex = items.findIndex(
				(item) => item.type === 'tab' && item.tab.id === removedId,
			);
			const removedBounds = horizontalLayout.bounds[removedVisualIndex];
			const remainingTabs = beforeClose.tabs.filter((tab) => tab.id !== removedId);
			const nextActive = remainingTabs[Math.min(removedModelIndex, remainingTabs.length - 1)];
			const nextVisualIndex = nextActive
				? items.findIndex((item) => item.type === 'tab' && item.tab.id === nextActive.id)
				: -1;
			const nextActiveBounds = horizontalLayout.bounds[nextVisualIndex];
			const trailing = horizontalLayout.bounds.at(-1);
			if (removedTab && removedBounds && trailing) {
				const width = calculateTabClosingModeWidth({
					currentOverride: tabClosingMode?.width ?? null,
					containerWidth: layoutWidth,
					modelCountAfterRemoval: remainingTabs.length,
					removedModelIndex,
					removedWidth: removedBounds.width,
					removedWasActive: beforeClose.activeTabId === removedId,
					removedWasPinned: removedTab.pinned,
					layoutDomain: horizontalLayout.domain,
					trailingIdealRight: trailing.x + trailing.width,
					tabOverlap: DEFAULT_TAB_STYLE_METRICS.tabOverlap,
					...(nextActiveBounds === undefined
						? {}
						: {
								nextActiveWidth: nextActiveBounds.width,
							}),
					...(nextActive === undefined
						? {}
						: {
								nextActivePinned: nextActive.pinned,
							}),
				});
				setTabClosingMode(
					width === null
						? null
						: {
								width,
								source,
							},
				);
			}
		}
		setClosingTabIds((current) => new Set([...current, ...ids]));
		const timer = window.setTimeout(() => {
			const ready = ids.filter((id) => closingTimers.current.get(id) === timer);
			ready.forEach((id) => closingTimers.current.delete(id));
			model.closeTabs(ready);
			if (ready.length === 1) announce(`Closed ${closingTitles[0]}`);
			else if (ready.length > 1) announce(`Closed ${ready.length} tabs`);
			setClosingTabIds((current) => {
				const next = new Set(current);
				ready.forEach((id) => next.delete(id));
				return next;
			});
		}, TAB_BOUNDS_ANIMATION_DURATION_MS);
		ids.forEach((id) => closingTimers.current.set(id, timer));
	};
	useEffect(() => {
		if (tabClosingMode?.source !== 'touch') return;
		let timer = 0;
		const finishWhenIdle = () => {
			timer = window.setTimeout(() => {
				if (dragRef.current?.started) finishWhenIdle();
				else setTabClosingMode(null);
			}, TAB_CLOSE_TOUCH_RESIZE_DELAY_MS);
		};
		finishWhenIdle();
		return () => window.clearTimeout(timer);
	}, [tabClosingMode?.source, tabClosingMode?.width]);
	useEffect(() => {
		if (tabClosingMode?.source !== 'mouse') return;
		const handlePointerMove = (event: PointerEvent) => {
			const element = stripRef.current;
			if (!element) return;
			const rect = element.getBoundingClientRect();
			const watchBounds = getTabClosingModeWatchBounds(
				{
					x: rect.left,
					y: rect.top,
					width: rect.width,
					height: rect.height,
				},
				getComputedStyle(element).direction === 'rtl',
			);
			if (
				!tabClosingModeContainsPoint(watchBounds, {
					x: event.clientX,
					y: event.clientY,
				})
			) {
				setTabClosingMode(null);
			}
		};
		window.addEventListener('pointermove', handlePointerMove);
		return () => window.removeEventListener('pointermove', handlePointerMove);
	}, [stripRef, tabClosingMode?.source]);
	useEffect(() => {
		expandOnHoverController.setEnvironment({
			shouldDisplayVerticalTabs: !horizontal,
			collapsed: verticalCollapsed,
			enabled: verticalExpandOnHover,
			frameActive: document.visibilityState !== 'hidden',
		});
	}, [expandOnHoverController, horizontal, verticalCollapsed, verticalExpandOnHover]);
	useEffect(() => {
		onVerticalExpandedOnHoverChange?.(expandedOnHover);
	}, [expandedOnHover, onVerticalExpandedOnHoverChange]);
	useEffect(() => {
		if (!menu || horizontal || !verticalCollapsed) return;
		const lock = expandOnHoverController.getLock(ExpandOnHoverLockType.KeepCurrentState);
		return () => lock.dispose();
	}, [expandOnHoverController, horizontal, menu, verticalCollapsed]);
	useEffect(() => {
		if (!comboButtonSnapshot.keepCurrentStateLock || horizontal || !verticalCollapsed) return;
		const lock = expandOnHoverController.getLock(ExpandOnHoverLockType.KeepCurrentState);
		return () => lock.dispose();
	}, [
		comboButtonSnapshot.keepCurrentStateLock,
		expandOnHoverController,
		horizontal,
		verticalCollapsed,
	]);
	useEffect(() => {
		if (!drag?.started || horizontal || !verticalCollapsed) return;
		const lock = expandOnHoverController.getLock(ExpandOnHoverLockType.KeepExpanded);
		return () => lock.dispose();
	}, [drag?.started, expandOnHoverController, horizontal, verticalCollapsed]);
	useEffect(() => {
		if (!urlDrop || horizontal || !verticalCollapsed) return;
		const lock = expandOnHoverController.getLock(ExpandOnHoverLockType.KeepExpanded);
		return () => lock.dispose();
	}, [expandOnHoverController, horizontal, urlDrop, verticalCollapsed]);
	useEffect(() => {
		if (!menu && !drag?.started) return;
		const lock = hoverCardController.getHideLock();
		setHover(null);
		return () => lock.dispose();
	}, [drag?.started, hoverCardController, menu]);
	const layoutWidth = Math.max(0, size.width - (showNewTabButton ? 42 : 0));
	const effectiveLayoutWidth = Math.min(layoutWidth, tabClosingMode?.width ?? layoutWidth);
	const expandedHeaderDragGroupId =
		horizontal && drag?.started && drag.groupHeaderId !== null && !drag.groupWasCollapsedBeforeDrag
			? drag.groupHeaderId
			: null;
	const horizontalLayout = useMemo(() => {
		if (!horizontal) return null;
		const activeTab = snapshot.tabs.find((tab) => tab.id === snapshot.activeTabId);
		const constraints = items.map((item) => {
			const group =
				item.type === 'tab' && item.tab.group !== null
					? snapshot.groups.find((candidate) => candidate.id === item.tab.group)
					: null;
			const splitActive =
				item.type === 'tab' && item.tab.split !== null && activeTab?.split === item.tab.split;
			const hiddenByGroupFocus =
				groupFocusActive &&
				(item.type === 'group'
					? item.group.id !== focusedGroup.id
					: !tabVisibleWhileFocused(item.tab));
			const hiddenByCollapsedGroup =
				!groupFocusActive &&
				item.type === 'tab' &&
				Boolean(group?.collapsed) &&
				group?.id !== expandedHeaderDragGroupId;
			const state = new TabLayoutState(
				hiddenByGroupFocus ||
					hiddenByCollapsedGroup ||
					(item.type === 'tab' && closingTabIds.has(item.tab.id))
					? TabOpen.Closed
					: TabOpen.Open,
				item.type === 'tab' && item.tab.pinned ? TabPinned.Pinned : TabPinned.Unpinned,
				item.type === 'tab' && (item.tab.id === snapshot.activeTabId || splitActive)
					? TabActive.Active
					: TabActive.Inactive,
				item.type === 'tab' ? item.tab.split : null,
			);
			const sizeItem =
				item.type === 'group' && item.group.id === expandedHeaderDragGroupId
					? {
							...item,
							group: {
								...item.group,
								collapsed: false,
							},
						}
					: item;
			return new TabWidthConstraints(state, sizeInfoForItem(sizeItem));
		});
		return calculateTabBounds(constraints, effectiveLayoutWidth);
	}, [
		closingTabIds,
		effectiveLayoutWidth,
		expandedHeaderDragGroupId,
		focusedGroup?.id,
		groupFocusActive,
		horizontal,
		items,
		tabVisibleWhileFocused,
		snapshot.groups,
		snapshot.activeTabId,
		snapshot.tabs,
	]);
	useEffect(() => {
		const runtime = horizontalAnimationRuntimeRef.current;
		if (!runtime || !horizontal || !horizontalLayout) {
			setAnimatedHorizontalFrame(null);
			return;
		}
		if (horizontalAnimationRequestRef.current !== null) {
			cancelManagedAnimationFrame(horizontalAnimationRequestRef.current);
			horizontalAnimationRequestRef.current = null;
		}
		const target = {
			// Tab-closing mode constrains child target bounds without resizing the
			// collection host. Passing effectiveLayoutWidth here would trigger
			// Chromium's real host-resize snap branch and skip close animation.
			hostSize: {
				width: layoutWidth,
				height: LAYOUT_CONSTANTS.tabStripHeight,
			},
			children: items.map((item, index) => {
				const bounds = horizontalLayout.bounds[index] ?? {
					x: 0,
					y: 0,
					width: 0,
					height: LAYOUT_CONSTANTS.tabStripHeight,
				};
				return {
					id: item.id,
					visible: bounds.width > 0,
					bounds: {
						...bounds,
					},
				};
			}),
		};
		const synced = runtime.sync(
			target,
			items.map((item) => ({
				id: item.id,
				group: item.type === 'group' ? item.group.id : item.tab.group,
				split: item.type === 'tab' ? item.tab.split : null,
				dragging:
					item.type === 'group'
						? Boolean(
								drag?.started &&
								drag.visualViews.some((view) => view.id === `group:${item.group.id}`),
							)
						: Boolean(drag?.started && drag.tabIds.includes(item.tab.id)),
			})),
		);
		setAnimatedHorizontalFrame(synced.frame);
		// A render with an unchanged target can still arrive while a previous
		// timeline is active (selection/collapse and drag-settle updates commonly
		// do this together). We canceled that timeline's RAF above, so it must be
		// resumed from its current progress instead of leaving a permanent
		// half-interpolated frame.
		if (!runtime.isAnimating) return;
		if (window.matchMedia?.('(prefers-reduced-motion: reduce)').matches) {
			setAnimatedHorizontalFrame(runtime.complete());
			return;
		}
		const startedAt = performance.now();
		const startProgress = synced.startProgress;
		const tick = (now: number) => {
			const elapsed = Math.max(0, now - startedAt);
			const timeline = Math.min(1, elapsed / TAB_BOUNDS_ANIMATION_DURATION_MS);
			const progress = startProgress + (1 - startProgress) * tabCollectionEaseInOut(timeline);
			setAnimatedHorizontalFrame(runtime.setProgress(progress));
			if (timeline < 1) {
				horizontalAnimationRequestRef.current = requestManagedAnimationFrame(tick);
			} else {
				horizontalAnimationRequestRef.current = null;
				setAnimatedHorizontalFrame(runtime.complete());
			}
		};
		horizontalAnimationRequestRef.current = requestManagedAnimationFrame(tick);
	}, [
		drag?.started,
		drag?.tabIds,
		drag?.visualViews,
		horizontal,
		horizontalLayout,
		items,
		layoutWidth,
	]);
	useEffect(
		() => () => {
			if (horizontalAnimationRequestRef.current !== null) {
				cancelManagedAnimationFrame(horizontalAnimationRequestRef.current);
				horizontalAnimationRequestRef.current = null;
			}
			horizontalAnimationRuntimeRef.current?.dispose();
		},
		[],
	);
	useEffect(() => {
		const previousCount = previousTabCountRef.current;
		previousTabCountRef.current = snapshot.tabs.length;
		if (snapshot.tabs.length > previousCount) setTabClosingMode(null);
	}, [snapshot.tabs.length]);
	useEffect(() => {
		if (!tabClosingMode || closingTabIds.size > 0) return;
		const trailing = horizontalLayout?.bounds.at(-1);
		if (trailing && shouldExitTabClosingMode(tabClosingMode.width, trailing.x + trailing.width)) {
			setTabClosingMode(null);
		}
	}, [closingTabIds, horizontalLayout, tabClosingMode]);
	const clearHover = useCallback(
		(updateType: HoverCardUpdateType = HoverCardUpdateType.Hover) => {
			if (hoverTimer.current !== null) window.clearTimeout(hoverTimer.current);
			hoverTimer.current = null;
			hoverCardController.updateHoverCard(null, updateType);
			setHover(null);
		},
		[hoverCardController],
	);
	useEffect(() => {
		if (!horizontalAnimationRuntimeRef.current?.isAnimating) return;
		// TabContainerImpl suppresses hover cards while any tab-slot bounds are
		// animating so cards never remain anchored to a moving view.
		clearHover(HoverCardUpdateType.Animating);
	}, [animatedHorizontalFrame, clearHover]);
	useEffect(() => () => clearHover(HoverCardUpdateType.Event), [clearHover]);
	const updateHoverForElement = (
		element: HTMLElement,
		id: string,
		updateType: HoverCardUpdateType,
		kind: HoverState['kind'] = 'tab',
	) => {
		const targetTab = kind === 'tab' ? (snapshot.tabs.find((tab) => tab.id === id) ?? null) : null;
		const pinnedBelowAnchor =
			!horizontal && targetTab?.pinned === true && !effectiveVerticalCollapsed;
		const anchor =
			targetTab !== null && targetTab.split !== null && !effectiveVerticalCollapsed
				? (element.closest<HTMLElement>('.ct-vertical-split') ?? element)
				: element;
		const rawRect = anchor.getBoundingClientRect();
		const slotId = kind === 'group' ? `group-${id}` : id;
		const slotBounds = horizontalAnimationRuntimeRef.current
			?.getSlot(slotId)
			?.getAnchorBoundsInScreen({
				x: rawRect.left,
				y: rawRect.top,
				width: rawRect.width,
				height: rawRect.height,
			});
		const rect = slotBounds
			? {
					left: slotBounds.x,
					top: slotBounds.y,
					right: slotBounds.x + slotBounds.width,
					bottom: slotBounds.y + slotBounds.height,
					width: slotBounds.width,
					height: slotBounds.height,
				}
			: rawRect;
		const targetWidth = horizontal
			? Math.max(
					rect.width,
					...[
						...(element.closest('[data-ct-root]')?.querySelectorAll<HTMLElement>('[data-tab-id]') ??
							[]),
					].map((tab) => tab.getBoundingClientRect().width),
				)
			: (element.closest<HTMLElement>('[data-ct-root]')?.getBoundingClientRect().width ??
				rect.width);
		setHover({
			kind,
			id,
			x:
				horizontal || pinnedBelowAnchor
					? Math.max(8, Math.min(rect.left, window.innerWidth - 340))
					: rect.right + 8,
			y: horizontal || pinnedBelowAnchor ? rect.bottom + 6 : Math.max(8, rect.top),
		});
		hoverCardController.updateHoverCard(
			{
				id: `${kind}:${id}`,
				width: targetWidth,
				minimumWidth: horizontal
					? TAB_SIZE.pinnedTabWidth
					: LAYOUT_CONSTANTS.verticalTabStripCollapsedWidth,
				standardWidth: horizontal ? TAB_SIZE.standardWidth : 240,
				focused: updateType === HoverCardUpdateType.Focus,
			},
			updateType,
		);
	};
	const onTabMouseEnter = (event: ReactMouseEvent<HTMLDivElement>, tabId: string) => {
		updateHoverForElement(event.currentTarget, tabId, HoverCardUpdateType.Hover);
	};
	const onGroupMouseEnter = (event: ReactMouseEvent<HTMLDivElement>, groupId: string) => {
		if (!showGroupHoverCards) return;
		updateHoverForElement(event.currentTarget, groupId, HoverCardUpdateType.Hover, 'group');
	};
	useEffect(() => {
		if (!hoverCardSnapshot.visible) return;
		const dismissForPointer = () => {
			hoverCardController.updateHoverCard(null, HoverCardUpdateType.Event);
			setHover(null);
		};
		const dismissForKey = (event: KeyboardEvent) => {
			const focusInStrip = event.target instanceof Node && stripRef.current?.contains(event.target);
			if (focusInStrip && event.key !== 'Enter' && event.key !== 'Escape') return;
			dismissForPointer();
		};
		window.addEventListener('pointerdown', dismissForPointer, true);
		window.addEventListener('keydown', dismissForKey, true);
		return () => {
			window.removeEventListener('pointerdown', dismissForPointer, true);
			window.removeEventListener('keydown', dismissForKey, true);
		};
	}, [hoverCardController, hoverCardSnapshot.visible, stripRef]);
	const selectFromPointer = (event: ReactPointerEvent, tab: Readonly<TabData>) => {
		const modifier = event.metaKey || event.ctrlKey;
		if (event.shiftKey && modifier) model.addSelectionFromAnchorTo(tab.id);
		else if (event.shiftKey) model.extendSelectionTo(tab.id);
		else if (modifier) model.toggleSelected(tab.id);
		else if (!model.getSnapshot().selectedTabIds.has(tab.id)) model.selectTab(tab.id);
	};
	const onTabPointerDown = (event: ReactPointerEvent<HTMLDivElement>, tab: Readonly<TabData>) => {
		if (event.button === 1) {
			middlePressRef.current = {
				tabId: tab.id,
				pointerId: event.pointerId,
			};
			event.preventDefault();
			return;
		}
		if (event.button !== 0) return;
		clearHover();
		const touchPointer = event.pointerType === 'touch';
		// Tab::OnGestureEvent handles kGestureTapDown exactly like the unmodified
		// selection portion of OnMousePressed: selection happens before the drag
		// controller is asked to start. OnTask only rejects MaybeStartDrag, so a
		// locked touch tab must remain selectable too.
		const restoreState = model.captureDragRestoreState();
		selectFromPointer(event, tab);
		if (lockedForOnTask) {
			return;
		}
		const pointerSnapshot = model.getSnapshot();
		if (
			(event.metaKey || event.ctrlKey) &&
			!event.shiftKey &&
			!pointerSnapshot.selectedTabIds.has(tab.id)
		) {
			return;
		}
		const initiallySelected = pointerSnapshot.selectedTabIds.has(tab.id)
			? pointerSnapshot.tabs
					.filter(
						(candidate) =>
							pointerSnapshot.selectedTabIds.has(candidate.id) && candidate.pinned === tab.pinned,
					)
					.map((candidate) => candidate.id)
			: [tab.id];
		const selectedIds = new Set(initiallySelected);
		const selectedSplitIds = new Set(
			pointerSnapshot.tabs
				.filter((candidate) => selectedIds.has(candidate.id) && candidate.split !== null)
				.map((candidate) => candidate.split),
		);
		const selected = pointerSnapshot.tabs
			.filter(
				(candidate) =>
					candidate.pinned === tab.pinned &&
					(selectedIds.has(candidate.id) ||
						(candidate.split !== null && selectedSplitIds.has(candidate.split))),
			)
			.map((candidate) => candidate.id);
		const draggedTabIds = selected.length ? selected : [tab.id];
		const draggedIdSet = new Set(draggedTabIds);
		const fullySelectedGroupIds = pointerSnapshot.groups
			.filter((group) => {
				const members = pointerSnapshot.tabs.filter((candidate) => candidate.group === group.id);
				return members.length > 0 && members.every((candidate) => draggedIdSet.has(candidate.id));
			})
			.map((group) => group.id);
		const visuals = captureDragVisuals({
			sourceElement: event.currentTarget,
			sourceId: tab.id,
			tabIds: draggedTabIds,
			sourceGroupId: null,
			groupIds: fullySelectedGroupIds,
			underlineGroupIds: [
				...new Set(
					pointerSnapshot.tabs
						.filter((candidate) => draggedIdSet.has(candidate.id) && candidate.group !== null)
						.map((candidate) => requireDefined(candidate.group)),
				),
			],
			orientation,
			pointer: {
				x: event.clientX,
				y: event.clientY,
			},
		});
		const next: DragState = {
			sourceId: tab.id,
			tabIds: draggedTabIds,
			pointerId: event.pointerId,
			pointerType: event.pointerType,
			touchDragReady: !touchPointer,
			startX: event.clientX,
			startY: event.clientY,
			shiftPressedOnPointerDown: event.shiftKey,
			x: event.clientX,
			y: event.clientY,
			started: false,
			targetIndex: null,
			targetGroup: null,
			targetIdealLeft: null,
			lastReorderCoordinate: horizontal ? event.clientX : event.clientY,
			restoreState,
			groupHeaderId: null,
			groupWasCollapsedBeforeDrag: false,
			horizontalGroupDrag: null,
			label: tab.title,
			...visuals,
			crossWindowRuntime:
				createCrossWindowDragRuntime?.(
					buildDragSessionData({
						snapshot: pointerSnapshot,
						tabIds: draggedTabIds,
						sourceId: tab.id,
						groupHeaderId: null,
						groupWasCollapsedBeforeDrag: false,
						pointerOffsetRatios: visuals.pointerOffsetRatios,
					}),
				) ?? null,
			settling: false,
		};
		setDrag(next);
		if (touchPointer) {
			if (touchDragTimer.current !== null) window.clearTimeout(touchDragTimer.current);
			touchDragTimer.current = window.setTimeout(() => {
				touchDragTimer.current = null;
				const current = dragRef.current;
				if (!current || current.pointerId !== event.pointerId || current.pointerType !== 'touch')
					return;
				const liveSnapshot = model.getSnapshot();
				if (!liveSnapshot.selectedTabIds.has(current.sourceId)) model.selectTab(current.sourceId);
				setDrag((candidate) =>
					candidate?.pointerId === event.pointerId
						? {
								...candidate,
								touchDragReady: true,
							}
						: candidate,
				);
			}, 500);
		} else {
			event.currentTarget.setPointerCapture?.(event.pointerId);
		}
	};
	useEffect(() => {
		const finishMiddlePress = (event: PointerEvent) => {
			const press = middlePressRef.current;
			if (!press || event.pointerId !== press.pointerId || event.button !== 1) return;
			middlePressRef.current = null;
			const hit =
				document
					.elementFromPoint?.(event.clientX, event.clientY)
					?.closest<HTMLElement>('[data-tab-id]') ?? null;
			if (hit?.dataset['tabId'] === press.tabId) {
				requestCloseTabs([press.tabId], 'mouse');
				return;
			}
			// Port of Tab::OnMouseReleased's rapid-close fallback: if the pressed
			// view moved away because it is already closing, close the tab that has
			// animated under the pointer instead.
			if (closingTimers.current.has(press.tabId) && hit?.dataset['tabId']) {
				requestCloseTabs([hit.dataset['tabId']], 'mouse');
			}
		};
		const cancelMiddlePress = (event: PointerEvent) => {
			if (middlePressRef.current?.pointerId === event.pointerId) middlePressRef.current = null;
		};
		window.addEventListener('pointerup', finishMiddlePress);
		window.addEventListener('pointercancel', cancelMiddlePress);
		return () => {
			window.removeEventListener('pointerup', finishMiddlePress);
			window.removeEventListener('pointercancel', cancelMiddlePress);
		};
	});
	const onGroupPointerDown = (
		event: ReactPointerEvent<HTMLDivElement>,
		group: Readonly<TabGroupData>,
	) => {
		if (event.button !== 0) return;
		if (menu?.kind === 'group') return;
		clearHover();
		if (lockedForOnTask) return;
		const touchPointer = event.pointerType === 'touch';
		const pointerSnapshot = model.getSnapshot();
		const groupTabs = pointerSnapshot.tabs.filter((tab) => tab.group === group.id);
		if (groupTabs.length === 0) return;
		const restoreState = model.captureDragRestoreState();
		const active =
			groupTabs.find((tab) => tab.id === pointerSnapshot.activeTabId) ??
			requireDefined(groupTabs[0]);
		const draggedTabIds = groupTabs.map((tab) => tab.id);
		const visuals = captureDragVisuals({
			sourceElement: event.currentTarget,
			sourceId: active.id,
			tabIds: draggedTabIds,
			sourceGroupId: group.id,
			groupIds: [group.id],
			underlineGroupIds: [group.id],
			orientation,
			pointer: {
				x: event.clientX,
				y: event.clientY,
			},
		});
		const next: DragState = {
			sourceId: active.id,
			tabIds: draggedTabIds,
			pointerId: event.pointerId,
			pointerType: event.pointerType,
			touchDragReady: !touchPointer,
			startX: event.clientX,
			startY: event.clientY,
			shiftPressedOnPointerDown: event.shiftKey,
			x: event.clientX,
			y: event.clientY,
			started: false,
			targetIndex: null,
			targetGroup: group.id,
			targetIdealLeft: null,
			lastReorderCoordinate: horizontal ? event.clientX : event.clientY,
			restoreState,
			groupHeaderId: group.id,
			groupWasCollapsedBeforeDrag: group.collapsed,
			horizontalGroupDrag: horizontal
				? createArmedHorizontalGroupDrag(pointerSnapshot, group.id)
				: null,
			label: group.title || 'Unnamed group',
			...visuals,
			crossWindowRuntime:
				createCrossWindowDragRuntime?.(
					buildDragSessionData({
						snapshot: pointerSnapshot,
						tabIds: draggedTabIds,
						sourceId: active.id,
						groupHeaderId: group.id,
						groupWasCollapsedBeforeDrag: group.collapsed,
						pointerOffsetRatios: visuals.pointerOffsetRatios,
					}),
				) ?? null,
			settling: false,
		};
		setDrag(next);
		if (touchPointer) {
			if (touchDragTimer.current !== null) window.clearTimeout(touchDragTimer.current);
			touchDragTimer.current = window.setTimeout(() => {
				touchDragTimer.current = null;
				setDrag((candidate) =>
					candidate?.pointerId === event.pointerId
						? {
								...candidate,
								touchDragReady: true,
							}
						: candidate,
				);
			}, 500);
		} else {
			event.currentTarget.setPointerCapture?.(event.pointerId);
		}
	};
	const toggleGroupFromHeader = (
		groupId: string,
		source: 'mouse' | 'touch' | 'keyboard' = 'mouse',
	) => {
		if (suppressGroupToggleRef.current === groupId) {
			suppressGroupToggleRef.current = null;
			return;
		}
		if (menu?.kind === 'group') {
			setMenu(null);
			return;
		}
		const group = snapshot.groups.find((candidate) => candidate.id === groupId);
		if (horizontal && group && !group.collapsed && source !== 'keyboard' && horizontalLayout) {
			const headerIndex = items.findIndex(
				(item) => item.type === 'group' && item.group.id === groupId,
			);
			const groupTabIndices = items
				.map((item, index) => (item.type === 'tab' && item.tab.group === groupId ? index : -1))
				.filter((index) => index !== -1);
			const headerBounds = horizontalLayout.bounds[headerIndex];
			const trailingGroupBounds = horizontalLayout.bounds[groupTabIndices.at(-1) ?? -1];
			const trailingBounds = horizontalLayout.bounds.at(-1);
			if (headerBounds && trailingGroupBounds && trailingBounds) {
				const groupWidth = trailingGroupBounds.x + trailingGroupBounds.width - headerBounds.x;
				const collapsedHeaderWidth = groupHeaderWidth(
					{
						...group,
						collapsed: true,
					},
					groupTabIndices.length,
				);
				setTabClosingMode({
					width: trailingBounds.x + trailingBounds.width - groupWidth + collapsedHeaderWidth,
					source,
				});
			}
		} else if (group?.collapsed) {
			setTabClosingMode(null);
		}
		model.toggleGroupCollapsed(groupId);
	};
	const onGroupKeyDown = (event: React.KeyboardEvent<HTMLDivElement>, groupId: string) => {
		if (!(event.ctrlKey || event.metaKey)) return;
		const previousKey = horizontal ? 'ArrowLeft' : 'ArrowUp';
		const nextKey = horizontal ? 'ArrowRight' : 'ArrowDown';
		let direction = 0;
		if (event.key === previousKey) direction = -1;
		else if (event.key === nextKey) direction = 1;
		if (direction === 0) return;
		event.preventDefault();
		model.shiftGroupRelative(groupId, direction < 0 ? -1 : 1);
		let directionLabel = direction < 0 ? 'up' : 'down';
		if (horizontal) directionLabel = direction < 0 ? 'left' : 'right';
		announce(`Group moved ${directionLabel}`);
	};
	useEffect(() => {
		if (!drag) return;
		const move = (event: PointerEvent) => {
			const dragAtStart = dragRef.current;
			if (!dragAtStart || dragAtStart.settling || dragAtStart.pointerId !== event.pointerId) return;
			let current: DragState = dragAtStart;
			const distance = Math.hypot(event.clientX - current.startX, event.clientY - current.startY);
			if (current.pointerType === 'touch' && !current.touchDragReady) {
				if (distance > 10) {
					if (touchDragTimer.current !== null) window.clearTimeout(touchDragTimer.current);
					touchDragTimer.current = null;
					setDrag(null);
				}
				return;
			}
			const started = current.started || distance > 10;
			if (started && !current.started) current.crossWindowRuntime?.start();
			if (started && !current.started && current.groupHeaderId !== null) {
				suppressGroupToggleRef.current = current.groupHeaderId;
				if (horizontal && current.horizontalGroupDrag !== null) {
					const activation = transitionHorizontalGroupDrag(current.horizontalGroupDrag, {
						type: 'activate-drag',
					});
					applyHorizontalGroupDragCommands(model, current.restoreState, activation.commands);
					assertHorizontalGroupDragRefinement(model, activation.state);
					current = {
						...current,
						horizontalGroupDrag: activation.state,
					};
				} else {
					// TabDragController::AttachImpl resets selection to every dragged tab.
					// For a header drag outside the active group, Init records tab zero as
					// active_tab_index_within_group. Do not expand a previously collapsed
					// group while applying that selection.
					model.setSelection(current.tabIds, current.sourceId, {
						expandGroups: false,
					});
					if (!current.groupWasCollapsedBeforeDrag) {
						model.updateGroup(current.groupHeaderId, {
							collapsed: true,
						});
					}
				}
			}
			let targetIndex: number | null = null;
			let targetGroup: TabData['group'] = null;
			let targetIdealLeft = current.targetIdealLeft;
			let calculatedHorizontalGroup = false;
			let horizontalTargetGroup: TabData['group'] = null;
			let lastReorderCoordinate = current.lastReorderCoordinate;
			if (started) {
				current.crossWindowRuntime?.update({
					x: event.screenX,
					y: event.screenY,
				});
				const externalTargetActive =
					current.crossWindowRuntime?.getSnapshot?.().targetActive === true;
				const liveSnapshot = model.getSnapshot();
				const source =
					liveSnapshot.tabs.find((candidate) => candidate.id === current.sourceId) ?? null;
				const root = stripRef.current;
				targetIndex = source?.pinned
					? liveSnapshot.tabs.findIndex((candidate) => !candidate.pinned)
					: liveSnapshot.tabs.length;
				if (targetIndex === -1) targetIndex = liveSnapshot.tabs.length;
				let targetExtent = horizontal ? TAB_SIZE.standardWidth : 30;
				let verticalNestedTargetGroup: TabData['group'] = null;
				const pinnedDragAxes = getPinnedTabContainerDragAxes(
					verticalCollapsed
						? VerticalTabStripCollapseState.Collapsed
						: VerticalTabStripCollapseState.Expanded,
				);
				const pinnedGridDrag = !horizontal && source?.pinned === true && pinnedDragAxes === 'both';
				if (!horizontal && root && source) {
					const rootRect = root.getBoundingClientRect();
					const tabElements = [...root.querySelectorAll<HTMLElement>('[data-tab-id]')];
					const splitElements = [...root.querySelectorAll<HTMLElement>('[data-split-id]')];
					const containsGroupVisual = current.visualViews.some((view) => view.kind === 'group');
					const children: Array<{
						id: string;
						visible: boolean;
						bounds: {
							x: number;
							y: number;
							width: number;
							height: number;
						};
						dragging: boolean;
					}> = [];
					const representedGroups = new Set<string>();
					for (let index = 0; index < liveSnapshot.tabs.length; index += 1) {
						const tab = requireDefined(liveSnapshot.tabs[index]);
						if (tab.pinned !== source.pinned) continue;
						const next = liveSnapshot.tabs[index + 1];
						if (containsGroupVisual && tab.group !== null) {
							if (representedGroups.has(tab.group)) continue;
							representedGroups.add(tab.group);
							const element = [
								...root.querySelectorAll<HTMLElement>('.ct-vertical-group[data-group-id]'),
							].find((candidate) => candidate.dataset['groupId'] === tab.group);
							if (!element) continue;
							const rect = element.getBoundingClientRect();
							children.push({
								// A group is one direct collection child. Its first tab id is
								// also Chromium's insertion index for the node.
								id: tab.id,
								visible: true,
								bounds: {
									x: rect.left - rootRect.left,
									y: rect.top - rootRect.top,
									width: rect.width,
									height: rect.height,
								},
								dragging: current.visualViews.some((view) => view.id === `group:${tab.group}`),
							});
							continue;
						}
						if (tab.split !== null && next?.split === tab.split) {
							const element = splitElements.find(
								(candidate) => candidate.dataset['splitId'] === tab.split,
							);
							if (element) {
								const rect = element.getBoundingClientRect();
								children.push({
									id: tab.id,
									visible: true,
									bounds: {
										x: rect.left - rootRect.left,
										y: rect.top - rootRect.top,
										width: rect.width,
										height: rect.height,
									},
									dragging: current.tabIds.includes(tab.id) || current.tabIds.includes(next.id),
								});
							}
							index += 1;
							continue;
						}
						const element = tabElements.find((candidate) => candidate.dataset['tabId'] === tab.id);
						if (!element) continue;
						const rect = element.getBoundingClientRect();
						children.push({
							id: tab.id,
							visible: true,
							bounds: {
								x: rect.left - rootRect.left,
								y: rect.top - rootRect.top,
								width: rect.width,
								height: rect.height,
							},
							dragging: current.tabIds.includes(tab.id),
						});
					}
					const sourceVisualId = getSourceVisualId(current);
					const verticalVisualViews = getVerticalDragVisualViews(current);
					const draggedLayout = buildDraggedTabsLayout({
						layout: source.pinned || verticalCollapsed ? 'squash' : 'vertical',
						pointerOffsetRatios: current.pointerOffsetRatios,
						sourceSlotSize: current.sourceSlotSize,
						sourceViewOffsetFromSlot: current.sourceViewOffsetFromSlot,
						views: verticalVisualViews.map((view) => ({
							id: view.id,
							bounds: {
								x: view.x,
								y: view.y,
								width: view.width,
								height: view.height,
							},
							source: view.id === sourceVisualId,
						})),
					});
					const draggedBoundsInScreen = getDraggingViewsBoundsAtPoint(
						draggedLayout.boundsRelativeToPointer,
						{
							x: event.clientX,
							y: event.clientY,
						},
					);
					if (!source.pinned && current.groupHeaderId === null) {
						const groupElements = [
							...root.querySelectorAll<HTMLElement>(
								'.ct-vertical-group[data-group-id], .ct-vertical-group',
							),
						];
						const currentGroupElement =
							current.targetGroup === null
								? null
								: (groupElements.find(
										(element) =>
											element.dataset['groupId'] === current.targetGroup ||
											element.querySelector<HTMLElement>('[data-group-id]')?.dataset['groupId'] ===
												current.targetGroup,
									) ?? null);
						if (currentGroupElement) {
							const groupData = liveSnapshot.groups.find(
								(group) => group.id === current.targetGroup,
							);
							const groupRect = currentGroupElement.getBoundingClientRect();
							const headerHeight =
								currentGroupElement
									.querySelector<HTMLElement>('[data-group-id]')
									?.getBoundingClientRect().height ?? 26;
							if (
								!groupData?.collapsed &&
								shouldRemainInVerticalTabGroup(
									draggedBoundsInScreen,
									{
										x: groupRect.left,
										y: groupRect.top,
										width: groupRect.width,
										height: groupRect.height,
									},
									headerHeight,
								)
							) {
								verticalNestedTargetGroup = current.targetGroup;
							}
						}
						if (verticalNestedTargetGroup === null) {
							for (const groupElement of groupElements) {
								const groupId =
									groupElement.dataset['groupId'] ??
									groupElement.querySelector<HTMLElement>('[data-group-id]')?.dataset['groupId'];
								const groupData = liveSnapshot.groups.find((group) => group.id === groupId);
								if (!groupId || groupData?.collapsed) continue;
								const groupRect = groupElement.getBoundingClientRect();
								const headerHeight =
									groupElement
										.querySelector<HTMLElement>('[data-group-id]')
										?.getBoundingClientRect().height ?? 26;
								if (
									!shouldEnterVerticalTabGroup(
										draggedBoundsInScreen,
										{
											x: groupRect.left,
											y: groupRect.top,
											width: groupRect.width,
											height: groupRect.height,
										},
										headerHeight,
									)
								)
									continue;
								verticalNestedTargetGroup = groupId;
								break;
							}
						}
					}
					const targetId = getTargetBeforeDraggedTabs({
						children,
						draggedBounds: {
							...draggedBoundsInScreen,
							x: draggedBoundsInScreen.x - rootRect.left,
							y: draggedBoundsInScreen.y - rootRect.top,
						},
						axes: source.pinned ? pinnedDragAxes : 'vertical-only',
						rtl: getComputedStyle(root).direction === 'rtl',
						hostWidth: rootRect.width,
					});
					if (targetId !== null) {
						targetIndex = liveSnapshot.tabs.findIndex((tab) => tab.id === targetId);
						const targetChild = children.find((child) => child.id === targetId);
						targetExtent = pinnedGridDrag
							? (targetChild?.bounds.width ?? 30)
							: (targetChild?.bounds.height ?? 30);
					}
				} else if (root && source && horizontalLayout) {
					const rootRect = root.getBoundingClientRect();
					const idealTabBounds = liveSnapshot.tabs.map((tab) => {
						const visualIndex = items.findIndex(
							(item) => item.type === 'tab' && item.tab.id === tab.id,
						);
						const ideal = horizontalLayout.bounds[visualIndex];
						const element = [...root.querySelectorAll<HTMLElement>('[data-tab-id]')].find(
							(candidate) => candidate.dataset['tabId'] === tab.id,
						);
						const rect = element?.getBoundingClientRect();
						if (rootRect.width > 0 && ideal) return ideal;
						return {
							x: (rect?.left ?? rootRect.left) - rootRect.left,
							y: 0,
							width: rect?.width ?? TAB_SIZE.standardWidth,
							height: LAYOUT_CONSTANTS.tabStripHeight,
						};
					});
					const groupHeaderWidths = new Map(
						liveSnapshot.groups.map((group) => {
							const visualIndex = items.findIndex(
								(item) => item.type === 'group' && item.group.id === group.id,
							);
							const element = [...root.querySelectorAll<HTMLElement>('[data-group-id]')].find(
								(candidate) => candidate.dataset['groupId'] === group.id,
							);
							const measuredWidth = element?.getBoundingClientRect().width ?? 0;
							return [
								group.id,
								rootRect.width > 0
									? (horizontalLayout.bounds[visualIndex]?.width ?? measuredWidth)
									: measuredWidth,
							] as const;
						}),
					);
					const sourceVisualId = getSourceVisualId(current);
					const compactGroupHeaderDrag =
						current.groupHeaderId !== null && current.groupWasCollapsedBeforeDrag;
					const targetDragViews = compactGroupHeaderDrag
						? current.visualViews.filter((view) => view.kind === 'group')
						: current.visualViews;
					const sourceWidthOverride =
						compactGroupHeaderDrag && current.groupHeaderId !== null
							? groupHeaderWidths.get(current.groupHeaderId)
							: undefined;
					const packedTargetDrag = packHorizontalDragViews({
						views: targetDragViews,
						sourceVisualId,
						pointerRatioX: current.pointerOffsetRatios.x,
						...(sourceWidthOverride === undefined
							? {}
							: {
									sourceWidthOverride,
								}),
					});
					const draggedWidth = packedTargetDrag.width;
					const firstDraggedTabIndex = liveSnapshot.tabs.findIndex((tab) =>
						current.tabIds.includes(tab.id),
					);
					const firstUnpinned = liveSnapshot.tabs.findIndex((candidate) => !candidate.pinned);
					const partition = firstUnpinned === -1 ? liveSnapshot.tabs.length : firstUnpinned;
					const draggedX = event.clientX - packedTargetDrag.mouseOffset - rootRect.left;
					const dragTarget = getHorizontalDragTarget({
						draggedBounds: {
							x: draggedX,
							y: 0,
							width: draggedWidth,
							height: LAYOUT_CONSTANTS.tabStripHeight,
						},
						tabs: liveSnapshot.tabs,
						idealTabBounds,
						firstDraggedTabIndex: Math.max(0, firstDraggedTabIndex),
						draggedTabCount: current.tabIds.length,
						containsGroupHeader: current.visualViews.some((view) => view.kind === 'group'),
						collapsedGroups: new Set(
							liveSnapshot.groups.filter((group) => group.collapsed).map((group) => group.id),
						),
						groupHeaderWidths,
						containerStartIndex: source.pinned ? 0 : partition,
						containerEndIndex: source.pinned ? partition : liveSnapshot.tabs.length,
					});
					targetIndex = dragTarget.index;
					targetIdealLeft = rootRect.left + dragTarget.idealX;
					targetExtent =
						idealTabBounds[Math.min(targetIndex, idealTabBounds.length - 1)]?.width ??
						TAB_SIZE.standardWidth;
					const draggedIds = new Set(current.tabIds);
					const draggedBeforeTarget = liveSnapshot.tabs
						.slice(0, targetIndex)
						.filter((tab) => draggedIds.has(tab.id)).length;
					const remaining = liveSnapshot.tabs.filter((tab) => !draggedIds.has(tab.id));
					const insertion = Math.max(
						0,
						Math.min(remaining.length, targetIndex - draggedBeforeTarget),
					);
					const leftTab = remaining[insertion - 1];
					const rightTab = remaining[insertion];
					const currentGroup =
						liveSnapshot.tabs.find((tab) => draggedIds.has(tab.id) && !tab.pinned)?.group ?? null;
					const leftTabIndex = leftTab
						? liveSnapshot.tabs.findIndex((tab) => tab.id === leftTab.id)
						: -1;
					const leftBounds = leftTabIndex === -1 ? null : idealTabBounds[leftTabIndex];
					const tabLeftInset = DEFAULT_TAB_STYLE_METRICS.tabOverlap / 2;
					horizontalTargetGroup = calculateGroupForDraggedTabs({
						containsGroupHeader: current.visualViews.some((view) => view.kind === 'group'),
						hasSelectedUnpinnedTab: current.tabIds.some(
							(id) => !liveSnapshot.tabs.find((tab) => tab.id === id)?.pinned,
						),
						leftGroup: leftTab?.group ?? null,
						rightGroup: rightTab?.group ?? null,
						currentGroup,
						leftGroupCollapsed: Boolean(
							liveSnapshot.groups.find((group) => group.id === leftTab?.group)?.collapsed,
						),
						rightGroupCollapsed: Boolean(
							liveSnapshot.groups.find((group) => group.id === rightTab?.group)?.collapsed,
						),
						selectedTabWidth: targetExtent,
						tabLeftInset,
						leftEdge: leftBounds ? leftBounds.x + leftBounds.width - tabLeftInset : tabLeftInset,
						selectedLeft: draggedX + tabLeftInset,
						selectedRight: draggedX + draggedWidth,
						tabDragAreaEnd: rootRect.width || current.dragAreaBounds.width,
					});
					calculatedHorizontalGroup = true;
				}
				const containsGroupHeader = current.visualViews.some((view) => view.kind === 'group');
				if (containsGroupHeader) targetGroup = null;
				else if (calculatedHorizontalGroup) targetGroup = horizontalTargetGroup;
				else if (horizontal || source?.pinned) {
					targetGroup = groupForDragInsertion(liveSnapshot, current.tabIds, targetIndex);
				} else targetGroup = verticalNestedTargetGroup;
				let coordinate = horizontal ? event.clientX : event.clientY;
				if (pinnedGridDrag) coordinate = event.clientY * 10_000 + event.clientX;
				const threshold =
					horizontal || pinnedGridDrag
						? Math.max(2, calculateHorizontalMoveThreshold(targetExtent, TAB_SIZE.standardWidth))
						: 8;
				const initialMoveNeedsReorder =
					!current.started &&
					!areDraggedTabIdsConsecutive(current.restoreState.tabs, current.tabIds);
				if (
					!externalTargetActive &&
					(Math.abs(coordinate - current.lastReorderCoordinate) > threshold ||
						initialMoveNeedsReorder)
				) {
					if (source) {
						const firstUnpinned = liveSnapshot.tabs.findIndex((candidate) => !candidate.pinned);
						const partition = firstUnpinned === -1 ? liveSnapshot.tabs.length : firstUnpinned;
						const beforeIndex = model.getIndexOfTab(current.sourceId);
						if (current.groupHeaderId !== null) {
							if (horizontal && current.horizontalGroupDrag !== null) {
								const movement = transitionHorizontalGroupDrag(current.horizontalGroupDrag, {
									type: 'move-to',
									preRemovalTargetIndex: targetIndex,
								});
								applyHorizontalGroupDragCommands(model, current.restoreState, movement.commands);
								assertHorizontalGroupDragRefinement(model, movement.state);
								current = {
									...current,
									horizontalGroupDrag: movement.state,
								};
							} else {
								model.moveGroupByDrag(current.groupHeaderId, targetIndex);
							}
						} else {
							model.moveTabsByDrag(
								current.tabIds,
								targetIndex,
								source.pinned ? 0 : partition,
								source.pinned ? partition : liveSnapshot.tabs.length,
								targetGroup,
								{
									coalesceNonconsecutive: horizontal && !current.started,
								},
							);
						}
						const moved = beforeIndex !== model.getIndexOfTab(current.sourceId);
						if (moved) lastReorderCoordinate = coordinate;
					}
				}
			}
			setDrag({
				...current,
				x: event.clientX,
				y: event.clientY,
				started,
				targetIndex,
				targetGroup,
				targetIdealLeft,
				lastReorderCoordinate,
			});
		};
		const finish = (event: PointerEvent) => {
			const dragAtFinish = dragRef.current;
			if (!dragAtFinish || dragAtFinish.settling || dragAtFinish.pointerId !== event.pointerId)
				return;
			let current: DragState = dragAtFinish;
			if (touchDragTimer.current !== null) window.clearTimeout(touchDragTimer.current);
			touchDragTimer.current = null;
			if (event.type === 'pointercancel') {
				if (dragSettleTimer.current !== null) window.clearTimeout(dragSettleTimer.current);
				if (current.started) current.crossWindowRuntime?.end('cancel');
				if (horizontal && current.horizontalGroupDrag !== null) {
					const canceled = transitionHorizontalGroupDrag(current.horizontalGroupDrag, {
						type: 'cancel',
					});
					applyHorizontalGroupDragCommands(model, current.restoreState, canceled.commands);
					assertHorizontalGroupDragRefinement(model, canceled.state);
				} else {
					model.restoreCanceledDrag(current.restoreState);
					if (
						!horizontal &&
						current.groupHeaderId !== null &&
						!current.groupWasCollapsedBeforeDrag &&
						model.getIndexOfTab(current.sourceId) !== -1
					) {
						model.setSelection([current.sourceId], current.sourceId, {
							expandGroups: false,
						});
					}
				}
				announce('Tab move canceled');
				setDrag(null);
				return;
			}
			const externalTargetActive =
				current.crossWindowRuntime?.getSnapshot?.().targetActive === true;
			if (current.started) current.crossWindowRuntime?.end('complete');
			if (current.started && !externalTargetActive && current.targetIndex !== null) {
				const liveSnapshot = model.getSnapshot();
				const source = liveSnapshot.tabs.find((tab) => tab.id === current.sourceId);
				if (source) {
					const firstUnpinned = liveSnapshot.tabs.findIndex((tab) => !tab.pinned);
					const partition = firstUnpinned === -1 ? liveSnapshot.tabs.length : firstUnpinned;
					if (current.groupHeaderId !== null) {
						if (horizontal && current.horizontalGroupDrag !== null) {
							const movement = transitionHorizontalGroupDrag(current.horizontalGroupDrag, {
								type: 'move-to',
								preRemovalTargetIndex: current.targetIndex,
							});
							applyHorizontalGroupDragCommands(model, current.restoreState, movement.commands);
							assertHorizontalGroupDragRefinement(model, movement.state);
							current = {
								...current,
								horizontalGroupDrag: movement.state,
							};
						} else {
							model.moveGroupByDrag(current.groupHeaderId, current.targetIndex);
						}
					} else {
						model.moveTabsByDrag(
							current.tabIds,
							current.targetIndex,
							source.pinned ? 0 : partition,
							source.pinned ? partition : liveSnapshot.tabs.length,
							current.targetGroup,
						);
					}
				}
			} else if (current.pointerType === 'touch' && current.touchDragReady) {
				// Chromium emits LongTap after LongPress when the pointer is released
				// without entering the drag loop. Tabs open their context menu and
				// group headers open the group editor; this is distinct from a tap.
				if (current.groupHeaderId === null) {
					setMenu({
						kind: 'tab',
						id: current.sourceId,
						x: event.clientX,
						y: event.clientY,
					});
				} else {
					suppressGroupToggleRef.current = current.groupHeaderId;
					openGroupEditor(current.groupHeaderId, event.clientX, event.clientY);
				}
			} else if (
				current.groupHeaderId === null &&
				!event.shiftKey &&
				!current.shiftPressedOnPointerDown &&
				!(event.metaKey || event.ctrlKey)
			) {
				model.selectTab(current.sourceId);
			}
			if (current.started) {
				if (horizontal && !externalTargetActive && current.horizontalGroupDrag !== null) {
					const committed = transitionHorizontalGroupDrag(current.horizontalGroupDrag, {
						type: 'commit',
					});
					applyHorizontalGroupDragCommands(model, current.restoreState, committed.commands);
					assertHorizontalGroupDragRefinement(model, committed.state);
					current = {
						...current,
						horizontalGroupDrag: committed.state,
					};
				}
				const liveSnapshot = model.getSnapshot();
				const finalIndex = liveSnapshot.tabs.findIndex((tab) => tab.id === current.sourceId);
				if (finalIndex !== -1) {
					announce(
						`${current.label} moved to position ${finalIndex + 1} of ${liveSnapshot.tabs.length}`,
					);
				}
				if (current.groupHeaderId !== null) {
					if (
						(current.horizontalGroupDrag === null || externalTargetActive) &&
						!current.groupWasCollapsedBeforeDrag &&
						model.getGroup(current.groupHeaderId)
					) {
						model.updateGroup(current.groupHeaderId, {
							collapsed: false,
						});
					}
					const completionActiveId = current.sourceId;
					if (
						(!horizontal || externalTargetActive) &&
						completionActiveId !== null &&
						model.getIndexOfTab(completionActiveId) !== -1
					) {
						// The active group member remains authoritative even while the
						// collapsed group header is its visible projection.
						model.setSelection([completionActiveId], completionActiveId, {
							expandGroups: false,
						});
					}
				}
			}
			if (
				current.started &&
				horizontal &&
				!externalTargetActive &&
				current.targetIdealLeft !== null
			) {
				const settlingDrag = {
					...current,
					x: event.clientX,
					y: event.clientY,
					settling: true,
				};
				setDrag(settlingDrag);
				dragSettleTimer.current = window.setTimeout(() => {
					dragSettleTimer.current = null;
					setDrag((candidate) => {
						if (!candidate?.settling) return candidate;
						if (horizontal && candidate.horizontalGroupDrag !== null) {
							const settled = transitionHorizontalGroupDrag(candidate.horizontalGroupDrag, {
								type: 'settle',
							});
							applyHorizontalGroupDragCommands(model, candidate.restoreState, settled.commands);
							assertHorizontalGroupDragRefinement(model, settled.state);
						}
						return null;
					});
				}, 200);
			} else {
				setDrag(null);
			}
		};
		window.addEventListener('pointermove', move);
		window.addEventListener('pointerup', finish);
		window.addEventListener('pointercancel', finish);
		const keydown = (event: KeyboardEvent) => {
			const current = dragRef.current;
			if (!current || current.settling || event.key !== 'Escape') return;
			event.preventDefault();
			if (touchDragTimer.current !== null) window.clearTimeout(touchDragTimer.current);
			touchDragTimer.current = null;
			if (dragSettleTimer.current !== null) window.clearTimeout(dragSettleTimer.current);
			if (current.started) current.crossWindowRuntime?.end('cancel');
			if (horizontal && current.horizontalGroupDrag !== null) {
				const canceled = transitionHorizontalGroupDrag(current.horizontalGroupDrag, {
					type: 'cancel',
				});
				applyHorizontalGroupDragCommands(model, current.restoreState, canceled.commands);
				assertHorizontalGroupDragRefinement(model, canceled.state);
			} else {
				model.restoreCanceledDrag(current.restoreState);
				if (
					!horizontal &&
					current.groupHeaderId !== null &&
					!current.groupWasCollapsedBeforeDrag &&
					model.getIndexOfTab(current.sourceId) !== -1
				) {
					model.setSelection([current.sourceId], current.sourceId, {
						expandGroups: false,
					});
				}
			}
			announce('Tab move canceled');
			setDrag(null);
		};
		window.addEventListener('keydown', keydown);
		return () => {
			window.removeEventListener('pointermove', move);
			window.removeEventListener('pointerup', finish);
			window.removeEventListener('pointercancel', finish);
			window.removeEventListener('keydown', keydown);
		};
	}, [
		horizontal,
		model,
		snapshot,
		stripRef,
		verticalCollapsed,
		horizontalLayout,
		items,
		drag,
		openGroupEditor,
	]);
	const onTabKeyDown = (event: React.KeyboardEvent, tabId: string) => {
		if (event.key === 'Enter') {
			if (snapshot.selectedTabIds.has(tabId)) return;
			event.preventDefault();
			model.selectTab(tabId);
			return;
		}
		if (event.key === ' ') {
			event.preventDefault();
			return;
		}
		const previousKey = horizontal ? 'ArrowLeft' : 'ArrowUp';
		const nextKey = horizontal ? 'ArrowRight' : 'ArrowDown';
		let direction = 0;
		if (event.key === previousKey) direction = -1;
		else if (event.key === nextKey) direction = 1;
		if (direction === 0 || !(event.ctrlKey || event.metaKey)) return;
		event.preventDefault();
		if (event.shiftKey) {
			if (direction < 0) model.moveTabFirst(tabId);
			else model.moveTabLast(tabId);
			announce(
				getTabMoveAnnouncement(
					direction < 0 ? 'first' : 'last',
					horizontal ? 'horizontal' : 'vertical',
				),
			);
		} else {
			const before = model.getSnapshot();
			const beforeTab = before.tabs.find((tab) => tab.id === tabId);
			const beforeGroup =
				beforeTab?.group === null || beforeTab?.group === undefined
					? null
					: (before.groups.find((group) => group.id === beforeTab.group) ?? null);
			const tabsInBeforeGroup = beforeGroup
				? before.tabs.filter((tab) => tab.group === beforeGroup.id)
				: [];
			model.shiftTabRelative(tabId, direction < 0 ? -1 : 1);
			const after = model.getSnapshot();
			const afterTab = after.tabs.find((tab) => tab.id === tabId);
			const afterGroup =
				afterTab?.group === null || afterTab?.group === undefined
					? null
					: (after.groups.find((group) => group.id === afterTab.group) ?? null);
			if (beforeTab?.group !== afterTab?.group && (beforeGroup || afterGroup)) {
				const targetGroup = afterGroup ?? requireDefined(beforeGroup);
				const groupTabs = afterGroup
					? before.tabs.filter((tab) => tab.group === afterGroup.id)
					: tabsInBeforeGroup;
				announce(
					getTabGroupMembershipAnnouncement(
						afterGroup ? 'added' : 'removed',
						targetGroup,
						groupTabs,
					),
				);
			} else {
				announce(
					getTabMoveAnnouncement(
						direction < 0 ? 'previous' : 'next',
						horizontal ? 'horizontal' : 'vertical',
					),
				);
			}
		}
	};
	const onTabKeyUp = (event: React.KeyboardEvent, tabId: string) => {
		if (event.key !== ' ') return;
		event.preventDefault();
		if (!snapshot.selectedTabIds.has(tabId)) model.selectTab(tabId);
	};
	const addTab = () => {
		setTabClosingMode(null);
		onNewTabButtonPressed?.();
		if (onNewTab) onNewTab();
		else
			model.addTab({
				title: 'New Tab',
				url: 'chrome://newtab',
				pinned: false,
				group: null,
				split: null,
			});
	};
	const updateUrlDrop = (event: ReactDragEvent<HTMLDivElement>) => {
		const url = urlFromDataTransfer(event.dataTransfer);
		if (!url) return;
		event.preventDefault();
		event.dataTransfer.dropEffect = 'link';
		const target =
			event.target instanceof Element
				? event.target.closest<HTMLElement>('[data-tab-id], [data-group-id]')
				: null;
		const liveSnapshot = model.getSnapshot();
		const rootRect = event.currentTarget.getBoundingClientRect();
		let node: TabLinkDropNode = {
			type: 'unpinned',
			tabCount: liveSnapshot.tabs.length,
		};
		let hint: DragPositionHint | null = DragPositionHint.After;
		let targetRect = rootRect;
		let dropAlongHorizontalAxis = horizontal;
		let dropAxisRtl = false;
		let replacingTab = false;
		let placeAfterTarget = false;
		let hoverTargetIndex: number | null = null;
		let horizontalTabIndex: TabDropIndex | null = null;
		if (target?.dataset['tabId']) {
			const tabIndex = liveSnapshot.tabs.findIndex((tab) => tab.id === target.dataset['tabId']);
			const tab = liveSnapshot.tabs[tabIndex];
			if (tab) {
				targetRect = target.getBoundingClientRect();
				dropAlongHorizontalAxis = horizontal || (tab.pinned && !verticalCollapsed);
				dropAxisRtl =
					dropAlongHorizontalAxis && getComputedStyle(event.currentTarget).direction === 'rtl';
				const coordinate = dropAlongHorizontalAxis ? event.clientX : event.clientY;
				const start = dropAlongHorizontalAxis ? targetRect.left : targetRect.top;
				const extent = dropAlongHorizontalAxis ? targetRect.width : targetRect.height;
				const physicalRatio = extent > 0 ? (coordinate - start) / extent : 0.5;
				const ratio = dropAxisRtl ? 1 - physicalRatio : physicalRatio;
				const margin = horizontal ? 0.25 : 0.2;
				hint = null;
				if (ratio < margin) hint = DragPositionHint.Before;
				else if (ratio > 1 - margin) hint = DragPositionHint.After;
				replacingTab = hint === null;
				if (replacingTab) hoverTargetIndex = tabIndex;
				const groupTabs =
					tab.group === null
						? []
						: liveSnapshot.tabs.filter((candidate) => candidate.group === tab.group);
				if (horizontal) {
					horizontalTabIndex = getHorizontalLinkDropIndexForTab(
						tabIndex,
						hint,
						groupTabs[0]?.id === tab.id,
					);
				}
				node = {
					type: 'tab',
					index: tabIndex,
					groupId: tab.group,
					isLastTabInGroup: groupTabs.at(-1)?.id === tab.id,
				};
			}
		} else if (target?.dataset['groupId']) {
			const group = liveSnapshot.groups.find(
				(candidate) => candidate.id === target.dataset['groupId'],
			);
			const groupIndices = liveSnapshot.tabs
				.map((tab, index) => (tab.group === group?.id ? index : -1))
				.filter((index) => index !== -1);
			if (group && groupIndices.length > 0) {
				targetRect = target.getBoundingClientRect();
				dropAxisRtl = horizontal && getComputedStyle(event.currentTarget).direction === 'rtl';
				const coordinate = horizontal ? event.clientX : event.clientY;
				const start = horizontal ? targetRect.left : targetRect.top;
				const extent = horizontal ? targetRect.width : targetRect.height;
				const physicalRatio = extent > 0 ? (coordinate - start) / extent : 0.5;
				const logicalRatio = dropAxisRtl ? 1 - physicalRatio : physicalRatio;
				placeAfterTarget = logicalRatio >= 0.5;
				hint = null;
				if (logicalRatio < 0.5) hint = DragPositionHint.Before;
				else if (!horizontal && group.collapsed) hint = DragPositionHint.After;
				node = {
					type: 'group',
					start: requireDefined(groupIndices[0]),
					end: requireDefined(groupIndices.at(-1)) + 1,
					// Horizontal TabContainerImpl treats the trailing header half as
					// insertion at the group's start even when the group is collapsed.
					collapsed: horizontal ? false : group.collapsed,
				};
			}
		}
		const index = horizontalTabIndex ?? getLinkDropIndexForNode(node, hint);
		const insertionAfter = hint === DragPositionHint.After;
		const placementAfter = insertionAfter || placeAfterTarget;
		let physicalHorizontalEdge: number;
		if (dropAxisRtl) physicalHorizontalEdge = placementAfter ? targetRect.left : targetRect.right;
		else physicalHorizontalEdge = placementAfter ? targetRect.right : targetRect.left;
		let verticalPlacementY = targetRect.top;
		if (replacingTab) verticalPlacementY = targetRect.top + targetRect.height / 2;
		else if (placementAfter) verticalPlacementY = targetRect.bottom;
		const placement: DropArrowPlacement = dropAlongHorizontalAxis
			? {
					bounds: {
						x:
							(replacingTab ? targetRect.left + targetRect.width / 2 : physicalHorizontalEdge) - 10,
						y: (horizontal ? rootRect.top : targetRect.top) - 8,
						width: 20,
						height: 20,
					},
					direction: DropArrowDirection.Down,
				}
			: {
					bounds: {
						x: rootRect.right - 8,
						y: verticalPlacementY - 10,
						width: 20,
						height: 20,
					},
					direction: DropArrowDirection.Left,
				};
		if (hoverTargetIndex === null) hoverTabSelector.cancelTabTransition();
		else hoverTabSelector.startTabTransition(hoverTargetIndex);
		setUrlDrop({
			index,
			placement,
		});
	};
	const finishUrlDrop = (event: ReactDragEvent<HTMLDivElement>) => {
		const url = urlFromDataTransfer(event.dataTransfer);
		const drop = urlDrop;
		hoverTabSelector.cancelTabTransition();
		setUrlDrop(null);
		if (!url || !drop) return;
		event.preventDefault();
		if (onUrlDrop) {
			onUrlDrop(url, drop.index);
			return;
		}
		const liveSnapshot = model.getSnapshot();
		if (drop.index.relativeToIndex === DropRelativeToIndex.ReplaceIndex) {
			const tab = liveSnapshot.tabs[drop.index.index];
			if (tab) {
				model.updateTab(tab.id, {
					url,
					title: titleForDroppedUrl(url),
				});
				model.selectTab(tab.id);
			}
			return;
		}
		const group =
			drop.index.groupInclusion === DropGroupInclusion.IncludeInGroup
				? (liveSnapshot.tabs[Math.min(drop.index.index, liveSnapshot.tabs.length - 1)]?.group ??
					null)
				: null;
		model.addTab(
			{
				title: titleForDroppedUrl(url),
				url,
				pinned: false,
				group,
				split: null,
			},
			{
				index: drop.index.index,
				activate: true,
			},
		);
	};
	const animatedHorizontalBoundsById = new Map(
		animatedHorizontalFrame?.layout.children.map((child) => [child.id, child.bounds]) ?? [],
	);
	const paintedHorizontalBounds = (index: number) => {
		const item = items[index];
		return item
			? (animatedHorizontalBoundsById.get(item.id) ?? horizontalLayout?.bounds[index])
			: horizontalLayout?.bounds[index];
	};
	const settleTargetLeft = (() => {
		if (!horizontal || !drag?.settling || !horizontalLayout) return null;
		const leadingDraggedView = drag.visualViews[0];
		if (!leadingDraggedView) return null;
		const visualItemId =
			leadingDraggedView.kind === 'group'
				? `group-${leadingDraggedView.id.slice('group:'.length)}`
				: leadingDraggedView.id;
		const visualIndex = items.findIndex((item) => item.id === visualItemId);
		const finalBounds = horizontalLayout.bounds[visualIndex];
		return finalBounds ? drag.dragAreaBounds.left + finalBounds.x : null;
	})();
	const rootClass = `ct-tab-strip ${className}`.trim();
	const rootStyle = focusedGroup
		? ({
				...style,
				'--ct-focused-group-color': GROUP_COLORS[focusedGroup.color],
			} satisfies CSSProperties & Record<'--ct-focused-group-color', string>)
		: style;
	return (
		<div
			ref={stripRef}
			className={rootClass}
			style={rootStyle}
			role='tablist'
			aria-label={ariaLabel}
			aria-orientation={horizontal ? 'horizontal' : 'vertical'}
			data-ct-root=''
			data-orientation={orientation}
			data-touch-ui={touchUi || undefined}
			data-collapsed={effectiveVerticalCollapsed || undefined}
			data-expanded-on-hover={expandedOnHover || undefined}
			data-waiting-to-expand={expandOnHoverSnapshot.waitingToExpand || undefined}
			data-hover-card-waiting={hoverCardSnapshot.waitingToShow || undefined}
			data-dragging={(drag?.started && !drag.settling) || undefined}
			data-focused-group={focusedGroup?.id}
			data-tab-closing-mode={tabClosingMode?.source}
			onScrollCapture={() => {
				clearHover(HoverCardUpdateType.Event);
				if (menu?.kind === 'group' && groupEditorTracker.onContentsScrolled()) {
					setMenu(null);
				}
			}}
			onMouseEnter={(event: ReactMouseEvent<HTMLDivElement>) => {
				const target = event.target;
				if (!horizontal && target instanceof Node && event.currentTarget.contains(target)) {
					expandOnHoverController.recordPointerPosition?.(
						event.clientX,
						event.currentTarget.getBoundingClientRect().right,
					);
					expandOnHoverController.setHovered(true);
				}
			}}
			onMouseMove={(event: ReactMouseEvent<HTMLDivElement>) => {
				const target = event.target;
				if (!horizontal && target instanceof Node && event.currentTarget.contains(target)) {
					expandOnHoverController.recordPointerPosition?.(
						event.clientX,
						event.currentTarget.getBoundingClientRect().right,
					);
				}
			}}
			onMouseLeave={(event: ReactMouseEvent<HTMLDivElement>) => {
				const target = event.target;
				if (!horizontal && target instanceof Node && event.currentTarget.contains(target)) {
					expandOnHoverController.setHovered(false);
				}
			}}
			onFocusCapture={(event: ReactFocusEvent<HTMLDivElement>) => {
				const target = event.target;
				if (!horizontal && target instanceof Node && event.currentTarget.contains(target)) {
					expandOnHoverController.setFocused(true);
				}
				const anchor =
					target instanceof Element
						? target.closest<HTMLElement>('[data-tab-id], [data-group-id]')
						: null;
				if (anchor?.dataset['tabId']) onTabKeyboardFocusChanged?.(anchor.dataset['tabId']);
				if (anchor && event.currentTarget.contains(anchor)) {
					if (anchor.dataset['tabId'])
						updateHoverForElement(anchor, anchor.dataset['tabId'], HoverCardUpdateType.Focus);
					else if (anchor.dataset['groupId'] && showGroupHoverCards) {
						updateHoverForElement(
							anchor,
							anchor.dataset['groupId'],
							HoverCardUpdateType.Focus,
							'group',
						);
					}
				}
			}}
			onBlurCapture={(event: ReactFocusEvent<HTMLDivElement>) => {
				const target = event.target;
				const next = event.relatedTarget;
				if (
					!horizontal &&
					target instanceof Node &&
					event.currentTarget.contains(target) &&
					(!(next instanceof Node) || !event.currentTarget.contains(next))
				) {
					expandOnHoverController.setFocused(false);
				}
				if (!(next instanceof Node) || !event.currentTarget.contains(next)) {
					onTabKeyboardFocusChanged?.(null);
				}
				const anchor =
					target instanceof Element
						? target.closest<HTMLElement>('[data-tab-id], [data-group-id]')
						: null;
				if (anchor && event.currentTarget.contains(anchor)) clearHover(HoverCardUpdateType.Focus);
			}}
			onDragOver={updateUrlDrop}
			onDragLeave={(event: ReactDragEvent<HTMLDivElement>) => {
				const next = event.relatedTarget;
				if (!(next instanceof Node) || !event.currentTarget.contains(next)) {
					hoverTabSelector.cancelTabTransition();
					setUrlDrop(null);
				}
			}}
			onDrop={finishUrlDrop}
			onDragEnd={() => {
				hoverTabSelector.cancelTabTransition();
				setUrlDrop(null);
			}}
		>
			{horizontal ? (
				<div className='ct-horizontal-track' data-source-layout=''>
					{snapshot.groups.map((group) => {
						if (groupFocusActive && group.id !== focusedGroup.id) return null;
						const groupItemIndex = items.findIndex(
							(item) => item.type === 'group' && item.group.id === group.id,
						);
						const groupTabIndices = items
							.map((item, index) =>
								item.type === 'tab' && item.tab.group === group.id ? index : -1,
							)
							.filter((index) => index !== -1);
						if (
							!horizontalLayout ||
							groupItemIndex < 0 ||
							groupTabIndices.length === 0 ||
							(group.collapsed &&
								fullyCollapsedGroupIds.has(group.id) &&
								focusedGroup?.id !== group.id)
						)
							return null;
						const start = paintedHorizontalBounds(groupItemIndex)?.x ?? 0;
						const last = paintedHorizontalBounds(requireDefined(groupTabIndices.at(-1)));
						if (!last) return null;
						const trailingTab = snapshot.tabs.filter((tab) => tab.group === group.id).at(-1);
						const underline = calculateHorizontalTabGroupUnderlineBounds({
							leadingHeaderX: start,
							trailingTabX: last.x,
							trailingTabWidth: last.width,
							trailingTabActive: trailingTab ? tabIsApparentlyActive(trailingTab, snapshot) : false,
						});
						return (
							<span
								key={group.id}
								className='ct-group-underline'
								data-group-underline-id={group.id}
								style={
									{
										left: underline.x,
										width: underline.width,
										zIndex: calculateTabContainerZValue({
											kind: 'group-underline',
										}),
										'--ct-group-color': GROUP_COLORS[group.color],
									} satisfies CSSProperties & Record<'--ct-group-color', string>
								}
							/>
						);
					})}
					{items.map((item, index) => {
						const bounds = paintedHorizontalBounds(index) ?? {
							x: 0,
							width: 0,
							y: 0,
							height: 41,
						};
						const animatedOpacity = animatedHorizontalFrame?.opacityById.get(item.id);
						const itemStyle = {
							left: bounds.x,
							width: bounds.width,
							...(animatedOpacity !== undefined && animatedOpacity < 1
								? {
										opacity: animatedOpacity,
									}
								: {}),
						};
						const hiddenByGroupFocus =
							groupFocusActive &&
							(item.type === 'group'
								? item.group.id !== focusedGroup.id
								: !tabVisibleWhileFocused(item.tab));
						if (item.type === 'group') {
							const activeGroupHeader =
								item.group.collapsed &&
								snapshot.tabs.some(
									(tab) => tab.id === snapshot.activeTabId && tab.group === item.group.id,
								);
							return (
								<div
									key={item.id}
									className='ct-horizontal-item ct-horizontal-group'
									style={{
										...itemStyle,
										zIndex: calculateTabContainerZValue({
											kind: 'group-header',
											active: activeGroupHeader,
										}),
									}}
									data-focus-hidden={hiddenByGroupFocus || undefined}
									aria-hidden={hiddenByGroupFocus || undefined}
									data-drag-source={
										(drag?.started &&
											drag.visualViews.some((view) => view.id === `group:${item.group.id}`)) ||
										undefined
									}
									data-slot-animating={
										horizontalAnimationRuntimeRef.current?.getSlot(item.id)?.animating || undefined
									}
								>
									<GroupHeader
										group={item.group}
										tabs={snapshot.tabs.filter((tab) => tab.group === item.group.id)}
										activeTabId={snapshot.activeTabId}
										orientation={orientation}
										collapsedStrip={false}
										onToggle={(source: 'mouse' | 'touch' | 'keyboard') =>
											toggleGroupFromHeader(item.group.id, source)
										}
										onPointerDown={(event: ReactPointerEvent<HTMLDivElement>) =>
											onGroupPointerDown(event, item.group)
										}
										onKeyDown={(event: ReactKeyboardEvent<HTMLDivElement>) =>
											onGroupKeyDown(event, item.group.id)
										}
										onContextMenu={(event: ReactMouseEvent<HTMLDivElement>) => {
											event.preventDefault();
											openGroupEditor(item.group.id, event.clientX, event.clientY);
										}}
										onMouseEnter={(event: ReactMouseEvent<HTMLDivElement>) =>
											onGroupMouseEnter(event, item.group.id)
										}
										onMouseLeave={clearHover}
										showHoverCard={showGroupHoverCards}
										frameCondensed={horizontalFrameCondensed}
									/>
								</div>
							);
						}
						const dragging = Boolean(drag?.started && drag.tabIds.includes(item.tab.id));
						const hiddenByCollapsedGroup =
							item.tab.group !== null &&
							fullyCollapsedGroupIds.has(item.tab.group) &&
							Boolean(snapshot.groups.find((group) => group.id === item.tab.group)?.collapsed) &&
							item.tab.group !== expandedHeaderDragGroupId &&
							focusedGroup?.id !== item.tab.group;
						const active = tabIsApparentlyActive(item.tab, snapshot);
						const selected = snapshot.selectedTabIds.has(item.tab.id);
						const hovering = hover?.kind === 'tab' && hover.id === item.tab.id;
						const slotState = horizontalAnimationRuntimeRef.current?.getSlot(item.id);
						const zIndex = dragging
							? 32
							: calculateTabContainerZValue({
									kind: 'tab',
									active,
									selected,
									hovering,
									hoverAnimationValue: hovering ? 1 : 0,
								});
						return (
							<div
								key={item.id}
								className='ct-horizontal-item'
								style={{
									...itemStyle,
									zIndex,
								}}
								data-focus-hidden={hiddenByGroupFocus || undefined}
								data-group-collapsed-hidden={hiddenByCollapsedGroup || undefined}
								aria-hidden={hiddenByGroupFocus || hiddenByCollapsedGroup || undefined}
								data-drag-source={dragging || undefined}
								data-closing={closingTabIds.has(item.tab.id) || undefined}
							>
								<TabButton
									tab={item.tab}
									snapshot={snapshot}
									width={bounds.width}
									orientation={orientation}
									collapsed={false}
									closing={closingTabIds.has(item.tab.id)}
									onPointerDown={onTabPointerDown}
									onClose={(event: ReactMouseEvent, id: string, source: TabCloseSource) => {
										event.stopPropagation();
										requestCloseTabs([id], source);
									}}
									onContextMenu={(event: ReactMouseEvent, id: string) => {
										event.preventDefault();
										setMenu({
											kind: 'tab',
											id,
											x: event.clientX,
											y: event.clientY,
										});
									}}
									onMouseEnter={onTabMouseEnter}
									onMouseLeave={clearHover}
									onKeyDown={onTabKeyDown}
									onKeyUp={onTabKeyUp}
									enableTabMuting={enableTabMuting}
									touchUi={touchUi}
									closeButtonAllowed={!lockedForOnTask}
									declutterEnabled={tabStripDeclutter}
									hoveredTabId={hover?.kind === 'tab' ? hover.id : null}
									frameCondensed={horizontalFrameCondensed}
									dragging={dragging}
									{...(slotState === undefined
										? {}
										: {
												slotState,
											})}
									onAudioMuteToggle={(tabId: string, muted: boolean) => {
										model.updateTab(tabId, {
											siteMuted: muted,
											alertState: muted ? 'audio-muting' : 'audio-playing',
										});
										onTabAudioMuteToggle?.(tabId, muted);
									}}
								/>
							</div>
						);
					})}
					{showNewTabButton ? (
						<TabStripControlButton
							className='ct-new-tab'
							type='button'
							aria-label='New tab'
							icon={<TabStripIcon name='plus' />}
							frameCondensed={horizontalFrameCondensed}
							onClick={addTab}
							onAuxClick={(event: ReactMouseEvent<HTMLButtonElement>) => {
								if (event.button === 1) onNewTabFromClipboard?.();
							}}
							onContextMenu={(event: ReactMouseEvent<HTMLButtonElement>) => {
								event.preventDefault();
								setMenu({
									kind: 'new-tab',
									x: event.clientX,
									y: event.clientY,
								});
							}}
							style={{
								left: Math.min(
									(paintedHorizontalBounds(items.length - 1)?.x ?? 0) +
										(paintedHorizontalBounds(items.length - 1)?.width ?? 0) +
										4,
									Math.max(0, size.width - 38),
								),
							}}
						/>
					) : null}
				</div>
			) : (
				<VerticalContents
					snapshot={snapshot}
					fullyCollapsedGroupIds={fullyCollapsedGroupIds}
					collapsed={verticalCollapsed}
					expandedOnHover={expandedOnHover}
					expandOnHoverEnabled={verticalExpandOnHover}
					topContainerCollapsed={verticalCollapsed}
					bottomContainerCollapsed={verticalCollapsed}
					availableWidth={
						size.width ||
						(effectiveVerticalCollapsed ? LAYOUT_CONSTANTS.verticalTabStripCollapsedWidth : 240)
					}
					drag={drag}
					model={model}
					closingTabIds={closingTabIds}
					requestCloseTabs={requestCloseTabs}
					onTabPointerDown={onTabPointerDown}
					onGroupPointerDown={onGroupPointerDown}
					toggleGroupFromHeader={toggleGroupFromHeader}
					onGroupKeyDown={onGroupKeyDown}
					onTabKeyDown={onTabKeyDown}
					onTabKeyUp={onTabKeyUp}
					onTabMouseEnter={onTabMouseEnter}
					onGroupMouseEnter={onGroupMouseEnter}
					showGroupHoverCards={showGroupHoverCards}
					clearHover={clearHover}
					setMenu={setMenu}
					onOpenGroupEditor={openGroupEditor}
					showNewTabButton={showNewTabButton}
					addTab={addTab}
					enableTabMuting={enableTabMuting}
					touchUi={touchUi}
					closeButtonAllowed={!lockedForOnTask}
					showTopControls={showVerticalTopControls}
					showProjectsButton={comboButtonSnapshot.startVisible}
					showTabSearchButton={comboButtonSnapshot.tabSearchVisible}
					projectsTargetVisible={comboButtonSnapshot.startTargetVisible}
					tabSearchTargetVisible={comboButtonSnapshot.tabSearchTargetVisible}
					projectsAnimating={comboButtonSnapshot.startAnimating}
					tabSearchAnimating={comboButtonSnapshot.tabSearchAnimating}
					startButtonLabel={
						comboButtonSnapshot.startKind === TabStripComboStartKind.EverythingMenu
							? 'Tab groups'
							: 'Projects'
					}
					groupFocused={verticalGroupFocused || groupFocusActive}
					focusedGroupId={focusedGroup?.id ?? null}
					showPinnedTabsInFocusedGroups={showPinnedTabsInFocusedGroups}
					projectsOpen={verticalProjectsOpen}
					toolbarHeight={verticalToolbarHeight}
					captionButtonWidth={verticalCaptionButtonWidth}
					isExitingExpandOnHover={verticalIsExitingExpandOnHover}
					onUnfocusGroup={() => {
						if (snapshot.focusedGroupId !== null) model.setFocusedGroup(null);
						onVerticalUnfocusGroup?.();
					}}
					onProjectsContextMenu={(event: ReactMouseEvent<HTMLButtonElement>) => {
						event.preventDefault();
						comboButtonController.onMenuOpened('start');
						setMenu({
							kind: 'combo-start',
							x: event.clientX,
							y: event.clientY,
						});
					}}
					onTabSearchContextMenu={(event: ReactMouseEvent<HTMLButtonElement>) => {
						event.preventDefault();
						comboButtonController.onMenuOpened('tab-search');
						setMenu({
							kind: 'combo-end',
							x: event.clientX,
							y: event.clientY,
						});
					}}
					onCollapseContextMenu={(event: ReactMouseEvent<HTMLButtonElement>) => {
						event.preventDefault();
						setMenu({
							kind: 'collapse',
							x: event.clientX,
							y: event.clientY,
						});
					}}
					{...(onNewTabFromClipboard === undefined
						? {}
						: {
								onNewTabFromClipboard,
							})}
					{...(onTabAudioMuteToggle === undefined
						? {}
						: {
								onTabAudioMuteToggle,
							})}
					{...(onVerticalCollapsedChange === undefined
						? {}
						: {
								onCollapsedChange: onVerticalCollapsedChange,
							})}
					{...(onVerticalProjectsClick === undefined
						? {}
						: {
								onProjectsClick: onVerticalProjectsClick,
							})}
					{...(onVerticalTabSearchClick === undefined
						? {}
						: {
								onTabSearchClick: onVerticalTabSearchClick,
							})}
				/>
			)}
			{drag?.started ? (
				<DragGhost
					drag={drag}
					snapshot={snapshot}
					orientation={orientation}
					collapsed={verticalCollapsed}
					settleTargetLeft={settleTargetLeft}
					enableTabMuting={enableTabMuting}
					touchUi={touchUi}
					closeButtonAllowed={!lockedForOnTask}
					declutterEnabled={tabStripDeclutter}
				/>
			) : null}
			{urlDrop ? <DropArrow placement={urlDrop.placement} /> : null}
			{menu ? (
				<ContextMenu
					state={menu}
					snapshot={snapshot}
					model={model}
					orientation={orientation}
					requestCloseTabs={requestCloseTabs}
					closeAllowed={!lockedForOnTask}
					verticalExpandOnHover={verticalExpandOnHover}
					comboButtonController={comboButtonController}
					enableTabMuting={enableTabMuting}
					enableTabGroupFocusing={enableTabGroupFocusing}
					showSavedGroupFooter={showSavedGroupFooter}
					enableTabGroupHome={enableTabGroupHome}
					bookmarksEditable={bookmarksEditable}
					confirmSavedGroupDestructiveActions={confirmSavedGroupDestructiveActions}
					{...(onVerticalExpandOnHoverChange === undefined
						? {}
						: {
								onVerticalExpandOnHoverChange,
							})}
					{...(onTabAudioMuteToggle === undefined
						? {}
						: {
								onTabAudioMuteToggle,
							})}
					{...(onMoveTabsToNewWindow === undefined
						? {}
						: {
								onMoveTabsToNewWindow,
							})}
					{...(onReloadTabs === undefined
						? {}
						: {
								onReloadTabs,
							})}
					{...(onAddTabsToReadingList === undefined
						? {}
						: {
								onAddTabsToReadingList,
							})}
					{...(onManageGroupSharing === undefined
						? {}
						: {
								onManageGroupSharing,
							})}
					{...(onShareGroup === undefined
						? {}
						: {
								onShareGroup,
							})}
					{...(onMoveGroupToNewWindow === undefined
						? {}
						: {
								onMoveGroupToNewWindow,
							})}
					{...(onDeleteSavedGroup === undefined
						? {}
						: {
								onDeleteSavedGroup,
							})}
					{...(onOpenTabGroupsLearnMore === undefined
						? {}
						: {
								onOpenTabGroupsLearnMore,
							})}
					{...(onOpenTabGroupHome === undefined
						? {}
						: {
								onOpenTabGroupHome,
							})}
					{...(onConvertGroupToBookmark === undefined
						? {}
						: {
								onConvertGroupToBookmark,
							})}
					{...(onAskGeminiAboutGroup === undefined
						? {}
						: {
								onAskGeminiAboutGroup,
							})}
					{...(onGroupActivityAction === undefined
						? {}
						: {
								onGroupActivityAction,
							})}
					{...(onSeeAllGroupActivity === undefined
						? {}
						: {
								onSeeAllGroupActivity,
							})}
					onClose={() => {
						if (menu.kind === 'combo-start' || menu.kind === 'combo-end')
							comboButtonController.onMenuClosed();
						setMenu(null);
					}}
				/>
			) : null}
			{pendingSavedClose
				? createPortal(
						<div
							className='ct-saved-group-confirmation ct-saved-group-confirmation--center'
							role='alertdialog'
							aria-modal='true'
							aria-labelledby='ct-close-saved-group-title'
							onPointerDown={(event: ReactPointerEvent<HTMLDivElement>) => event.stopPropagation()}
						>
							<strong id='ct-close-saved-group-title'>{'Close saved group?'}</strong>
							<span>{'Closing the remaining tabs will remove this saved group.'}</span>

							<div className='ct-saved-group-confirmation__actions'>
								<button type='button' onClick={() => setPendingSavedClose(null)}>
									{'Cancel'}
								</button>

								<button
									type='button'
									onClick={() => {
										const pending = pendingSavedClose;
										setPendingSavedClose(null);
										requestCloseTabs(pending.tabIds, pending.source, true);
									}}
								>
									{'Close group'}
								</button>
							</div>
						</div>,
						document.body,
					)
				: null}
			{hover &&
			hoverCardSnapshot.visible &&
			hoverCardSnapshot.targetId === `${hover.kind}:${hover.id}` &&
			!drag?.started &&
			!menu ? (
				<HoverCard
					hover={hover}
					snapshot={snapshot}
					showPreviews={showHoverCardPreviews}
					showMemoryUsage={showHoverCardMemoryUsage}
					transitioning={hoverCardSnapshot.transitioning}
					thumbnailSnapshot={thumbnailSnapshot}
				/>
			) : null}

			<span
				key={announcement.sequence}
				className='ct-sr-only'
				role='status'
				aria-live='polite'
				aria-atomic='true'
			>
				{announcement.text}
			</span>
		</div>
	);
}
function groupForDragInsertion(
	snapshot: TabModelSnapshot,
	draggedIds: readonly string[],
	targetIndex: number,
): TabData['group'] {
	const draggedIdSet = new Set(draggedIds);
	const dragged = snapshot.tabs.filter((tab) => draggedIdSet.has(tab.id));
	if (dragged.length === 0 || dragged.some((tab) => tab.pinned)) return null;
	const clampedTarget = Math.min(snapshot.tabs.length, Math.max(0, targetIndex));
	const draggedBeforeTarget = snapshot.tabs
		.slice(0, clampedTarget)
		.filter((tab) => draggedIdSet.has(tab.id)).length;
	const remaining = snapshot.tabs.filter((tab) => !draggedIdSet.has(tab.id));
	const insertion = Math.min(remaining.length, Math.max(0, clampedTarget - draggedBeforeTarget));
	const leftGroup = remaining[insertion - 1]?.group ?? null;
	const rightGroup = remaining[insertion]?.group ?? null;
	if (leftGroup === rightGroup) return leftGroup;
	const sourceGroups = new Set(dragged.map((tab) => tab.group));
	if (sourceGroups.size === 1) {
		const sourceGroup = dragged[0]?.group ?? null;
		const sourceGroupData =
			sourceGroup === null ? null : snapshot.groups.find((group) => group.id === sourceGroup);
		if (
			sourceGroup !== null &&
			!sourceGroupData?.collapsed &&
			(leftGroup === sourceGroup || rightGroup === sourceGroup)
		) {
			return sourceGroup;
		}
	}
	return null;
}
function VerticalSplitView({
	tabs,
	collapsed,
	pinned,
	fallbackWidth,
	dragging = false,
	renderTab,
}: {
	tabs: readonly [Readonly<TabData>, Readonly<TabData>];
	collapsed: boolean;
	pinned: boolean;
	fallbackWidth: number;
	dragging?: boolean;
	renderTab: (
		tab: Readonly<TabData>,
		width: number,
		bounds: CSSProperties,
		splitLayout: 'side-by-side' | 'stacked',
		splitHovered: boolean,
	) => ReactNode;
}) {
	const [ref, size] = useElementSize<HTMLDivElement>();
	const [hovered, setHovered] = useState(false);
	const width = size.width || fallbackWidth;
	const childHeight = pinned
		? LAYOUT_CONSTANTS.verticalTabPinnedHeight
		: LAYOUT_CONSTANTS.verticalTabHeight;
	const layout = calculateSplitTabViewLayout({
		orientation: TabStripOrientation.Vertical,
		sizeBounds: {
			width,
			height: null,
		},
		children: [
			{
				width: Math.floor(width / 2),
				height: childHeight,
			},
			{
				width: Math.ceil(width / 2),
				height: childHeight,
			},
		],
		collapsed,
		pinned,
	});
	const splitLayout =
		layout.children[0]?.bounds.y === layout.children[1]?.bounds.y ? 'side-by-side' : 'stacked';
	return (
		<div
			ref={ref}
			className='ct-vertical-split'
			data-layout={splitLayout}
			data-pinned={pinned || undefined}
			data-split-id={tabs[0].split ?? undefined}
			data-drag-source={dragging || undefined}
			data-hovered={hovered || undefined}
			style={{
				height: layout.hostSize.height,
			}}
			onMouseEnter={() => setHovered(true)}
			onMouseMove={() => setHovered(true)}
			onMouseLeave={() => setHovered(false)}
		>
			{layout.children.map((child, index) => {
				const tab = tabs[index];
				if (tab === undefined) return null;
				return renderTab(
					tab,
					child.bounds.width,
					{
						left: child.bounds.x,
						top: child.bounds.y,
						width: child.bounds.width,
						height: child.bounds.height,
					},
					splitLayout,
					hovered,
				);
			})}
		</div>
	);
}
function VerticalContents({
	snapshot,
	fullyCollapsedGroupIds,
	collapsed,
	expandedOnHover,
	expandOnHoverEnabled,
	topContainerCollapsed,
	bottomContainerCollapsed,
	availableWidth,
	drag,
	model,
	closingTabIds,
	requestCloseTabs,
	onTabPointerDown,
	onGroupPointerDown,
	toggleGroupFromHeader,
	onGroupKeyDown,
	onTabKeyDown,
	onTabKeyUp,
	onTabMouseEnter,
	onGroupMouseEnter,
	showGroupHoverCards,
	clearHover,
	setMenu,
	onOpenGroupEditor,
	showNewTabButton,
	addTab,
	onNewTabFromClipboard,
	enableTabMuting,
	touchUi,
	closeButtonAllowed,
	onTabAudioMuteToggle,
	showTopControls,
	showProjectsButton,
	showTabSearchButton,
	projectsTargetVisible,
	tabSearchTargetVisible,
	projectsAnimating,
	tabSearchAnimating,
	startButtonLabel,
	groupFocused,
	focusedGroupId,
	showPinnedTabsInFocusedGroups,
	projectsOpen,
	toolbarHeight,
	captionButtonWidth,
	isExitingExpandOnHover,
	onCollapsedChange,
	onUnfocusGroup,
	onProjectsClick,
	onTabSearchClick,
	onProjectsContextMenu,
	onTabSearchContextMenu,
	onCollapseContextMenu,
}: {
	snapshot: TabModelSnapshot;
	fullyCollapsedGroupIds: ReadonlySet<string>;
	collapsed: boolean;
	expandedOnHover: boolean;
	expandOnHoverEnabled: boolean;
	topContainerCollapsed: boolean;
	bottomContainerCollapsed: boolean;
	availableWidth: number;
	drag: DragState | null;
	model: TabModel;
	closingTabIds: ReadonlySet<string>;
	requestCloseTabs: (tabIds: readonly string[], source?: TabCloseSource) => void;
	onTabPointerDown: (event: ReactPointerEvent<HTMLDivElement>, tab: Readonly<TabData>) => void;
	onGroupPointerDown: (
		event: ReactPointerEvent<HTMLDivElement>,
		group: Readonly<TabGroupData>,
	) => void;
	toggleGroupFromHeader: (groupId: string, source?: 'mouse' | 'touch' | 'keyboard') => void;
	onGroupKeyDown: (event: React.KeyboardEvent<HTMLDivElement>, groupId: string) => void;
	onTabKeyDown: (event: React.KeyboardEvent, tabId: string) => void;
	onTabKeyUp: (event: React.KeyboardEvent, tabId: string) => void;
	onTabMouseEnter: (event: ReactMouseEvent<HTMLDivElement>, tabId: string) => void;
	onGroupMouseEnter: (event: ReactMouseEvent<HTMLDivElement>, groupId: string) => void;
	showGroupHoverCards: boolean;
	clearHover: () => void;
	setMenu: (menu: MenuState) => void;
	onOpenGroupEditor: (groupId: string, x: number, y: number) => void;
	showNewTabButton: boolean;
	addTab: () => void;
	onNewTabFromClipboard?: () => void;
	enableTabMuting: boolean;
	touchUi: boolean;
	closeButtonAllowed: boolean;
	onTabAudioMuteToggle?: (tabId: string, muted: boolean) => void;
	showTopControls: boolean;
	showProjectsButton: boolean;
	showTabSearchButton: boolean;
	projectsTargetVisible: boolean;
	tabSearchTargetVisible: boolean;
	projectsAnimating: boolean;
	tabSearchAnimating: boolean;
	startButtonLabel: string;
	groupFocused: boolean;
	focusedGroupId: string | null;
	showPinnedTabsInFocusedGroups: boolean;
	projectsOpen: boolean;
	toolbarHeight: number;
	captionButtonWidth: number;
	isExitingExpandOnHover: boolean;
	onCollapsedChange?: (collapsed: boolean) => void;
	onUnfocusGroup?: () => void;
	onProjectsClick?: () => void;
	onTabSearchClick?: () => void;
	onProjectsContextMenu: (event: ReactMouseEvent<HTMLButtonElement>) => void;
	onTabSearchContextMenu: (event: ReactMouseEvent<HTMLButtonElement>) => void;
	onCollapseContextMenu: (event: ReactMouseEvent<HTMLButtonElement>) => void;
}) {
	const scrollRef = useRef<HTMLDivElement>(null);
	const pinnedScrollRef = useRef<HTMLDivElement>(null);
	const [regionsRef, regionsSize] = useElementSize<HTMLDivElement>();
	const previousAvailableWidth = useRef(availableWidth);
	const sizeAnimationTimer = useRef<number | null>(null);
	const [isAnimatingSize, setIsAnimatingSize] = useState(false);
	const dragScrollHandler = useRef<TabDragScrollHandler | null>(null);
	if (dragScrollHandler.current === null) dragScrollHandler.current = new TabDragScrollHandler();
	useEffect(() => () => dragScrollHandler.current?.destroy(), []);
	useEffect(() => {
		if (previousAvailableWidth.current === availableWidth) return;
		previousAvailableWidth.current = availableWidth;
		setIsAnimatingSize(true);
		if (sizeAnimationTimer.current !== null) window.clearTimeout(sizeAnimationTimer.current);
		sizeAnimationTimer.current = window.setTimeout(() => {
			sizeAnimationTimer.current = null;
			setIsAnimatingSize(false);
		}, TAB_BOUNDS_ANIMATION_DURATION_MS);
	}, [availableWidth]);
	useEffect(
		() => () => {
			if (sizeAnimationTimer.current !== null) window.clearTimeout(sizeAnimationTimer.current);
		},
		[],
	);
	useEffect(() => {
		const handler = dragScrollHandler.current;
		const sourceTab = drag?.tabIds.length
			? snapshot.tabs.find((tab) => tab.id === drag.tabIds[0])
			: null;
		const element = sourceTab?.pinned ? pinnedScrollRef.current : scrollRef.current;
		if (!element || !handler || !drag?.started) {
			handler?.stopScrolling();
			return;
		}
		const rect = element.getBoundingClientRect();
		handler.onDraggedTabPositionUpdated(createDomTabDragScrollable(element), {
			x: drag.x - rect.left,
			y: drag.y - rect.top,
			width: Math.max(0, availableWidth - LAYOUT_CONSTANTS.verticalTabStripHorizontalPadding * 2),
			height: 32,
		});
	}, [
		availableWidth,
		drag?.started,
		drag?.tabIds,
		drag?.x,
		drag?.y,
		snapshot.revision,
		snapshot.tabs,
	]);
	const pinned = snapshot.tabs.filter(
		(tab) => tab.pinned && (focusedGroupId === null || showPinnedTabsInFocusedGroups),
	);
	const unpinned = snapshot.tabs.filter(
		(tab) => !tab.pinned && (focusedGroupId === null || tab.group === focusedGroupId),
	);
	const groups = new Map(snapshot.groups.map((group) => [group.id, group]));
	const draggedWholeGroupIds = new Set(
		drag?.started
			? drag.visualViews.flatMap((view) =>
					view.kind === 'group' ? [view.id.slice('group:'.length)] : [],
				)
			: [],
	);
	const draggedWholeSplitIds = new Set(
		drag?.started
			? drag.visualViews.flatMap((view) => {
					if (view.kind !== 'tab' || view.tabIds?.length !== 2) return [];
					const splitId = snapshot.tabs.find((tab) => tab.id === view.tabIds?.[0])?.split;
					return splitId === null || splitId === undefined ? [] : [splitId];
				})
			: [],
	);
	const renderedGroups = new Set<string>();
	const renderedSplits = new Set<string>();
	const defaultTabWidth = Math.max(
		0,
		availableWidth - LAYOUT_CONSTANTS.verticalTabStripHorizontalPadding * 2,
	);
	const renderTab = (
		tab: Readonly<TabData>,
		width = defaultTabWidth,
		splitBounds?: CSSProperties,
		splitLayout?: 'side-by-side' | 'stacked',
		splitHovered = false,
	) => (
		<div
			key={tab.id}
			className='ct-vertical-tab-wrap'
			data-split-child={splitBounds ? '' : undefined}
			style={splitBounds}
			data-drag-source={
				(drag?.started &&
					drag.tabIds.includes(tab.id) &&
					(tab.group === null || !draggedWholeGroupIds.has(tab.group)) &&
					(tab.split === null || !draggedWholeSplitIds.has(tab.split))) ||
				undefined
			}
			data-closing={closingTabIds.has(tab.id) || undefined}
		>
			<TabButton
				tab={tab}
				snapshot={snapshot}
				width={width}
				orientation={TabStripOrientation.Vertical}
				collapsed={collapsed}
				closing={closingTabIds.has(tab.id)}
				onPointerDown={onTabPointerDown}
				onClose={(event: ReactMouseEvent, id: string, source: TabCloseSource) => {
					event.stopPropagation();
					requestCloseTabs([id], source);
				}}
				onContextMenu={(event: ReactMouseEvent, id: string) => {
					event.preventDefault();
					setMenu({
						kind: 'tab',
						id,
						x: event.clientX,
						y: event.clientY,
					});
				}}
				onMouseEnter={onTabMouseEnter}
				onMouseLeave={clearHover}
				onKeyDown={onTabKeyDown}
				onKeyUp={onTabKeyUp}
				enableTabMuting={enableTabMuting}
				touchUi={touchUi}
				closeButtonAllowed={closeButtonAllowed}
				verticalExpandedOnHover={expandedOnHover}
				hoveredTabId={splitHovered ? tab.id : null}
				{...(splitLayout === undefined
					? {}
					: {
							splitLayout,
						})}
				onAudioMuteToggle={(tabId: string, muted: boolean) => {
					model.updateTab(tabId, {
						siteMuted: muted,
						alertState: muted ? 'audio-muting' : 'audio-playing',
					});
					onTabAudioMuteToggle?.(tabId, muted);
				}}
			/>
		</div>
	);
	const renderTabSequence = (
		tabs: readonly Readonly<TabData>[],
		options: {
			pinned?: boolean;
			grouped?: boolean;
		} = {},
	): ReactNode[] => {
		const result: ReactNode[] = [];
		for (let index = 0; index < tabs.length; index += 1) {
			const tab = requireDefined(tabs[index]);
			const next = tabs[index + 1];
			if (tab.split !== null && next?.split === tab.split) {
				const groupedInset = collapsed ? 12 : 10;
				let fallbackWidth = defaultTabWidth;
				if (!options.pinned && options.grouped) {
					fallbackWidth = Math.max(0, availableWidth - groupedInset);
				}
				result.push(
					<VerticalSplitView
						key={`split-${tab.split}`}
						tabs={[tab, next]}
						collapsed={collapsed}
						pinned={options.pinned === true}
						fallbackWidth={fallbackWidth}
						dragging={tab.split !== null && draggedWholeSplitIds.has(tab.split)}
						renderTab={renderTab}
					/>,
				);
				index += 1;
			} else {
				result.push(renderTab(tab));
			}
		}
		return result;
	};
	type PinnedUnit =
		| {
				kind: 'tab';
				id: string;
				tab: Readonly<TabData>;
		  }
		| {
				kind: 'split';
				id: string;
				tabs: readonly [Readonly<TabData>, Readonly<TabData>];
		  };
	const pinnedUnits: PinnedUnit[] = [];
	for (let index = 0; index < pinned.length; index += 1) {
		const tab = requireDefined(pinned[index]);
		const next = pinned[index + 1];
		if (tab.split !== null && next?.split === tab.split) {
			pinnedUnits.push({
				kind: 'split',
				id: `split-${tab.split}`,
				tabs: [tab, next],
			});
			index += 1;
		} else {
			pinnedUnits.push({
				kind: 'tab',
				id: tab.id,
				tab,
			});
		}
	}
	// SplitTabView and PinnedTabContainerView only enter their strict collapsed
	// layout after the regular strip-width animation. Expand-on-hover retains
	// the collapsed controller state even while the overlay is wide.
	const pinnedLayoutCollapsed =
		collapsed &&
		(expandedOnHover || availableWidth <= LAYOUT_CONSTANTS.verticalTabStripCollapsedWidth);
	const pinnedLayoutWidth = Math.max(
		0,
		availableWidth - LAYOUT_CONSTANTS.verticalTabStripHorizontalPadding,
	);
	const pinnedLayout = calculatePinnedTabContainerLayout({
		sizeBounds: {
			width: pinnedLayoutWidth,
			height: null,
		},
		collapseState: pinnedLayoutCollapsed
			? VerticalTabStripCollapseState.Collapsed
			: VerticalTabStripCollapseState.Expanded,
		expandOnHoverEnabled,
		children: pinnedUnits.map((unit) => ({
			id: unit.id,
			split: unit.kind === 'split',
			preferredSize: {
				width: LAYOUT_CONSTANTS.verticalTabMinWidth * (unit.kind === 'split' ? 2 : 1),
				height:
					unit.kind === 'split' && pinnedLayoutCollapsed
						? LAYOUT_CONSTANTS.verticalTabPinnedHeight * 2
						: LAYOUT_CONSTANTS.verticalTabPinnedHeight,
			},
		})),
	});
	const unpinnedDirectChildren: Array<{
		id: string;
		group?: boolean;
		preferredSize: {
			width: number;
			height: number;
		};
		minimumSize: {
			width: number;
			height: number;
		};
	}> = [];
	const countedGroups = new Set<string>();
	const countedSplits = new Set<string>();
	const countDirectUnits = (tabs: readonly Readonly<TabData>[]) => {
		let count = 0;
		for (let index = 0; index < tabs.length; index += 1) {
			const tab = requireDefined(tabs[index]);
			const next = tabs[index + 1];
			count += 1;
			if (tab.split !== null && next?.split === tab.split) index += 1;
		}
		return count;
	};
	for (const tab of unpinned) {
		if (tab.group !== null) {
			if (countedGroups.has(tab.group)) continue;
			countedGroups.add(tab.group);
			const group = groups.get(tab.group);
			const groupTabs = unpinned.filter((candidate) => candidate.group === tab.group);
			const directChildCount = countDirectUnits(groupTabs);
			const fullyCollapsed = Boolean(
				group?.collapsed && focusedGroupId !== group.id && fullyCollapsedGroupIds.has(group.id),
			);
			unpinnedDirectChildren.push({
				id: `group-${tab.group}`,
				group: true,
				preferredSize: {
					width: availableWidth,
					height: fullyCollapsed ? 34 : 36 + directChildCount * 32,
				},
				minimumSize: {
					width: LAYOUT_CONSTANTS.verticalTabMinWidth,
					height: 34,
				},
			});
			continue;
		}
		if (tab.split !== null) {
			if (countedSplits.has(tab.split)) continue;
			countedSplits.add(tab.split);
		}
		unpinnedDirectChildren.push({
			id: tab.split === null ? tab.id : `split-${tab.split}`,
			preferredSize: {
				width: defaultTabWidth,
				height: LAYOUT_CONSTANTS.verticalTabHeight,
			},
			minimumSize: {
				width: LAYOUT_CONSTANTS.verticalTabMinWidth,
				height: LAYOUT_CONSTANTS.verticalTabHeight,
			},
		});
	}
	const collapseState = collapsed
		? VerticalTabStripCollapseState.Collapsed
		: VerticalTabStripCollapseState.Expanded;
	const unpinnedNaturalLayout = calculateUnpinnedTabContainerLayout({
		orientation: TabStripOrientation.Vertical,
		sizeBounds: {
			width: availableWidth,
			height: null,
		},
		children: unpinnedDirectChildren,
		collapseState,
	});
	const tabStripViewLayout = calculateTabStripViewLayout({
		orientation: TabStripOrientation.Vertical,
		sizeBounds: {
			width: availableWidth,
			height: regionsSize.height > 0 ? regionsSize.height : null,
		},
		pinnedPreferred: pinnedLayout.hostSize,
		pinnedMinimum: calculatePinnedTabContainerMinimumSize(pinnedUnits.length, collapseState),
		pinnedVisible: pinnedUnits.length > 0,
		unpinnedPreferred: unpinnedNaturalLayout.hostSize,
		unpinnedMinimum: calculateUnpinnedTabContainerMinimumSize(
			TabStripOrientation.Vertical,
			unpinnedDirectChildren,
		),
		unpinnedVisible: unpinnedDirectChildren.length > 0,
		separatorPreferredHeight: 1,
		collapsed,
	});
	return (
		<>
			{showTopControls ? (
				<VerticalTabStripTopContainer
					availableWidth={availableWidth}
					collapsed={topContainerCollapsed}
					toolbarHeight={toolbarHeight}
					captionButtonWidth={captionButtonWidth}
					isExitingExpandOnHover={isExitingExpandOnHover}
					showUnfocusButton={groupFocused}
					showProjectsButton={showProjectsButton}
					showTabSearchButton={showTabSearchButton}
					projectsTargetVisible={projectsTargetVisible}
					tabSearchTargetVisible={tabSearchTargetVisible}
					projectsAnimating={projectsAnimating}
					tabSearchAnimating={tabSearchAnimating}
					startButtonLabel={startButtonLabel}
					projectsOpen={projectsOpen}
					onProjectsContextMenu={onProjectsContextMenu}
					onTabSearchContextMenu={onTabSearchContextMenu}
					onCollapseContextMenu={onCollapseContextMenu}
					{...(onCollapsedChange === undefined
						? {}
						: {
								onCollapsedChange,
							})}
					{...(onUnfocusGroup === undefined
						? {}
						: {
								onUnfocusGroup,
							})}
					{...(onProjectsClick === undefined
						? {}
						: {
								onProjectsClick,
							})}
					{...(onTabSearchClick === undefined
						? {}
						: {
								onTabSearchClick,
							})}
				/>
			) : null}

			<div ref={regionsRef} className='ct-vertical-regions'>
				<div
					className='ct-vertical-pinned-scroll-shell'
					aria-hidden={!tabStripViewLayout.pinnedVisible || undefined}
					style={{
						left: tabStripViewLayout.pinnedBounds.x,
						top: tabStripViewLayout.pinnedBounds.y,
						width: tabStripViewLayout.pinnedBounds.width,
						height: tabStripViewLayout.pinnedBounds.height,
					}}
				>
					<div ref={pinnedScrollRef} className='ct-vertical-pinned-scroll'>
						<div
							className='ct-vertical-pinned'
							style={{
								height: pinnedLayout.hostSize.height,
							}}
						>
							{pinnedLayout.children.map((child, index) => {
								const unit = requireDefined(pinnedUnits[index]);
								return (
									<div
										key={unit.id}
										className='ct-vertical-pinned__unit'
										style={{
											left: child.bounds.x,
											top: child.bounds.y,
											width: child.bounds.width,
											height: child.bounds.height,
										}}
									>
										{unit.kind === 'split' ? (
											<VerticalSplitView
												tabs={unit.tabs}
												collapsed={pinnedLayoutCollapsed}
												pinned={true}
												fallbackWidth={child.bounds.width}
												dragging={
													unit.tabs[0].split !== null &&
													draggedWholeSplitIds.has(unit.tabs[0].split)
												}
												renderTab={renderTab}
											/>
										) : (
											renderTab(unit.tab, child.bounds.width)
										)}
									</div>
								);
							})}
						</div>
					</div>

					<RoundedScrollbar
						viewportRef={pinnedScrollRef}
						isAnimatingSize={isAnimatingSize}
						shouldHaveRightMargin={!collapsed}
					/>
				</div>
				{tabStripViewLayout.separatorVisible ? (
					<div
						className='ct-vertical-separator'
						style={{
							left: tabStripViewLayout.separatorBounds.x,
							top: tabStripViewLayout.separatorBounds.y,
							width: tabStripViewLayout.separatorBounds.width,
							height: tabStripViewLayout.separatorBounds.height,
						}}
					/>
				) : null}

				<div
					className='ct-vertical-scroll-shell'
					style={{
						left: tabStripViewLayout.unpinnedBounds.x,
						top: tabStripViewLayout.unpinnedBounds.y,
						width: tabStripViewLayout.unpinnedBounds.width,
						height: tabStripViewLayout.unpinnedBounds.height,
					}}
				>
					<div ref={scrollRef} className='ct-vertical-scroll'>
						<div className='ct-vertical-scroll__content'>
							{unpinned.map((tab) => {
								if (tab.group === null) {
									if (tab.split !== null) {
										if (renderedSplits.has(tab.split)) return null;
										const pair = unpinned.filter(
											(candidate) => candidate.group === null && candidate.split === tab.split,
										);
										if (pair.length === 2) {
											renderedSplits.add(tab.split);
											return renderTabSequence(pair);
										}
									}
									return renderTab(tab);
								}
								if (renderedGroups.has(tab.group)) return null;
								renderedGroups.add(tab.group);
								const group = groups.get(tab.group);
								if (!group) return renderTab(tab);
								const groupTabs = unpinned.filter((candidate) => candidate.group === group.id);
								const fullyCollapsed =
									group.collapsed &&
									focusedGroupId !== group.id &&
									fullyCollapsedGroupIds.has(group.id);
								const collapsing =
									group.collapsed && focusedGroupId !== group.id && !fullyCollapsed;
								return (
									<section
										key={group.id}
										className='ct-vertical-group'
										data-group-id={group.id}
										data-group-collapsed={fullyCollapsed || undefined}
										data-group-collapsing={collapsing || undefined}
										data-drag-source={draggedWholeGroupIds.has(group.id) || undefined}
										style={
											{
												'--ct-group-color': GROUP_COLORS[group.color],
											} satisfies CSSProperties & Record<'--ct-group-color', string>
										}
									>
										<GroupHeader
											group={group}
											tabs={groupTabs}
											activeTabId={snapshot.activeTabId}
											orientation={TabStripOrientation.Vertical}
											collapsedStrip={collapsed}
											onToggle={(source: 'mouse' | 'touch' | 'keyboard') =>
												toggleGroupFromHeader(group.id, source)
											}
											onPointerDown={(event: ReactPointerEvent<HTMLDivElement>) =>
												onGroupPointerDown(event, group)
											}
											onKeyDown={(event: ReactKeyboardEvent<HTMLDivElement>) =>
												onGroupKeyDown(event, group.id)
											}
											onContextMenu={(event: ReactMouseEvent<HTMLDivElement>) => {
												event.preventDefault();
												onOpenGroupEditor(group.id, event.clientX, event.clientY);
											}}
											onOpenEditor={(x: number, y: number) => onOpenGroupEditor(group.id, x, y)}
											onMouseEnter={(event: ReactMouseEvent<HTMLDivElement>) =>
												onGroupMouseEnter(event, group.id)
											}
											onMouseLeave={clearHover}
											showHoverCard={showGroupHoverCards}
										/>

										<div
											className='ct-vertical-group__tabs'
											aria-hidden={fullyCollapsed || undefined}
										>
											<div className='ct-vertical-group__tabs-inner'>
												{renderTabSequence(groupTabs, {
													grouped: true,
												})}
											</div>
										</div>
									</section>
								);
							})}
						</div>
					</div>

					<RoundedScrollbar
						viewportRef={scrollRef}
						isAnimatingSize={isAnimatingSize}
						shouldHaveRightMargin={!collapsed}
					/>
				</div>
			</div>
			{showNewTabButton ? (
				<VerticalTabStripBottomContainer
					collapseState={
						bottomContainerCollapsed
							? VerticalTabStripCollapseState.Collapsed
							: VerticalTabStripCollapseState.Expanded
					}
					onNewTab={addTab}
					onContextMenu={(event: ReactMouseEvent<HTMLButtonElement>) => {
						event.preventDefault();
						setMenu({
							kind: 'new-tab',
							x: event.clientX,
							y: event.clientY,
						});
					}}
					{...(onNewTabFromClipboard === undefined
						? {}
						: {
								onNewTabFromClipboard,
							})}
				/>
			) : null}
		</>
	);
}
