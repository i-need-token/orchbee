import { describe, expect, it } from 'vitest';
import {
	buildPreeditState,
	projectPreeditLayout,
	shouldSuppressComposingControlInput,
} from '../preedit';

describe('buildPreeditState', () => {
	it('maps a UTF-16 selection to wide terminal cell offsets', () => {
		expect(buildPreeditState('A你好', { start: 1, end: 2 }, width)).toEqual({
			cells: [
				{ text: 'A', width: 1 },
				{ text: '你', width: 2 },
				{ text: '好', width: 2 },
			],
			width: 5,
			selection: { start: 1, end: 3 },
		});
	});

	it('keeps a caret after a surrogate-pair grapheme on a valid cell boundary', () => {
		expect(buildPreeditState('🎉a', { start: 2, end: 2 }, width).selection).toEqual({
			start: 2,
			end: 2,
		});
	});

	it('defaults the marked-text caret to the end', () => {
		expect(buildPreeditState('个人', undefined, width).selection).toEqual({ start: 4, end: 4 });
	});

	it('keeps combining marks and zero-width joiners in their grapheme cell', () => {
		const state = buildPreeditState('e\u0301👩‍💻', undefined, (codepoint) => {
			if (codepoint === 0x301 || codepoint === 0x200d) return 0;
			return codepoint > 0x7f ? 2 : 1;
		});
		expect(state.cells).toEqual([
			{ text: 'e\u0301', width: 1 },
			{ text: '👩‍💻', width: 2 },
		]);
		expect(state.width).toBe(3);
	});
});

describe('projectPreeditLayout', () => {
	it('wraps narrow and wide cells row-major without splitting a wide cell', () => {
		expect(
			projectPreeditLayout(
				[
					{ text: 'A', width: 1 },
					{ text: '你', width: 2 },
					{ text: 'B', width: 1 },
					{ text: 'C', width: 1 },
				],
				{ start: 1, end: 4 },
				{ column: 3, row: 0 },
				5,
				3,
			),
		).toEqual({
			caret: { column: 3, row: 1 },
			clipped: false,
			rows: [
				{
					cells: [{ text: 'A', width: 1 }],
					end: 4,
					row: 0,
					selectionEnd: 1,
					selectionStart: 1,
					start: 3,
					width: 1,
				},
				{
					cells: [
						{ text: '你', width: 2 },
						{ text: 'B', width: 1 },
						{ text: 'C', width: 1 },
					],
					end: 4,
					row: 1,
					selectionEnd: 3,
					selectionStart: 0,
					start: 0,
					width: 4,
				},
			],
		});
	});

	it('clips after the final visible row and anchors the caret at the visible boundary', () => {
		expect(
			projectPreeditLayout(
				[
					{ text: 'A', width: 1 },
					{ text: 'B', width: 1 },
				],
				{ start: 2, end: 2 },
				{ column: 4, row: 1 },
				5,
				2,
			),
		).toEqual({
			caret: { column: 4, row: 1 },
			clipped: true,
			rows: [
				{
					cells: [{ text: 'A', width: 1 }],
					end: 5,
					row: 1,
					selectionEnd: 1,
					selectionStart: 1,
					start: 4,
					width: 1,
				},
			],
		});
	});
});

describe('shouldSuppressComposingControlInput', () => {
	it.each([
		['\u0008', true],
		['\u001f', true],
		['\u007f', false],
		[' ', false],
		['h', false],
		['', false],
		['\u0009x', false],
		['\u0009\u0009', false],
	])('classifies %j while composing', (text, expected) => {
		expect(shouldSuppressComposingControlInput(text, true)).toBe(expected);
	});

	it('preserves control input outside composition and missing text', () => {
		expect(shouldSuppressComposingControlInput('\u0008', false)).toBe(false);
		expect(shouldSuppressComposingControlInput(null, true)).toBe(false);
	});
});

function width(codepoint: number): number {
	return codepoint > 0x7f ? 2 : 1;
}
