import { requireDefined } from '../../utils/requireDefined';
import { describe, expect, it, vi } from 'vitest';
import {
	calculateDraggedTabsInsertion,
	TabModel,
	ToggleTabGroupCollapsedStateOrigin,
	type TabData,
	type TabGroupData,
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
describe('TabModel selection', () => {
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
	it('stores focused-group state and resets it when the group is deleted', () => {
		const model = new TabModel({
			tabs: [makeTab('a', false, 'work'), makeTab('b', false, 'work'), makeTab('outside')],
			groups: [requireDefined(groups[0])],
		});
		expect(model.setFocusedGroup('work')).toBe(true);
		expect(model.getFocusedGroup()).toBe('work');
		expect(model.getSnapshot().focusedGroupId).toBe('work');
		expect(model.setFocusedGroup('work')).toBe(false);
		expect(() => model.setFocusedGroup('missing')).toThrow('Unknown group missing');
		model.ungroup('work');
		expect(model.getFocusedGroup()).toBeNull();
	});
	it('restores focused-group state when a live drag is canceled', () => {
		const model = new TabModel({
			tabs: [makeTab('a', false, 'work'), makeTab('b', false, 'work'), makeTab('outside')],
			groups: [requireDefined(groups[0])],
			focusedGroupId: 'work',
		});
		const restore = model.captureDragRestoreState();
		model.moveTabsByDrag(['a', 'b'], 3, 0, 3, null);
		expect(model.getFocusedGroup()).toBe('work');
		model.restoreCanceledDrag(restore);
		expect(model.getFocusedGroup()).toBe('work');
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
