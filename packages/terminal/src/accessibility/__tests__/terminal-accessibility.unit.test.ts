import { describe, expect, it } from 'vitest';
import {
	AccessibleTextModel,
	buildAccessibleRow,
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
