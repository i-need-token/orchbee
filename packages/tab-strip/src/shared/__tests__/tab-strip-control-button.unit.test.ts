import { describe, expect, it } from 'vitest';
import {
	getScaledTabStripControlButtonRadius,
	getTabStripControlButtonPreferredSize,
	getTabStripControlButtonRadii,
	TabStripControlButtonEdge,
} from '../tab-strip-control-button';

describe('TabStripControlButton geometry', () => {
	it("uses Chromium's 28px content size plus view insets", () => {
		expect(getTabStripControlButtonPreferredSize()).toEqual({ width: 28, height: 28 });
		expect(getTabStripControlButtonPreferredSize({ top: 4, bottom: 4, left: 2, right: 3 })).toEqual(
			{ width: 33, height: 36 },
		);
	});

	it('makes a fixed edge flat and interpolates an animated edge', () => {
		expect(
			getScaledTabStripControlButtonRadius(14, TabStripControlButtonEdge.Left, {
				fixedFlatEdge: TabStripControlButtonEdge.Left,
			}),
		).toBe(0);
		expect(
			getScaledTabStripControlButtonRadius(14, TabStripControlButtonEdge.Right, {
				animatedFlatEdge: TabStripControlButtonEdge.Right,
				flatEdgeFactor: 0.5,
			}),
		).toBe(7);
	});

	it('supports nudge-style nonzero flat radii and custom side radii', () => {
		expect(
			getTabStripControlButtonRadii({
				fixedFlatEdge: TabStripControlButtonEdge.Right,
				leftCornerRadius: 10,
				rightCornerRadius: 12,
				flatCornerRadius: 4,
			}),
		).toEqual({ topLeft: 10, topRight: 4, bottomRight: 4, bottomLeft: 10 });
	});

	it('extends a condensed frame mask to square top corners only', () => {
		expect(getTabStripControlButtonRadii({ frameCondensed: true })).toEqual({
			topLeft: 0,
			topRight: 0,
			bottomRight: 14,
			bottomLeft: 14,
		});
	});
});
