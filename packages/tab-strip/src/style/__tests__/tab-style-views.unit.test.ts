import { describe, expect, it } from 'vitest';
import {
	getHorizontalTabPath,
	getHorizontalTabSeparatorBounds,
	getHorizontalTabSeparatorTargets,
	getHorizontalTabTopCornerRadius,
	isPointInHorizontalTabHitTest,
	TAB_CONTENT_BOTTOM_INSET,
	TAB_CONTENT_HORIZONTAL_INSET,
	TAB_CONTENT_TOP_INSET,
} from '../tab-style-views';

describe('TabStyleViews horizontal geometry', () => {
	it('ports the content insets from TabStyle and the toolbar overlap', () => {
		expect(TAB_CONTENT_HORIZONTAL_INSET).toBe(20);
		expect(TAB_CONTENT_TOP_INSET).toBe(12);
		expect(TAB_CONTENT_BOTTOM_INSET).toBe(13);
	});

	it('shrinks the top corner radius on narrow tabs', () => {
		expect(getHorizontalTabTopCornerRadius(256)).toBe(10);
		expect(getHorizontalTabTopCornerRadius(32)).toBe(4);
		expect(getHorizontalTabTopCornerRadius(18)).toBe(0);
	});

	it('uses a detached highlight and a toolbar-connected active path', () => {
		const highlight = getHorizontalTabPath({ width: 100, pathType: 'highlight' });
		const active = getHorizontalTabPath({ width: 100, pathType: 'active' });
		expect(highlight).toContain('M 22 6');
		expect(highlight).toContain('L 88 24');
		expect(highlight).toContain('78 34');
		expect(active).toContain('M 0 41');
		expect(active).toContain('L 100 41');
	});

	it('insets an active grouped fill by the group stroke thickness', () => {
		const active = getHorizontalTabPath({
			width: 100,
			pathType: 'active',
			strokeThickness: 2,
		});
		expect(active).toContain('L 2 39');
		expect(active).toContain('L 13 16');
		expect(active).toContain('A 9 9');
		expect(active).toContain('L 78 7');
		expect(active).toContain('L 87 28');
	});

	it('extends a condensed-frame hit target to the top with square corners', () => {
		const normal = getHorizontalTabPath({ width: 100, pathType: 'hit-test' });
		const condensed = getHorizontalTabPath({
			width: 100,
			pathType: 'hit-test',
			frameCondensed: true,
		});
		expect(normal).toContain('M 19 6');
		expect(condensed).toContain('M 9 0');
		expect(condensed).toContain('L 91 0');
		expect(condensed).toContain('L 91 41');
	});

	it('uses the hit-test squarcle instead of the rectangular tab bounds', () => {
		expect(isPointInHorizontalTabHitTest({ width: 100, x: 9, y: 6 })).toBe(false);
		expect(isPointInHorizontalTabHitTest({ width: 100, x: 22, y: 6 })).toBe(true);
		expect(isPointInHorizontalTabHitTest({ width: 100, x: 9, y: 40 })).toBe(true);
		expect(isPointInHorizontalTabHitTest({ width: 100, x: 9, y: 0, frameCondensed: true })).toBe(
			true,
		);
	});

	it('ports the 2 by 16 separator geometry', () => {
		expect(getHorizontalTabSeparatorBounds(100)).toEqual({
			leading: { x: 8, y: 12, width: 2, height: 16 },
			trailing: { x: 90, y: 12, width: 2, height: 16 },
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
