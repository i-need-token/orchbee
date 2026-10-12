import { array, assert as assertProperty, nat, property as defineProperty } from 'fast-check';

import { requireDefined } from '../../utils/requireDefined';
import { describe, expect, it, vi } from 'vitest';
import {
	calculateDraggedTabsInsertion,
	TabModel,
	ToggleTabGroupCollapsedStateOrigin,
	type TabData,
	type TabDragRestoreState,
	type TabGroupData,
	type TabModelSnapshot,
} from '../tab-model';
const groups: TabGroupData[] = [
	{
		id: 'work',
		title: 'Work',
		color: 'blue',
		collapsed: false,
	},
	{
		id: 'read',
		title: 'Read',
		color: 'green',
		collapsed: false,
	},
];
const makeTab = (id: string, pinned = false, group: string | null = null): TabData => ({
	id,
	title: id,
	pinned,
	group,
});
const ids = (model: TabModel) => model.getSnapshot().tabs.map((tab) => tab.id);
const groupIds = (model: TabModel) => model.getSnapshot().tabs.map((tab) => tab.group);

function assertValidTabModelSnapshot(snapshot: TabModelSnapshot): void {
	const tabIds = new Set<string>();
	const groupIdentitySet = new Set<string>();
	for (const group of snapshot.groups) {
		if (groupIdentitySet.has(group.id)) {
			throw new Error(`Duplicate group identity: ${group.id}`);
		}
		groupIdentitySet.add(group.id);
	}

	let sawUnpinnedTab = false;
	let previousGroupId: string | null = null;
	const completedGroupIds = new Set<string>();
	for (const tab of snapshot.tabs) {
		if (tabIds.has(tab.id)) {
			throw new Error(`Duplicate tab identity: ${tab.id}`);
		}
		tabIds.add(tab.id);
		if (tab.pinned) {
			if (sawUnpinnedTab) {
				throw new Error(`Pinned tab appears after an unpinned tab: ${tab.id}`);
			}
			if (tab.group !== null) {
				throw new Error(`Pinned tab belongs to a group: ${tab.id}`);
			}
		} else {
			sawUnpinnedTab = true;
		}
		if (tab.group !== null && !groupIdentitySet.has(tab.group)) {
			throw new Error(`Tab references an unknown group: ${tab.id}`);
		}
		if (previousGroupId !== tab.group && previousGroupId !== null) {
			completedGroupIds.add(previousGroupId);
		}
		if (tab.group !== null && completedGroupIds.has(tab.group)) {
			throw new Error(`Group is not contiguous: ${tab.group}`);
		}
		previousGroupId = tab.group;
	}

	if (snapshot.tabs.length === 0 && snapshot.activeTabId !== null) {
		throw new Error(`Empty model has an active tab: ${snapshot.activeTabId}`);
	}
	if (snapshot.tabs.length > 0 && snapshot.activeTabId === null) {
		throw new Error('Non-empty model has no active tab.');
	}
	if (snapshot.activeTabId !== null && !tabIds.has(snapshot.activeTabId)) {
		throw new Error(`Active tab does not exist: ${snapshot.activeTabId}`);
	}
	for (const selectedTabId of snapshot.selectedTabIds) {
		if (!tabIds.has(selectedTabId)) {
			throw new Error(`Selected tab does not exist: ${selectedTabId}`);
		}
	}
	if (snapshot.activeTabId !== null && !snapshot.selectedTabIds.has(snapshot.activeTabId)) {
		throw new Error(`Active tab is not selected: ${snapshot.activeTabId}`);
	}
	if (snapshot.selectionAnchorId !== null && !tabIds.has(snapshot.selectionAnchorId)) {
		throw new Error(`Selection anchor does not exist: ${snapshot.selectionAnchorId}`);
	}
	if (snapshot.mostRecentGroupId !== null && !groupIdentitySet.has(snapshot.mostRecentGroupId)) {
		throw new Error(`Most-recent group does not exist: ${snapshot.mostRecentGroupId}`);
	}
}

function tabModelStateKey(snapshot: TabModelSnapshot): string {
	return JSON.stringify({
		activeTabId: snapshot.activeTabId,
		groups: snapshot.groups.map(({ collapsed, id }) => ({ collapsed, id })),
		mostRecentGroupId: snapshot.mostRecentGroupId,
		selectedTabIds: snapshot.tabs
			.filter((tab) => snapshot.selectedTabIds.has(tab.id))
			.map((tab) => tab.id),
		selectionAnchorId: snapshot.selectionAnchorId,
		tabs: snapshot.tabs.map(({ group, id, pinned }) => ({ group, id, pinned })),
	});
}

interface BoundedTabModelAction {
	readonly apply: (model: TabModel) => void;
	readonly name: string;
}

function nonEmptyTabIdSubsets(tabIds: readonly string[]): readonly (readonly string[])[] {
	const subsets: string[][] = [];
	for (let mask = 1; mask < 1 << tabIds.length; mask += 1) {
		subsets.push(tabIds.filter((_, index) => (mask & (1 << index)) !== 0));
	}
	return subsets;
}

function boundedTabModelActions(snapshot: TabModelSnapshot): readonly BoundedTabModelAction[] {
	const actions: BoundedTabModelAction[] = [];
	const tabIds = snapshot.tabs.map((tab) => tab.id);
	for (const tabId of tabIds) {
		actions.push(
			{ apply: (model) => model.selectTab(tabId), name: `select(${tabId})` },
			{ apply: (model) => model.toggleSelected(tabId), name: `toggle-selected(${tabId})` },
			{ apply: (model) => model.extendSelectionTo(tabId), name: `extend(${tabId})` },
			{ apply: (model) => model.addSelectionFromAnchorTo(tabId), name: `add-range(${tabId})` },
			{ apply: (model) => model.closeTab(tabId), name: `close(${tabId})` },
			{ apply: (model) => model.closeOtherTabs(tabId), name: `close-others(${tabId})` },
			{ apply: (model) => model.closeTabsToRight(tabId), name: `close-right(${tabId})` },
			{ apply: (model) => model.moveTabFirst(tabId), name: `move-first(${tabId})` },
			{ apply: (model) => model.moveTabLast(tabId), name: `move-last(${tabId})` },
			{ apply: (model) => model.shiftTabRelative(tabId, -1), name: `shift-left(${tabId})` },
			{ apply: (model) => model.shiftTabRelative(tabId, 1), name: `shift-right(${tabId})` },
		);
	}
	for (const group of snapshot.groups) {
		actions.push(
			{
				apply: (model) => model.toggleGroupCollapsed(group.id),
				name: `toggle-group(${group.id})`,
			},
			{ apply: (model) => model.ungroup(group.id), name: `ungroup(${group.id})` },
			{ apply: (model) => model.closeGroup(group.id), name: `close-group(${group.id})` },
			{
				apply: (model) => model.shiftGroupRelative(group.id, -1),
				name: `shift-group-left(${group.id})`,
			},
			{
				apply: (model) => model.shiftGroupRelative(group.id, 1),
				name: `shift-group-right(${group.id})`,
			},
		);
		for (let targetIndex = 0; targetIndex <= snapshot.tabs.length; targetIndex += 1) {
			actions.push({
				apply: (model) => model.moveGroupTo(group.id, targetIndex),
				name: `move-group(${group.id},${targetIndex})`,
			});
		}
	}
	for (const tabIdSubset of nonEmptyTabIdSubsets(tabIds)) {
		const label = tabIdSubset.join(',');
		actions.push(
			{ apply: (model) => model.closeTabs(tabIdSubset), name: `close-many(${label})` },
			{
				apply: (model) => model.setTabsPinned(tabIdSubset, true),
				name: `pin(${label})`,
			},
			{
				apply: (model) => model.setTabsPinned(tabIdSubset, false),
				name: `unpin(${label})`,
			},
			{
				apply: (model) => model.moveTabsToGroup(tabIdSubset, null),
				name: `remove-from-group(${label})`,
			},
		);
		for (const group of snapshot.groups) {
			actions.push({
				apply: (model) => model.moveTabsToGroup(tabIdSubset, group.id),
				name: `move-to-group(${label},${group.id})`,
			});
		}
	}
	return actions;
}

function createBoundedTabModel(): TabModel {
	return new TabModel({
		activeTabId: 'a',
		groups: [requireDefined(groups[0])],
		tabs: [makeTab('a', false, 'work'), makeTab('b', false, 'work'), makeTab('c')],
	});
}

describe('TabModel selection', () => {
	it('restores an explicit selection anchor from a Client View projection', () => {
		const model = new TabModel({
			activeTabId: 'c',
			selectedTabIds: ['b', 'c'],
			selectionAnchorId: 'b',
			tabs: ['a', 'b', 'c'].map((id) => makeTab(id)),
		});

		expect(model.getSnapshot().selectionAnchorId).toBe('b');
		expect([...model.getSnapshot().selectedTabIds]).toEqual(['b', 'c']);
	});

	it('supports Chrome anchor range selection and additive ranges', () => {
		const model = new TabModel({
			tabs: ['a', 'b', 'c', 'd'].map((id) => makeTab(id)),
			activeTabId: 'b',
		});
		model.extendSelectionTo('d');
		expect([...model.getSnapshot().selectedTabIds]).toEqual(['b', 'c', 'd']);
		model.selectTab('c');
		model.addSelectionFromAnchorTo('a');
		expect([...model.getSnapshot().selectedTabIds]).toEqual(['c', 'a', 'b']);
	});
	it('publishes cached immutable snapshots to subscribers', () => {
		const model = new TabModel({
			tabs: [makeTab('a')],
		});
		const before = model.getSnapshot();
		expect(model.getSnapshot()).toBe(before);
		const listener = vi.fn<() => void>();
		const unsubscribe = model.subscribe(listener);
		model.addTab(
			{
				title: 'b',
			},
			{
				id: 'b',
			},
		);
		expect(listener).toHaveBeenCalledOnce();
		expect(model.getSnapshot()).not.toBe(before);
		unsubscribe();
	});
	it('skips generated IDs already present in restored state', () => {
		const model = new TabModel({
			tabs: [makeTab('tab-1')],
		});
		expect(
			model.addTab({
				title: 'new',
			}),
		).toBe('tab-2');
	});
	it('notifies OnTabWillBeAdded observers before mutating the model', () => {
		const model = new TabModel({
			tabs: ['a', 'b'].map((id) => makeTab(id)),
			activeTabId: 'a',
		});
		const counts: number[] = [];
		const unsubscribe = model.subscribeBeforeTabAdded(() => {
			counts.push(model.getSnapshot().tabs.length);
		});
		model.addTab(
			{
				title: 'new',
			},
			{
				id: 'new',
			},
		);
		unsubscribe();
		model.addTab(
			{
				title: 'later',
			},
			{
				id: 'later',
			},
		);
		expect(counts).toEqual([2]);
		expect(model.getSnapshot().tabs).toHaveLength(4);
	});
	it('retains renderer state on inserted tabs', () => {
		const model = new TabModel();
		const id = model.addTab({
			title: 'Loading',
			shouldRenderLoadingTitle: true,
		});
		expect(model.getSnapshot().tabs.find((tab) => tab.id === id)).toMatchObject({
			shouldRenderLoadingTitle: true,
		});
	});
	it('retains the most recently activated group after selecting an ungrouped tab', () => {
		const model = new TabModel({
			tabs: [
				makeTab('work-tab', false, 'work'),
				makeTab('read-tab', false, 'read'),
				makeTab('outside'),
			],
			groups: [requireDefined(groups[1]), requireDefined(groups[0])],
			activeTabId: 'work-tab',
		});
		expect(model.getSnapshot().mostRecentGroupId).toBe('work');
		model.selectTab('read-tab');
		model.selectTab('outside');
		expect(model.getSnapshot().mostRecentGroupId).toBe('read');
	});
	it('rejects invalid grouped insertion before changing the model', () => {
		const model = new TabModel({
			tabs: [makeTab('a')],
		});
		const before = model.getSnapshot();
		expect(() =>
			model.addTab({
				title: 'bad',
				group: 'missing',
			}),
		).toThrow('Unknown group missing');
		expect(() =>
			model.addTab({
				title: 'bad',
				pinned: true,
				group: 'missing',
			}),
		).toThrow('Unknown group missing');
		expect(model.getSnapshot()).toBe(before);
		expect(ids(model)).toEqual(['a']);
	});
	it('rejects invalid restored selection and duplicate group identities', () => {
		expect(
			() =>
				new TabModel({
					tabs: [makeTab('a')],
					selectedTabIds: ['missing'],
				}),
		).toThrow('Unknown selected tab missing');
		expect(
			() =>
				new TabModel({
					tabs: [makeTab('a', false, 'work')],
					groups: [requireDefined(groups[0]), requireDefined(groups[0])],
				}),
		).toThrow('Duplicate group id work');
	});
	it('closes multiple tabs atomically and activates the next surviving slot', () => {
		const model = new TabModel({
			tabs: ['a', 'b', 'c', 'd'].map((id) => makeTab(id)),
			activeTabId: 'b',
			selectedTabIds: ['b', 'c'],
		});
		expect(model.closeTabs(['b', 'c'])).toBe(true);
		expect(ids(model)).toEqual(['a', 'd']);
		expect(model.getSnapshot().activeTabId).toBe('d');
		expect([...model.getSnapshot().selectedTabIds]).toEqual(['d']);
	});
	it('repairs the selection anchor when closing tabs to the right removes it', () => {
		const model = createBoundedTabModel();
		model.moveTabLast('a');
		model.toggleSelected('b');

		model.closeTabsToRight('b');

		expect(ids(model)).toEqual(['b']);
		expect(model.getSnapshot().activeTabId).toBe('b');
		expect(model.getSnapshot().selectionAnchorId).toBe('b');
	});
});
describe('TabStripCollectionController keyboard movement ports', () => {
	it('keeps pinned/unpinned boundaries and moves out of a group at the boundary', () => {
		const model = new TabModel({
			tabs: [
				makeTab('p', true),
				makeTab('a', false, 'work'),
				makeTab('b', false, 'work'),
				makeTab('c'),
			],
			groups: [requireDefined(groups[0])],
			activeTabId: 'a',
		});
		model.shiftTabRelative('a', -1);
		expect(ids(model)).toEqual(['p', 'a', 'b', 'c']);
		expect(groupIds(model)).toEqual([null, null, 'work', null]);
		model.shiftTabRelative('a', -1);
		expect(ids(model)).toEqual(['p', 'a', 'b', 'c']);
	});
	it('adds an ungrouped boundary tab to an expanded group without reordering', () => {
		const model = new TabModel({
			tabs: [makeTab('a'), makeTab('b', false, 'work'), makeTab('c', false, 'work')],
			groups: [requireDefined(groups[0])],
		});
		model.shiftTabRelative('a', 1);
		expect(ids(model)).toEqual(['a', 'b', 'c']);
		expect(groupIds(model)).toEqual(['work', 'work', 'work']);
	});
	it('skips an entire collapsed group as a single visual slot', () => {
		const model = new TabModel({
			tabs: [makeTab('a'), makeTab('b', false, 'work'), makeTab('c', false, 'work'), makeTab('d')],
			groups: [
				{
					...requireDefined(groups[0]),
					collapsed: true,
				},
			],
		});
		model.shiftTabRelative('a', 1);
		expect(ids(model)).toEqual(['b', 'c', 'a', 'd']);
		expect(requireDefined(model.getSnapshot().tabs[2]).group).toBeNull();
	});
	it('moves first/last only within the pin partition and removes group membership', () => {
		const model = new TabModel({
			tabs: [
				makeTab('p1', true),
				makeTab('p2', true),
				makeTab('a'),
				makeTab('b', false, 'work'),
				makeTab('c', false, 'work'),
			],
			groups: [requireDefined(groups[0])],
		});
		model.moveTabLast('p1');
		expect(ids(model).slice(0, 2)).toEqual(['p2', 'p1']);
		model.moveTabFirst('c');
		expect(ids(model)).toEqual(['p2', 'p1', 'c', 'a', 'b']);
		expect(requireDefined(model.getSnapshot().tabs[2]).group).toBeNull();
	});
	it('moves a group over one tab or a whole adjacent group', () => {
		const model = new TabModel({
			tabs: [
				makeTab('a', false, 'work'),
				makeTab('b', false, 'work'),
				makeTab('x'),
				makeTab('c', false, 'read'),
				makeTab('d', false, 'read'),
			],
			groups,
		});
		model.shiftGroupRelative('work', 1);
		expect(ids(model)).toEqual(['x', 'a', 'b', 'c', 'd']);
		model.shiftGroupRelative('work', 1);
		expect(ids(model)).toEqual(['x', 'c', 'd', 'a', 'b']);
	});
});
describe('groups and collapsing', () => {
	it('preserves tab identity order when creating a group around any contiguous tab block', () => {
		const originalOrder = ['a', 'b', 'c', 'd'];
		const contiguousSelections = [
			['a'],
			['b'],
			['c'],
			['d'],
			['a', 'b'],
			['b', 'c'],
			['c', 'd'],
			['a', 'b', 'c'],
			['b', 'c', 'd'],
			originalOrder,
		];

		contiguousSelections.forEach((selection) => {
			const model = new TabModel({
				tabs: originalOrder.map((id) => makeTab(id)),
				activeTabId: requireDefined(selection[0]),
			});
			model.createGroup(requireDefined(groups[0]), selection);

			expect(ids(model)).toEqual(originalOrder);
			expect(
				model
					.getSnapshot()
					.tabs.filter((tab) => selection.includes(tab.id))
					.map((tab) => tab.group),
			).toEqual(selection.map(() => 'work'));
		});
	});
	it('preserves tab identity order when removing every member of a group', () => {
		const originalOrder = ['before', 'grouped-a', 'grouped-b', 'after'];
		const model = new TabModel({
			tabs: [
				makeTab('before'),
				makeTab('grouped-a', false, 'work'),
				makeTab('grouped-b', false, 'work'),
				makeTab('after'),
			],
			groups: [requireDefined(groups[0])],
		});

		model.moveTabsToGroup(['grouped-a', 'grouped-b'], null);

		expect(ids(model)).toEqual(originalOrder);
		expect(groupIds(model)).toEqual([null, null, null, null]);
		expect(model.getGroup('work')).toBeNull();
	});
	it('preserves a single-member group position when removing its tab', () => {
		const model = new TabModel({
			tabs: [makeTab('before'), makeTab('grouped', false, 'work'), makeTab('after')],
			groups: [requireDefined(groups[0])],
		});

		model.moveTabsToGroup(['grouped'], null);

		expect(ids(model)).toEqual(['before', 'grouped', 'after']);
		expect(groupIds(model)).toEqual([null, null, null]);
		expect(model.getGroup('work')).toBeNull();
	});

	it('rejects empty groups but unpins tabs when grouping them', () => {
		const model = new TabModel({
			tabs: [makeTab('pinned', true), makeTab('loose')],
		});
		expect(() => model.createGroup(requireDefined(groups[0]), [])).toThrow(
			'at least one known tab',
		);
		expect(model.getSnapshot().groups).toHaveLength(0);
		model.createGroup(requireDefined(groups[0]), ['pinned']);
		expect(model.getSnapshot().groups).toHaveLength(1);
		expect(model.getSnapshot().tabs.find((tab) => tab.id === 'pinned')).toMatchObject({
			pinned: false,
			group: 'work',
		});
	});
	it('keeps the active identity in its group when the group is collapsed', () => {
		const model = new TabModel({
			tabs: [makeTab('a', false, 'work'), makeTab('b', false, 'work'), makeTab('outside')],
			groups: [requireDefined(groups[0])],
			activeTabId: 'a',
		});
		model.toggleGroupCollapsed('work');
		expect(model.getSnapshot().activeTabId).toBe('a');
		expect([...model.getSnapshot().selectedTabIds]).toEqual(['a']);
		expect(model.getGroup('work')?.collapsed).toBe(true);
		model.toggleGroupCollapsed('work');
		expect(model.getSnapshot().activeTabId).toBe('a');
		expect(model.getGroup('work')?.collapsed).toBe(false);
	});
	it('expands a collapsed group only when an active tab is added', () => {
		const model = new TabModel({
			tabs: [makeTab('grouped', false, 'work'), makeTab('active'), makeTab('background')],
			groups: [
				{
					...requireDefined(groups[0]),
					collapsed: true,
				},
			],
			activeTabId: 'active',
		});
		model.moveTabsToGroup(['background'], 'work');
		expect(model.getGroup('work')?.collapsed).toBe(true);
		model.moveTabsToGroup(['active'], 'work');
		expect(model.getGroup('work')?.collapsed).toBe(false);
		model.updateGroup('work', {
			collapsed: true,
		});
		model.addTab(
			{
				title: 'Foreground',
				group: 'work',
				pinned: false,
			},
			{
				activate: true,
			},
		);
		expect(model.getGroup('work')?.collapsed).toBe(false);
	});
	it('can apply the drag controller selection without expanding a collapsed group', () => {
		const model = new TabModel({
			tabs: [makeTab('a', false, 'work'), makeTab('b', false, 'work'), makeTab('outside')],
			groups: [
				{
					...requireDefined(groups[0]),
					collapsed: true,
				},
			],
			activeTabId: 'outside',
		});
		model.setSelection(['a', 'b'], 'a', {
			expandGroups: false,
		});
		expect(model.getGroup('work')?.collapsed).toBe(true);
		expect([...model.getSnapshot().selectedTabIds]).toEqual(['a', 'b']);
	});
	it('collapses an active-only group without manufacturing another active tab', () => {
		const model = new TabModel({
			tabs: [makeTab('a', false, 'work')],
			groups: [requireDefined(groups[0])],
			activeTabId: 'a',
		});
		model.toggleGroupCollapsed('work', ToggleTabGroupCollapsedStateOrigin.MenuAction);
		expect(model.getSnapshot().tabs).toHaveLength(1);
		expect(model.getSnapshot().activeTabId).toBe('a');
		expect(model.getGroup('work')?.collapsed).toBe(true);
	});
	it('pins tabs at the partition boundary and removes them from groups', () => {
		const model = new TabModel({
			tabs: [
				makeTab('p', true),
				makeTab('a', false, 'work'),
				makeTab('b', false, 'work'),
				makeTab('c'),
			],
			groups: [requireDefined(groups[0])],
		});
		model.setTabsPinned(['b'], true);
		expect(ids(model)).toEqual(['p', 'b', 'a', 'c']);
		expect(model.getSnapshot().tabs[1]).toMatchObject({
			pinned: true,
			group: null,
		});
	});
	it('does not reorder already unpinned tabs across group boundaries', () => {
		const model = createBoundedTabModel();
		const before = model.getSnapshot();

		model.setTabsPinned(['a', 'c'], false);

		expect(model.getSnapshot()).toBe(before);
		expect(ids(model)).toEqual(['a', 'b', 'c']);
		expect(groupIds(model)).toEqual(['work', 'work', null]);
	});
	it('repairs the selection anchor when closing its group while another tab is active', () => {
		const model = createBoundedTabModel();
		model.toggleSelected('c');

		model.closeGroup('work');

		expect(ids(model)).toEqual(['c']);
		expect(model.getSnapshot().activeTabId).toBe('c');
		expect(model.getSnapshot().selectionAnchorId).toBe('c');
	});
	it('does not move already ungrouped pinned tabs into the unpinned partition', () => {
		const model = createBoundedTabModel();
		model.setTabsPinned(['a'], true);
		const before = model.getSnapshot();

		model.moveTabsToGroup(['a'], null);

		expect(model.getSnapshot()).toBe(before);
		expect(ids(model)).toEqual(['a', 'b', 'c']);
		expect(model.getSnapshot().tabs[0]).toMatchObject({
			group: null,
			pinned: true,
		});
	});
	it('retains already pinned tabs when pinning a mixed selection', () => {
		const model = createBoundedTabModel();
		model.setTabsPinned(['a'], true);

		model.setTabsPinned(['a', 'b'], true);

		expect(ids(model)).toEqual(['a', 'b', 'c']);
		expect(
			model
				.getSnapshot()
				.tabs.slice(0, 2)
				.map((tab) => tab.pinned),
		).toEqual([true, true]);
		expect(model.getSnapshot().activeTabId).toBe('a');
	});
});
describe('tab drag index adjustment', () => {
	it('converts a group drag target before moving across another complete group', () => {
		const model = new TabModel({
			tabs: [
				makeTab('r1', false, 'research'),
				makeTab('r2', false, 'research'),
				makeTab('b1', false, 'build'),
				makeTab('b2', false, 'build'),
			],
			groups: [
				{
					id: 'research',
					title: 'Research',
					color: 'blue',
					collapsed: false,
				},
				{
					id: 'build',
					title: 'Build',
					color: 'purple',
					collapsed: false,
				},
			],
		});

		// CalculateInsertionIndex reports 4 in the live/pre-removal model. After
		// removing Research, MoveGroupTo must receive 2.
		model.moveGroupByDrag('research', 4);
		expect(ids(model)).toEqual(['b1', 'b2', 'r1', 'r2']);
	});
	it('subtracts dragged tabs before the pre-removal target', () => {
		expect(
			calculateDraggedTabsInsertion({
				containerStartIndex: 0,
				containerEndIndex: 6,
				targetIndex: 5,
				draggedIndices: [1, 3],
				draggedGroups: [null, null],
				targetGroup: null,
				firstSelectedIndex: 1,
			}),
		).toEqual({
			adjustedTargetIndex: 3,
			noOp: false,
		});
	});
	it('recognizes the Chromium no-op condition', () => {
		expect(
			calculateDraggedTabsInsertion({
				containerStartIndex: 0,
				containerEndIndex: 4,
				targetIndex: 1,
				draggedIndices: [1, 2],
				draggedGroups: ['work', 'work'],
				targetGroup: 'work',
				firstSelectedIndex: 1,
			}).noOp,
		).toBe(true);
	});
	it('allows the classic horizontal drag session to coalesce a nonconsecutive no-op target', () => {
		const model = new TabModel({
			tabs: ['a', 'b', 'c', 'd'].map((id) => makeTab(id)),
			activeTabId: 'a',
			selectedTabIds: ['a', 'c'],
		});
		expect(
			model.moveTabsByDrag(['a', 'c'], 0, 0, 4, null, {
				coalesceNonconsecutive: true,
			}),
		).toBe(true);
		expect(ids(model)).toEqual(['a', 'c', 'b', 'd']);
	});
	it('moves selected tabs in stable order into a target group', () => {
		const model = new TabModel({
			tabs: [
				makeTab('a'),
				makeTab('b'),
				makeTab('c', false, 'work'),
				makeTab('d', false, 'work'),
				makeTab('e'),
			],
			groups: [requireDefined(groups[0])],
			activeTabId: 'a',
			selectedTabIds: ['a', 'b'],
		});
		expect(model.moveSelectedTabsByDrag(4, 2, 4, 'work')).toBe(true);
		expect(ids(model)).toEqual(['c', 'd', 'a', 'b', 'e']);
		expect(groupIds(model)).toEqual(['work', 'work', 'work', 'work', null]);
	});
	it('snaps a dragged group around another group instead of splitting it', () => {
		const model = new TabModel({
			tabs: [
				makeTab('r1', false, 'research'),
				makeTab('r2', false, 'research'),
				makeTab('b1', false, 'build'),
				makeTab('b2', false, 'build'),
				makeTab('tail'),
			],
			groups: [
				{
					id: 'research',
					title: 'Research',
					color: 'blue',
					collapsed: false,
				},
				{
					id: 'build',
					title: 'Build',
					color: 'purple',
					collapsed: false,
				},
			],
		});
		expect(model.moveTabsByDrag(['r1', 'r2'], 3, 0, 5, 'research')).toBe(true);
		expect(ids(model)).toEqual(['b1', 'b2', 'r1', 'r2', 'tail']);
		expect(
			model
				.getSnapshot()
				.tabs.filter((tab) => tab.group === 'build')
				.map((tab) => tab.id),
		).toEqual(['b1', 'b2']);
	});
	it('snaps an ungrouped tab around a group instead of splitting it', () => {
		const model = new TabModel({
			tabs: [
				makeTab('moving'),
				makeTab('b1', false, 'build'),
				makeTab('b2', false, 'build'),
				makeTab('tail'),
			],
			groups: [
				{
					id: 'build',
					title: 'Build',
					color: 'purple',
					collapsed: false,
				},
			],
		});
		expect(model.moveTabsByDrag(['moving'], 2, 0, 4, null)).toBe(true);
		expect(ids(model)).toEqual(['b1', 'b2', 'moving', 'tail']);
	});
	it('retains complete group collections during a live drag and restores source order on cancel', () => {
		const model = new TabModel({
			tabs: [
				makeTab('p1', true),
				makeTab('p2', true),
				makeTab('a', false, 'group-a'),
				makeTab('b', false, 'group-a'),
				makeTab('c'),
				makeTab('d'),
			],
			groups: [
				{
					id: 'group-a',
					title: 'Group A',
					color: 'blue',
					collapsed: false,
				},
			],
			activeTabId: 'a',
		});
		const restore = model.captureDragRestoreState();
		model.moveTabsByDrag(['a', 'b'], 6, 2, 6, null);
		expect(model.getGroup('group-a')).not.toBeNull();
		expect(
			model
				.getSnapshot()
				.tabs.filter((tab) => tab.group === 'group-a')
				.map((tab) => tab.id),
		).toEqual(['a', 'b']);
		expect(model.getSnapshot().tabs.map((tab) => tab.id)).not.toEqual(
			restore.tabs.map((tab) => tab.id),
		);
		model.restoreCanceledDrag(restore);
		expect(model.getSnapshot().tabs.map((tab) => tab.id)).toEqual(
			restore.tabs.map((tab) => tab.id),
		);
		expect(
			model
				.getSnapshot()
				.tabs.filter((tab) => tab.group === 'group-a')
				.map((tab) => tab.id),
		).toEqual(['a', 'b']);
		expect(model.getGroup('group-a')).not.toBeNull();
	});
	it('retains only completely selected group collections in a mixed drag', () => {
		const model = new TabModel({
			tabs: [
				makeTab('loose'),
				makeTab('a', false, 'group-a'),
				makeTab('b', false, 'group-a'),
				makeTab('partial', false, 'group-b'),
				makeTab('left-behind', false, 'group-b'),
				makeTab('tail'),
			],
			groups: [
				{
					id: 'group-a',
					title: 'Group A',
					color: 'blue',
					collapsed: false,
				},
				{
					id: 'group-b',
					title: 'Group B',
					color: 'red',
					collapsed: false,
				},
			],
		});
		expect(model.moveTabsByDrag(['loose', 'a', 'b', 'partial'], 6, 0, 6, null)).toBe(true);
		const byId = new Map(model.getSnapshot().tabs.map((tab) => [tab.id, tab]));
		expect(byId.get('a')?.group).toBe('group-a');
		expect(byId.get('b')?.group).toBe('group-a');
		expect(byId.get('loose')?.group).toBeNull();
		expect(byId.get('partial')?.group).toBeNull();
		expect(byId.get('left-behind')?.group).toBe('group-b');
	});
});
describe('cross-window tab transfer', () => {
	it('moves a grouped selection between models with IDs, metadata, and activation intact', () => {
		const source = new TabModel({
			tabs: [
				makeTab('p', true),
				makeTab('a', false, 'work'),
				makeTab('b', false, 'work'),
				makeTab('tail'),
			],
			groups: [requireDefined(groups[0])],
			activeTabId: 'b',
			selectedTabIds: ['a', 'b'],
		});
		const target = new TabModel({
			tabs: [makeTab('target-p', true), makeTab('target')],
		});
		expect(
			source.transferTabsTo(target, ['a', 'b'], {
				index: 2,
			}),
		).toBe(true);
		expect(ids(source)).toEqual(['p', 'tail']);
		expect(ids(target)).toEqual(['target-p', 'target', 'a', 'b']);
		expect(
			target
				.getSnapshot()
				.tabs.slice(2)
				.map((tab) => tab.group),
		).toEqual(['work', 'work']);
		expect(target.getGroup('work')).toMatchObject({
			title: 'Work',
			color: 'blue',
		});
		expect(target.getSnapshot().activeTabId).toBe('b');
		expect([...target.getSnapshot().selectedTabIds]).toEqual(['a', 'b']);
	});
	it('can round-trip a transfer and restore both drag snapshots on cancel', () => {
		const source = new TabModel({
			tabs: [makeTab('a'), makeTab('b', false, 'work'), makeTab('c', false, 'work')],
			groups: [requireDefined(groups[0])],
			activeTabId: 'b',
			selectedTabIds: ['b', 'c'],
		});
		const target = new TabModel({
			tabs: [makeTab('x'), makeTab('y')],
		});
		const sourceRestore = source.captureDragRestoreState();
		const targetRestore = target.captureDragRestoreState();
		expect(
			source.transferTabsTo(target, ['b', 'c'], {
				index: 1,
			}),
		).toBe(true);
		expect(
			target.transferTabsTo(source, ['b', 'c'], {
				index: 1,
			}),
		).toBe(true);
		source.restoreCanceledDrag(sourceRestore);
		target.restoreCanceledDrag(targetRestore);
		expect(ids(source)).toEqual(['a', 'b', 'c']);
		expect(groupIds(source)).toEqual([null, 'work', 'work']);
		expect(ids(target)).toEqual(['x', 'y']);
		expect(source.getSnapshot().activeTabId).toBe('b');
	});
	it('rejects duplicate IDs without mutating either model', () => {
		const source = new TabModel({
			tabs: [makeTab('same')],
		});
		const target = new TabModel({
			tabs: [makeTab('same'), makeTab('other')],
		});
		expect(source.transferTabsTo(target, ['same'])).toBe(false);
		expect(ids(source)).toEqual(['same']);
		expect(ids(target)).toEqual(['same', 'other']);
	});
	it('honors an explicit null target group for a same-model transfer', () => {
		const model = new TabModel({
			tabs: [makeTab('a', false, 'work'), makeTab('b', false, 'work'), makeTab('tail')],
			groups: [requireDefined(groups[0])],
		});
		expect(
			model.transferTabsTo(model, ['b'], {
				index: 3,
				targetGroup: null,
			}),
		).toBe(true);
		expect(ids(model)).toEqual(['a', 'tail', 'b']);
		expect(groupIds(model)).toEqual(['work', null, null]);
	});
});

describe('TabModel restoration commit boundary', () => {
	const createModel = () =>
		new TabModel({
			tabs: [
				makeTab('p', true),
				makeTab('a', false, 'work'),
				makeTab('b', false, 'work'),
				makeTab('tail'),
			],
			groups: [requireDefined(groups[0])],
			activeTabId: 'a',
			selectedTabIds: ['a', 'b'],
		});
	const invalidArrangements: ReadonlyArray<{
		name: string;
		error: string;
		prepare(model: TabModel, state: TabDragRestoreState): TabDragRestoreState;
	}> = [
		{
			name: 'duplicate tab with changed membership',
			error: 'Duplicate tab id a.',
			prepare: (_model, state) => ({
				...state,
				tabs: [
					{ id: 'a', group: null },
					{ id: 'a', group: null },
				],
			}),
		},
		{
			name: 'duplicate group',
			error: 'Duplicate group id work.',
			prepare: (_model, state) => ({
				...state,
				groups: [...state.groups, ...state.groups],
			}),
		},
		{
			name: 'broken pinned prefix',
			error: 'Pinned tabs must form a contiguous prefix.',
			prepare: (_model, state) => ({
				...state,
				tabs: [...state.tabs].reverse(),
			}),
		},
		{
			name: 'split group',
			error: 'Group work must be contiguous.',
			prepare: (_model, state) => ({
				...state,
				tabs: [
					{ id: 'p', group: null },
					{ id: 'a', group: 'work' },
					{ id: 'tail', group: null },
					{ id: 'b', group: 'work' },
				],
			}),
		},
		{
			name: 'new pinned tab after capture',
			error: 'Pinned tabs must form a contiguous prefix.',
			prepare: (model, state) => {
				model.addTab({ title: 'New pinned', pinned: true }, { id: 'new' });
				return state;
			},
		},
		{
			name: 'changed pinned state after capture',
			error: 'Pinned tabs must form a contiguous prefix.',
			prepare: (model, state) => {
				model.setTabsPinned(['tail'], true);
				return state;
			},
		},
		{
			name: 'new group after capture',
			error: 'Tab new references unknown group read.',
			prepare: (model, state) => {
				model.addTab({ title: 'New grouped' }, { id: 'new' });
				model.createGroup(requireDefined(groups[1]), ['new']);
				return state;
			},
		},
	];
	it.each(invalidArrangements)(
		'rejects $name without changing live authority',
		({ prepare, error }) => {
			const model = createModel();
			const invalid = prepare(model, model.captureDragRestoreState());
			const before = model.getSnapshot();
			const beforeCapture = model.captureDragRestoreState();
			const beforeIndices = before.tabs.map((tab) => model.getIndexOfTab(tab.id));
			const observer = vi.fn<() => void>();
			model.subscribe(observer);
			expect(() => model.restoreCanceledDrag(invalid)).toThrow(error);
			expect(model.getSnapshot()).toBe(before);
			expect(model.captureDragRestoreState()).toEqual(beforeCapture);
			expect(before.tabs.map((tab) => model.getIndexOfTab(tab.id))).toEqual(beforeIndices);
			expect(observer).not.toHaveBeenCalled();
			expect(() => model.updateTab('a', { title: 'Still usable' })).not.toThrow();
			expect(model.getSnapshot().revision).toBe(before.revision + 1);
			expect(observer).toHaveBeenCalledOnce();
			assertValidTabModelSnapshot(model.getSnapshot());
		},
	);

	it('keeps supported stale membership filtering and appended ungrouped tabs', () => {
		const model = createModel();
		const captured = model.captureDragRestoreState();
		model.closeTab('a');
		model.addTab({ title: 'New ungrouped' }, { id: 'new' });
		model.restoreCanceledDrag(captured);
		expect(ids(model)).toEqual(['p', 'b', 'tail', 'new']);
		expect(groupIds(model)).toEqual([null, 'work', null, null]);
		expect(model.getSnapshot().activeTabId).toBe('p');
		expect(model.getSnapshot().selectedTabIds).toEqual(new Set(['b', 'p']));
		assertValidTabModelSnapshot(model.getSnapshot());
	});

	it('preserves the committed transition and peer publications after an observer throws', () => {
		const model = createModel();
		const observer = vi.fn<() => void>();
		model.subscribe(() => {
			throw new Error('observer failed');
		});
		model.subscribe(observer);
		const before = model.getSnapshot();
		expect(() => model.updateTab('a', { title: 'Committed' })).not.toThrow();
		expect(model.getSnapshot().revision).toBe(before.revision + 1);
		expect(model.getSnapshot().tabs.find((tab) => tab.id === 'a')?.title).toBe('Committed');
		expect(observer).toHaveBeenCalledOnce();
	});

	it('captures one publication cohort and honors removals before their turn', () => {
		const model = createModel();
		const added = vi.fn<() => void>();
		const removed = vi.fn<() => void>();
		let first = true;
		const observer = vi.fn<() => void>(() => {
			if (!first) return;
			first = false;
			unsubscribe();
			model.subscribe(observer);
			model.subscribe(added);
			removePeer();
		});
		const unsubscribe = model.subscribe(observer);
		const removePeer = model.subscribe(removed);
		model.updateTab('a', { title: 'First' });
		expect(observer).toHaveBeenCalledOnce();
		expect(added).not.toHaveBeenCalled();
		expect(removed).not.toHaveBeenCalled();
		model.updateTab('a', { title: 'Second' });
		expect(observer).toHaveBeenCalledTimes(2);
		expect(added).toHaveBeenCalledOnce();
	});
});

describe('TabModel bounded state-space exploration', () => {
	it('preserves identity and structural invariants across every three-action trace', () => {
		interface ExplorationNode {
			readonly model: TabModel;
			readonly trace: readonly BoundedTabModelAction[];
		}

		let frontier: readonly ExplorationNode[] = [
			{
				model: createBoundedTabModel(),
				trace: [],
			},
		];
		const seenStates = new Set<string>([
			tabModelStateKey(requireDefined(frontier[0]).model.getSnapshot()),
		]);
		let exploredTransitions = 0;

		for (let depth = 0; depth < 3; depth += 1) {
			const nextFrontier: ExplorationNode[] = [];
			for (const node of frontier) {
				for (const action of boundedTabModelActions(node.model.getSnapshot())) {
					const trace = [...node.trace, action];
					const model = createBoundedTabModel();
					try {
						for (const step of trace) {
							step.apply(model);
						}
						assertValidTabModelSnapshot(model.getSnapshot());
					} catch (error) {
						throw new Error(
							`Invalid TabModel trace: ${trace.map((step) => step.name).join(' -> ')}`,
							{
								cause: error,
							},
						);
					}
					exploredTransitions += 1;
					const stateKey = tabModelStateKey(model.getSnapshot());
					if (!seenStates.has(stateKey)) {
						seenStates.add(stateKey);
						nextFrontier.push({
							model,
							trace,
						});
					}
				}
			}
			frontier = nextFrontier;
		}

		expect({
			exploredTransitions,
			reachableStates: seenStates.size,
		}).toEqual({
			exploredTransitions: 15_497,
			reachableStates: 952,
		});
	});
});

describe('TabModel property testing', () => {
	it('preserves invariants across reproducible deep action sequences', () => {
		expect(() => {
			assertProperty(
				defineProperty(
					array(nat(), {
						minLength: 1,
						maxLength: 100,
					}),
					(actionChoices) => {
						const model = createBoundedTabModel();
						const trace: string[] = [];
						for (const [stepIndex, actionChoice] of actionChoices.entries()) {
							const actions = boundedTabModelActions(model.getSnapshot());
							if (actions.length === 0) {
								const tabId = `recovered-${stepIndex}`;
								model.addTab(
									{
										title: tabId,
									},
									{
										id: tabId,
									},
								);
								trace.push(`add(${tabId})`);
							} else {
								const action = requireDefined(actions[actionChoice % actions.length]);
								action.apply(model);
								trace.push(action.name);
							}
							try {
								assertValidTabModelSnapshot(model.getSnapshot());
							} catch (error) {
								throw new Error(`Invalid TabModel trace: ${trace.join(' -> ')}`, {
									cause: error,
								});
							}
						}
					},
				),
				{
					numRuns: 200,
					seed: 20_260_726,
				},
			);
		}).not.toThrow();
	});
});
