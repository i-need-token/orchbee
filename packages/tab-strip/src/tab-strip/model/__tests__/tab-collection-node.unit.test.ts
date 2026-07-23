import { requireDefined } from '../../utils/requireDefined';
import { TabStripOrientation } from '../../types/tab-strip-types';
import { describe, expect, it, vi } from 'vitest';
import { TabModel, type TabData } from '../tab-model';
import {
	buildTabCollectionDescriptor,
	MemoryTabCollectionViewAdapter,
	RootTabCollectionNode,
	TabCollectionNode,
	TabCollectionNodeType,
	type TabCollectionMemoryView,
} from '../tab-collection-node';
function tab(id: string, update: Partial<TabData> = {}): TabData {
	return {
		id,
		title: id,
		pinned: false,
		group: null,
		...update,
	};
}
describe('TabCollectionNode source port', () => {
	it('builds root, pin partition, group, and tab nodes', () => {
		const model = new TabModel({
			tabs: [
				tab('p1', { pinned: true }),
				tab('p2', { pinned: true }),
				tab('a', { group: 'work' }),
				tab('b', { group: 'work' }),
				tab('loose'),
			],
			groups: [
				{
					id: 'work',
					title: 'Work',
					color: 'blue',
					collapsed: false,
				},
			],
		});
		const descriptor = buildTabCollectionDescriptor(model.getSnapshot());
		expect(descriptor.children?.map((child) => child.type)).toEqual([
			TabCollectionNodeType.Pinned,
			TabCollectionNodeType.Unpinned,
		]);
		expect(descriptor.children?.[0]?.children?.map((child) => child.handle)).toEqual([
			'tab:p1',
			'tab:p2',
		]);
		expect(descriptor.children?.[1]?.children?.[0]).toMatchObject({
			handle: 'group:work',
			type: TabCollectionNodeType.Group,
		});
		expect(descriptor.children?.[1]?.children?.[0]?.children?.map((child) => child.handle)).toEqual(
			['tab:a', 'tab:b'],
		);
		expect(descriptor.children?.[1]?.children?.[1]?.handle).toBe('tab:loose');
	});
	it('initializes recursively, finds parents, and maintains focus order', () => {
		const root = new TabCollectionNode(
			{
				handle: 'root',
				type: TabCollectionNodeType.Unpinned,
				children: [
					{
						handle: 'a',
						type: TabCollectionNodeType.Tab,
					},
					{
						handle: 'b',
						type: TabCollectionNodeType.Tab,
					},
					{
						handle: 'c',
						type: TabCollectionNodeType.Tab,
					},
				],
			},
			TabStripOrientation.Vertical,
			new MemoryTabCollectionViewAdapter(),
		);
		root.initialize();
		expect(root.getNodeForHandle('b')?.handle).toBe('b');
		expect(root.getParentNodeForHandle('b')).toBe(root);
		expect(root.getDirectChildren().map((view) => view.handle)).toEqual(['a', 'b', 'c']);
		expect(root.getDirectChildren()[1]?.focusPrevious?.handle).toBe('a');
		expect(root.getDirectChildren()[1]?.focusNext?.handle).toBe('c');
		expect(root.moveChild('a', 2)).toBe(true);
		expect(root.children.map((child) => child.handle)).toEqual(['b', 'c', 'a']);
		expect(root.getDirectChildren()[2]?.zOrder).toBeGreaterThan(0);
		expect(root.getDirectChildren()[1]?.focusNext?.handle).toBe('a');
	});
	it('reparents a live view and restores its focus', () => {
		const root = new TabCollectionNode(
			{
				handle: 'root',
				type: TabCollectionNodeType.TabStrip,
				children: [
					{
						handle: 'left',
						type: TabCollectionNodeType.Pinned,
						children: [
							{
								handle: 'tab',
								type: TabCollectionNodeType.Tab,
							},
						],
					},
					{
						handle: 'right',
						type: TabCollectionNodeType.Unpinned,
					},
				],
			},
			TabStripOrientation.Vertical,
			new MemoryTabCollectionViewAdapter(),
		);
		root.initialize();
		const left = requireDefined(root.getNodeForHandle('left'));
		const right = requireDefined(root.getNodeForHandle('right'));
		const node = requireDefined(root.getNodeForHandle('tab'));
		(node.view as TabCollectionMemoryView).focused = true;
		const moved = TabCollectionNode.moveChildAcrossParents('tab', 0, left, right);
		expect(moved).toBe(node);
		expect(left.children).toHaveLength(0);
		expect(right.children[0]).toBe(node);
		expect((node.view as TabCollectionMemoryView).parent).toBe(right.view);
		expect((node.view as TabCollectionMemoryView).focused).toBe(true);
	});
	it('fires will-destroy before removing the associated view', () => {
		const log: string[] = [];
		class Adapter extends MemoryTabCollectionViewAdapter {
			public override removeChild(
				parent: TabCollectionMemoryView,
				child: TabCollectionMemoryView,
			): void {
				log.push('remove-view');
				super.removeChild(parent, child);
			}
		}
		const root = new TabCollectionNode(
			{
				handle: 'root',
				type: TabCollectionNodeType.Unpinned,
				children: [
					{
						handle: 'tab',
						type: TabCollectionNodeType.Tab,
					},
				],
			},
			TabStripOrientation.Horizontal,
			new Adapter(),
		);
		root.initialize();
		root.getNodeForHandle('tab')?.registerWillDestroyCallback(() => log.push('destroy-node'));
		root.removeChild('tab', true);
		expect(log).toEqual(['destroy-node', 'remove-view']);
	});
});
describe('RootTabCollectionNode model observation', () => {
	it('preserves nodes on reorder, recreates them across pin partitions, and publishes callbacks', () => {
		const model = new TabModel({
			tabs: [tab('a', { group: 'work' }), tab('b', { group: 'work' }), tab('loose')],
			groups: [
				{
					id: 'work',
					title: 'Work',
					color: 'blue',
					collapsed: false,
				},
			],
			activeTabId: 'loose',
		});
		const tree = new RootTabCollectionNode(
			model,
			TabStripOrientation.Vertical,
			new MemoryTabCollectionViewAdapter(),
		);
		tree.init();
		const added = vi.fn<() => void>();
		const removed = vi.fn<() => void>();
		const moved = vi.fn<() => void>();
		const active = vi.fn<() => void>();
		tree.registerOnChildrenAddedCallback(added);
		tree.registerOnChildRemovedCallback(removed);
		tree.registerOnChildMovedCallback(moved);
		tree.registerOnActiveTabChangedCallback(active);
		const originalLoose = tree.getNodeForHandle('tab:loose');
		model.moveTabFirst('loose');
		expect(tree.getNodeForHandle('tab:loose')).toBe(originalLoose);
		expect(moved).toHaveBeenCalled();
		const aSelection = vi.fn<() => void>();
		const bSelection = vi.fn<() => void>();
		tree.getNodeForHandle('tab:a')?.registerTabSelectionChangedCallback(aSelection);
		tree.getNodeForHandle('tab:b')?.registerTabSelectionChangedCallback(bSelection);
		model.selectTab('a');
		expect(active).toHaveBeenLastCalledWith('a');
		expect(aSelection).toHaveBeenCalled();
		expect(bSelection).not.toHaveBeenCalled();
		const destroyLoose = vi.fn<() => void>();
		originalLoose?.registerWillDestroyCallback(destroyLoose);
		model.setTabsPinned(['loose'], true);
		expect(destroyLoose).toHaveBeenCalledOnce();
		expect(tree.getNodeForHandle('tab:loose')).not.toBe(originalLoose);
		const newId = model.addTab(tab('new'));
		expect(added).toHaveBeenCalled();
		model.closeTab(newId);
		expect(removed).toHaveBeenCalled();
		tree.reset();
	});
	it('detaches the root view on reset and rebuilds from the latest model on re-init', () => {
		const model = new TabModel({
			tabs: [tab('a')],
		});
		const tree = new RootTabCollectionNode(
			model,
			TabStripOrientation.Horizontal,
			new MemoryTabCollectionViewAdapter(),
		);
		const firstView = tree.init();
		tree.reset();
		expect(tree.root.view).toBeNull();
		const addedId = model.addTab(tab('b'));
		const secondView = tree.init();
		expect(secondView).not.toBe(firstView);
		expect(tree.getNodeForHandle('tab:a')).not.toBeNull();
		expect(tree.getNodeForHandle(`tab:${addedId}`)).not.toBeNull();
		tree.reset();
	});
});
