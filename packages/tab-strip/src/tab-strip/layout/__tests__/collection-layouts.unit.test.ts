import { requireDefined } from '../../utils/requireDefined';
import { TabStripOrientation, VerticalTabStripCollapseState } from '../../types/tab-strip-types';
import { describe, expect, it } from 'vitest';
import {
	calculateTabGroupViewLayout,
	calculateTabGroupViewMinimumSize,
} from '../tab-group-view-layout';
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
				height: 41,
			},
			children: [child('a', 80), child('b', 120)],
		});
		expect(layout.children.map((item) => item.bounds)).toEqual([
			{
				x: 0,
				y: 0,
				width: 80,
				height: 41,
			},
			{
				x: 80,
				y: 0,
				width: 120,
				height: 41,
			},
		]);
		expect(layout.hostSize).toEqual({
			width: 200,
			height: 41,
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
describe('TabGroupViewLayout', () => {
	it('lays horizontal header and visible children sequentially', () => {
		const layout = calculateTabGroupViewLayout({
			orientation: TabStripOrientation.Horizontal,
			sizeBounds: {
				width: null,
				height: 41,
			},
			headerId: 'header',
			headerPreferredSize: {
				width: 48,
				height: 26,
			},
			children: [
				child('a', 100),
				{
					...child('hidden', 80),
					visible: false,
				},
				child('b', 120),
			],
		});
		expect(layout.children.map((item) => item.bounds.x)).toEqual([0, 0, 48, 148, 148]);
		expect(layout.hostSize).toEqual({
			width: 268,
			height: 41,
		});
	});
	it('shrinks a collapsed horizontal group to its header', () => {
		const input = {
			orientation: TabStripOrientation.Horizontal,
			sizeBounds: {
				width: null,
				height: 41,
			},
			headerId: 'header',
			headerPreferredSize: {
				width: 48,
				height: 26,
			},
			children: [child('a'), child('b')],
			groupCollapsed: true,
		} as const;
		expect(calculateTabGroupViewLayout(input).hostSize.width).toBe(48);
		expect(calculateTabGroupViewMinimumSize(input).width).toBe(48);
	});
	it('lays an expanded vertical group with inset tabs and a full group line', () => {
		const layout = calculateTabGroupViewLayout({
			orientation: TabStripOrientation.Vertical,
			sizeBounds: {
				width: 240,
				height: null,
			},
			headerId: 'header',
			headerPreferredSize: {
				width: 80,
				height: 26,
			},
			children: [child('a'), child('b')],
			collapseState: VerticalTabStripCollapseState.Expanded,
		});
		expect(requireDefined(layout.children[0]).bounds).toEqual({
			x: 0,
			y: 4,
			width: 240,
			height: 26,
		});
		expect(requireDefined(layout.children[1]).bounds).toEqual({
			x: 10,
			y: 36,
			width: 230,
			height: 30,
		});
		expect(requireDefined(layout.children[2]).bounds).toEqual({
			x: 10,
			y: 68,
			width: 230,
			height: 30,
		});
		expect(requireDefined(layout.children[3]).bounds).toEqual({
			x: 4,
			y: 36,
			width: 2,
			height: 62,
		});
		expect(layout.hostSize).toEqual({
			width: 240,
			height: 100,
		});
	});
	it('uses the compact header-only height when the group is collapsed', () => {
		const layout = calculateTabGroupViewLayout({
			orientation: TabStripOrientation.Vertical,
			sizeBounds: {
				width: 72,
				height: null,
			},
			headerId: 'header',
			headerPreferredSize: {
				width: 60,
				height: 26,
			},
			children: [],
			collapseState: VerticalTabStripCollapseState.Collapsed,
			groupCollapsed: true,
		});
		expect(requireDefined(layout.children[0]).bounds.x).toBe(12);
		expect(requireDefined(layout.children.at(-1)).bounds.x).toBe(6);
		expect(layout.hostSize).toEqual({
			width: 60,
			height: 34,
		});
	});
});
