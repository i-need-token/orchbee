import { describe, expect, it } from 'vitest';
import {
	ACTIVE_TAB_Z_VALUE_BIT,
	GROUP_VIEW_Z_VALUE_BIT,
	calculateTabContainerZValue,
	getTabStyleZValue,
} from '../z-orderable-tab-container-element';

describe('ZOrderableTabContainerElement', () => {
	it('reserves the source bit ranges for active tabs and group views', () => {
		expect(ACTIVE_TAB_Z_VALUE_BIT).toBe(16);
		expect(GROUP_VIEW_Z_VALUE_BIT).toBe(8);
	});

	it('encodes selected, hovered, and animated tab style state', () => {
		expect(getTabStyleZValue({})).toBe(0);
		expect(getTabStyleZValue({ hoverAnimationValue: 0.5 })).toBe(0.5);
		expect(getTabStyleZValue({ selected: true, hovering: true, hoverAnimationValue: 0.5 })).toBe(
			6.5,
		);
	});

	it('paints group underlines above every tab and group header state', () => {
		expect(calculateTabContainerZValue({ kind: 'group-highlight' })).toBe(0);
		expect(calculateTabContainerZValue({ kind: 'tab' })).toBe(1);
		expect(
			calculateTabContainerZValue({ kind: 'tab', hovering: true, hoverAnimationValue: 1 }),
		).toBe(4);
		expect(calculateTabContainerZValue({ kind: 'group-header' })).toBe(8);
		expect(calculateTabContainerZValue({ kind: 'group-header', active: true })).toBe(24);
		expect(calculateTabContainerZValue({ kind: 'group-underline' })).toBe(25);
		expect(calculateTabContainerZValue({ kind: 'tab', active: true, selected: true })).toBe(21);
		expect(
			calculateTabContainerZValue({
				kind: 'tab',
				active: true,
				selected: true,
				hovering: true,
				hoverAnimationValue: 1,
			}),
		).toBe(24);
	});
});
