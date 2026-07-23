import { describe, expect, it } from 'vitest';
import { DragSessionData, DragSlotViewType, type TabDragData } from '../drag-session-data';
import {
	areDraggedTabsConsecutive,
	calculateAttachedDragPoint,
	calculateDragMouseOffset,
	calculateDraggedViewBounds,
	calculateGroupForDraggedTabs,
	calculateHorizontalMoveThreshold,
	shouldReorderDraggedTabs,
} from '../dragging-tabs-session';

function tab(index: number | null, id: string): TabDragData {
	return {
		sourceModelIndex: index,
		viewType: DragSlotViewType.Tab,
		contentsId: id,
		attachedViewId: id,
		pinned: false,
		tabGroupData: null,
	};
}

function session(tabs: readonly TabDragData[], sourceViewIndex = 0, ratio = 0.5) {
	return new DragSessionData({
		tabDragData: tabs,
		sourceViewIndex,
		mouseOffsetToSizeRatios: { x: ratio, y: 0.5 },
	});
}

describe('DragSessionData', () => {
	it('exposes source, attached, group-header, and tab-count helpers', () => {
		const data = new DragSessionData({
			tabDragData: [
				{ ...tab(null, 'header'), viewType: DragSlotViewType.GroupHeader, contentsId: null },
				tab(2, 'a'),
				{ ...tab(3, 'b'), attachedViewId: null },
			],
			groupHeaderDragData: {
				group: 'group',
				activeTabIndexWithinGroup: 1,
				wasCollapsedFromDrag: false,
			},
			draggingGroups: new Set(['group']),
			sourceViewIndex: 1,
			mouseOffsetToSizeRatios: { x: 0.25, y: 0.5 },
		});
		expect(data.groupHeaderId).toBe('group');
		expect(data.sourceDraggedContentsId).toBe('a');
		expect(data.numDraggingTabs).toBe(2);
		expect(data.attachedViewIds).toEqual(['header', 'a']);
	});
});

describe('DraggingTabsSession geometry and movement ports', () => {
	it('detects nonconsecutive source tabs while ignoring header gaps', () => {
		expect(areDraggedTabsConsecutive(session([tab(1, 'a'), tab(2, 'b')]))).toBe(true);
		expect(areDraggedTabsConsecutive(session([tab(1, 'a'), tab(3, 'b')]))).toBe(false);
		expect(areDraggedTabsConsecutive(session([tab(null, 'header'), tab(3, 'b')]))).toBe(true);
	});

	it('keeps the same fractional point in the source view under the cursor', () => {
		const data = session([tab(1, 'a'), tab(2, 'b')], 1, 0.25);
		expect(
			calculateDragMouseOffset(data, (ids) => ids.length * 100 - (ids.length - 1) * 18, 100),
		).toBe(107);
	});

	it('clamps the attached point to the tab drag area', () => {
		expect(
			calculateAttachedDragPoint({
				contextPoint: { x: 10, y: 30 },
				mouseOffset: 40,
				tabDragAreaWidth: 300,
				draggedViewsSize: 120,
			}),
		).toEqual({ x: 0, y: 0 });
		expect(
			calculateAttachedDragPoint({
				contextPoint: { x: 400, y: 30 },
				mouseOffset: 40,
				tabDragAreaWidth: 300,
				draggedViewsSize: 120,
			}),
		).toEqual({ x: 180, y: 0 });
		expect(
			calculateAttachedDragPoint({
				contextPoint: { x: 20, y: 30 },
				mirroredX: 200,
				mouseOffset: 40,
				tabDragAreaWidth: 300,
				draggedViewsSize: 120,
			}),
		).toEqual({ x: 160, y: 0 });
	});

	it('calculates attached and unattached dragged bounds', () => {
		expect(
			calculateDraggedViewBounds({
				point: { x: 20, y: 0 },
				draggedBounds: [
					{ x: 0, y: 0, width: 100, height: 35 },
					{ x: 82, y: 0, width: 100, height: 35 },
				],
				sourceViewAttached: true,
				sourceViewHeight: 35,
				standardTabWidth: 256,
				tabHeight: 35,
			}),
		).toEqual({ x: 20, y: 0, width: 182, height: 35 });
		expect(
			calculateDraggedViewBounds({
				point: { x: 20, y: 5 },
				draggedBounds: [],
				sourceViewAttached: false,
				sourceViewHeight: 0,
				standardTabWidth: 256,
				tabHeight: 35,
			}),
		).toEqual({ x: 20, y: 5, width: 256, height: 35 });
	});

	it("scales Chromium's 16 DIP reorder threshold with the target tab width", () => {
		expect(calculateHorizontalMoveThreshold(256, 256)).toBe(16);
		expect(calculateHorizontalMoveThreshold(128, 256)).toBe(8);
		expect(
			shouldReorderDraggedTabs({
				justAttached: false,
				movementSinceLastReorder: 8,
				threshold: 8,
				initialMove: false,
				tabsConsecutive: true,
			}),
		).toBe(false);
		expect(
			shouldReorderDraggedTabs({
				justAttached: false,
				movementSinceLastReorder: 9,
				threshold: 8,
				initialMove: false,
				tabsConsecutive: true,
			}),
		).toBe(true);
		expect(
			shouldReorderDraggedTabs({
				justAttached: false,
				movementSinceLastReorder: 0,
				threshold: 8,
				initialMove: true,
				tabsConsecutive: false,
			}),
		).toBe(true);
	});
});

describe('CalculateGroupForDraggedTabs', () => {
	const base = {
		containsGroupHeader: false,
		hasSelectedUnpinnedTab: true,
		leftGroup: 'left',
		rightGroup: 'right',
		currentGroup: null,
		leftGroupCollapsed: false,
		rightGroupCollapsed: false,
		selectedTabWidth: 100,
		tabLeftInset: 9,
		leftEdge: 100,
		selectedLeft: 100,
		selectedRight: 200,
		tabDragAreaEnd: 500,
	} as const;

	it('uses matching adjacent membership and rejects pinned/header drags', () => {
		expect(calculateGroupForDraggedTabs({ ...base, leftGroup: 'same', rightGroup: 'same' })).toBe(
			'same',
		);
		expect(calculateGroupForDraggedTabs({ ...base, containsGroupHeader: true })).toBeNull();
		expect(calculateGroupForDraggedTabs({ ...base, hasSelectedUnpinnedTab: false })).toBeNull();
	});

	it('leans toward expanded adjacent groups and treats collapsed groups as closed', () => {
		expect(calculateGroupForDraggedTabs({ ...base, selectedLeft: 70, selectedRight: 170 })).toBe(
			'left',
		);
		expect(calculateGroupForDraggedTabs({ ...base, selectedLeft: 130, selectedRight: 230 })).toBe(
			'right',
		);
		expect(
			calculateGroupForDraggedTabs({
				...base,
				selectedLeft: 70,
				selectedRight: 170,
				leftGroupCollapsed: true,
			}),
		).toBeNull();
	});

	it('ungroups at the drag-area edge and applies sticky current-group buffers', () => {
		expect(
			calculateGroupForDraggedTabs({ ...base, selectedLeft: 70, selectedRight: 500 }),
		).toBeNull();
		expect(
			calculateGroupForDraggedTabs({
				...base,
				currentGroup: 'left',
				selectedLeft: 90,
				selectedRight: 190,
			}),
		).toBe('left');
		expect(
			calculateGroupForDraggedTabs({
				...base,
				currentGroup: 'right',
				selectedLeft: 110,
				selectedRight: 210,
			}),
		).toBe('right');
	});
});
