import { describe, expect, it } from 'vitest';
import {
	TabStripOrientation,
	VerticalTabStripCollapseState,
} from '../../../../../../../../types/tab-strip-types';
import { getVerticalBottomContainerPresentation } from '../getVerticalBottomContainerPresentation';

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
});
