import type {
	TabData,
	TabDragRestoreState,
	TabGroupColor,
	TabGroupData,
	TabModel,
	TabModelSnapshot,
} from '../model/tab-model';
import { DragSessionData, DragSlotViewType, type TabDragData } from '../dragging/drag-session-data';
import { type HorizontalGroupDragState } from '../dragging/horizontal-group-drag-machine';
import { VERTICAL_TAB_GROUP_COLLAPSED_HEIGHT } from '../layout/tab-group-view-layout';
import { type TabSizeInfo } from '../layout/tab-strip-layout-types';
import { requireDefined } from './requireDefined';
import { TabStripOrientation } from '../types/tab-strip-types';
import { DEFAULT_TAB_STYLE_METRICS } from '../layout/tab-style-metrics';
import { LAYOUT_CONSTANTS } from '../layout/layout-constants';
import { calculateHorizontalTabGroupHeaderLayout } from '../layout/tab-group-style';
import {
	buildDraggedTabsLayout,
	getDragStartAnimationOffset,
} from '../dragging/dragged-tabs-container-layout';

export const TAB_SIZE: Readonly<TabSizeInfo> = {
	pinnedTabWidth: 64,
	minActiveWidth: 56,
	minInactiveWidth: 32,
	standardWidth: 200,
};

export const TAB_BOUNDS_ANIMATION_DURATION_MS = 200;

export function joinClassNames(...classNames: ReadonlyArray<string | undefined>): string {
	return classNames.filter((className) => className !== undefined).join(' ');
}

export function requestManagedAnimationFrame(callback: FrameRequestCallback): number {
	if (typeof window.requestAnimationFrame === 'function') {
		return window.requestAnimationFrame(callback);
	}
	return window.setTimeout(() => callback(performance.now()), 16);
}

export function cancelManagedAnimationFrame(handle: number): void {
	if (typeof window.cancelAnimationFrame === 'function') window.cancelAnimationFrame(handle);
	else window.clearTimeout(handle);
}

export const GROUP_COLORS: Record<TabGroupColor, string> = {
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

export function getMaxContrastForeground(background: string): '#202124' | '#fff' {
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

/** The TUI's palette order. */
export const GROUP_COLOR_NAMES: readonly TabGroupColor[] = [
	'blue',
	'cyan',
	'green',
	'grey',
	'orange',
	'pink',
	'purple',
	'red',
	'yellow',
];

/** The color the TUI gives a group that New group… creates. */
export const NEW_TAB_GROUP_COLOR: TabGroupColor = 'blue';

export type VisualItem =
	| { type: 'tab'; id: string; tab: Readonly<TabData> }
	| { type: 'group'; id: string; group: Readonly<TabGroupData> };

export interface DragState {
	sourceId: string;
	tabIds: string[];
	pointerId: number;
	pointerType: string;
	touchLongPressRecognized: boolean;
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
	pointerOffsetRatios: Readonly<{ x: number; y: number }>;
	pointerOffsetFromVisualOrigin: Readonly<{ x: number; y: number }>;
	sourceViewOffsetFromSlot: Readonly<{ x: number; y: number }>;
	sourceSlotSize: Readonly<{ width: number; height: number }>;
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
	dragAreaBounds: Readonly<{ left: number; top: number; width: number; height: number }>;
	dragSessionData: DragSessionData<Readonly<TabGroupData>>;
	dragStartAnimationRequired: boolean;
	dragStartAnimationStartedAt: number | null;
	dragStartAnimationProgress: number;
	settling: boolean;
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

interface HorizontalDragPresentation {
	readonly compactGroupHeaderDrag: boolean;
	readonly left: number;
	readonly sourceVisualId: string;
	readonly views: readonly PackedHorizontalDragView[];
	readonly width: number;
}

export function packHorizontalDragViews(input: {
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
		const packed = { ...view, x: cursor, y: 0, width };
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

export function calculateHorizontalDragPresentation(input: {
	drag: DragState;
	snapshot: Pick<TabModelSnapshot, 'groups' | 'tabs'>;
	settleTargetLeft?: number | null;
}): HorizontalDragPresentation {
	const { drag } = input;
	const sourceVisualId = getSourceVisualId(drag);
	const compactGroupHeaderDrag = drag.groupHeaderId !== null && drag.groupWasCollapsedBeforeDrag;
	const sourceViews = compactGroupHeaderDrag
		? drag.visualViews.filter((view) => view.kind === 'group')
		: drag.visualViews;
	const group =
		drag.groupHeaderId === null
			? null
			: (input.snapshot.groups.find((candidate) => candidate.id === drag.groupHeaderId) ?? null);
	const draggedTabCount = drag.tabIds.filter((id) =>
		input.snapshot.tabs.some((tab) => tab.id === id),
	).length;
	const packed = packHorizontalDragViews({
		views: sourceViews,
		sourceVisualId,
		pointerRatioX: drag.pointerOffsetRatios.x,
		...(compactGroupHeaderDrag && group
			? { sourceWidthOverride: groupHeaderWidth(group, draggedTabCount) }
			: {}),
	});
	const mouseOffset = packed.mouseOffset || drag.pointerOffsetFromVisualOrigin.x;
	const unclampedLeft =
		drag.settling && input.settleTargetLeft !== null && input.settleTargetLeft !== undefined
			? input.settleTargetLeft
			: drag.x - mouseOffset;
	const left = Math.min(
		Math.max(
			drag.dragAreaBounds.left,
			drag.dragAreaBounds.left + drag.dragAreaBounds.width - packed.width,
		),
		Math.max(drag.dragAreaBounds.left, unclampedLeft),
	);
	return Object.freeze({
		compactGroupHeaderDrag,
		left,
		sourceVisualId,
		views: packed.views,
		width: packed.width,
	});
}

export function calculateHorizontalTabStripTrailingEdge(input: {
	idealTrailingEdge: number;
	paintedTrailingEdge: number;
	dragTrailingEdge: number | null;
}): number {
	return input.dragTrailingEdge === null
		? input.paintedTrailingEdge
		: Math.max(input.idealTrailingEdge, input.dragTrailingEdge);
}

export function getSourceVisualId(
	drag: Pick<DragState, 'sourceId' | 'groupHeaderId' | 'visualViews'>,
): string {
	if (drag.groupHeaderId !== null) return `group:${drag.groupHeaderId}`;
	return drag.visualViews.find((view) => view.tabIds?.includes(drag.sourceId))?.id ?? drag.sourceId;
}

export function getVerticalDragVisualViews(
	drag: Pick<DragState, 'groupHeaderId' | 'groupWasCollapsedBeforeDrag' | 'visualViews'>,
): readonly DragVisualView[] {
	if (drag.groupHeaderId === null || !drag.groupWasCollapsedBeforeDrag) return drag.visualViews;
	const draggedGroupId = `group:${drag.groupHeaderId}`;
	return drag.visualViews.map((view) =>
		view.id === draggedGroupId
			? Object.freeze({ ...view, height: VERTICAL_TAB_GROUP_COLLAPSED_HEIGHT })
			: view,
	);
}

export function dragRequiresStartCompaction(input: {
	drag: DragState;
	snapshot: Readonly<{
		groups: readonly Readonly<TabGroupData>[];
		tabs: readonly Readonly<TabData>[];
	}>;
	horizontal: boolean;
	verticalCollapsed: boolean;
}): boolean {
	const sourceVisualId = getSourceVisualId(input.drag);
	if (input.horizontal) {
		const compactGroupHeaderDrag =
			input.drag.groupHeaderId !== null && input.drag.groupWasCollapsedBeforeDrag;
		const views = compactGroupHeaderDrag
			? input.drag.visualViews.filter((view) => view.kind === 'group')
			: input.drag.visualViews;
		const group =
			input.drag.groupHeaderId === null
				? null
				: (input.snapshot.groups.find((candidate) => candidate.id === input.drag.groupHeaderId) ??
					null);
		const sourceWidthOverride =
			compactGroupHeaderDrag && group
				? groupHeaderWidth(
						group,
						input.drag.tabIds.filter((id) => input.snapshot.tabs.some((tab) => tab.id === id))
							.length,
					)
				: undefined;
		return packHorizontalDragViews({
			views,
			sourceVisualId,
			pointerRatioX: input.drag.pointerOffsetRatios.x,
			...(sourceWidthOverride === undefined ? {} : { sourceWidthOverride }),
		}).views.some((view) => view.initialOffsetX !== 0);
	}

	const sourceTab = input.snapshot.tabs.find((tab) => tab.id === input.drag.sourceId);
	const verticalViews = getVerticalDragVisualViews(input.drag);
	const layout = buildDraggedTabsLayout({
		layout: input.verticalCollapsed || sourceTab?.pinned === true ? 'squash' : 'vertical',
		pointerOffsetRatios: input.drag.pointerOffsetRatios,
		sourceSlotSize: input.drag.sourceSlotSize,
		sourceViewOffsetFromSlot: input.drag.sourceViewOffsetFromSlot,
		views: verticalViews.map((view) => ({
			id: view.id,
			bounds: { x: view.x, y: view.y, width: view.width, height: view.height },
			source: view.id === sourceVisualId,
		})),
	});
	const visualData = new Map(layout.views.map((view) => [view.id, view]));
	const sourceView = verticalViews.find((view) => view.id === sourceVisualId) ?? verticalViews[0];
	const sourceTarget = sourceView ? visualData.get(sourceView.id) : undefined;
	if (!sourceView || !sourceTarget) return false;
	return verticalViews.some((view) => {
		const target = visualData.get(view.id);
		if (!target) return false;
		const offset = getDragStartAnimationOffset({
			view,
			sourceView,
			targetOffset: target.offset,
			sourceTargetOffset: sourceTarget.offset,
		});
		return offset.x !== 0 || offset.y !== 0;
	});
}

export function captureDragVisuals(input: {
	sourceElement: HTMLElement;
	sourceId: string;
	tabIds: readonly string[];
	sourceGroupId: string | null;
	groupIds: readonly string[];
	underlineGroupIds: readonly string[];
	orientation: TabStripOrientation;
	pointer: Readonly<{ x: number; y: number }>;
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
			? input.sourceElement.closest<HTMLElement>('[data-tab-strip-scroll-viewport]')
			: null;
	const horizontalDragViewport =
		input.orientation === TabStripOrientation.Horizontal
			? input.sourceElement.closest<HTMLElement>('[data-horizontal-track]')
			: null;
	const dragAreaRect =
		verticalDragViewport?.getBoundingClientRect() ??
		horizontalDragViewport?.getBoundingClientRect() ??
		rootRect;
	const sourceRect = input.sourceElement.getBoundingClientRect();
	let fallbackSourceWidth = 216;
	if (input.orientation !== TabStripOrientation.Vertical) {
		fallbackSourceWidth = input.sourceGroupId === null ? TAB_SIZE.standardWidth : 52;
	}
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
	const rawViews: Array<DragVisualView & { left: number; top: number }> = [];
	const selectedTabs = new Set(input.tabIds);
	const selectedGroups = new Set(input.groupIds);
	const tabsRepresentedByGroups = new Set<string>();
	if (input.orientation === TabStripOrientation.Vertical) {
		for (const element of root?.querySelectorAll<HTMLElement>(
			'[data-tab-strip-group][data-group-id]',
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
			? '[data-tab-strip-group-header][data-group-id], [data-tab-id]'
			: '[data-tab-id]';
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
		const rect = element.getBoundingClientRect();
		rawViews.push({
			id,
			kind: 'tab',
			tabIds: Object.freeze([id]),
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
		// bounds are the equivalent ordering source after group deduping.
		rawViews.sort((a, b) => a.top - b.top || a.left - b.left);
	}

	if (rawViews.length === 0) {
		rawViews.push({
			id:
				input.sourceGroupId === null
					? (input.tabIds[0] ?? 'drag-source')
					: `group:${input.sourceGroupId}`,
			kind: input.sourceGroupId === null ? 'tab' : 'group',
			...(input.sourceGroupId === null ? { tabIds: Object.freeze([...input.tabIds]) } : {}),
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
		sourceSlotSize: Object.freeze({ width: sourceWidth, height: sourceHeight }),
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

export function buildDragSessionData(input: {
	snapshot: TabModelSnapshot;
	tabIds: readonly string[];
	sourceId: string;
	groupHeaderId: string | null;
	groupWasCollapsedBeforeDrag: boolean;
	pointerOffsetRatios: Readonly<{ x: number; y: number }>;
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
		...(group ? { draggingGroups: new Set([group.id]) } : {}),
	});
}

export function areDraggedTabIdsConsecutive(
	tabs: readonly Readonly<{ id: string }>[],
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

export type MenuState =
	| { kind: 'tab'; id: string; x: number; y: number }
	| { kind: 'group'; id: string; x: number; y: number }
	| { kind: 'new-tab'; x: number; y: number };

export type HoverState =
	| { kind: 'tab'; id: string; x: number; y: number }
	| { kind: 'group'; id: string; x: number; y: number };

function nextAvailableEntityId(prefix: string, existingIds: ReadonlySet<string>): string {
	let sequence = 1;
	while (existingIds.has(`${prefix}-${sequence}`)) sequence += 1;
	return `${prefix}-${sequence}`;
}

export function createGroupForTabs(
	model: TabModel,
	tabIds: readonly string[],
	title: string,
): string | null {
	const snapshot = model.getSnapshot();
	const eligible = snapshot.tabs.filter((tab) => tabIds.includes(tab.id)).map((tab) => tab.id);
	if (eligible.length === 0) return null;
	const groupId = nextAvailableEntityId('group', new Set(snapshot.groups.map((group) => group.id)));
	model.createGroup({ id: groupId, title, color: NEW_TAB_GROUP_COLOR, collapsed: false }, eligible);
	return groupId;
}

export function buildVisualItems(snapshot: TabModelSnapshot): VisualItem[] {
	const groups = new Map(snapshot.groups.map((group) => [group.id, group]));
	const seen = new Set<string>();
	const items: VisualItem[] = [];
	for (const tab of snapshot.tabs) {
		if (tab.group !== null && !seen.has(tab.group)) {
			const group = groups.get(tab.group);
			if (group) {
				items.push({ type: 'group', id: `group-${group.id}`, group });
				seen.add(tab.group);
			}
		}
		items.push({ type: 'tab', id: tab.id, tab });
	}
	return items;
}

let groupTitleMeasurementContext: OffscreenCanvasRenderingContext2D | null | undefined;

export function measuredGroupTitleWidth(title: string): number {
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

export function groupHeaderWidth(group: Readonly<TabGroupData>, tabCount = 1): number {
	return Math.round(
		calculateHorizontalTabGroupHeaderLayout({
			title: group.title,
			measuredTitleWidth: measuredGroupTitleWidth(group.title),
			collapsed: group.collapsed,
			tabCount,
			...(group.attention === undefined ? {} : { attention: group.attention }),
		}).desiredWidth,
	);
}

export function sizeInfoForItem(item: VisualItem): TabSizeInfo {
	if (item.type === 'tab') return TAB_SIZE;
	const width = groupHeaderWidth(item.group);
	return {
		pinnedTabWidth: width,
		minActiveWidth: width,
		minInactiveWidth: width,
		standardWidth: width,
	};
}

export function tabIsApparentlyActive(tab: Readonly<TabData>, snapshot: TabModelSnapshot): boolean {
	return tab.id === snapshot.activeTabId;
}

export function groupForDragInsertion(
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
