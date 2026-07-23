import { requireDefined } from '../../utils/requireDefined';
import { TabStripOrientation, VerticalTabStripCollapseState } from '../../types/tab-strip-types';
import { describe, expect, it } from 'vitest';
import {
	calculatePinnedTabContainerLayout,
	calculatePinnedTabContainerMinimumSize,
	getPinnedTabContainerDragAxes,
} from '../pinned-tab-container-layout';
import {
	calculatePinnedContainerMainAxisSize,
	calculateTabStripViewLayout,
} from '../tab-strip-view-layout';
describe('TabStripViewLayout', () => {
	it('caps the pinned region at half unless unpinned tabs leave room', () => {
		expect(calculatePinnedContainerMainAxisSize(500, 500, 600, 64)).toBe(300);
		expect(calculatePinnedContainerMainAxisSize(500, 100, 600, 64)).toBe(500);
		expect(calculatePinnedContainerMainAxisSize(20, 500, 600, 64)).toBe(64);
	});
	it('lays horizontal pinned and unpinned scroll regions side by side', () => {
		const layout = calculateTabStripViewLayout({
			orientation: TabStripOrientation.Horizontal,
			sizeBounds: {
				width: 600,
				height: 41,
			},
			pinnedPreferred: {
				width: 500,
				height: 41,
			},
			pinnedMinimum: {
				width: 64,
				height: 41,
			},
			unpinnedPreferred: {
				width: 500,
				height: 41,
			},
			unpinnedMinimum: {
				width: 32,
				height: 41,
			},
		});
		expect(layout.pinnedBounds).toEqual({
			x: 0,
			y: 0,
			width: 300,
			height: 41,
		});
		expect(layout.unpinnedBounds).toEqual({
			x: 300,
			y: 0,
			width: 300,
			height: 41,
		});
		expect(layout.separatorVisible).toBe(false);
	});
	it('shows the pinned separator only for a collapsed vertical strip', () => {
		const expanded = calculateTabStripViewLayout({
			orientation: TabStripOrientation.Vertical,
			sizeBounds: {
				width: 240,
				height: 500,
			},
			pinnedPreferred: {
				width: 228,
				height: 90,
			},
			pinnedMinimum: {
				width: 32,
				height: 49,
			},
			unpinnedPreferred: {
				width: 240,
				height: 600,
			},
			unpinnedMinimum: {
				width: 32,
				height: 47,
			},
			collapsed: false,
		});
		expect(expanded.separatorVisible).toBe(false);
		const collapsed = calculateTabStripViewLayout({
			orientation: TabStripOrientation.Vertical,
			sizeBounds: {
				width: 72,
				height: 500,
			},
			pinnedPreferred: {
				width: 60,
				height: 90,
			},
			pinnedMinimum: {
				width: 32,
				height: 49,
			},
			unpinnedPreferred: {
				width: 72,
				height: 600,
			},
			unpinnedMinimum: {
				width: 32,
				height: 47,
			},
			separatorPreferredHeight: 1,
			collapsed: true,
		});
		expect(collapsed.separatorVisible).toBe(true);
		expect(collapsed.pinnedBounds.x).toBe(12);
		expect(collapsed.separatorBounds).toEqual({
			x: 12,
			y: 98,
			width: 48,
			height: 1,
		});
		expect(collapsed.unpinnedBounds.y).toBe(107);
		expect(collapsed.hostSize).toEqual({
			width: 72,
			height: 500,
		});
	});
});
describe('PinnedTabContainerView vertical grid', () => {
	const pinnedChildren = Array.from(
		{
			length: 5,
		},
		(_, index) => ({
			id: `tab-${index}`,
			preferredSize: {
				width: 32,
				height: 32,
			},
		}),
	);
	it('wraps expanded pinned tabs and distributes leftover row width', () => {
		const layout = calculatePinnedTabContainerLayout({
			sizeBounds: {
				width: 120,
				height: null,
			},
			children: pinnedChildren,
			collapseState: VerticalTabStripCollapseState.Expanded,
		});
		expect(layout.children.slice(0, 3).map((child) => child.bounds)).toEqual([
			{
				x: 0,
				y: 0,
				width: 33,
				height: 32,
			},
			{
				x: 37,
				y: 0,
				width: 33,
				height: 32,
			},
			{
				x: 74,
				y: 0,
				width: 33,
				height: 32,
			},
		]);
		expect(requireDefined(layout.children[3]).bounds.y).toBe(36);
		expect(layout.hostSize).toEqual({
			width: 120,
			height: 68,
		});
	});
	it('uses one child per row when collapsed with expand-on-hover enabled', () => {
		const layout = calculatePinnedTabContainerLayout({
			sizeBounds: {
				width: 72,
				height: null,
			},
			children: pinnedChildren.slice(0, 2),
			collapseState: VerticalTabStripCollapseState.Collapsed,
			expandOnHoverEnabled: true,
		});
		expect(layout.children.map((child) => child.bounds.y)).toEqual([0, 36]);
		expect(layout.children.map((child) => child.bounds.width)).toEqual([60, 60]);
	});
	it('calculates the Chrome row-and-a-half minimum height', () => {
		expect(
			calculatePinnedTabContainerMinimumSize(3, VerticalTabStripCollapseState.Collapsed),
		).toEqual({
			width: 32,
			height: 49,
		});
		expect(
			calculatePinnedTabContainerMinimumSize(3, VerticalTabStripCollapseState.Expanded),
		).toEqual({
			width: 32,
			height: 34,
		});
	});
	it('uses only the vertical drag axis in the fully collapsed state', () => {
		expect(getPinnedTabContainerDragAxes(VerticalTabStripCollapseState.Expanded)).toBe('both');
		expect(getPinnedTabContainerDragAxes(VerticalTabStripCollapseState.Collapsing)).toBe('both');
		expect(getPinnedTabContainerDragAxes(VerticalTabStripCollapseState.Collapsed)).toBe(
			'vertical-only',
		);
		expect(getPinnedTabContainerDragAxes(VerticalTabStripCollapseState.Expanding)).toBe('both');
	});
});
