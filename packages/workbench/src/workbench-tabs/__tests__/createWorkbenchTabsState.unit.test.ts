import { describe, expect, it } from 'vitest';

import { createWorkbenchTabsState } from '../createWorkbenchTabsState';

describe('createWorkbenchTabsState', () => {
	it('canonicalizes pinning, membership, group population, ordering, and active identity', () => {
		const state = createWorkbenchTabsState({
			activeTabId: 'grouped',
			tabGroups: [
				{ color: 'blue', id: 'group-a', isCollapsed: true, label: 'Group A' },
				{ color: 'red', id: 'empty', label: 'Empty' },
			],
			tabs: [
				{ groupId: 'group-a', id: 'grouped', label: 'Grouped' },
				{ groupId: 'group-a', id: 'pinned', isPinned: true, label: 'Pinned' },
				{ groupId: 'missing', id: 'orphan', label: 'Orphan' },
			],
		});

		expect(state).toEqual({
			activeTabId: 'grouped',
			tabGroups: [{ color: 'blue', id: 'group-a', isCollapsed: true, label: 'Group A' }],
			tabs: [
				{ id: 'pinned', isPinned: true, label: 'Pinned' },
				{ groupId: 'group-a', id: 'grouped', label: 'Grouped' },
				{ id: 'orphan', label: 'Orphan' },
			],
		});
	});

	it('preserves canonical arrays and the requested visible active tab', () => {
		const tabs = [
			{ id: 'pinned', isPinned: true, label: 'Pinned' },
			{ groupId: 'group-a', id: 'grouped', label: 'Grouped' },
			{ id: 'plain', label: 'Plain' },
		] as const;
		const tabGroups = [{ color: 'blue', id: 'group-a', label: 'Group A' }] as const;

		const state = createWorkbenchTabsState({ activeTabId: 'grouped', tabGroups, tabs });

		expect(state.activeTabId).toBe('grouped');
		expect(state.tabs).toBe(tabs);
		expect(state.tabGroups).toBe(tabGroups);
	});

	it('allows only an empty state to have no active tab', () => {
		expect(createWorkbenchTabsState({ tabs: [] }).activeTabId).toBeUndefined();
		expect(
			createWorkbenchTabsState({
				tabGroups: [{ color: 'blue', id: 'group-a', isCollapsed: true, label: 'Group A' }],
				tabs: [{ groupId: 'group-a', id: 'grouped', label: 'Grouped' }],
			}).activeTabId,
		).toBe('grouped');
	});

	it('rejects duplicate tab and group identifiers', () => {
		expect(() =>
			createWorkbenchTabsState({
				tabs: [
					{ id: 'duplicate', label: 'First' },
					{ id: 'duplicate', label: 'Second' },
				],
			}),
		).toThrow('Duplicate tab identifier: duplicate');
		expect(() =>
			createWorkbenchTabsState({
				tabGroups: [
					{ color: 'blue', id: 'duplicate', label: 'First' },
					{ color: 'red', id: 'duplicate', label: 'Second' },
				],
				tabs: [],
			}),
		).toThrow('Duplicate tab group identifier: duplicate');
	});
});
