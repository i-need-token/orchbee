import { describe, expect, it } from 'vitest';

import { cropLiveGrid } from '../cropLiveGrid';

// The cross-Client rule: a smaller viewer keeps the prompt in view without resizing the terminal.
describe('cropLiveGrid', () => {
	it('shows the whole grid from its origin when the surface is large enough', () => {
		expect(
			cropLiveGrid(
				{ columns: 80, cursor: { column: 79, row: 23, visible: true }, rows: 24 },
				100,
				30,
			),
		).toEqual({ clipped: false, column: 0, columns: 80, row: 0, rows: 24 });
	});

	it('keeps a cursor beyond the surface on its last visible row and column', () => {
		expect(
			cropLiveGrid(
				{ columns: 120, cursor: { column: 100, row: 39, visible: true }, rows: 40 },
				80,
				24,
			),
		).toEqual({ clipped: true, column: 21, columns: 80, row: 16, rows: 24 });
	});

	it('stays at the origin while the cursor is already visible there', () => {
		expect(
			cropLiveGrid(
				{ columns: 120, cursor: { column: 2, row: 0, visible: true }, rows: 40 },
				80,
				24,
			),
		).toEqual({ clipped: true, column: 0, columns: 80, row: 0, rows: 24 });
	});

	it('shows the bottom-left corner while the cursor is hidden', () => {
		expect(
			cropLiveGrid(
				{ columns: 120, cursor: { column: 100, row: 3, visible: false }, rows: 40 },
				80,
				24,
			),
		).toEqual({ clipped: true, column: 0, columns: 80, row: 16, rows: 24 });
	});

	it('crops only the axis that does not fit', () => {
		expect(
			cropLiveGrid(
				{ columns: 60, cursor: { column: 59, row: 39, visible: true }, rows: 40 },
				80,
				24,
			),
		).toEqual({ clipped: true, column: 0, columns: 60, row: 16, rows: 24 });
	});
});
