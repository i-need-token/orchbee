import { requireDefined } from '../../common';
import { beforeEach, describe, expect, it } from 'vitest';
import { rectRight } from '../../geometry';
import { TabPinned, type TabGroupId } from '../../model';
import {
	TabStripLayoutHelper,
	type TabSlotViewData,
	type TabStripLayoutController,
} from '../TabStripLayoutHelper';
import type { TabSizeInfo } from '../tab-strip-layout-types';
const TAB_SIZE: TabSizeInfo = {
	pinnedTabWidth: 64,
	minActiveWidth: 56,
	minInactiveWidth: 32,
	standardWidth: 256,
};
function tab(id: string, group: TabGroupId | null = null): TabSlotViewData {
	return {
		id,
		type: 'tab',
		group,
		sizeInfo: TAB_SIZE,
	};
}
function header(group: TabGroupId, width = 48): TabSlotViewData {
	const sizeInfo = {
		pinnedTabWidth: width,
		minActiveWidth: width,
		minInactiveWidth: width,
		standardWidth: width,
	};
	return {
		id: `header-${group}`,
		type: 'group-header',
		group,
		sizeInfo,
	};
}
class Controller implements TabStripLayoutController {
	public activeIndex: number | null = 0;
	public focusedGroup: TabGroupId | null = null;
	public readonly collapsedGroups = new Set<TabGroupId>();
	public getActiveIndex() {
		return this.activeIndex;
	}
	public getFocusedGroup() {
		return this.focusedGroup;
	}
	public isGroupCollapsed(group: TabGroupId) {
		return this.collapsedGroups.has(group);
	}
}
describe('TabStripLayoutHelper', () => {
	let controller: Controller;
	let helper: TabStripLayoutHelper;
	beforeEach(() => {
		controller = new Controller();
		helper = new TabStripLayoutHelper(controller);
	});
	it('maintains model order while group headers share the first tab index', () => {
		helper.insertTabAt(0, tab('a'), TabPinned.Unpinned);
		helper.insertTabAt(1, tab('b', 'work'), TabPinned.Unpinned);
		helper.insertTabAt(2, tab('c', 'work'), TabPinned.Unpinned);
		helper.insertGroupHeader('work', header('work'));
		expect(helper.getTabSlotViews().map((view) => view.id)).toEqual(['a', 'header-work', 'b', 'c']);
		helper.insertTabAt(1, tab('new', 'work'), TabPinned.Unpinned);
		expect(helper.getTabSlotViews().map((view) => view.id)).toEqual([
			'a',
			'header-work',
			'new',
			'b',
			'c',
		]);
	});
	it('retains a closing slot but excludes it from model-index lookup', () => {
		helper.insertTabAt(0, tab('a'), TabPinned.Unpinned);
		helper.insertTabAt(1, tab('b'), TabPinned.Unpinned);
		helper.insertTabAt(2, tab('c'), TabPinned.Unpinned);
		helper.markTabAsClosing(1, 'b');
		helper.insertTabAt(1, tab('replacement'), TabPinned.Unpinned);
		expect(helper.getTabs().map((view) => view.id)).toEqual(['a', 'b', 'replacement', 'c']);
		const layout = helper.updateIdealBounds(1000);
		expect(layout.tabBounds.has('b')).toBe(false);
		expect(layout.slotBounds[1]?.width).toBe(18);
		expect(layout.tabBounds.get('replacement')?.x).toBe(238);
	});
	it('keeps a collapsed group header visible and collapses only its tabs', () => {
		helper.insertTabAt(0, tab('a', 'work'), TabPinned.Unpinned);
		helper.insertTabAt(1, tab('b', 'work'), TabPinned.Unpinned);
		helper.insertTabAt(2, tab('c'), TabPinned.Unpinned);
		helper.insertGroupHeader('work', header('work', 48));
		controller.activeIndex = 2;
		controller.collapsedGroups.add('work');
		const layout = helper.updateIdealBounds(1000);
		expect(layout.slotBounds.map((bounds) => bounds.width)).toEqual([48, 18, 18, 256]);
		expect(layout.groupHeaderBounds.get('work')?.width).toBe(48);
		expect(layout.tabBounds.get('a')?.width).toBe(18);
		expect(layout.tabBounds.get('c')?.width).toBe(256);
	});
	it('focuses one group by collapsing every other slot, including headers', () => {
		helper.insertTabAt(0, tab('a', 'one'), TabPinned.Unpinned);
		helper.insertTabAt(1, tab('b', 'two'), TabPinned.Unpinned);
		helper.insertGroupHeader('one', header('one', 48));
		helper.insertGroupHeader('two', header('two', 48));
		controller.focusedGroup = 'two';
		controller.activeIndex = 1;
		expect(helper.updateIdealBounds(1000).slotBounds.map((bounds) => bounds.width)).toEqual([
			18, 18, 48, 256,
		]);
	});
	it('moves a grouped tab and restores its header before the first group member', () => {
		helper.insertTabAt(0, tab('a', 'work'), TabPinned.Unpinned);
		helper.insertTabAt(1, tab('b', 'work'), TabPinned.Unpinned);
		helper.insertTabAt(2, tab('c'), TabPinned.Unpinned);
		helper.insertGroupHeader('work', header('work'));
		helper.moveTab('work', 0, 1);
		expect(helper.getTabSlotViews().map((view) => view.id)).toEqual(['header-work', 'b', 'a', 'c']);
	});
	it('reports Chromium minimum/preferred widths and pinned count', () => {
		helper.insertTabAt(0, tab('pinned'), TabPinned.Pinned);
		helper.insertTabAt(1, tab('active'), TabPinned.Unpinned);
		helper.insertTabAt(2, tab('inactive'), TabPinned.Unpinned);
		expect(helper.getPinnedTabCount()).toBe(1);
		// The active tab is pinned, so both unpinned tabs use the inactive minimum.
		expect(helper.calculateMinimumWidth()).toBe(64 + (32 - 18) * 2);
		expect(helper.calculatePreferredWidth()).toBe(64 + (256 - 18) * 2);
		expect(rectRight(requireDefined(helper.updateIdealBounds(400).slotBounds.at(-1)))).toBe(400);
	});
});
