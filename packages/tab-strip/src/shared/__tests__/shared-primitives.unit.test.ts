import { describe, expect, it, vi } from 'vitest';
import { TabStripOrientation } from '../../common';
import {
	DropArrowDirection,
	DropArrowState,
	DropGroupInclusion,
	DropRelativeToIndex,
} from '../drop-arrow';
import { calculateRoundedScrollbarThumb, roundedScrollbarThickness } from '../rounded-scrollbar';
import {
	getComboButtonFlatEdges,
	getFlatEdgeButtonCornerRadii,
	getFlatEdgeButtonPreferredSize,
	TabStripFlatEdge,
} from '../tab-strip-flat-edge-button';
import { TabStripObserverList } from '../TabStripObserverList';

describe('RoundedScrollBar geometry', () => {
	it('matches Chromium thumb thickness with and without trailing margin', () => {
		expect(roundedScrollbarThickness()).toBe(9);
		expect(roundedScrollbarThickness(false)).toBe(5);
	});

	it('sizes and positions an overlay thumb proportionally', () => {
		expect(
			calculateRoundedScrollbarThumb({ viewportHeight: 100, contentHeight: 400, scrollTop: 100 }),
		).toEqual({
			visible: true,
			top: 25,
			height: 25,
			maximumScroll: 300,
		});
		expect(
			calculateRoundedScrollbarThumb({ viewportHeight: 100, contentHeight: 80, scrollTop: 0 })
				.visible,
		).toBe(false);
	});
});

describe('DropArrowState', () => {
	it('recomputes bounds and direction whenever the drop index changes', () => {
		const state = new DropArrowState(
			{
				index: 1,
				relativeToIndex: DropRelativeToIndex.InsertBeforeIndex,
				groupInclusion: DropGroupInclusion.DontIncludeInGroup,
			},
			(index) => ({
				bounds: { x: index.index * 20, y: 5, width: 20, height: 20 },
				direction: index.index > 1 ? DropArrowDirection.Left : DropArrowDirection.Down,
			}),
		);
		expect(state.placement).toEqual({
			bounds: { x: 20, y: 5, width: 20, height: 20 },
			direction: DropArrowDirection.Down,
		});
		expect(
			state.setIndex({
				index: 3,
				relativeToIndex: DropRelativeToIndex.ReplaceIndex,
				groupInclusion: DropGroupInclusion.IncludeInGroup,
			}),
		).toEqual({
			bounds: { x: 60, y: 5, width: 20, height: 20 },
			direction: DropArrowDirection.Left,
		});
	});
});

describe('TabStripObserverList', () => {
	it('dispatches add, move, and remove notifications and supports unsubscribe', () => {
		const added = vi.fn<() => void>();
		const moved = vi.fn<() => void>();
		const removed = vi.fn<() => void>();
		const list = new TabStripObserverList();
		const unsubscribe = list.add({ onTabAdded: added, onTabMoved: moved, onTabRemoved: removed });
		list.notifyTabAdded(2);
		list.notifyTabMoved(2, 4);
		list.notifyTabRemoved(4);
		expect(added).toHaveBeenCalledWith(2);
		expect(moved).toHaveBeenCalledWith(2, 4);
		expect(removed).toHaveBeenCalledWith(4);
		unsubscribe();
		list.notifyTabAdded(1);
		expect(added).toHaveBeenCalledTimes(1);
	});
});

describe('TabStripFlatEdgeButton geometry', () => {
	it('ports flat-edge radii and animated preferred-size math', () => {
		expect(getFlatEdgeButtonCornerRadii(TabStripFlatEdge.Right, 1)).toEqual({
			topLeft: 10,
			topRight: 2,
			bottomRight: 2,
			bottomLeft: 10,
		});
		expect(getFlatEdgeButtonCornerRadii(TabStripFlatEdge.Top, 0.5)).toEqual({
			topLeft: 6,
			topRight: 6,
			bottomRight: 10,
			bottomLeft: 10,
		});
		expect(getFlatEdgeButtonPreferredSize(0.5, TabStripOrientation.Horizontal)).toEqual({
			width: 14,
			height: 28,
		});
		expect(getFlatEdgeButtonPreferredSize(0.5, TabStripOrientation.Vertical)).toEqual({
			width: 28,
			height: 14,
		});
	});

	it('assigns combo-button shared edges for orientation and RTL', () => {
		expect(getComboButtonFlatEdges(TabStripOrientation.Vertical, true)).toEqual({
			start: TabStripFlatEdge.Bottom,
			end: TabStripFlatEdge.Top,
		});
		expect(getComboButtonFlatEdges(TabStripOrientation.Horizontal, true, true)).toEqual({
			start: TabStripFlatEdge.Left,
			end: TabStripFlatEdge.Right,
		});
		expect(getComboButtonFlatEdges(TabStripOrientation.Horizontal, false)).toEqual({
			start: TabStripFlatEdge.None,
			end: TabStripFlatEdge.None,
		});
	});
});
