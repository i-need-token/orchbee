import { describe, expect, it } from 'vitest';

import { resolveWorkbenchTabGroupDragActiveTabId } from '../resolveWorkbenchTabGroupDragActiveTabId';

const TABS = [
	{ id: 'outside', label: 'Outside' },
	{ groupId: 'group-a', id: 'first', label: 'First' },
	{ groupId: 'group-a', id: 'second', label: 'Second' },
] as const;

describe('resolveWorkbenchTabGroupDragActiveTabId', () => {
	it('preserves the active tab when it already belongs to the dragged group', () => {
		expect(resolveWorkbenchTabGroupDragActiveTabId(TABS, 'second', 'group-a')).toBe('second');
	});

	it('selects the first group member when the active tab is outside the dragged group', () => {
		expect(resolveWorkbenchTabGroupDragActiveTabId(TABS, 'outside', 'group-a')).toBe('first');
	});

	it('returns no target for a group without members', () => {
		expect(resolveWorkbenchTabGroupDragActiveTabId(TABS, 'outside', 'missing')).toBeUndefined();
	});
});
