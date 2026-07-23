import { describe, expect, it } from 'vitest';
import {
	TabStripOrientation,
	VerticalTabStripCollapseState,
} from '../../../../../../../../types/tab-strip-types';
import {
	getVerticalBottomContainerPresentation,
	isPositionInVerticalBottomContainerCaption,
} from '../vertical-tab-strip-bottom-container';

describe('VerticalTabStripBottomContainer', () => {
	it('switches styling at the start of collapse and keeps expansion styling until fully expanded', () => {
		expect(
			getVerticalBottomContainerPresentation(VerticalTabStripCollapseState.Expanded),
		).toMatchObject({
			collapsedStyling: false,
			orientation: TabStripOrientation.Horizontal,
			crossAxisAlignment: 'start',
			maximumFlexSize: 'unbounded',
			insets: { top: 5, right: 14, bottom: 5, left: 14 },
		});
		for (const state of [
			VerticalTabStripCollapseState.Collapsing,
			VerticalTabStripCollapseState.Collapsed,
			VerticalTabStripCollapseState.Expanding,
		]) {
			expect(getVerticalBottomContainerPresentation(state)).toMatchObject({
				collapsedStyling: true,
				orientation: TabStripOrientation.Vertical,
				crossAxisAlignment: 'stretch',
				maximumFlexSize: 'preferred',
				insets: { top: 5, right: 6, bottom: 5, left: 6 },
				buttonSize: 32,
				iconSize: 20,
			});
		}
	});

	it('treats the whole visible button bounds, including inset space, as non-caption', () => {
		const children = [{ visible: true, bounds: { x: 0, y: 10, width: 72, height: 32 } }];
		expect(isPositionInVerticalBottomContainerCaption({ x: 36, y: 11 }, children)).toBe(false);
		expect(isPositionInVerticalBottomContainerCaption({ x: 36, y: 9 }, children)).toBe(true);
	});
});
