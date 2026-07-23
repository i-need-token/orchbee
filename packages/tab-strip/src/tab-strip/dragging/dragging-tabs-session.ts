import { requireDefined } from '../utils/requireDefined';
import type { Rect } from '../utils/rectRight';
import type { TabGroupId } from '../model/tab-types';
import type { DragSessionData } from './drag-session-data';
import { DragSlotViewType } from './drag-session-data';
export interface DragPoint {
	x: number;
	y: number;
}
export function areDraggedTabsConsecutive(data: DragSessionData): boolean {
	for (let index = 1; index < data.tabDragData.length; index += 1) {
		const previous = requireDefined(data.tabDragData[index - 1]).sourceModelIndex;
		const current = requireDefined(data.tabDragData[index]).sourceModelIndex;
		if (previous !== null && current !== null && previous + 1 !== current) return false;
	}
	return true;
}

/** Port of DraggingTabsSession's CalculateMouseOffset helper. */
export function calculateDragMouseOffset(
	data: DragSessionData,
	getSizeNeededForViews: (ids: readonly string[]) => number,
	sourceViewWidth: number,
): number {
	const throughSource = data.attachedViewIds.slice(0, data.sourceViewIndex + 1);
	return (
		getSizeNeededForViews(throughSource) -
		sourceViewWidth +
		Math.round(data.mouseOffsetToSizeRatios.x * sourceViewWidth)
	);
}

/** Port of GetAttachedDragPoint, with mirroring supplied by the caller. */
export function calculateAttachedDragPoint(input: {
	contextPoint: Readonly<DragPoint>;
	mirroredX?: number;
	mouseOffset: number;
	tabDragAreaWidth: number;
	draggedViewsSize: number;
}): DragPoint {
	const x = (input.mirroredX ?? input.contextPoint.x) - input.mouseOffset;
	const maximum = Math.max(0, input.tabDragAreaWidth - input.draggedViewsSize);
	return {
		x: Math.min(maximum, Math.max(0, x)),
		y: 0,
	};
}

/** Port of GetDraggedViewTabStripBounds. */
export function calculateDraggedViewBounds(input: {
	point: Readonly<DragPoint>;
	draggedBounds: readonly Readonly<Rect>[];
	sourceViewAttached: boolean;
	sourceViewHeight: number;
	standardTabWidth: number;
	tabHeight: number;
}): Rect {
	if (input.sourceViewAttached && input.draggedBounds.length > 0) {
		const first = requireDefined(input.draggedBounds[0]);
		const last = requireDefined(input.draggedBounds.at(-1));
		return {
			x: input.point.x,
			y: input.point.y,
			width: last.x + last.width - first.x,
			height: input.sourceViewHeight,
		};
	}
	return {
		x: input.point.x,
		y: input.point.y,
		width: input.standardTabWidth,
		height: input.tabHeight,
	};
}
export function calculateHorizontalMoveThreshold(
	targetTabWidth: number,
	standardTabWidth: number,
): number {
	return Math.round((targetTabWidth / standardTabWidth) * 16);
}
export function shouldReorderDraggedTabs(input: {
	justAttached: boolean;
	movementSinceLastReorder: number;
	threshold: number;
	initialMove: boolean;
	tabsConsecutive: boolean;
}): boolean {
	return (
		input.justAttached ||
		Math.abs(input.movementSinceLastReorder) > input.threshold ||
		(input.initialMove && !input.tabsConsecutive)
	);
}
export interface DragGroupCalculationInput {
	containsGroupHeader: boolean;
	hasSelectedUnpinnedTab: boolean;
	leftGroup: TabGroupId | null;
	rightGroup: TabGroupId | null;
	currentGroup: TabGroupId | null;
	leftGroupCollapsed: boolean;
	rightGroupCollapsed: boolean;
	selectedTabWidth: number;
	tabLeftInset: number;
	leftEdge: number;
	selectedLeft: number;
	selectedRight: number;
	tabDragAreaEnd: number;
}

/** Pure port of DraggingTabsSession::CalculateGroupForDraggedTabs. */
export function calculateGroupForDraggedTabs(input: DragGroupCalculationInput): TabGroupId | null {
	if (input.containsGroupHeader || !input.hasSelectedUnpinnedTab) return null;
	if (input.leftGroup === input.rightGroup) return input.leftGroup;
	const buffer = Math.trunc(input.selectedTabWidth / 4);
	let leftEdge = input.leftEdge;
	if (input.leftGroup !== null && input.leftGroup === input.currentGroup) leftEdge += buffer;
	if (
		input.rightGroup !== null &&
		input.rightGroup === input.currentGroup &&
		leftEdge > input.tabLeftInset
	) {
		leftEdge -= buffer;
	}
	if (input.leftGroup !== null && !input.leftGroupCollapsed) {
		if (input.selectedRight >= input.tabDragAreaEnd) return null;
		if (input.selectedLeft <= leftEdge - buffer) return input.leftGroup;
	}
	if (
		input.selectedLeft >= leftEdge + buffer &&
		input.rightGroup !== null &&
		!input.rightGroupCollapsed
	) {
		return input.rightGroup;
	}
	return null;
}
export function dragContainsGroupHeader(data: DragSessionData): boolean {
	return data.tabDragData.some((datum) => datum.viewType === DragSlotViewType.GroupHeader);
}
