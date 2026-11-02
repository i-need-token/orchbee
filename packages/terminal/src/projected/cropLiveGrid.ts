/** The cells of a presented grid that a surface shows; `clipped` says the grid did not fit. */
export interface LiveCrop {
	readonly clipped: boolean;
	readonly column: number;
	readonly columns: number;
	readonly row: number;
	readonly rows: number;
}

interface LiveCropGrid {
	readonly columns: number;
	readonly cursor: { readonly column: number; readonly row: number; readonly visible: boolean };
	readonly rows: number;
}

/**
 * Choose what a surface smaller than the grid shows, as the native Client's live view does: the
 * visible cursor on both axes, or the bottom-left corner while it is hidden. Only the controller's
 * surface sizes the terminal, so a viewer crops locally instead of competing for the size.
 */
export function cropLiveGrid(
	grid: LiveCropGrid,
	surfaceColumns: number,
	surfaceRows: number,
): LiveCrop {
	const columns = Math.min(surfaceColumns, grid.columns);
	const rows = Math.min(surfaceRows, grid.rows);
	const { cursor } = grid;
	const column = cursor.visible ? Math.max(0, cursor.column - (columns - 1)) : 0;
	const row = cursor.visible ? Math.max(0, cursor.row - (rows - 1)) : grid.rows - rows;
	return {
		clipped: grid.columns > columns || grid.rows > rows,
		column: Math.min(column, grid.columns - columns),
		columns,
		row: Math.min(row, grid.rows - rows),
		rows,
	};
}
