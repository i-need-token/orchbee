import { requireDefined } from '../../../../../../../../utils/requireDefined';
import { TabStripOrientation } from '../../../../../../../../types/tab-strip-types';
import { describe, expect, it } from 'vitest';
import {
	calculateVerticalTopContainerLayout,
	getVerticalTopContainerPreferredWidth,
	willVerticalTopContainerWrap,
} from '../vertical-tab-strip-top-container-layout';
describe('VerticalTabStripTopContainer layout', () => {
	it('right-aligns the combo and shares a baseline with the collapse button', () => {
		const layout = calculateVerticalTopContainerLayout({
			availableWidth: 240,
			toolbarHeight: 1,
			captionButtonWidth: 1,
			comboEndVisible: true,
		});
		const collapse = requireDefined(layout.children.find((child) => child.id === 'collapse'));
		const combo = requireDefined(layout.children.find((child) => child.id === 'combo'));
		expect(layout.orientation).toBe(TabStripOrientation.Horizontal);
		expect(combo.bounds.x + combo.bounds.width).toBe(240);
		expect(collapse.bounds.x).toBe(1);
		expect(collapse.bounds.y + collapse.bounds.height / 2).toBe(
			combo.bounds.y + combo.bounds.height / 2,
		);
	});
	it('moves controls below a full-width caption exclusion zone', () => {
		const layout = calculateVerticalTopContainerLayout({
			availableWidth: 240,
			toolbarHeight: 50,
			captionButtonWidth: 240,
			comboEndVisible: true,
		});
		const collapse = requireDefined(layout.children.find((child) => child.id === 'collapse'));
		const combo = requireDefined(layout.children.find((child) => child.id === 'combo'));
		expect(layout.wrappedDueToOverflow).toBe(true);
		expect(layout.hostSize.height).toBe(84);
		expect(collapse.bounds).toMatchObject({
			x: 0,
			y: 48,
		});
		expect(combo.bounds).toMatchObject({
			x: 212,
			y: 50,
		});
	});
	it('stacks visible buttons and fills the collapsed combo width minus two-pixel gutters', () => {
		const layout = calculateVerticalTopContainerLayout({
			availableWidth: 72,
			collapsed: true,
			comboEndVisible: true,
		});
		const collapse = requireDefined(layout.children.find((child) => child.id === 'collapse'));
		const combo = requireDefined(layout.children.find((child) => child.id === 'combo'));
		expect(layout.orientation).toBe(TabStripOrientation.Vertical);
		expect(collapse.bounds).toEqual({
			x: 0,
			y: 7,
			width: 32,
			height: 32,
		});
		expect(combo.bounds).toEqual({
			x: 2,
			y: 47,
			width: 68,
			height: 28,
		});
		expect(layout.hostSize.height).toBe(83);
	});
	it('uses preferred-width and caption hit-test semantics', () => {
		const input = {
			availableWidth: 65,
			captionButtonWidth: 1,
			comboEndVisible: true,
		};
		expect(getVerticalTopContainerPreferredWidth(input)).toBe(64);
		expect(willVerticalTopContainerWrap(input)).toBe(true);
		const layout = calculateVerticalTopContainerLayout({
			...input,
			availableWidth: 240,
		});
		expect(
			layout.isPositionInCaption({
				x: 100,
				y: 8,
			}),
		).toBe(true);
		expect(
			layout.isPositionInCaption({
				x: 4,
				y: 16,
			}),
		).toBe(false);
	});
});
