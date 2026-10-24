import type { Rect } from '../utils/rectRight';
import type { TabGroupId } from '../model/tab-types';
import { DEFAULT_TAB_STYLE_METRICS } from '../layout/tab-style-metrics';

interface DragPositionTab {
	group: TabGroupId | null;
}

interface HorizontalDragInsertionInput {
	draggedBounds: Readonly<Rect>;
	tabs: readonly Readonly<DragPositionTab>[];
	idealTabBounds: readonly Readonly<Rect>[];
	firstDraggedTabIndex: number;
	draggedTabCount: number;
	containsGroupHeader: boolean;
	collapsedGroups: ReadonlySet<TabGroupId>;
	groupHeaderWidths?: ReadonlyMap<TabGroupId, number>;
	containerStartIndex?: number;
	containerEndIndex?: number;
	tabOverlap?: number;
	/** Dragged views have already been reparented out of the supplied layout. */
	draggedViewsRemovedFromLayout?: boolean;
}

function isValidInsertionIndex(
	input: HorizontalDragInsertionInput,
	candidateIndex: number,
): boolean {
	const start = input.containerStartIndex ?? 0;
	if (candidateIndex === start) return true;
	if (
		candidateIndex > input.firstDraggedTabIndex &&
		candidateIndex <= input.firstDraggedTabIndex + input.draggedTabCount
	) {
		return false;
	}

	const left = input.tabs[candidateIndex - 1];
	const right = input.tabs[candidateIndex];
	if (!left) return false;
	if (right && left.group !== null && left.group === right.group) {
		if (input.containsGroupHeader || input.collapsedGroups.has(left.group)) return false;
	}
	return true;
}

function calculateIdealX(
	input: HorizontalDragInsertionInput,
	candidateIndex: number,
	tabOverlap: number,
): number {
	const start = input.containerStartIndex ?? 0;
	if (candidateIndex === start) return input.idealTabBounds[start]?.x ?? 0;
	const leftBounds = input.idealTabBounds[candidateIndex - 1];
	if (!leftBounds) return input.idealTabBounds.at(-1)?.x ?? 0;
	let idealX = leftBounds.x + leftBounds.width;
	if (!input.draggedViewsRemovedFromLayout && candidateIndex - 1 > input.firstDraggedTabIndex) {
		idealX -= input.draggedBounds.width - tabOverlap;
	}
	return idealX - tabOverlap;
}

function groupHeaderAdjustment(
	input: HorizontalDragInsertionInput,
	candidateIndex: number,
	tabOverlap: number,
): number {
	if (input.containsGroupHeader) return 0;
	const right = input.tabs[candidateIndex];
	if (!right || right.group === null) return 0;
	const leftGroup = input.tabs[candidateIndex - 1]?.group ?? null;
	if (leftGroup === right.group || input.collapsedGroups.has(right.group)) return 0;
	return Math.max(0, (input.groupHeaderWidths?.get(right.group) ?? 0) - tabOverlap);
}

/**
 * Port of TabStrip::TabDragContextImpl::CalculateInsertionIndex.
 *
 * `preRemovalIndex` is expressed in the current model so it can be passed to
 * the DOM model's drag adapters. `insertionIndex` matches Chromium's returned
 * index after the dragged tabs are conceptually removed.
 */
export function getHorizontalDragTarget(
	input: HorizontalDragInsertionInput,
): Readonly<{ preRemovalIndex: number; insertionIndex: number; idealX: number }> {
	if (input.tabs.length === 0) {
		return { preRemovalIndex: 0, insertionIndex: 0, idealX: 0 };
	}
	const start = Math.max(0, input.containerStartIndex ?? 0);
	const end = Math.min(input.tabs.length, input.containerEndIndex ?? input.tabs.length);
	const overlap = input.tabOverlap ?? DEFAULT_TAB_STYLE_METRICS.tabOverlap;
	let bestIndex = start;
	let bestIdealX = input.idealTabBounds[start]?.x ?? 0;
	let minimumDistance = Number.POSITIVE_INFINITY;

	for (let candidate = start; candidate <= end; candidate += 1) {
		if (!isValidInsertionIndex(input, candidate)) continue;
		const leftIdealX = calculateIdealX(input, candidate, overlap);
		const rightIdealX = leftIdealX + groupHeaderAdjustment(input, candidate, overlap);
		const distance = Math.min(
			Math.abs(input.draggedBounds.x - leftIdealX),
			Math.abs(input.draggedBounds.x - rightIdealX),
		);
		if (distance < minimumDistance) {
			minimumDistance = distance;
			bestIndex = candidate;
			bestIdealX =
				Math.abs(input.draggedBounds.x - leftIdealX) <=
				Math.abs(input.draggedBounds.x - rightIdealX)
					? leftIdealX
					: rightIdealX;
		}
	}
	return {
		preRemovalIndex: bestIndex,
		insertionIndex:
			bestIndex > input.firstDraggedTabIndex ? bestIndex - input.draggedTabCount : bestIndex,
		idealX: bestIdealX,
	};
}

export function getHorizontalDragInsertionIndex(input: HorizontalDragInsertionInput): number {
	return getHorizontalDragTarget(input).insertionIndex;
}
