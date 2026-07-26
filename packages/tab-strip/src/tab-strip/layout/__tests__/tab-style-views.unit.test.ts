import { describe, expect, it } from 'vitest';
import {
	getHorizontalTabSeparatorBounds,
	getHorizontalTabSeparatorTargets,
	TAB_CONTENT_HORIZONTAL_INSET,
	HORIZONTAL_TAB_TOP_INSET,
} from '../tab-style-views';
import { LAYOUT_CONSTANTS } from '../layout-constants';

describe('TabStyleViews horizontal geometry', () => {
	it('uses the full 40px surface inside the even-pixel strip', () => {
		expect(LAYOUT_CONSTANTS.tabStripHeight).toBe(40);
		expect(HORIZONTAL_TAB_TOP_INSET).toBe(0);
		expect(LAYOUT_CONSTANTS.tabStripHeight - HORIZONTAL_TAB_TOP_INSET).toBe(40);
		expect(TAB_CONTENT_HORIZONTAL_INSET).toBe(18);
	});

	it('uses the 1 by 20 separator geometry', () => {
		expect(getHorizontalTabSeparatorBounds(100)).toEqual({
			leading: { x: 7, y: 10, width: 1, height: 20 },
			trailing: { x: 92, y: 10, width: 1, height: 20 },
		});
	});
});

describe('TabStyleViews separator targets', () => {
	const plain = { active: false, selected: false, hovered: false, group: null };

	it('shows only separators that have no visible neighboring shape', () => {
		expect(
			getHorizontalTabSeparatorTargets({ tab: plain, previous: null, next: plain, tabCount: 2 }),
		).toEqual({ leading: 0, trailing: 1 });
		expect(
			getHorizontalTabSeparatorTargets({
				tab: plain,
				previous: { ...plain, hovered: true },
				next: null,
				tabCount: 2,
			}),
		).toEqual({ leading: 0, trailing: 1 });
	});

	it('assigns one separator to a shared boundary between plain tabs', () => {
		const first = getHorizontalTabSeparatorTargets({
			tab: plain,
			previous: null,
			next: plain,
			tabCount: 2,
		});
		const second = getHorizontalTabSeparatorTargets({
			tab: plain,
			previous: plain,
			next: null,
			tabCount: 2,
		});

		expect(first.trailing).toBe(1);
		expect(second.leading).toBe(0);
		expect(first.trailing + second.leading).toBe(1);
	});

	it('hides the leading separator after a group header', () => {
		expect(
			getHorizontalTabSeparatorTargets({
				tab: { ...plain, group: 'g' },
				previous: { ...plain, group: null },
				next: null,
				tabCount: 2,
			}).leading,
		).toBe(0);
	});

	it('removes all separators in a decluttered strip of at least 20 tabs', () => {
		expect(
			getHorizontalTabSeparatorTargets({
				tab: plain,
				previous: plain,
				next: plain,
				tabCount: 20,
				declutterEnabled: true,
			}),
		).toEqual({ leading: 0, trailing: 0 });
	});
});
