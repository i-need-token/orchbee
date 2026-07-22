import { describe, expect, it } from 'vitest';

import { createWorkbenchTabsState } from '../createWorkbenchTabsState';
import { reduceWorkbenchTabs } from '../reduceWorkbenchTabs';
import type {
	WorkbenchTab,
	WorkbenchTabsAction,
	WorkbenchTabsState,
} from '../workbench-tabs-state';

function createState(): WorkbenchTabsState {
	return createWorkbenchTabsState({
		activeTabId: 'group-first',
		tabGroups: [
			{ color: 'blue', id: 'group-a', label: 'Group A' },
			{ color: 'red', id: 'group-b', label: 'Group B' },
		],
		tabs: [
			{ id: 'pinned-first', isPinned: true, label: 'Pinned First' },
			{ id: 'pinned-second', isPinned: true, label: 'Pinned Second' },
			{ groupId: 'group-a', id: 'group-first', label: 'Group First' },
			{ groupId: 'group-a', id: 'group-second', label: 'Group Second' },
			{ id: 'plain', label: 'Plain' },
			{ groupId: 'group-b', id: 'group-b-only', label: 'Group B Only' },
		],
	});
}

function tabIds(state: WorkbenchTabsState): readonly string[] {
	return state.tabs.map((tab) => tab.id);
}

describe('reduceWorkbenchTabs', () => {
	it('activates existing tabs, including members whose group trigger is collapsed', () => {
		const state = createState();
		const collapsed = reduceWorkbenchTabs(state, {
			groupId: 'group-b',
			isCollapsed: true,
			kind: 'set-tab-group-collapsed',
		});

		expect(reduceWorkbenchTabs(state, { kind: 'activate-tab', tabId: 'plain' }).activeTabId).toBe(
			'plain',
		);
		expect(reduceWorkbenchTabs(state, { kind: 'activate-tab', tabId: 'missing' })).toBe(state);
		expect(
			reduceWorkbenchTabs(collapsed, { kind: 'activate-tab', tabId: 'group-b-only' }).activeTabId,
		).toBe('group-b-only');
	});

	it('inserts tabs at the end of an expanded group and activates the new tab', () => {
		const collapsed = reduceWorkbenchTabs(createState(), {
			groupId: 'group-a',
			isCollapsed: true,
			kind: 'set-tab-group-collapsed',
		});
		const state = reduceWorkbenchTabs(collapsed, {
			groupId: 'group-a',
			kind: 'insert-tab',
			tab: { id: 'new', label: 'New' },
		});

		expect(tabIds(state)).toEqual([
			'pinned-first',
			'pinned-second',
			'group-first',
			'group-second',
			'new',
			'plain',
			'group-b-only',
		]);
		expect(state.tabs[4]?.groupId).toBe('group-a');
		expect(state.tabGroups[0]?.isCollapsed).toBe(false);
		expect(state.activeTabId).toBe('new');
	});

	it('uses the insert action as the only authority for pinning and group placement', () => {
		const state = createState();
		const staleGroupedTab: WorkbenchTab = {
			groupId: 'group-a',
			id: 'ungrouped-insert',
			label: 'Ungrouped Insert',
		};
		const ungrouped = reduceWorkbenchTabs(state, {
			groupId: undefined,
			kind: 'insert-tab',
			tab: staleGroupedTab,
		});
		expect(ungrouped.tabs.at(-1)).toEqual({
			id: 'ungrouped-insert',
			label: 'Ungrouped Insert',
		});
		const orphanGroupedTab: WorkbenchTab = {
			groupId: 'missing',
			id: 'orphan-insert',
			label: 'Orphan Insert',
		};
		const orphan = reduceWorkbenchTabs(state, {
			groupId: undefined,
			kind: 'insert-tab',
			tab: orphanGroupedTab,
		});
		expect(orphan.tabs.at(-1)).toEqual({
			id: 'orphan-insert',
			label: 'Orphan Insert',
		});

		const stalePinnedGroupTab: WorkbenchTab = {
			groupId: 'group-b',
			id: 'grouped-insert',
			isPinned: true,
			label: 'Grouped Insert',
		};
		const grouped = reduceWorkbenchTabs(state, {
			groupId: 'group-a',
			kind: 'insert-tab',
			tab: stalePinnedGroupTab,
		});
		expect(grouped.tabs.filter((tab) => tab.groupId === 'group-a').map((tab) => tab.id)).toEqual([
			'group-first',
			'group-second',
			'grouped-insert',
		]);
		expect(grouped.tabs.find((tab) => tab.id === 'grouped-insert')?.isPinned).toBeUndefined();

		const pinned = reduceWorkbenchTabs(state, {
			groupId: undefined,
			kind: 'insert-tab',
			tab: { ...stalePinnedGroupTab, id: 'pinned-insert', label: 'Pinned Insert' },
		});
		expect(tabIds(pinned).slice(0, 3)).toEqual(['pinned-first', 'pinned-second', 'pinned-insert']);
		expect(pinned.tabs[2]).toEqual({
			id: 'pinned-insert',
			isPinned: true,
			label: 'Pinned Insert',
		});
	});

	it('rejects insertion into a missing group without changing state', () => {
		const state = createState();

		expect(
			reduceWorkbenchTabs(state, {
				groupId: 'missing',
				kind: 'insert-tab',
				tab: { id: 'new', label: 'New' },
			}),
		).toBe(state);
	});

	it('closes active first, middle, last, bulk, group, and all tab selections deterministically', () => {
		const state = createState();
		const middle = reduceWorkbenchTabs(state, {
			kind: 'close-tabs',
			tabIds: ['group-first'],
		});
		expect(middle.activeTabId).toBe('group-second');

		const first = reduceWorkbenchTabs(
			{ ...state, activeTabId: 'pinned-first' },
			{ kind: 'close-tabs', tabIds: ['pinned-first'] },
		);
		expect(first.activeTabId).toBe('pinned-second');

		const last = reduceWorkbenchTabs(
			{ ...state, activeTabId: 'group-b-only' },
			{ kind: 'close-tabs', tabIds: ['group-b-only'] },
		);
		expect(last.activeTabId).toBe('plain');
		expect(last.tabGroups.map((group) => group.id)).toEqual(['group-a']);

		const bulk = reduceWorkbenchTabs(state, {
			kind: 'close-tabs',
			tabIds: ['group-first', 'group-second'],
		});
		expect(bulk.activeTabId).toBe('plain');
		expect(bulk.tabGroups.map((group) => group.id)).toEqual(['group-b']);

		const closedGroup = reduceWorkbenchTabs(state, {
			groupId: 'group-a',
			kind: 'close-tab-group',
		});
		expect(tabIds(closedGroup)).not.toContain('group-first');
		expect(closedGroup.tabGroups.map((group) => group.id)).toEqual(['group-b']);

		const empty = reduceWorkbenchTabs(state, {
			kind: 'close-tabs',
			tabIds: tabIds(state),
		});
		expect(empty).toEqual({ activeTabId: undefined, tabGroups: [], tabs: [] });
	});

	it('pins and unpins at the shared boundary while removing group membership', () => {
		const pinned = reduceWorkbenchTabs(createState(), {
			isPinned: true,
			kind: 'set-tab-pinned',
			tabId: 'group-first',
		});
		expect(tabIds(pinned).slice(0, 3)).toEqual(['pinned-first', 'pinned-second', 'group-first']);
		expect(pinned.tabs[2]?.groupId).toBeUndefined();
		expect(pinned.tabs[2]?.isPinned).toBe(true);

		const unpinned = reduceWorkbenchTabs(pinned, {
			isPinned: false,
			kind: 'set-tab-pinned',
			tabId: 'pinned-first',
		});
		expect(tabIds(unpinned).slice(0, 3)).toEqual(['pinned-second', 'group-first', 'pinned-first']);
	});

	it('creates, assigns, removes, and dissolves groups without ghost metadata', () => {
		const created = reduceWorkbenchTabs(createState(), {
			group: { color: 'green', id: 'group-c', label: 'Group C' },
			kind: 'create-tab-group',
			tabIds: ['plain', 'group-b-only'],
		});
		expect(created.tabGroups.map((group) => group.id)).toEqual(['group-a', 'group-c']);
		expect(created.tabs.filter((tab) => tab.groupId === 'group-c').map((tab) => tab.id)).toEqual([
			'plain',
			'group-b-only',
		]);

		const assigned = reduceWorkbenchTabs(created, {
			groupId: 'group-c',
			kind: 'assign-tab-group',
			tabId: 'group-first',
		});
		expect(assigned.tabs.filter((tab) => tab.groupId === 'group-c').map((tab) => tab.id)).toEqual([
			'group-first',
			'plain',
			'group-b-only',
		]);

		const removed = reduceWorkbenchTabs(assigned, {
			groupId: undefined,
			kind: 'assign-tab-group',
			tabId: 'group-second',
		});
		expect(removed.tabs.find((tab) => tab.id === 'group-second')?.groupId).toBeUndefined();
		expect(removed.tabGroups.map((group) => group.id)).not.toContain('group-a');

		const dissolved = reduceWorkbenchTabs(removed, {
			groupId: 'group-c',
			kind: 'ungroup-tab-group',
		});
		expect(dissolved.tabGroups).toEqual([]);
		expect(dissolved.tabs.every((tab) => tab.groupId === undefined)).toBe(true);
	});

	it('assigns adjacent tabs at the approached edge of a target group', () => {
		const state = createWorkbenchTabsState({
			tabGroups: [
				{ color: 'red', id: 'source-group', label: 'Source Group' },
				{ color: 'blue', id: 'target-group', label: 'Target Group' },
			],
			tabs: [
				{ groupId: 'source-group', id: 'left', label: 'Left' },
				{ groupId: 'target-group', id: 'target-first', label: 'Target First' },
				{ groupId: 'target-group', id: 'target-second', label: 'Target Second' },
				{ id: 'right', label: 'Right' },
			],
		});
		const fromLeft = reduceWorkbenchTabs(state, {
			groupId: 'target-group',
			kind: 'assign-tab-group',
			tabId: 'left',
		});
		expect(
			fromLeft.tabs.filter((tab) => tab.groupId === 'target-group').map((tab) => tab.id),
		).toEqual(['left', 'target-first', 'target-second']);
		expect(fromLeft.tabGroups.map((group) => group.id)).toEqual(['target-group']);

		const fromRight = reduceWorkbenchTabs(state, {
			groupId: 'target-group',
			kind: 'assign-tab-group',
			tabId: 'right',
		});
		expect(
			fromRight.tabs.filter((tab) => tab.groupId === 'target-group').map((tab) => tab.id),
		).toEqual(['target-first', 'target-second', 'right']);
	});

	it('unpins tabs before adding them to existing or new groups', () => {
		const state = createState();
		const assigned = reduceWorkbenchTabs(state, {
			groupId: 'group-a',
			kind: 'assign-tab-group',
			tabId: 'pinned-first',
		});
		expect(assigned.tabs.find((tab) => tab.id === 'pinned-first')).toMatchObject({
			groupId: 'group-a',
		});
		expect(assigned.tabs.find((tab) => tab.id === 'pinned-first')?.isPinned).toBeUndefined();

		const created = reduceWorkbenchTabs(state, {
			group: { color: 'green', id: 'group-c', label: 'Group C' },
			kind: 'create-tab-group',
			tabIds: ['pinned-first'],
		});
		expect(created.tabs.find((tab) => tab.id === 'pinned-first')).toMatchObject({
			groupId: 'group-c',
		});
		expect(created.tabs.find((tab) => tab.id === 'pinned-first')?.isPinned).toBeUndefined();
	});

	it('renames and recolors tabs and groups while rejecting blank or unchanged updates', () => {
		const actions: readonly WorkbenchTabsAction[] = [
			{ kind: 'rename-tab', label: ' Renamed ', tabId: 'plain' },
			{ color: 'purple', kind: 'set-tab-color', tabId: 'plain' },
			{
				groupId: 'group-a',
				kind: 'update-tab-group',
				update: { color: 'orange', label: ' Renamed Group ' },
			},
		];
		const state = actions.reduce(reduceWorkbenchTabs, createState());

		expect(state.tabs.find((tab) => tab.id === 'plain')).toMatchObject({
			color: 'purple',
			label: 'Renamed',
		});
		expect(state.tabGroups.find((group) => group.id === 'group-a')).toMatchObject({
			color: 'orange',
			label: 'Renamed Group',
		});
		expect(reduceWorkbenchTabs(state, { kind: 'rename-tab', label: ' ', tabId: 'plain' })).toBe(
			state,
		);
	});

	it('collapses groups without changing the active member', () => {
		const collapsed = reduceWorkbenchTabs(createState(), {
			groupId: 'group-a',
			isCollapsed: true,
			kind: 'set-tab-group-collapsed',
		});
		expect(collapsed.activeTabId).toBe('group-first');

		const collapsedAtEnd = reduceWorkbenchTabs(
			{ ...createState(), activeTabId: 'group-b-only' },
			{
				groupId: 'group-b',
				isCollapsed: true,
				kind: 'set-tab-group-collapsed',
			},
		);
		expect(collapsedAtEnd.activeTabId).toBe('group-b-only');

		const onlyGroup = createWorkbenchTabsState({
			activeTabId: 'first',
			tabGroups: [{ color: 'blue', id: 'group-a', label: 'Group A' }],
			tabs: [
				{ groupId: 'group-a', id: 'first', label: 'First' },
				{ groupId: 'group-a', id: 'second', label: 'Second' },
			],
		});
		expect(
			reduceWorkbenchTabs(onlyGroup, {
				groupId: 'group-a',
				isCollapsed: true,
				kind: 'set-tab-group-collapsed',
			}).activeTabId,
		).toBe('first');
	});

	it('moves tabs symmetrically and rejects both directions across the pinned boundary', () => {
		const state = createState();
		const toEnd = reduceWorkbenchTabs(state, {
			groupId: undefined,
			kind: 'move-tab',
			tabId: 'plain',
			targetIndex: 5,
		});
		expect(tabIds(toEnd)).toEqual([
			'pinned-first',
			'pinned-second',
			'group-first',
			'group-second',
			'group-b-only',
			'plain',
		]);
		const toStart = reduceWorkbenchTabs(toEnd, {
			groupId: undefined,
			kind: 'move-tab',
			tabId: 'plain',
			targetIndex: 2,
		});
		expect(tabIds(toStart).slice(0, 3)).toEqual(['pinned-first', 'pinned-second', 'plain']);

		expect(
			reduceWorkbenchTabs(state, {
				groupId: undefined,
				kind: 'move-tab',
				tabId: 'plain',
				targetIndex: 0,
			}),
		).toBe(state);
		expect(
			reduceWorkbenchTabs(state, {
				groupId: undefined,
				kind: 'move-tab',
				tabId: 'pinned-first',
				targetIndex: 2,
			}),
		).toBe(state);
	});

	it('moves expanded and collapsed group blocks in both directions while preserving members', () => {
		const state = createState();
		const toEnd = reduceWorkbenchTabs(state, {
			groupId: 'group-a',
			kind: 'move-tab-group',
			targetIndex: 4,
		});
		expect(tabIds(toEnd)).toEqual([
			'pinned-first',
			'pinned-second',
			'plain',
			'group-b-only',
			'group-first',
			'group-second',
		]);

		const collapsed = reduceWorkbenchTabs(toEnd, {
			groupId: 'group-a',
			isCollapsed: true,
			kind: 'set-tab-group-collapsed',
		});
		const toStart = reduceWorkbenchTabs(collapsed, {
			groupId: 'group-a',
			kind: 'move-tab-group',
			targetIndex: 2,
		});
		expect(tabIds(toStart).slice(2, 4)).toEqual(['group-first', 'group-second']);
		expect(toStart.tabGroups.find((group) => group.id === 'group-a')?.isCollapsed).toBe(true);
	});

	it('preserves object identity for missing, invalid, and unchanged actions', () => {
		const state = createState();
		const actions: readonly WorkbenchTabsAction[] = [
			{ kind: 'activate-tab', tabId: state.activeTabId ?? '' },
			{ kind: 'close-tabs', tabIds: ['missing'] },
			{ groupId: 'missing', kind: 'close-tab-group' },
			{ isPinned: false, kind: 'set-tab-pinned', tabId: 'plain' },
			{ groupId: 'missing', kind: 'move-tab-group', targetIndex: 2 },
			{ groupId: undefined, kind: 'move-tab', tabId: 'plain', targetIndex: -1 },
		];

		for (const action of actions) expect(reduceWorkbenchTabs(state, action)).toBe(state);
	});
});
