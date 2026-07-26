import { describe, expect, it } from 'vitest';

import { getPanelDropDirection } from '../getPanelDropDirection';

const rect = {
	bottom: 600,
	height: 400,
	left: 100,
	right: 900,
	top: 200,
	width: 800,
};

describe('getPanelDropDirection', () => {
	it.each([
		[{ x: 120, y: 400 }, 'left'],
		[{ x: 880, y: 400 }, 'right'],
		[{ x: 500, y: 220 }, 'above'],
		[{ x: 500, y: 580 }, 'below'],
	] as const)('selects the nearest eligible edge at %o', (point, direction) => {
		expect(getPanelDropDirection(rect, point)).toBe(direction);
	});

	it('uses normalized edge distance for panels with unequal width and height', () => {
		expect(getPanelDropDirection(rect, { x: 220, y: 250 })).toBe('above');
	});

	it('leaves the center and empty geometry without a drop action', () => {
		expect(getPanelDropDirection(rect, { x: 500, y: 400 })).toBeNull();
		expect(getPanelDropDirection({ ...rect, height: 0, width: 0 }, { x: 500, y: 400 })).toBeNull();
	});
});
