import { describe, expect, it } from 'vitest';
import { calculateTabContainerZValue } from '../calculateTabContainerZValue';

describe('ZOrderableTabContainerElement', () => {
	it('encodes selected, hovered, and animated tab style state', () => {
		expect(calculateTabContainerZValue({ kind: 'tab' })).toBe(1);
		expect(calculateTabContainerZValue({ kind: 'tab', hoverAnimationValue: 0.5 })).toBe(1.5);
		expect(
			calculateTabContainerZValue({
				kind: 'tab',
				selected: true,
				hovering: true,
				hoverAnimationValue: 0.5,
			}),
		).toBe(7.5);
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
