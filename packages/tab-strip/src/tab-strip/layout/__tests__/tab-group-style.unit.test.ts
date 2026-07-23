import { describe, expect, it } from 'vitest';
import {
	calculateHorizontalTabGroupUnderlineBounds,
	calculateHorizontalTabGroupHeaderLayout,
	getTabGroupHighlightPath,
	getTabGroupTitleVisualCenteringOffset,
	mergeDraggedGroupUnderlineBounds,
	shouldHideTabGroupUnderline,
} from '../tab-group-style';

describe('TabGroupStyle', () => {
	it('lays out an empty 20px chip with Chromium overlap margins', () => {
		expect(
			calculateHorizontalTabGroupHeaderLayout({ title: '', measuredTitleWidth: 0 }),
		).toMatchObject({ chipWidth: 20, chipHeight: 20, chipX: 16, chipY: 10, desiredWidth: 52 });
	});

	it('caps title text to half a standard tab excluding overlap', () => {
		expect(
			calculateHorizontalTabGroupHeaderLayout({ title: 'Long group', measuredTitleWidth: 500 }),
		).toMatchObject({ textMaxWidth: 119, textWidth: 119, chipWidth: 131, desiredWidth: 163 });
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

	it('hides the underline only when the group has no tab between the same header endpoints', () => {
		expect(shouldHideTabGroupUnderline('header', 'header')).toBe(true);
		expect(shouldHideTabGroupUnderline('header', 'tab')).toBe(false);
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

	it('builds the source tab-shaped drag highlight path', () => {
		expect(getTabGroupHighlightPath(200, 41)).toBe(
			'M 0 41 A 12 12 0 0 0 12 29 L 12 18 A 12 12 0 0 1 24 6 L 176 6 A 12 12 0 0 1 188 18 L 188 29 A 12 12 0 0 0 200 41 Z',
		);
	});

	it('extends a dragged underline toward the normal underline without passing dragged views', () => {
		expect(
			mergeDraggedGroupUnderlineBounds({
				normal: { x: 28, width: 154 },
				dragged: { x: 100, width: 82 },
				leadingDraggedView: { x: 100, width: 100 },
				trailingDraggedView: { x: 100, width: 100 },
			}),
		).toEqual({ x: 100, width: 82 });
		expect(
			mergeDraggedGroupUnderlineBounds({
				normal: { x: 28, width: 104 },
				dragged: { x: 45, width: 82 },
				leadingDraggedView: { x: 45, width: 100 },
				trailingDraggedView: { x: 45, width: 100 },
			}),
		).toEqual({ x: 45, width: 87 });
		expect(
			mergeDraggedGroupUnderlineBounds({
				normal: { x: 28, width: 104 },
				dragged: { x: 23, width: 82 },
				leadingDraggedView: { x: 5, width: 100 },
				trailingDraggedView: { x: 5, width: 100 },
			}),
		).toEqual({ x: 23, width: 82 });
		expect(
			mergeDraggedGroupUnderlineBounds({
				normal: { x: -32, width: 164 },
				dragged: { x: -32, width: 82 },
				leadingDraggedView: { x: -50, width: 100 },
				trailingDraggedView: { x: -50, width: 100 },
			}),
		).toEqual({ x: -32, width: 82 });
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
