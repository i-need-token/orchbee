import { describe, expect, it } from 'vitest';

import { matchesPanelLayoutTopology } from '../matchesPanelLayoutTopology';

describe('matchesPanelLayoutTopology', () => {
	it('accepts a layout with exactly one finite weight for every current child', () => {
		expect(
			matchesPanelLayoutTopology(
				{
					'split-1--panel-1': 50,
					'split-1--panel-2': 25,
					'split-1--panel-3': 25,
				},
				['split-1--panel-1', 'split-1--panel-2', 'split-1--panel-3'],
			),
		).toBe(true);
	});

	it('rejects the stale two-panel layout after a third same-axis child is inserted', () => {
		expect(
			matchesPanelLayoutTopology(
				{
					'split-1--panel-1': 50,
					'split-1--panel-2': 50,
				},
				['split-1--panel-1', 'split-1--panel-2', 'split-1--panel-3'],
			),
		).toBe(false);
	});

	it('rejects stale, extra, or non-finite child weights', () => {
		expect(
			matchesPanelLayoutTopology(
				{
					'split-1--panel-1': 50,
					'split-1--panel-2': 25,
					'split-1--panel-3': 25,
					'split-1--panel-4': 0,
				},
				['split-1--panel-1', 'split-1--panel-2', 'split-1--panel-3'],
			),
		).toBe(false);
		expect(
			matchesPanelLayoutTopology(
				{
					'split-1--panel-1': 50,
					'split-1--panel-2': Number.NaN,
					'split-1--panel-3': 25,
				},
				['split-1--panel-1', 'split-1--panel-2', 'split-1--panel-3'],
			),
		).toBe(false);
	});
});
