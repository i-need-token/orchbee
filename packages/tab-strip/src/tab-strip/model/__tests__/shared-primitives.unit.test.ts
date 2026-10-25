import { describe, expect, it } from 'vitest';
import { TabStripOrientation } from '../../types/tab-strip-types';
import { calculateRoundedScrollbarThumb } from '../../components/tab-strip-view/components/vertical-contents/components/rounded-scrollbar/utils/rounded-scrollbar';
import {
	getComboButtonFlatEdges,
	getFlatEdgeButtonCornerRadii,
	TabStripFlatEdge,
} from '../../components/tab-strip-view/components/vertical-contents/components/flat-edge-button/utils/tab-strip-flat-edge-button';

describe('RoundedScrollBar geometry', () => {
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
