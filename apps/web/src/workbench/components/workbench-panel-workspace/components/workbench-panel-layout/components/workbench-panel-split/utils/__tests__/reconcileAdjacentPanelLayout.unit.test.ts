import { assert, integer, property } from 'fast-check';
import { describe, expect, it } from 'vitest';

import { reconcileAdjacentPanelLayout } from '../reconcileAdjacentPanelLayout';

describe('reconcileAdjacentPanelLayout', () => {
	it('treats an equivalent primitive layout as an intentional no-op', () => {
		expect(
			reconcileAdjacentPanelLayout({
				availablePanelSizePx: 1_000,
				candidateLayout: { a: 50, b: 25, c: 25 },
				childIds: ['a', 'b', 'c'],
				minimumPanelSizePx: 30,
				modelLayout: { a: 50, b: 25, c: 25 },
			}),
		).toEqual({ kind: 'unchanged' });
	});

	it('accepts one adjacent change without correcting an already valid candidate', () => {
		expect(
			reconcileAdjacentPanelLayout({
				availablePanelSizePx: 1_000,
				candidateLayout: { a: 60, b: 15, c: 25 },
				childIds: ['a', 'b', 'c'],
				minimumPanelSizePx: 30,
				modelLayout: { a: 50, b: 25, c: 25 },
			}),
		).toEqual({
			kind: 'accepted',
			candidateNeedsCorrection: false,
			layout: { a: 60, b: 15, c: 25 },
			leadingWeight: 60,
			modelNeedsUpdate: true,
			separatorIndex: 0,
		});
	});

	it('derives a trailing separator without changing its leading sibling', () => {
		expect(
			reconcileAdjacentPanelLayout({
				availablePanelSizePx: 1_000,
				candidateLayout: { a: 25, b: 40, c: 35 },
				childIds: ['a', 'b', 'c'],
				minimumPanelSizePx: 30,
				modelLayout: { a: 25, b: 50, c: 25 },
			}),
		).toEqual({
			kind: 'accepted',
			candidateNeedsCorrection: false,
			layout: { a: 25, b: 40, c: 35 },
			leadingWeight: 40,
			modelNeedsUpdate: true,
			separatorIndex: 1,
		});
	});

	it('corrects a candidate at the pixel minimum without inventing a model update', () => {
		expect(
			reconcileAdjacentPanelLayout({
				availablePanelSizePx: 1_000,
				candidateLayout: { a: 80, b: -5, c: 25 },
				childIds: ['a', 'b', 'c'],
				minimumPanelSizePx: 30,
				modelLayout: { a: 72, b: 3, c: 25 },
			}),
		).toEqual({
			kind: 'accepted',
			candidateNeedsCorrection: true,
			layout: { a: 72, b: 3, c: 25 },
			leadingWeight: 72,
			modelNeedsUpdate: false,
			separatorIndex: 0,
		});
	});

	it('rejects invalid topology, geometry, and ambiguous changes explicitly', () => {
		const input = {
			availablePanelSizePx: 1_000,
			candidateLayout: { a: 60, b: 15, c: 25 },
			childIds: ['a', 'b', 'c'],
			minimumPanelSizePx: 30,
			modelLayout: { a: 50, b: 25, c: 25 },
		} as const;

		expect(
			reconcileAdjacentPanelLayout({
				...input,
				childIds: ['a', 'a', 'c'],
			}),
		).toEqual({ kind: 'rejected', reason: 'invalid-child-ids' });
		expect(
			reconcileAdjacentPanelLayout({
				...input,
				modelLayout: { a: 50, b: 25 },
			}),
		).toEqual({ kind: 'rejected', reason: 'invalid-model-layout' });
		expect(
			reconcileAdjacentPanelLayout({
				...input,
				candidateLayout: { a: 60, b: Number.NaN, c: 25 },
			}),
		).toEqual({ kind: 'rejected', reason: 'invalid-candidate-layout' });
		expect(
			reconcileAdjacentPanelLayout({
				...input,
				availablePanelSizePx: -1,
			}),
		).toEqual({ kind: 'rejected', reason: 'invalid-geometry' });
		expect(
			reconcileAdjacentPanelLayout({
				...input,
				availablePanelSizePx: 0,
			}),
		).toEqual({ kind: 'rejected', reason: 'invalid-geometry' });
		expect(
			reconcileAdjacentPanelLayout({
				...input,
				candidateLayout: { a: 60, b: 25, c: 15 },
			}),
		).toEqual({ kind: 'rejected', reason: 'non-adjacent-change' });
		expect(
			reconcileAdjacentPanelLayout({
				...input,
				candidateLayout: { a: 60, b: 20, c: 20 },
			}),
		).toEqual({ kind: 'rejected', reason: 'non-adjacent-change' });
	});

	it('preserves the adjacent pair and every unrelated sibling across generated candidates', () => {
		assert(
			property(
				integer({ min: 1, max: 100 }),
				integer({ min: 1, max: 100 }),
				integer({ min: 1, max: 100 }),
				integer({ min: -200, max: 200 }).filter((delta) => delta !== 0),
				integer({ min: 1, max: 2_000 }),
				integer({ min: 0, max: 100 }),
				(leading, trailing, unrelated, delta, availablePanelSizePx, minimumPanelSizePx) => {
					const reconciliation = reconcileAdjacentPanelLayout({
						availablePanelSizePx,
						candidateLayout: {
							a: leading + delta,
							b: trailing - delta,
							c: unrelated,
						},
						childIds: ['a', 'b', 'c'],
						minimumPanelSizePx,
						modelLayout: { a: leading, b: trailing, c: unrelated },
					});
					if (reconciliation.kind !== 'accepted') {
						throw new Error(
							`Expected an accepted reconciliation, received ${reconciliation.kind}.`,
						);
					}

					const effectiveMinimum = Math.min(
						(minimumPanelSizePx / availablePanelSizePx) * 100,
						(leading + trailing) / 2,
					);
					expect((reconciliation.layout['a'] ?? 0) + 0.000_001).toBeGreaterThanOrEqual(
						effectiveMinimum,
					);
					expect((reconciliation.layout['b'] ?? 0) + 0.000_001).toBeGreaterThanOrEqual(
						effectiveMinimum,
					);
					expect((reconciliation.layout['a'] ?? 0) + (reconciliation.layout['b'] ?? 0)).toBeCloseTo(
						leading + trailing,
						10,
					);
					expect(reconciliation.layout['c']).toBe(unrelated);
				},
			),
		);
	});
});
