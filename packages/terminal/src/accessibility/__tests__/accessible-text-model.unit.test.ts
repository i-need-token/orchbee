import { describe, expect, it } from 'vitest';

import { AccessibleTextModel, buildAccessibleRow } from '../accessible-text-model';

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
});
