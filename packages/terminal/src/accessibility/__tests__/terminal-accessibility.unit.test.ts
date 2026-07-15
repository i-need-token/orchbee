import { describe, expect, it } from 'vitest';
import {
	AccessibleTextModel,
	buildAccessibleRow,
	createAccessibilityAnnouncement,
	detectAccessibilityDeletion,
	normalizeAccessibleRows,
} from '../terminal-accessibility';

describe('normalizeAccessibleRows', () => {
	it('preserves row order while removing invisible terminal padding', () => {
		expect(normalizeAccessibleRows(['prompt   ', '\u0000value\u0000', '   '])).toEqual([
			'prompt',
			' value',
			' ',
		]);
	});

	it('does not trim meaningful leading spaces', () => {
		expect(normalizeAccessibleRows(['  indented  '])).toEqual(['  indented']);
	});
});

describe('AccessibleTextModel', () => {
	it('maps UTF-16 offsets to terminal columns across wide graphemes', () => {
		const first = buildAccessibleRow(
			[
				{ text: 'A', width: 1 },
				{ text: '你', width: 2 },
				{ text: '', width: 0 },
				{ text: '😀', width: 2 },
				{ text: '', width: 0 },
				{ text: ' ', width: 1 },
			],
			6,
		);
		const second = buildAccessibleRow(
			[
				{ text: 'x', width: 1 },
				{ text: 'y', width: 1 },
				{ text: 'z', width: 1 },
			],
			3,
		);
		const model = new AccessibleTextModel();
		model.update([first, second]);

		expect(model.getText()).toBe('A你😀\nxyz');
		expect(model.getVisibleRange()).toEqual({ start: 0, end: 8 });
		expect(model.positionToOffset({ row: 0, column: 2 })).toBe(1);
		expect(model.positionToOffset({ row: 0, column: 3 })).toBe(2);
		expect(model.positionToOffset({ row: 1, column: 0 })).toBe(5);
		expect(model.offsetToPosition(3, 'backward')).toEqual({ row: 0, column: 3 });
		expect(model.offsetToPosition(3, 'forward')).toEqual({ row: 0, column: 5 });
	});

	it('converts inclusive terminal selections to DOM offset ranges', () => {
		const row = buildAccessibleRow(
			[
				{ text: 'A', width: 1 },
				{ text: '你', width: 2 },
				{ text: '', width: 0 },
				{ text: '😀', width: 2 },
				{ text: '', width: 0 },
			],
			5,
		);
		const model = new AccessibleTextModel();
		model.update([row]);

		const offsets = model.terminalRangeToOffsets({
			start: { row: 0, column: 1 },
			end: { row: 0, column: 3 },
		});
		expect(offsets).toEqual({ start: 1, end: 4 });
		expect(model.offsetsToTextRange(offsets)).toEqual({
			start: { row: 0, column: 1 },
			end: { row: 0, column: 5 },
		});
	});
});

describe('createAccessibilityAnnouncement', () => {
	it('skips unchanged old cursor content and empty lines', () => {
		expect(createAccessibilityAnnouncement(['pwd', ' ', 'Users/deonnel'], ['pwd', ' ', ' '])).toBe(
			'Users/deonnel',
		);
	});

	it('includes a changed old cursor line and subsequent output', () => {
		expect(createAccessibilityAnnouncement(['hello', 'world'], ['echo hello', ' '])).toBe(
			'hello\nworld',
		);
	});

	it('skips only the unchanged occurrence of repeated cursor text', () => {
		expect(createAccessibilityAnnouncement(['prompt', 'prompt'], ['prompt', ' '])).toBe('prompt');
	});
});

describe('detectAccessibilityDeletion', () => {
	it.each([
		{
			name: 'backspace single character',
			before: 'hello',
			after: 'hell',
			oldCursor: 5,
			newCursor: 4,
			expected: 'o',
		},
		{
			name: 'backspace multiple characters',
			before: 'hello world',
			after: 'hello ',
			oldCursor: 11,
			newCursor: 6,
			expected: 'world',
		},
		{
			name: 'backspace to beginning of line',
			before: 'hello',
			after: '',
			oldCursor: 5,
			newCursor: 0,
			expected: 'hello',
		},
		{
			name: 'cursor moved left without content changing',
			before: 'hello',
			after: 'hello',
			oldCursor: 5,
			newCursor: 4,
			expected: null,
		},
		{
			name: 'forward delete single character',
			before: 'hello',
			after: 'hllo',
			oldCursor: 1,
			newCursor: 1,
			expected: 'e',
		},
		{
			name: 'kill to end of line',
			before: 'hello world',
			after: 'hello',
			oldCursor: 5,
			newCursor: 5,
			expected: 'world',
		},
		{
			name: 'no content or cursor change',
			before: 'hello',
			after: 'hello',
			oldCursor: 3,
			newCursor: 3,
			expected: null,
		},
		{
			name: 'cursor moved right',
			before: 'hello',
			after: 'hello',
			oldCursor: 3,
			newCursor: 4,
			expected: null,
		},
		{
			name: 'empty rows',
			before: '',
			after: '',
			oldCursor: 0,
			newCursor: 0,
			expected: null,
		},
	])('$name', ({ before, after, oldCursor, newCursor, expected }) => {
		expect(
			detectAccessibilityDeletion(
				accessibleAsciiRow(before),
				accessibleAsciiRow(after),
				oldCursor,
				newCursor,
			),
		).toBe(expected);
	});

	it('detects deletion of a surrogate-pair emoji as one terminal cell', () => {
		const before = buildAccessibleRow(
			[
				{ text: 'a', width: 1 },
				{ text: '😀', width: 2 },
				{ text: '', width: 0 },
				{ text: 'b', width: 1 },
			],
			4,
		);
		const after = accessibleAsciiRow('ab');
		expect(detectAccessibilityDeletion(before, after, 3, 1)).toBe('😀');
	});
});

function accessibleAsciiRow(text: string) {
	return buildAccessibleRow(
		[...text].map((character) => ({ text: character, width: 1 })),
		[...text].length,
	);
}
