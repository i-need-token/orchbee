import { describe, expect, it } from 'vitest';

import {
	AccessibleTextModel,
	buildAccessibleRow,
	createAccessibilityAnnouncement,
	detectAccessibilityDeletion,
} from '../accessible-text-model';

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
		expect(model.positionToOffset({ row: 0, column: 2 })).toBe(1);
		expect(model.positionToOffset({ row: 0, column: 3 })).toBe(2);
		expect(model.positionToOffset({ row: 1, column: 0 })).toBe(5);
		expect(model.offsetToPosition(3, 'backward')).toEqual({ row: 0, column: 3 });
		expect(model.offsetToPosition(3, 'forward')).toEqual({ row: 0, column: 5 });
	});

	it('converts an exclusive DOM range without splitting a wide grapheme', () => {
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

		expect(model.offsetsToInclusiveTerminalRange({ start: 1, end: 3 })).toEqual({
			start: { row: 0, column: 1 },
			end: { row: 0, column: 3 },
		});
		expect(
			model.inclusiveTerminalRangeToOffsets({
				start: { row: 0, column: 1 },
				end: { row: 0, column: 3 },
			}),
		).toEqual({ start: 1, end: 4 });
	});

	it('maps an exclusive row-boundary end to the last visible cell before the newline', () => {
		const model = new AccessibleTextModel();
		model.update([
			buildAccessibleRow([{ text: 'A', width: 1 }], 1),
			buildAccessibleRow([{ text: 'B', width: 1 }], 1),
		]);

		expect(model.offsetsToInclusiveTerminalRange({ start: 0, end: 2 })).toEqual({
			start: { row: 0, column: 0 },
			end: { row: 0, column: 0 },
		});
	});

	it('announces only non-empty rows whose visible text changed', () => {
		expect(createAccessibilityAnnouncement(['pwd', ' ', 'Users/orchbee'], ['pwd', ' ', ' '])).toBe(
			'Users/orchbee',
		);
		expect(createAccessibilityAnnouncement(['hello', 'world'], ['echo hello', ' '])).toBe(
			'hello\nworld',
		);
		expect(createAccessibilityAnnouncement(['prompt', 'prompt'], ['prompt', ' '])).toBe('prompt');
	});

	it.each([
		{
			after: 'hell',
			before: 'hello',
			expected: 'o',
			name: 'backspace single character',
			newCursor: 4,
			oldCursor: 5,
		},
		{
			after: 'hello ',
			before: 'hello world',
			expected: 'world',
			name: 'backspace multiple characters',
			newCursor: 6,
			oldCursor: 11,
		},
		{
			after: '',
			before: 'hello',
			expected: 'hello',
			name: 'backspace to beginning of line',
			newCursor: 0,
			oldCursor: 5,
		},
		{
			after: 'hello',
			before: 'hello',
			expected: null,
			name: 'cursor moved left without content changing',
			newCursor: 4,
			oldCursor: 5,
		},
		{
			after: 'hllo',
			before: 'hello',
			expected: 'e',
			name: 'forward delete single character',
			newCursor: 1,
			oldCursor: 1,
		},
		{
			after: 'hello',
			before: 'hello world',
			expected: 'world',
			name: 'kill to end of line',
			newCursor: 5,
			oldCursor: 5,
		},
		{
			after: 'hello',
			before: 'hello',
			expected: null,
			name: 'no content or cursor change',
			newCursor: 3,
			oldCursor: 3,
		},
		{
			after: 'hello',
			before: 'hello',
			expected: null,
			name: 'cursor moved right',
			newCursor: 4,
			oldCursor: 3,
		},
		{
			after: '',
			before: '',
			expected: null,
			name: 'empty rows',
			newCursor: 0,
			oldCursor: 0,
		},
	])('$name', ({ after, before, expected, newCursor, oldCursor }) => {
		expect(
			detectAccessibilityDeletion(
				accessibleAsciiRow(before),
				accessibleAsciiRow(after),
				oldCursor,
				newCursor,
			),
		).toBe(expected);
	});

	it('detects deletion of a wide grapheme without splitting it', () => {
		const previous = buildAccessibleRow(
			[
				{ text: 'A', width: 1 },
				{ text: '你', width: 2 },
				{ text: '', width: 0 },
			],
			3,
		);
		const shortened = buildAccessibleRow([{ text: 'A', width: 1 }], 3);

		expect(detectAccessibilityDeletion(previous, shortened, 3, 1)).toBe('你');
		expect(detectAccessibilityDeletion(previous, previous, 3, 3)).toBeNull();
	});

	it('detects deletion of a surrogate-pair emoji as one terminal cell', () => {
		const previous = buildAccessibleRow(
			[
				{ text: 'a', width: 1 },
				{ text: '😀', width: 2 },
				{ text: '', width: 0 },
				{ text: 'b', width: 1 },
			],
			4,
		);

		expect(detectAccessibilityDeletion(previous, accessibleAsciiRow('ab'), 3, 1)).toBe('😀');
	});
});

function accessibleAsciiRow(text: string) {
	return buildAccessibleRow(
		[...text].map((character) => ({ text: character, width: 1 })),
		[...text].length,
	);
}
