import { describe, expect, it } from 'vitest';
import { getHorizontalDragInsertionIndex } from '../tab-strip-drag-position';

const bounds = [0, 82, 164, 246].map((x) => ({ x, y: 0, width: 100, height: 40 }));
const plain = { group: null };

describe('horizontal tab drag insertion', () => {
	it('chooses the valid ideal position closest to the dragged view', () => {
		expect(
			getHorizontalDragInsertionIndex({
				draggedBounds: { x: 160, y: 0, width: 100, height: 40 },
				tabs: [plain, plain, plain, plain],
				idealTabBounds: bounds,
				firstDraggedTabIndex: 0,
				draggedTabCount: 1,
				containsGroupHeader: false,
				collapsedGroups: new Set(),
			}),
		).toBe(3);
	});

	it('keeps group-header drags out of another group and tabs out of collapsed groups', () => {
		const tabs = [plain, { group: 'work' }, { group: 'work' }, plain];
		const shared = {
			draggedBounds: { x: 100, y: 0, width: 182, height: 40 },
			tabs,
			idealTabBounds: bounds,
			firstDraggedTabIndex: 0,
			draggedTabCount: 1,
			groupHeaderWidths: new Map([['work', 52]]),
		};
		expect(
			getHorizontalDragInsertionIndex({
				...shared,
				containsGroupHeader: true,
				collapsedGroups: new Set(),
			}),
		).not.toBe(2);
		expect(
			getHorizontalDragInsertionIndex({
				...shared,
				containsGroupHeader: false,
				collapsedGroups: new Set(['work']),
			}),
		).not.toBe(2);
	});

	it('moves a complete group only after crossing the far edge of its neighbor', () => {
		const tabs = [
			{ group: 'research' },
			{ group: 'research' },
			{ group: 'build' },
			{ group: 'build' },
		];
		expect(
			getHorizontalDragInsertionIndex({
				draggedBounds: { x: 164, y: 0, width: 182, height: 40 },
				tabs,
				idealTabBounds: bounds,
				firstDraggedTabIndex: 0,
				draggedTabCount: 2,
				containsGroupHeader: true,
				collapsedGroups: new Set(['research']),
			}),
		).toBe(4);
	});

	it('does not subtract dragged width twice after views leave the source layout', () => {
		const removedSourceBounds = [
			{ x: 0, y: 0, width: 18, height: 40 },
			{ x: 0, y: 0, width: 18, height: 40 },
			{ x: 0, y: 0, width: 100, height: 40 },
			{ x: 82, y: 0, width: 100, height: 40 },
		];
		expect(
			getHorizontalDragInsertionIndex({
				draggedBounds: { x: 164, y: 0, width: 182, height: 40 },
				tabs: [
					{ group: 'research' },
					{ group: 'research' },
					{ group: 'build' },
					{ group: 'build' },
				],
				idealTabBounds: removedSourceBounds,
				firstDraggedTabIndex: 0,
				draggedTabCount: 2,
				containsGroupHeader: true,
				collapsedGroups: new Set(['research']),
				draggedViewsRemovedFromLayout: true,
			}),
		).toBe(4);
	});
});
