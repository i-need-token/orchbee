import { describe, expect, it, vi } from 'vitest';

import { ScopedTabModel } from '../ScopedTabModel';
import { TabModel, type TabData, type TabGroupData } from '../tab-model';

function tab(
	id: string,
	options: Readonly<{
		group?: string | null;
		pinned?: boolean;
	}> = {},
): TabData {
	return {
		group: options.group ?? null,
		id,
		pinned: options.pinned ?? false,
		title: id,
	};
}

function ids(model: TabModel): string[] {
	return model.getSnapshot().tabs.map((candidate) => candidate.id);
}

describe('ScopedTabModel', () => {
	it('projects only assigned tabs and keeps a window-local active tab', () => {
		const source = new TabModel({
			activeTabId: 'hidden-a',
			tabs: [tab('visible-a'), tab('hidden-a'), tab('visible-b'), tab('hidden-b')],
		});
		const scoped = new ScopedTabModel(source, {
			visibleTabIds: ['visible-a', 'visible-b'],
		});

		expect(ids(scoped)).toEqual(['visible-a', 'visible-b']);
		expect(scoped.getSnapshot().activeTabId).toBe('visible-a');
		expect(source.getSnapshot().activeTabId).toBe('visible-a');

		scoped.setVisibleTabIds(['hidden-a', 'hidden-b']);

		expect(ids(scoped)).toEqual(['hidden-a', 'hidden-b']);
		expect(scoped.getSnapshot().activeTabId).toBe('hidden-a');
		expect(source.getSnapshot().activeTabId).toBe('hidden-a');
	});

	it('limits close-relative commands and selection ranges to the visible window', () => {
		const source = new TabModel({
			tabs: [tab('a'), tab('hidden-a'), tab('b'), tab('hidden-b'), tab('c')],
		});
		const scoped = new ScopedTabModel(source, { visibleTabIds: ['a', 'b', 'c'] });

		scoped.selectTab('a');
		scoped.extendSelectionTo('c');

		expect([...source.getSnapshot().selectedTabIds]).toEqual(['a', 'b', 'c']);

		scoped.closeTabsToRight('a');

		expect(ids(source)).toEqual(['a', 'hidden-a', 'hidden-b']);
		expect(ids(scoped)).toEqual(['a']);
	});

	it('preserves hidden tabs when closing other visible tabs', () => {
		const source = new TabModel({
			tabs: [
				tab('pinned', { pinned: true }),
				tab('hidden-a'),
				tab('keep'),
				tab('visible-close'),
				tab('hidden-b'),
			],
		});
		const scoped = new ScopedTabModel(source, {
			visibleTabIds: ['pinned', 'keep', 'visible-close'],
		});

		scoped.closeOtherTabs('keep');

		expect(ids(source)).toEqual(['pinned', 'hidden-a', 'keep', 'hidden-b']);
		expect(source.getSnapshot().activeTabId).toBe('keep');
	});

	it('translates local insertion and reorder indices without reordering hidden tabs', () => {
		const onTabCreated = vi.fn<(tabId: string) => void>();
		const source = new TabModel({
			tabs: [tab('a'), tab('hidden-a'), tab('b'), tab('hidden-b'), tab('c')],
		});
		const scoped = new ScopedTabModel(source, {
			onTabCreated,
			visibleTabIds: ['a', 'b', 'c'],
		});

		const createdId = scoped.addTab(
			{ title: 'created' },
			{ activate: false, id: 'created', index: 1 },
		);
		scoped.shiftTabRelative('a', 1);

		expect(createdId).toBe('created');
		expect(onTabCreated).toHaveBeenCalledWith('created');
		expect(ids(scoped)).toEqual(['created', 'a', 'b', 'c']);
		expect(ids(source).filter((id) => id.startsWith('hidden'))).toEqual(['hidden-a', 'hidden-b']);
		expect(ids(source)).toEqual(['hidden-a', 'created', 'a', 'b', 'hidden-b', 'c']);
	});

	it('keeps a fragmented group identity and applies group metadata globally', () => {
		const group: TabGroupData = {
			collapsed: false,
			color: 'blue',
			id: 'group-a',
			title: 'Group A',
		};
		const source = new TabModel({
			groups: [group],
			tabs: [
				tab('visible-a', { group: group.id }),
				tab('hidden', { group: group.id }),
				tab('visible-b', { group: group.id }),
			],
		});
		const scoped = new ScopedTabModel(source, {
			visibleTabIds: ['visible-a', 'visible-b'],
		});

		expect(scoped.getSnapshot().groups).toEqual([group]);
		scoped.updateGroup(group.id, { title: 'Renamed' });
		scoped.ungroup(group.id);

		expect(source.getSnapshot().tabs.every((candidate) => candidate.group === null)).toBe(true);
		expect(source.getSnapshot().groups).toEqual([]);
	});

	it('stops publishing after disposal', () => {
		const source = new TabModel({ tabs: [tab('a'), tab('b')] });
		const scoped = new ScopedTabModel(source, { visibleTabIds: ['a'] });
		const listener = vi.fn<() => void>();
		scoped.subscribe(listener);

		scoped.dispose();
		source.updateTab('a', { title: 'updated' });

		expect(listener).not.toHaveBeenCalled();
	});
});
