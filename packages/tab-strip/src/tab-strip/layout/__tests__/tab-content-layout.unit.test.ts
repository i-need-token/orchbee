import fc from 'fast-check';
import './tab-content-layout.examples';
import { describe, expect, it } from 'vitest';
import {
	calculateHorizontalTabChildVisibility,
	HORIZONTAL_TAB_MINIMUM_INACTIVE_WIDTH,
	TAB_AFTER_TITLE_PADDING,
	TAB_CLOSE_DESIGN_WIDTH,
	TAB_FAVICON_DESIGN_WIDTH,
} from '../tab-content-layout';
import { getHorizontalTabSeparatorTargets } from '../tab-style-views';

describe('compact horizontal tab layout properties', () => {
	it('keeps the active close control independent of hover at every renderable width', () => {
		fc.assert(
			fc.property(
				fc.integer({ min: HORIZONTAL_TAB_MINIMUM_INACTIVE_WIDTH, max: 320 }),
				fc.boolean(),
				(width, hovered) => {
					const contentsWidth = Math.max(0, width - 40);
					const visibility = calculateHorizontalTabChildVisibility({
						width,
						contentsWidth,
						active: true,
						pinned: false,
						hovered,
						declutterEnabled: true,
						hasFavicon: true,
						hasAlert: false,
					});

					expect(visibility.showClose).toBe(true);
					expect(visibility.showFavicon).toBe(
						contentsWidth >=
							TAB_CLOSE_DESIGN_WIDTH + TAB_AFTER_TITLE_PADDING + TAB_FAVICON_DESIGN_WIDTH,
					);
				},
			),
		);
	});

	it('changes separator visibility only at the Chromium declutter count boundary', () => {
		const plain = {
			active: false,
			selected: false,
			hovered: false,
			group: null,
		} as const;

		fc.assert(
			fc.property(fc.integer({ min: 1, max: 80 }), (tabCount) => {
				const targets = getHorizontalTabSeparatorTargets({
					tab: plain,
					previous: plain,
					next: plain,
					tabCount,
					declutterEnabled: true,
				});
				expect(targets).toEqual(
					tabCount >= 20 ? { leading: 0, trailing: 0 } : { leading: 0, trailing: 1 },
				);
			}),
		);
	});
});
