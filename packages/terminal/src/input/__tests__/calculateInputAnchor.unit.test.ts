import { describe, expect, it } from 'vitest';

import { calculateInputAnchor } from '../calculateInputAnchor';

describe('calculateInputAnchor', () => {
	it('positions the native input at the terminal cursor', () => {
		expect(
			calculateInputAnchor({
				canvasLeft: 380,
				canvasTop: 230,
				elementLeft: 100,
				elementTop: 147,
				cursorColumn: 5,
				cursorRow: 7,
				columns: 80,
				rows: 24,
				cellWidth: 8,
				cellHeight: 17,
				preeditCellWidth: 4,
				preeditCaretPixelOffset: 15.5,
			}),
		).toEqual({ left: 308.5, top: 202, width: 32, height: 17 });
	});

	it('clamps cursor and preedit width to the rendered grid', () => {
		expect(
			calculateInputAnchor({
				canvasLeft: 0,
				canvasTop: 0,
				elementLeft: 0,
				elementTop: 0,
				cursorColumn: 99,
				cursorRow: -1,
				columns: 10,
				rows: 3,
				cellWidth: 8,
				cellHeight: 17,
				preeditCellWidth: 8,
				preeditCaretPixelOffset: 64,
			}),
		).toEqual({ left: 12, top: 0, width: 64, height: 17 });
	});

	it('compensates for composition caret movement while keeping a stable IME point', () => {
		const base = {
			canvasLeft: 10,
			canvasTop: 20,
			elementLeft: 0,
			elementTop: 0,
			cursorColumn: 5,
			cursorRow: 2,
			columns: 80,
			rows: 24,
			cellWidth: 8,
			cellHeight: 17,
			preeditCellWidth: 3,
		} as const;

		const start = calculateInputAnchor({ ...base, preeditCaretPixelOffset: 0 });
		const end = calculateInputAnchor({ ...base, preeditCaretPixelOffset: 23.25 });
		expect(start.left).toBe(54);
		expect(end.left + 23.25).toBe(start.left);
	});
});
