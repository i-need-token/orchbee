import { describe, expect, it } from 'vitest';

import { resolveActiveTabMovement } from '../resolveActiveTabMovement';

describe('resolveActiveTabMovement', () => {
	it('rejects missing active identities and singleton pin partitions', () => {
		expect(
			resolveActiveTabMovement({ activeTabId: null, direction: 'forward', tabs: [] }),
		).toBeNull();
		expect(
			resolveActiveTabMovement({
				activeTabId: 'missing',
				direction: 'backward',
				tabs: [tab('only', false)],
			}),
		).toBeNull();
		expect(
			resolveActiveTabMovement({
				activeTabId: 'pinned',
				direction: 'forward',
				tabs: [tab('pinned', true), tab('unpinned', false)],
			}),
		).toBeNull();
	});

	it('delegates both relative directions inside one pin partition', () => {
		const tabs = [tab('first', false), tab('middle', false), tab('last', false)];
		expect(
			resolveActiveTabMovement({ activeTabId: 'middle', direction: 'backward', tabs }),
		).toEqual({ direction: 'previous', tabId: 'middle' });
		expect(resolveActiveTabMovement({ activeTabId: 'middle', direction: 'forward', tabs })).toEqual(
			{ direction: 'next', tabId: 'middle' },
		);
	});

	it('wraps both boundaries inside the unpinned partition', () => {
		const tabs = [tab('pinned', true), tab('first', false), tab('last', false)];
		expect(resolveActiveTabMovement({ activeTabId: 'first', direction: 'backward', tabs })).toEqual(
			{ direction: 'last', tabId: 'first' },
		);
		expect(resolveActiveTabMovement({ activeTabId: 'last', direction: 'forward', tabs })).toEqual({
			direction: 'first',
			tabId: 'last',
		});
	});

	it('wraps both boundaries inside the pinned prefix', () => {
		const tabs = [tab('first', true), tab('last', true), tab('unpinned', false)];
		expect(resolveActiveTabMovement({ activeTabId: 'first', direction: 'backward', tabs })).toEqual(
			{ direction: 'last', tabId: 'first' },
		);
		expect(resolveActiveTabMovement({ activeTabId: 'last', direction: 'forward', tabs })).toEqual({
			direction: 'first',
			tabId: 'last',
		});
	});
});

function tab(id: string, pinned: boolean) {
	return { id, pinned } as const;
}
