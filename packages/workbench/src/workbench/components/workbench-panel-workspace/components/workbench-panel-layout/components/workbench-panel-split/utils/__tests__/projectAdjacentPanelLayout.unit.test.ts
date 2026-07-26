import { describe, expect, it } from 'vitest';

import { projectAdjacentPanelLayout } from '../projectAdjacentPanelLayout';

describe('projectAdjacentPanelLayout', () => {
	it('restores unrelated siblings and preserves the adjacent pair total', () => {
		const projection = projectAdjacentPanelLayout(
			['a', 'b', 'c'],
			0,
			{ a: 50, b: 25, c: 25 },
			{ a: 65, b: 20, c: 15 },
			0,
		);

		expect(projection).toEqual({
			leadingWeight: 65,
			layout: { a: 65, b: 10, c: 25 },
		});
	});

	it('clamps at the adjacent pair boundary instead of consuming a third panel', () => {
		const projection = projectAdjacentPanelLayout(
			['a', 'b', 'c'],
			0,
			{ a: 50, b: 25, c: 25 },
			{ a: 90, b: 0, c: 10 },
			0,
		);

		expect(projection).toEqual({
			leadingWeight: 75,
			layout: { a: 75, b: 0, c: 25 },
		});
	});

	it('rejects incomplete separator layouts', () => {
		expect(projectAdjacentPanelLayout(['a'], 0, { a: 100 }, { a: 100 }, 0)).toBeNull();
	});

	it('keeps both adjacent panels above the measured pixel minimum', () => {
		const projection = projectAdjacentPanelLayout(
			['a', 'b', 'c'],
			0,
			{ a: 50, b: 25, c: 25 },
			{ a: 90, b: 0, c: 10 },
			2,
		);

		expect(projection).toEqual({
			leadingWeight: 73,
			layout: { a: 73, b: 2, c: 25 },
		});
	});
});
