import { describe, expect, it } from 'vitest';
import { DragSessionData, DragSlotViewType, type TabDragData } from '../drag-session-data';
import {
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
	it("scales Chromium's 16 DIP reorder threshold with the target tab width", () => {
		expect(calculateHorizontalMoveThreshold(256, 256)).toBe(16);
		expect(calculateHorizontalMoveThreshold(128, 256)).toBe(8);
		expect(calculateHorizontalMoveThreshold(18, 256)).toBe(1);
		expect(
			shouldReorderDraggedTabs({
				justAttached: true,
				movementSinceLastReorder: 0,
				threshold: 16,
				initialMove: false,
				tabsConsecutive: true,
			}),
		).toBe(true);
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
