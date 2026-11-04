import { describe, expect, it } from 'vitest';

import { getPanelSeparatorIndexAtPoint } from '../getPanelSeparatorIndexAtPoint';

describe('getPanelSeparatorIndexAtPoint', () => {
	it('finds a horizontal separator from its geometric hit target without interaction state', () => {
		expect(
			getPanelSeparatorIndexAtPoint(
				'horizontal',
				{ x: 104, y: 200 },
				[{ height: 400, left: 100, top: 0, width: 1 }],
				10,
			),
		).toBe(0);
	});

	it('finds a vertical separator from its geometric hit target', () => {
		expect(
			getPanelSeparatorIndexAtPoint(
				'vertical',
				{ x: 200, y: 96 },
				[{ height: 1, left: 0, top: 100, width: 400 }],
				10,
			),
		).toBe(0);
	});

	it('does not select a separator outside the expanded hit target', () => {
		expect(
			getPanelSeparatorIndexAtPoint(
				'horizontal',
				{ x: 106, y: 200 },
				[{ height: 400, left: 100, top: 0, width: 1 }],
				10,
			),
		).toBeNull();
	});

	it('selects the closest separator when hit targets overlap', () => {
		expect(
			getPanelSeparatorIndexAtPoint(
				'horizontal',
				{ x: 103, y: 200 },
				[
					{ height: 400, left: 100, top: 0, width: 1 },
					{ height: 400, left: 104, top: 0, width: 1 },
				],
				10,
			),
		).toBe(1);
	});
});
