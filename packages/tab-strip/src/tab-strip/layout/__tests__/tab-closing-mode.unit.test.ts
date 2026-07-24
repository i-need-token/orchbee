import { describe, expect, it } from 'vitest';
import { LayoutDomain } from '../tab-strip-layout-types';
import {
	calculateTabClosingModeWidth,
	getTabClosingModeWatchBounds,
	shouldExitTabClosingMode,
	TAB_CLOSE_TOUCH_RESIZE_DELAY_MS,
	tabClosingModeContainsPoint,
} from '../tab-closing-mode';

const base = {
	currentOverride: null,
	containerWidth: 800,
	modelCountAfterRemoval: 4,
	removedModelIndex: 1,
	removedWidth: 120,
	removedWasActive: false,
	removedWasPinned: false,
	layoutDomain: LayoutDomain.InactiveWidthEqualsActiveWidth,
	trailingIdealRight: 650,
	tabOverlap: 18,
} as const;

describe('TabContainerImpl closing-mode ports', () => {
	it('keeps trailing closes at the current override and exits for the last tab', () => {
		expect(calculateTabClosingModeWidth({ ...base, removedModelIndex: 4 })).toBe(800);
		expect(calculateTabClosingModeWidth({ ...base, modelCountAfterRemoval: 0 })).toBeNull();
	});

	it('subtracts the removed effective width so repeated close targets stay put', () => {
		expect(calculateTabClosingModeWidth(base)).toBe(548);
	});

	it('uses the next inactive width when an active tab closes below crossover', () => {
		expect(
			calculateTabClosingModeWidth({
				...base,
				removedWasActive: true,
				layoutDomain: LayoutDomain.InactiveWidthBelowActiveWidth,
				nextActiveWidth: 88,
			}),
		).toBe(580);
	});

	it('watches 40px below and 60px beyond the logical trailing edge', () => {
		const ltr = getTabClosingModeWatchBounds({ x: 10, y: 20, width: 500, height: 40 });
		const rtl = getTabClosingModeWatchBounds({ x: 10, y: 20, width: 500, height: 40 }, true);
		expect(ltr).toEqual({ x: 10, y: 20, width: 560, height: 80 });
		expect(rtl).toEqual({ x: -50, y: 20, width: 560, height: 80 });
		expect(tabClosingModeContainsPoint(ltr, { x: 569, y: 99 })).toBe(true);
		expect(tabClosingModeContainsPoint(ltr, { x: 570, y: 99 })).toBe(false);
		expect(TAB_CLOSE_TOUCH_RESIZE_DELAY_MS).toBe(2_000);
	});

	it('exits only after the override stops constraining the ideal bounds', () => {
		expect(shouldExitTabClosingMode(600, 599)).toBe(true);
		expect(shouldExitTabClosingMode(600, 600)).toBe(false);
		expect(shouldExitTabClosingMode(600, 601)).toBe(false);
	});
});
