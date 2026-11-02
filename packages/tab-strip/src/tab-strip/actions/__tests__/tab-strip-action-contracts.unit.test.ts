import { describe, expect, it } from 'vitest';
import registry from '../../../../../../schemas/workbench/v1/workbench-action-identities-v1.json';

import {
	TAB_STRIP_ACTION_IDS,
	booleanValueInput,
	tabArrangementInput,
	tabCloseInput,
	tabCreateInput,
	tabGroupCollapsedInput,
	tabGroupMoveInput,
	tabGroupShiftInput,
	tabGroupUpdateInput,
	tabIdsInput,
	tabMoveInput,
	tabPinnedInput,
	tabSelectionInput,
} from '../tab-strip-action-contracts';

describe('tab strip action contracts', () => {
	it('owns one stable id for each menu semantic', () => {
		expect(new Set(Object.values(TAB_STRIP_ACTION_IDS)).size).toBe(
			Object.keys(TAB_STRIP_ACTION_IDS).length,
		);
		expect(TAB_STRIP_ACTION_IDS.closeTabs).toBe('tabs.tab.close');
		expect(TAB_STRIP_ACTION_IDS.moveTabsToGroup).toBe('tabs.tab.move-to-group');
		expect(TAB_STRIP_ACTION_IDS.selectTab).toBe('tabs.tab.select');
	});

	it('owns every TabStrip identity in the shared cross-Client registry', () => {
		expect(registry.contractVersion).toBe(1);
		const owned = new Set<string>(Object.values(TAB_STRIP_ACTION_IDS));
		expect(registry.catalogs.tabs.filter((id) => !owned.has(id))).toEqual([]);
	});

	it('parses create, selection, movement, grouping, pinning, and group update inputs', () => {
		expect(
			tabCreateInput.parse({
				groupId: null,
				title: 'Terminal 1',
			}),
		).toEqual({
			data: {
				groupId: null,
				title: 'Terminal 1',
			},
			success: true,
		});
		expect(tabIdsInput.parse({ tabIds: ['tab-1', 'tab-2'] })).toMatchObject({
			success: true,
		});
		expect(tabSelectionInput.parse({ mode: 'extend', tabId: 'tab-2' })).toMatchObject({
			success: true,
		});
		expect(tabMoveInput.parse({ direction: 'first', tabId: 'tab-2' })).toMatchObject({
			success: true,
		});
		expect(tabCloseInput.parse({ closeSource: 'mouse', tabIds: ['tab-1'] })).toMatchObject({
			success: true,
		});
		expect(tabGroupMoveInput.parse({ groupId: 'group-1', tabIds: ['tab-1'] })).toMatchObject({
			success: true,
		});
		expect(tabGroupCollapsedInput.parse({ collapsed: true, groupId: 'group-1' })).toMatchObject({
			success: true,
		});
		expect(tabGroupShiftInput.parse({ direction: -1, groupId: 'group-1' })).toMatchObject({
			success: true,
		});
		expect(tabPinnedInput.parse({ pinned: true, tabIds: ['tab-1'] })).toMatchObject({
			success: true,
		});
		expect(booleanValueInput.parse({ value: true })).toMatchObject({ success: true });
		const arrangement = {
			activeTabId: 'tab-1',
			groups: [
				{
					collapsed: false,
					color: 'blue',
					id: 'group-1',
					title: 'Group 1',
				},
			],
			mostRecentGroupId: 'group-1',
			selectedTabIds: ['tab-1'],
			selectionAnchorId: 'tab-1',
			tabs: [{ group: 'group-1', id: 'tab-1' }],
		};
		expect(
			tabArrangementInput.parse({ before: arrangement, state: arrangement, tabIds: ['tab-1'] }),
		).toMatchObject({ success: true });
		expect(tabGroupUpdateInput.parse({ color: 'blue', groupId: 'group-1' })).toMatchObject({
			success: true,
		});
	});

	it('rejects malformed arrays, indices, colors, and empty updates', () => {
		expect(tabIdsInput.parse({ tabIds: ['tab-1', 2] })).toMatchObject({ success: false });
		expect(tabSelectionInput.parse({ mode: 'append', tabId: 'tab-1' })).toMatchObject({
			success: false,
		});
		expect(tabMoveInput.parse({ direction: 'up', tabId: 'tab-1' })).toMatchObject({
			success: false,
		});
		expect(tabGroupCollapsedInput.parse({ collapsed: 'yes', groupId: 'group-1' })).toMatchObject({
			success: false,
		});
		expect(tabGroupShiftInput.parse({ direction: 2, groupId: 'group-1' })).toMatchObject({
			success: false,
		});
		expect(
			tabCreateInput.parse({
				groupId: null,
				index: -1,
				title: 'Tab',
			}),
		).toMatchObject({ success: false });
		expect(tabGroupUpdateInput.parse({ color: 'black', groupId: 'group-1' })).toMatchObject({
			success: false,
		});
		expect(tabGroupUpdateInput.parse({ groupId: 'group-1' })).toMatchObject({
			success: false,
		});
		expect(booleanValueInput.parse({ value: 'true' })).toMatchObject({ success: false });
		expect(tabArrangementInput.parse({ state: { tabs: 'tab-1' } })).toMatchObject({
			success: false,
		});
	});

	it.each(['tabs', 'groups'] as const)(
		'rejects duplicate identities in arrangement %s',
		(field) => {
			const state = {
				activeTabId: 'tab-1',
				groups: [{ id: 'group-1', title: 'Group', color: 'blue', collapsed: false }],
				mostRecentGroupId: 'group-1',
				selectedTabIds: ['tab-1'],
				selectionAnchorId: 'tab-1',
				tabs: [{ group: 'group-1', id: 'tab-1' }],
			};
			const input = { before: state, tabIds: ['tab-1'] };
			expect(tabArrangementInput.parse({ ...input, state })).toMatchObject({ success: true });
			expect(
				tabArrangementInput.parse({
					...input,
					state: { ...state, [field]: [...state[field], ...state[field]] },
				}),
			).toEqual({ success: false, code: 'expected-tab-arrangement-input' });
		},
	);

	it('returns detached canonical snapshots for mutable collection input', () => {
		const sourceTabIds = ['tab-1'];
		const parsedTabIds = tabIdsInput.parse({ tabIds: sourceTabIds });
		expect(parsedTabIds.success).toBe(true);
		if (!parsedTabIds.success) {
			throw new Error('Expected tab IDs to parse');
		}
		sourceTabIds.push('tab-2');
		expect(parsedTabIds.data.tabIds).toEqual(['tab-1']);

		const sourceSelectedTabIds = ['tab-1'];
		const sourceTab = { group: 'group-1', id: 'tab-1' };
		const sourceTabs = [sourceTab];
		const sourceGroup = {
			collapsed: false,
			color: 'blue',
			id: 'group-1',
			title: 'Group 1',
		};
		const sourceGroups = [sourceGroup];
		const sourceState = {
			activeTabId: 'tab-1',
			groups: sourceGroups,
			mostRecentGroupId: 'group-1',
			selectedTabIds: sourceSelectedTabIds,
			selectionAnchorId: 'tab-1',
			tabs: sourceTabs,
		};
		const parsedArrangement = tabArrangementInput.parse({
			before: sourceState,
			state: sourceState,
			tabIds: ['tab-1'],
		});
		expect(parsedArrangement.success).toBe(true);
		if (!parsedArrangement.success) {
			throw new Error('Expected tab arrangement to parse');
		}
		sourceSelectedTabIds.push('tab-2');
		sourceTab.id = 'tab-2';
		sourceGroup.title = 'Changed';
		expect(parsedArrangement.data.state).toMatchObject({
			groups: [{ id: 'group-1', title: 'Group 1' }],
			selectedTabIds: ['tab-1'],
			tabs: [{ id: 'tab-1' }],
		});
	});
});
