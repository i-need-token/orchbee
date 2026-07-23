import { describe, expect, it } from 'vitest';
import {
	buildDraggedTabsLayout,
	getAnimatedDraggedViewPosition,
	getSourceViewOffsetFromPointer,
	getTargetBeforeDraggedTabs,
	hasMinimumOverlap,
	shouldEnterVerticalTabGroup,
	shouldRemainInVerticalTabGroup,
} from '../dragged-tabs-container-layout';

describe('DraggedTabsContainer layout port', () => {
	it('derives the source offset from slot-relative pointer ratios', () => {
		expect(
			getSourceViewOffsetFromPointer({
				sourceSlotSize: { width: 100, height: 30 },
				sourceViewOffsetFromSlot: { x: 4, y: 2 },
				pointerOffsetRatios: { x: 0.25, y: 0.5 },
			}),
		).toEqual({ x: -29, y: -17 });
	});

	it('stacks vertical views with 2px spacing and anchors the source', () => {
		const result = buildDraggedTabsLayout({
			layout: 'vertical',
			pointerOffsetRatios: { x: 0.5, y: 0.5 },
			views: [
				{ id: 'one', bounds: { x: 10, y: 0, width: 100, height: 30 } },
				{ id: 'two', source: true, bounds: { x: 10, y: 32, width: 100, height: 30 } },
				{ id: 'three', bounds: { x: 10, y: 64, width: 100, height: 30 } },
			],
		});
		expect(result.views.map((view) => view.offset)).toEqual([
			{ x: 10, y: 0 },
			{ x: 10, y: 32 },
			{ x: 10, y: 64 },
		]);
		expect(result.boundsRelativeToPointer).toEqual({ x: -60, y: -47, width: 110, height: 96 });
	});

	it('squashes multiple views into the source and hides the remainder', () => {
		const result = buildDraggedTabsLayout({
			layout: 'squash',
			pointerOffsetRatios: { x: 0.5, y: 0.5 },
			views: [
				{ id: 'one', source: true, bounds: { x: 0, y: 0, width: 48, height: 32 } },
				{ id: 'two', bounds: { x: 0, y: 32, width: 48, height: 32 } },
			],
		});
		expect(result.boundsRelativeToPointer).toEqual({ x: -24, y: -16, width: 52, height: 32 });
		expect(result.views.map((view) => view.shouldHide)).toEqual([false, true]);
	});

	it('finds vertical insertion targets while discounting dragged height', () => {
		const children = [
			{ id: 'a', visible: true, bounds: { x: 0, y: 0, width: 100, height: 30 } },
			{
				id: 'drag',
				visible: true,
				dragging: true,
				bounds: { x: 0, y: 32, width: 100, height: 30 },
			},
			{ id: 'b', visible: true, bounds: { x: 0, y: 64, width: 100, height: 30 } },
		];
		expect(
			getTargetBeforeDraggedTabs({
				children,
				draggedBounds: { x: 0, y: 35, width: 100, height: 32 },
				axes: 'vertical-only',
			}),
		).toBe('b');
	});

	it('finds targets within grid rows and honors RTL logical x', () => {
		const children = [
			{ id: 'a', visible: true, bounds: { x: 0, y: 0, width: 40, height: 30 } },
			{ id: 'b', visible: true, bounds: { x: 42, y: 0, width: 40, height: 30 } },
			{ id: 'c', visible: true, bounds: { x: 0, y: 32, width: 40, height: 30 } },
		];
		expect(
			getTargetBeforeDraggedTabs({
				children,
				draggedBounds: { x: 45, y: 0, width: 30, height: 30 },
				axes: 'both',
			}),
		).toBe('b');
		expect(
			getTargetBeforeDraggedTabs({
				children,
				draggedBounds: { x: 5, y: 0, width: 30, height: 30 },
				axes: 'both',
				rtl: true,
				hostWidth: 82,
			}),
		).toBe('b');
	});

	it('ports overlap checks and drag-start interpolation', () => {
		expect(
			hasMinimumOverlap(
				{ x: 0, y: 0, width: 10, height: 10 },
				{ x: 8, y: 5, width: 10, height: 10 },
				2,
				5,
			),
		).toBe(true);
		expect(
			getAnimatedDraggedViewPosition({
				axes: 'vertical-only',
				draggingBounds: { x: 20, y: 40, width: 100, height: 60 },
				targetOffset: { x: 10, y: 12 },
				animationStartOffset: { x: 8, y: -20 },
				animationValue: 0.5,
			}),
		).toEqual({ x: 4, y: 42 });
	});

	it('uses Chromium group entry and exit hysteresis', () => {
		const group = { x: 0, y: 100, width: 200, height: 100 };
		expect(shouldEnterVerticalTabGroup({ x: 0, y: 90, width: 200, height: 20 }, group, 20)).toBe(
			true,
		);
		expect(shouldEnterVerticalTabGroup({ x: 0, y: 95, width: 200, height: 10 }, group, 20)).toBe(
			false,
		);
		expect(shouldRemainInVerticalTabGroup({ x: 0, y: 98, width: 200, height: 40 }, group, 20)).toBe(
			true,
		);
		expect(shouldRemainInVerticalTabGroup({ x: 0, y: 97, width: 200, height: 40 }, group, 20)).toBe(
			false,
		);
	});
});
