import { describe, expect, it } from 'vitest';
import { balanceSurfacePadding, calculateSurfaceGrid } from '../surface-size';

describe('calculateSurfaceGrid', () => {
	it('keeps the mature two-column minimum when a positive content area is narrower', () => {
		expect(
			calculateSurfaceGrid(
				{ width: 9, height: 20 },
				{ width: 8, height: 16 },
				{ top: 0, bottom: 0, left: 0, right: 0 },
			),
		).toEqual({ columns: 2, rows: 1 });
	});

	it('returns no grid when explicit padding consumes either content dimension', () => {
		expect(
			calculateSurfaceGrid(
				{ width: 16, height: 16 },
				{ width: 8, height: 16 },
				{ top: 8, bottom: 8, left: 0, right: 0 },
			),
		).toBeUndefined();
		expect(
			calculateSurfaceGrid(
				{ width: 16, height: 16 },
				{ width: 8, height: 16 },
				{ top: 0, bottom: 0, left: 8, right: 8 },
			),
		).toBeUndefined();
	});
});

describe('balanceSurfacePadding', () => {
	it('distributes whitespace equally after explicit padding', () => {
		const padding = balanceSurfacePadding(
			{ width: 1_050, height: 850 },
			{ width: 10, height: 20 },
			{ top: 4, bottom: 4, left: 4, right: 4 },
			'equal',
		);

		expect(padding.left).toBe(padding.right);
		expect(padding.top).toBe(padding.bottom);
		expect(padding.top).toBeGreaterThan(0);
		expect(padding).toEqual({ top: 5, bottom: 5, left: 5, right: 5 });
	});

	it('shifts excess top padding to the bottom in the capped mode', () => {
		const padding = balanceSurfacePadding(
			{ width: 1_090, height: 1_070 },
			{ width: 20, height: 40 },
			{ top: 0, bottom: 0, left: 0, right: 0 },
			true,
		);

		expect(padding.left).toBe(padding.right);
		expect(padding.top).toBeLessThan(padding.bottom);
		expect(padding).toEqual({ top: 10, bottom: 20, left: 5, right: 5 });
	});

	it('never returns negative padding for a zero-sized surface', () => {
		expect(
			balanceSurfacePadding(
				{ width: 0, height: 0 },
				{ width: 10, height: 20 },
				{ top: 0, bottom: 0, left: 0, right: 0 },
				'equal',
			),
		).toEqual({ top: 0, bottom: 0, left: 0, right: 0 });
	});
});
