import { describe, expect, it } from 'vitest';
import { getHorizontalDragTarget } from '../getHorizontalDragTarget';

const bounds = [0, 82, 164, 246].map((x) => ({ x, y: 0, width: 100, height: 40 }));
const plain = { group: null };

describe('horizontal tab drag insertion', () => {
	it('keeps the model-adapter boundary separate from the post-removal insertion index', () => {
		expect(
			getHorizontalDragTarget({
				draggedBounds: { x: 160, y: 0, width: 100, height: 40 },
				tabs: [plain, plain, plain, plain],
				idealTabBounds: bounds,
				firstDraggedTabIndex: 0,
				draggedTabCount: 1,
				containsGroupHeader: false,
				collapsedGroups: new Set(),
			}),
		).toMatchObject({
			preRemovalIndex: 3,
			insertionIndex: 2,
		});
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
			getHorizontalDragTarget({
				...shared,
				containsGroupHeader: true,
				collapsedGroups: new Set(),
			}).preRemovalIndex,
		).not.toBe(2);
		expect(
			getHorizontalDragTarget({
				...shared,
				containsGroupHeader: false,
				collapsedGroups: new Set(['work']),
			}).preRemovalIndex,
		).not.toBe(2);
	});
});
