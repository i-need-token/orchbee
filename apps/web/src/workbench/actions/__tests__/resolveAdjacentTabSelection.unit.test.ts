import { describe, expect, it } from 'vitest';

import { resolveAdjacentTabSelection } from '../resolveAdjacentTabSelection';

describe('resolveAdjacentTabSelection', () => {
	it('rejects empty, singleton, and missing-active snapshots', () => {
		expect(
			resolveAdjacentTabSelection({ activeTabId: null, direction: 'next', tabIds: [] }),
		).toBeNull();
		expect(
			resolveAdjacentTabSelection({
				activeTabId: 'only',
				direction: 'previous',
				tabIds: ['only'],
			}),
		).toBeNull();
		expect(
			resolveAdjacentTabSelection({
				activeTabId: 'missing',
				direction: 'next',
				tabIds: ['pinned', 'grouped'],
			}),
		).toBeNull();
	});

	it('selects both adjacent directions from canonical interior order', () => {
		const tabIds = ['pinned', 'grouped-a', 'grouped-b', 'loose'];
		expect(
			resolveAdjacentTabSelection({
				activeTabId: 'grouped-a',
				direction: 'previous',
				tabIds,
			}),
		).toBe('pinned');
		expect(
			resolveAdjacentTabSelection({
				activeTabId: 'grouped-a',
				direction: 'next',
				tabIds,
			}),
		).toBe('grouped-b');
	});

	it('wraps previous from the first tab and next from the last tab', () => {
		const tabIds = ['first', 'middle', 'last'];
		expect(
			resolveAdjacentTabSelection({
				activeTabId: 'first',
				direction: 'previous',
				tabIds,
			}),
		).toBe('last');
		expect(
			resolveAdjacentTabSelection({
				activeTabId: 'last',
				direction: 'next',
				tabIds,
			}),
		).toBe('first');
	});
});
