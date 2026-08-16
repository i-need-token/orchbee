export interface CompositionSelection {
	readonly start: number;
	readonly end: number;
}

export interface PreeditCellData {
	readonly text: string;
	readonly width: number;
}

export interface PreeditState {
	readonly cells: readonly PreeditCellData[];
	readonly width: number;
	readonly selection: { readonly start: number; readonly end: number };
}

export interface PreeditLayoutRow {
	readonly cells: readonly PreeditCellData[];
	readonly end: number;
	readonly row: number;
	readonly selectionEnd: number;
	readonly selectionStart: number;
	readonly start: number;
	readonly width: number;
}

export interface PreeditLayout {
	readonly caret: { readonly column: number; readonly row: number };
	readonly clipped: boolean;
	readonly rows: readonly PreeditLayoutRow[];
}

export function shouldSuppressComposingControlInput(
	text: string | null,
	composing: boolean,
): boolean {
	if (!composing || text === null) return false;
	const codepoints = [...text];
	return codepoints.length === 1 && (codepoints[0]?.codePointAt(0) ?? 0x20) < 0x20;
}

/** Map DOM UTF-16 composition offsets to terminal cell offsets. */
export function buildPreeditState(
	data: string,
	selection: CompositionSelection | undefined,
	getWidth: (codepoint: number) => number,
): PreeditState {
	const cells: PreeditCellData[] = [];
	const boundaries: Array<{ readonly utf16: number; readonly cells: number }> = [];
	let utf16Offset = 0;
	let cellOffset = 0;
	let joinNext = false;
	for (const text of data) {
		const codepoint = text.codePointAt(0) ?? 0;
		const measuredWidth = Math.max(0, Math.min(2, getWidth(codepoint)));
		const previous = cells.at(-1);
		if (previous && (measuredWidth === 0 || joinNext)) {
			cells[cells.length - 1] = { text: previous.text + text, width: previous.width };
		} else {
			const width = Math.max(1, measuredWidth);
			cells.push({ text, width });
			cellOffset += width;
		}
		utf16Offset += text.length;
		boundaries.push({ utf16: utf16Offset, cells: cellOffset });
		joinNext = codepoint === 0x200d;
	}
	const start = mapUtf16Offset(selection?.start ?? data.length, boundaries);
	const end = mapUtf16Offset(selection?.end ?? selection?.start ?? data.length, boundaries);
	return {
		cells,
		width: cellOffset,
		selection: { start, end: Math.max(start, end) },
	};
}

/** Project transient marked text over visible terminal cells without mutating canonical rows. */
export function projectPreeditLayout(
	cells: readonly PreeditCellData[],
	selection: CompositionSelection,
	cursor: { readonly column: number; readonly row: number },
	columns: number,
	rows: number,
): PreeditLayout | null {
	const columnCount = Number.isFinite(columns) ? Math.floor(columns) : 0;
	const rowCount = Number.isFinite(rows) ? Math.floor(rows) : 0;
	const normalizedCells = cells
		.filter((cell) => cell.text.length > 0 && cell.width > 0)
		.map((cell) => ({ text: cell.text, width: cell.width >= 2 ? 2 : 1 }));
	if (columnCount <= 0 || rowCount <= 0 || normalizedCells.length === 0) return null;

	const totalWidth = normalizedCells.reduce((total, cell) => total + cell.width, 0);
	const selectionStart = clampCellOffset(selection.start, 0, totalWidth);
	const selectionEnd = clampCellOffset(selection.end, selectionStart, totalWidth);
	let column = clampCellOffset(cursor.column, 0, columnCount - 1);
	let row = clampCellOffset(cursor.row, 0, rowCount - 1);
	const origin = { column, row };
	let sourceOffset = 0;
	let clipped = false;
	let caret: PreeditLayout['caret'] | undefined = selectionEnd === 0 ? { column, row } : undefined;
	type MutableRow = {
		cells: PreeditCellData[];
		row: number;
		sourceEnd: number;
		sourceStart: number;
		start: number;
		width: number;
	};
	const projectedRows: MutableRow[] = [];

	for (let index = 0; index < normalizedCells.length; index++) {
		const cell = normalizedCells[index];
		if (cell === undefined) continue;
		if (cell.width > columnCount) {
			clipped = true;
			break;
		}
		if (column + cell.width > columnCount) {
			row++;
			column = 0;
		}
		if (row >= rowCount) {
			clipped = true;
			break;
		}

		let projectedRow = projectedRows.at(-1);
		if (
			projectedRow === undefined ||
			projectedRow.row !== row ||
			projectedRow.start + projectedRow.width !== column
		) {
			projectedRow = {
				cells: [],
				row,
				sourceEnd: sourceOffset,
				sourceStart: sourceOffset,
				start: column,
				width: 0,
			};
			projectedRows.push(projectedRow);
		}
		projectedRow.cells.push(cell);
		projectedRow.width += cell.width;
		projectedRow.sourceEnd += cell.width;

		if (caret === undefined && selectionEnd <= sourceOffset + cell.width) {
			const offsetInCell = clampCellOffset(selectionEnd - sourceOffset, 0, cell.width);
			let caretColumn = column + offsetInCell;
			let caretRow = row;
			if (caretColumn >= columnCount) {
				caretColumn = 0;
				caretRow++;
			}
			caret =
				caretRow < rowCount
					? { column: caretColumn, row: caretRow }
					: visibleLayoutBoundary(projectedRows, columnCount, rowCount, origin);
		}

		sourceOffset += cell.width;
		column += cell.width;
	}

	if (sourceOffset < totalWidth) clipped = true;
	caret ??= visibleLayoutBoundary(projectedRows, columnCount, rowCount, origin);
	return {
		caret,
		clipped,
		rows: projectedRows.map((projectedRow) => ({
			cells: projectedRow.cells,
			end: projectedRow.start + projectedRow.width,
			row: projectedRow.row,
			selectionEnd: clampCellOffset(selectionEnd - projectedRow.sourceStart, 0, projectedRow.width),
			selectionStart: clampCellOffset(
				selectionStart - projectedRow.sourceStart,
				0,
				projectedRow.width,
			),
			start: projectedRow.start,
			width: projectedRow.width,
		})),
	};
}

function visibleLayoutBoundary(
	rows: readonly { readonly row: number; readonly start: number; readonly width: number }[],
	columns: number,
	rowCount: number,
	fallback: PreeditLayout['caret'],
): PreeditLayout['caret'] {
	const last = rows.at(-1);
	if (last === undefined) return fallback;
	const end = last.start + last.width;
	if (end < columns) return { column: end, row: last.row };
	if (last.row + 1 < rowCount) return { column: 0, row: last.row + 1 };
	return { column: columns - 1, row: rowCount - 1 };
}

function clampCellOffset(value: number, minimum: number, maximum: number): number {
	const finite = Number.isFinite(value) ? Math.floor(value) : minimum;
	return Math.max(minimum, Math.min(maximum, finite));
}

function mapUtf16Offset(
	offset: number,
	boundaries: readonly { readonly utf16: number; readonly cells: number }[],
): number {
	if (offset <= 0) return 0;
	for (const boundary of boundaries) {
		if (offset <= boundary.utf16) return boundary.cells;
	}
	return boundaries.at(-1)?.cells ?? 0;
}
