import { requireDefined } from '../../utils/requireDefined';
import { TabStripOrientation, VerticalTabStripCollapseState } from '../../types/tab-strip-types';
import { describe, expect, it } from 'vitest';

import {
	calculateUnpinnedTabContainerLayout,
	calculateUnpinnedTabContainerMinimumSize,
} from '../unpinned-tab-container-view-layout';
const child = (id: string, width = 100, height = 30) => ({
	id,
	preferredSize: {
		width,
		height,
	},
	minimumSize: {
		width: 32,
		height,
	},
});
describe('UnpinnedTabContainerViewLayout', () => {
	it('places horizontal collection children in order', () => {
		const layout = calculateUnpinnedTabContainerLayout({
			orientation: TabStripOrientation.Horizontal,
			sizeBounds: {
				width: null,
				height: 40,
			},
			children: [child('a', 80), child('b', 120)],
		});
		expect(layout.children.map((item) => item.bounds)).toEqual([
			{
				x: 0,
				y: 0,
				width: 80,
				height: 40,
			},
			{
				x: 80,
				y: 0,
				width: 120,
				height: 40,
			},
		]);
		expect(layout.hostSize).toEqual({
			width: 200,
			height: 40,
		});
	});
	it('insets vertical tabs while letting collapsed group views draw into the gutter', () => {
		const layout = calculateUnpinnedTabContainerLayout({
			orientation: TabStripOrientation.Vertical,
			sizeBounds: {
				width: 240,
				height: null,
			},
			collapseState: VerticalTabStripCollapseState.Collapsed,
			children: [
				child('plain'),
				{
					...child('group', 100, 70),
					group: true,
				},
			],
		});
		expect(requireDefined(layout.children[0]).bounds).toEqual({
			x: 12,
			y: 0,
			width: 216,
			height: 30,
		});
		expect(requireDefined(layout.children[1]).bounds).toEqual({
			x: 0,
			y: 32,
			width: 228,
			height: 70,
		});
		expect(layout.hostSize).toEqual({
			width: 228,
			height: 102,
		});
	});
	it('keeps the host tall enough for a floating drag and clamps it to bounded height', () => {
		const layout = calculateUnpinnedTabContainerLayout({
			orientation: TabStripOrientation.Vertical,
			sizeBounds: {
				width: 240,
				height: 100,
			},
			children: [child('a')],
			handlingDrag: true,
			draggingViewsBottom: 160,
		});
		expect(layout.hostSize.height).toBe(100);
	});
	it('matches row-and-a-half vertical minimum sizing', () => {
		expect(
			calculateUnpinnedTabContainerMinimumSize(TabStripOrientation.Vertical, [
				child('a'),
				child('b'),
			]),
		).toEqual({
			width: 32,
			height: 47,
		});
		expect(
			calculateUnpinnedTabContainerMinimumSize(TabStripOrientation.Horizontal, [
				child('a'),
				child('b'),
			]),
		).toEqual({
			width: 64,
			height: 35,
		});
	});
});
