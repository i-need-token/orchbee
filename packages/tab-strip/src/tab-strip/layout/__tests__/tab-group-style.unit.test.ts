import { describe, expect, it } from 'vitest';
import {
	calculateHorizontalTabGroupUnderlineBounds,
	calculateHorizontalTabGroupHeaderLayout,
	getTabGroupTitleVisualCenteringOffset,
} from '../tab-group-style';

describe('TabGroupStyle', () => {
	it('lays out an empty 20px chip with balanced horizontal breathing room', () => {
		expect(
			calculateHorizontalTabGroupHeaderLayout({ title: '', measuredTitleWidth: 0 }),
		).toMatchObject({ chipWidth: 20, chipHeight: 20, chipX: 20, chipY: 8, desiredWidth: 60 });
	});

	it('caps title text to half a standard tab excluding overlap', () => {
		expect(
			calculateHorizontalTabGroupHeaderLayout({ title: 'Long group', measuredTitleWidth: 500 }),
		).toMatchObject({ textMaxWidth: 119, textWidth: 119, chipWidth: 131, desiredWidth: 171 });
	});

	it('shows group attention only when the group is collapsed', () => {
		const expanded = calculateHorizontalTabGroupHeaderLayout({
			title: 'Work',
			measuredTitleWidth: 30,
			attention: true,
			collapsed: false,
			tabCount: 2,
		});
		const collapsed = calculateHorizontalTabGroupHeaderLayout({
			title: 'Work',
			measuredTitleWidth: 30,
			attention: true,
			collapsed: true,
			tabCount: 2,
		});
		expect(expanded).toMatchObject({
			showAttentionIndicator: false,
			chipWidth: 42,
		});
		expect(collapsed).toMatchObject({
			showAttentionIndicator: true,
			chipWidth: 50,
		});
	});

	it('expands an empty attention chip to fit its glyph', () => {
		expect(
			calculateHorizontalTabGroupHeaderLayout({
				title: '',
				measuredTitleWidth: 0,
				attention: true,
				collapsed: true,
				tabCount: 1,
			}),
		).toMatchObject({ chipWidth: 20, showAttentionIndicator: true });
	});

	it('keeps the underline inside the trailing tab layout boundary', () => {
		expect(
			calculateHorizontalTabGroupUnderlineBounds({
				leadingHeaderX: 10,
				trailingTabX: 100,
				trailingTabWidth: 80,
			}),
		).toEqual({ x: 28, width: 134 });
	});

	it('applies the macOS single-color-emoji visual centering correction', () => {
		const offset = (title: string, rtl = false, isMac = true) =>
			getTabGroupTitleVisualCenteringOffset(title, { isMac, rtl });
		expect(offset('😊')).toBe(1);
		expect(offset('😊', true)).toBe(-1);
		expect(offset('👍🏽')).toBe(1);
		expect(offset('👨‍👩‍👧‍👦')).toBe(1);
		expect(offset('🇺🇸')).toBe(1);
		expect(offset('☺️')).toBe(0);
		expect(offset('♦️')).toBe(0);
		expect(offset('😊😊')).toBe(0);
		expect(offset('😊', false, false)).toBe(0);
	});
});
