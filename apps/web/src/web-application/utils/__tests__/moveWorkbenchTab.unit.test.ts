import { describe, expect, it } from 'vitest';

import { moveWorkbenchTab } from '../moveWorkbenchTab';

const TABS = [
	{ id: 'first', label: 'First' },
	{ id: 'second', label: 'Second' },
	{ id: 'third', label: 'Third' },
] as const;

describe('moveWorkbenchTab', () => {
	it('moves a tab to its final requested index without changing its identity', () => {
		const tabs = moveWorkbenchTab(TABS, 'first', 2);

		expect(tabs.map((tab) => tab.id)).toEqual(['second', 'third', 'first']);
		expect(tabs[2]).toBe(TABS[0]);
	});

	it('moves a tab toward the start', () => {
		expect(moveWorkbenchTab(TABS, 'third', 0).map((tab) => tab.id)).toEqual([
			'third',
			'first',
			'second',
		]);
	});

	it('keeps the original array for invalid or unchanged requests', () => {
		expect(moveWorkbenchTab(TABS, 'second', 1)).toBe(TABS);
		expect(moveWorkbenchTab(TABS, 'missing', 1)).toBe(TABS);
		expect(moveWorkbenchTab(TABS, 'first', -1)).toBe(TABS);
		expect(moveWorkbenchTab(TABS, 'first', TABS.length)).toBe(TABS);
	});
});
