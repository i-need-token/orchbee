import { describe, expect, it } from 'vitest';

import {
	createWorkbenchTabStripModel,
	projectWorkbenchTabsState,
	readWorkbenchTabStripModel,
} from '../workbench-tab-strip-model';

describe('workbench tab strip model', () => {
	it('represents pinned tabs, ungrouped tabs, and groups as separate structural regions', () => {
		const model = createWorkbenchTabStripModel({
			activeTabId: 'group-first',
			tabGroups: [{ color: 'blue', id: 'group-a', label: 'Group A' }],
			tabs: [
				{ id: 'pinned', isPinned: true, label: 'Pinned' },
				{ id: 'plain', label: 'Plain' },
				{ groupId: 'group-a', id: 'group-first', label: 'Group First' },
				{ groupId: 'group-a', id: 'group-second', label: 'Group Second' },
			],
		});

		expect(model.pinnedTabs.map((tab) => tab.id)).toEqual(['pinned']);
		expect(model.items.map((item) => item.kind)).toEqual(['tab', 'group']);
		expect(model.items[1]).toMatchObject({
			group: { id: 'group-a' },
			kind: 'group',
		});
		expect(model.selection).toEqual({
			activeTabId: 'group-first',
			anchorTabId: 'group-first',
			selectedTabIds: ['group-first'],
		});
	});

	it('normalizes only at the input boundary and rejects invalid authoritative topology', () => {
		const normalized = projectWorkbenchTabsState(
			createWorkbenchTabStripModel({
				tabGroups: [{ color: 'blue', id: 'group-a', label: 'Group A' }],
				tabs: [
					{ groupId: 'group-a', id: 'first', label: 'First' },
					{ id: 'plain', label: 'Plain' },
					{ groupId: 'group-a', id: 'second', label: 'Second' },
				],
			}),
		);
		expect(normalized.tabs.map((tab) => tab.id)).toEqual(['first', 'second', 'plain']);
		expect(() =>
			readWorkbenchTabStripModel({
				activeTabId: 'first',
				tabGroups: [{ color: 'blue', id: 'group-a', label: 'Group A' }],
				tabs: [
					{ groupId: 'group-a', id: 'first', label: 'First' },
					{ id: 'plain', label: 'Plain' },
					{ groupId: 'group-a', id: 'second', label: 'Second' },
				],
			}),
		).toThrow('violates the tab strip topology invariants');
	});
});
