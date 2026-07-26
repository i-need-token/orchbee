import { describe, expect, it } from 'vitest';
import { calculateVerticalDragCompositeBounds } from '../calculateVerticalDragCompositeBounds';

function calculateGroupHeaderCompositeBounds(groupWasCollapsedBeforeDrag: boolean) {
	return calculateVerticalDragCompositeBounds({
		collapsed: false,
		drag: {
			groupHeaderId: 'group-1',
			groupWasCollapsedBeforeDrag,
			pointerOffsetRatios: { x: 0.5, y: 0.5 },
			sourceId: 'tab-1',
			sourceSlotSize: { width: 200, height: 26 },
			sourceViewOffsetFromSlot: { x: 0, y: 0 },
			visualViews: [
				{
					id: 'group:group-1',
					kind: 'group',
					tabIds: ['tab-1'],
					x: 0,
					y: 0,
					width: 200,
					height: 66,
				},
			],
			x: 150,
			y: 90,
		},
		sourcePinned: false,
		viewportBounds: { x: 0, y: 20, width: 240, height: 100 },
	});
}

describe('calculateVerticalDragCompositeBounds', () => {
	it('uses the complete dragged vertical composite for edge detection', () => {
		const bounds = calculateVerticalDragCompositeBounds({
			collapsed: false,
			drag: {
				groupHeaderId: null,
				groupWasCollapsedBeforeDrag: false,
				pointerOffsetRatios: { x: 0.5, y: 0.5 },
				sourceId: 'tab-1',
				sourceSlotSize: { width: 200, height: 32 },
				sourceViewOffsetFromSlot: { x: 0, y: 0 },
				visualViews: [
					{ id: 'tab-1', kind: 'tab', x: 0, y: 0, width: 200, height: 32 },
					{ id: 'tab-2', kind: 'tab', x: 0, y: 34, width: 200, height: 32 },
				],
				x: 150,
				y: 90,
			},
			sourcePinned: false,
			viewportBounds: { x: 0, y: 20, width: 240, height: 100 },
		});

		expect(bounds).toEqual({ x: 50, y: 54, width: 200, height: 68 });
	});

	it('uses squash bounds for collapsed or pinned drag presentations', () => {
		const bounds = calculateVerticalDragCompositeBounds({
			collapsed: true,
			drag: {
				groupHeaderId: null,
				groupWasCollapsedBeforeDrag: false,
				pointerOffsetRatios: { x: 0.5, y: 0.5 },
				sourceId: 'tab-1',
				sourceSlotSize: { width: 80, height: 32 },
				sourceViewOffsetFromSlot: { x: 0, y: 0 },
				visualViews: [
					{ id: 'tab-1', kind: 'tab', x: 0, y: 0, width: 80, height: 32 },
					{ id: 'tab-2', kind: 'tab', x: 82, y: 0, width: 80, height: 32 },
				],
				x: 100,
				y: 50,
			},
			sourcePinned: false,
			viewportBounds: { x: 10, y: 10, width: 100, height: 100 },
		});

		expect(bounds).toEqual({ x: 50, y: 24, width: 84, height: 32 });
	});

	it('uses pre-drag group visibility for group-header composite height', () => {
		expect(calculateGroupHeaderCompositeBounds(false).height).toBe(68);
		expect(calculateGroupHeaderCompositeBounds(true).height).toBe(36);
	});
});
