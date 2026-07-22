import { describe, expect, it } from 'vitest';

import { getWorkbenchTabCloseTargets } from '../getWorkbenchTabCloseTargets';

const TABS = [
	{ id: 'first', label: 'First' },
	{ id: 'second', label: 'Second' },
	{ id: 'third', label: 'Third' },
] as const;

describe('getWorkbenchTabCloseTargets', () => {
	it('prepares atomic close groups for the target tab', () => {
		expect(getWorkbenchTabCloseTargets(TABS, 1)).toEqual({
			allTabIds: ['first', 'second', 'third'],
			otherTabIds: ['first', 'third'],
			rightTabIds: ['third'],
		});
	});

	it('returns no right-side targets for the final tab', () => {
		expect(getWorkbenchTabCloseTargets(TABS, 2).rightTabIds).toEqual([]);
	});
});
