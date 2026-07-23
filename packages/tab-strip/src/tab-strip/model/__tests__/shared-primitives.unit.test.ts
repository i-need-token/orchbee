import { describe, expect, it, vi } from 'vitest';
import { TabStripOrientation } from '../../types/tab-strip-types';
import {
	calculateRoundedScrollbarThumb,
	roundedScrollbarThickness,
} from '../../components/tab-strip-view/components/vertical-contents/components/rounded-scrollbar/utils/rounded-scrollbar';
import {
	getComboButtonFlatEdges,
	getFlatEdgeButtonCornerRadii,
	getFlatEdgeButtonPreferredSize,
	TabStripFlatEdge,
} from '../../components/tab-strip-view/components/vertical-contents/components/flat-edge-button/utils/tab-strip-flat-edge-button';
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
