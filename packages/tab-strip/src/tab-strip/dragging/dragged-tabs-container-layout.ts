import { requireDefined } from '../utils/requireDefined';
import type { Rect } from '../utils/rectRight';
export type DraggedTabsAxes = 'vertical-only' | 'both';
export type DraggedTabsLayoutMode = 'vertical' | 'squash';
export interface DraggedLayoutView {
	id: string;
	bounds: Rect;
	source?: boolean;
}
export interface DraggedViewVisualData {
	id: string;
	offset: Readonly<{
		x: number;
		y: number;
	}>;
	shouldHide: boolean;
	shouldFloat: boolean;
}
export interface DraggedTabsLayoutResult {
	boundsRelativeToPointer: Rect;
	views: readonly DraggedViewVisualData[];
}
export const DRAGGED_VIEW_VERTICAL_PADDING = 2;
export const DRAGGED_VIEW_HORIZONTAL_PADDING = 4;
export const DRAG_START_ANIMATION_DURATION_MS = 200;

export function getDragStartAnimationProgress(
	startedAt: number,
	now: number,
	durationMs = DRAG_START_ANIMATION_DURATION_MS,
): number {
	if (durationMs <= 0) return 1;
	return Math.min(1, Math.max(0, (now - startedAt) / durationMs));
}

export function getDragStartAnimationOffset(input: {
	view: Readonly<{ x: number; y: number }>;
	sourceView: Readonly<{ x: number; y: number }>;
	targetOffset: Readonly<{ x: number; y: number }>;
	sourceTargetOffset: Readonly<{ x: number; y: number }>;
}): {
	x: number;
	y: number;
} {
	return {
		x: input.view.x - input.sourceView.x - (input.targetOffset.x - input.sourceTargetOffset.x),
		y: input.view.y - input.sourceView.y - (input.targetOffset.y - input.sourceTargetOffset.y),
	};
}

export function clampDraggedViewPosition(
	position: Readonly<{ x: number; y: number }>,
	size: Readonly<{ width: number; height: number }>,
	bounds: Readonly<Rect>,
): {
	x: number;
	y: number;
} {
	return {
		x: Math.min(
			Math.max(bounds.x, bounds.x + bounds.width - size.width),
			Math.max(bounds.x, position.x),
		),
		y: Math.min(
			Math.max(bounds.y, bounds.y + bounds.height - size.height),
			Math.max(bounds.y, position.y),
		),
	};
}
export function getSourceViewOffsetFromPointer(input: {
	sourceSlotSize: Readonly<{
		width: number;
		height: number;
	}>;
	sourceViewOffsetFromSlot?: Readonly<{
		x: number;
		y: number;
	}>;
	pointerOffsetRatios: Readonly<{
		x: number;
		y: number;
	}>;
}): {
	x: number;
	y: number;
} {
	const viewOffset = input.sourceViewOffsetFromSlot ?? {
		x: 0,
		y: 0,
	};
	return {
		x: -viewOffset.x - Math.trunc(input.pointerOffsetRatios.x * input.sourceSlotSize.width),
		y: -viewOffset.y - Math.trunc(input.pointerOffsetRatios.y * input.sourceSlotSize.height),
	};
}

/** Port of DraggedTabsContainer::BuildDragLayout and its two layout modes. */
export function buildDraggedTabsLayout(input: {
	views: readonly Readonly<DraggedLayoutView>[];
	layout: DraggedTabsLayoutMode;
	pointerOffsetRatios: Readonly<{
		x: number;
		y: number;
	}>;
	sourceSlotSize?: Readonly<{
		width: number;
		height: number;
	}>;
	sourceViewOffsetFromSlot?: Readonly<{
		x: number;
		y: number;
	}>;
}): DraggedTabsLayoutResult {
	const uniqueViews = input.views.filter(
		(view, index, views) => views.findIndex((candidate) => candidate.id === view.id) === index,
	);
	const source = uniqueViews.find((view) => view.source) ?? uniqueViews[0];
	if (!source)
		return {
			boundsRelativeToPointer: {
				x: 0,
				y: 0,
				width: 0,
				height: 0,
			},
			views: [],
		};
	const sourceOffset = getSourceViewOffsetFromPointer({
		sourceSlotSize: input.sourceSlotSize ?? source.bounds,
		pointerOffsetRatios: input.pointerOffsetRatios,
		...(input.sourceViewOffsetFromSlot === undefined
			? {}
			: {
					sourceViewOffsetFromSlot: input.sourceViewOffsetFromSlot,
				}),
	});
	const bounds: Rect = {
		x: sourceOffset.x,
		y: sourceOffset.y,
		width: 0,
		height: 0,
	};
	const visuals: DraggedViewVisualData[] = [];
	if (input.layout === 'squash') {
		bounds.width = source.bounds.width + DRAGGED_VIEW_HORIZONTAL_PADDING;
		bounds.height = source.bounds.height;
		uniqueViews.forEach((view) => {
			visuals.push({
				id: view.id,
				offset: {
					x: 0,
					y: 0,
				},
				shouldHide: view.id !== source.id,
				shouldFloat: false,
			});
		});
	} else {
		let contentWidth = 0;
		uniqueViews.forEach((view) => {
			const offset = {
				x: view.bounds.x,
				y: bounds.height,
			};
			visuals.push({
				id: view.id,
				offset,
				shouldHide: false,
				shouldFloat: false,
			});
			bounds.height += view.bounds.height + DRAGGED_VIEW_VERTICAL_PADDING;
			contentWidth = Math.max(contentWidth, view.bounds.x + view.bounds.width);
			if (view.id === source.id) {
				bounds.x -= offset.x;
				bounds.y -= offset.y;
			}
		});
		// Chromium only needs vertical extent for vertical-only transforms. The
		// width is retained here for DOM clamping and visual previews.
		bounds.width = contentWidth;
	}
	return {
		boundsRelativeToPointer: Object.freeze({
			...bounds,
		}),
		views: Object.freeze(
			visuals.map((visual) =>
				Object.freeze({
					...visual,
					offset: Object.freeze({
						...visual.offset,
					}),
				}),
			),
		),
	};
}
export function getDraggingViewsBoundsAtPoint(
	boundsRelativeToPointer: Readonly<Rect>,
	point: Readonly<{
		x: number;
		y: number;
	}>,
): Rect {
	return {
		...boundsRelativeToPointer,
		x: boundsRelativeToPointer.x + point.x,
		y: boundsRelativeToPointer.y + point.y,
	};
}
export function hasMinimumOverlap(
	a: Readonly<Rect>,
	b: Readonly<Rect>,
	minimumX?: number,
	minimumY?: number,
): boolean {
	if (minimumY !== undefined) {
		const overlap = Math.max(0, Math.min(a.y + a.height, b.y + b.height) - Math.max(a.y, b.y));
		if (overlap < minimumY) return false;
	}
	if (minimumX !== undefined) {
		const overlap = Math.max(0, Math.min(a.x + a.width, b.x + b.width) - Math.max(a.x, b.x));
		if (overlap < minimumX) return false;
	}
	return true;
}

/**
 * Ports UnpinnedTabContainerView's asymmetric group-target hysteresis.
 * Entering requires 40% of the group header height to overlap the group;
 * after entry, only 10% of the header height may peek outside before exit.
 */
export function shouldEnterVerticalTabGroup(
	draggedBounds: Readonly<Rect>,
	groupBounds: Readonly<Rect>,
	groupHeaderHeight: number,
): boolean {
	return hasMinimumOverlap(draggedBounds, groupBounds, undefined, groupHeaderHeight * 0.4);
}
export function shouldRemainInVerticalTabGroup(
	draggedBounds: Readonly<Rect>,
	groupBounds: Readonly<Rect>,
	groupHeaderHeight: number,
): boolean {
	return hasMinimumOverlap(
		draggedBounds,
		groupBounds,
		undefined,
		draggedBounds.height - groupHeaderHeight * 0.1,
	);
}
export interface DragTargetLayoutChild {
	id: string;
	visible: boolean;
	bounds: Rect;
	dragging?: boolean;
	hasCollectionNode?: boolean;
}

/** Port of GetTargetForTabDrag and GetTargetForTabDragInRow. */
export function getTargetBeforeDraggedTabs(input: {
	children: readonly Readonly<DragTargetLayoutChild>[];
	draggedBounds: Readonly<Rect>;
	axes: DraggedTabsAxes;
	rtl?: boolean;
	hostWidth?: number;
}): string | null {
	let afterDraggedViews = false;
	for (let index = 0; index < input.children.length; index += 1) {
		const child = requireDefined(input.children[index]);
		if (child.dragging) {
			afterDraggedViews = true;
			continue;
		}
		if (!child.visible || child.hasCollectionNode === false) continue;
		if (input.axes === 'both') {
			if (input.draggedBounds.y > child.bounds.y + child.bounds.height / 2) continue;
			return getTargetBeforeDraggedTabsInRow({
				...input,
				rowY: child.bounds.y,
				rowStartIndex: index,
				afterDraggedViews,
			});
		}
		const adjustedY = child.bounds.y - (afterDraggedViews ? input.draggedBounds.height : 0);
		if (input.draggedBounds.y < adjustedY + child.bounds.height / 2) return child.id;
	}
	return null;
}
function getTargetBeforeDraggedTabsInRow(input: {
	children: readonly Readonly<DragTargetLayoutChild>[];
	draggedBounds: Readonly<Rect>;
	axes: DraggedTabsAxes;
	rtl?: boolean;
	hostWidth?: number;
	rowY: number;
	rowStartIndex: number;
	afterDraggedViews: boolean;
}): string | null {
	const logicalX = input.rtl
		? (input.hostWidth ?? 0) - input.draggedBounds.x - input.draggedBounds.width
		: input.draggedBounds.x;
	let afterDraggedViews = input.afterDraggedViews;
	for (let index = input.rowStartIndex; index < input.children.length; index += 1) {
		const child = requireDefined(input.children[index]);
		if (child.dragging) {
			afterDraggedViews = true;
			continue;
		}
		if (!child.visible || child.hasCollectionNode === false) continue;
		if (child.bounds.y !== input.rowY) return child.id;
		const adjustedX = child.bounds.x - (afterDraggedViews ? input.draggedBounds.width : 0);
		if (logicalX < adjustedX + child.bounds.width / 2) return child.id;
	}
	return null;
}
export function getAnimatedDraggedViewPosition(input: {
	axes: DraggedTabsAxes;
	draggingBounds: Readonly<Rect>;
	targetOffset: Readonly<{
		x: number;
		y: number;
	}>;
	animationStartOffset?: Readonly<{
		x: number;
		y: number;
	}>;
	animationValue?: number;
}): {
	x: number;
	y: number;
} {
	const value = Math.min(1, Math.max(0, input.animationValue ?? 1));
	const start = input.animationStartOffset ?? {
		x: 0,
		y: 0,
	};
	return {
		x:
			(input.axes === 'both' ? input.draggingBounds.x + input.targetOffset.x : 0) +
			Math.round(start.x * (1 - value)),
		y: input.draggingBounds.y + input.targetOffset.y + Math.round(start.y * (1 - value)),
	};
}
