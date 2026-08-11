export interface AccessibleCellInput {
	readonly text: string;
	readonly width: number;
}

export interface AccessibleTextPosition {
	readonly column: number;
	readonly row: number;
}

export interface AccessibleOffsetRange {
	readonly end: number;
	readonly start: number;
}

interface AccessibleRowSegment {
	readonly column: number;
	readonly end: number;
	readonly start: number;
	readonly width: number;
}

export interface AccessibleRow {
	readonly columnCount: number;
	readonly columnOffsets: readonly number[];
	readonly segments: readonly AccessibleRowSegment[];
	readonly text: string;
}

/** Build one concise live-region update from visible rows whose text changed in place. */
export function createAccessibilityAnnouncement(
	rows: readonly string[],
	previousRows: readonly string[],
): string {
	return rows
		.filter((text, row) => text.trim().length > 0 && text !== previousRows[row])
		.join('\n');
}

/** Detect the grapheme removed around a stable or left-moving terminal cursor. */
export function detectAccessibilityDeletion(
	previousRow: AccessibleRow,
	currentRow: AccessibleRow,
	previousCursorColumn: number,
	currentCursorColumn: number,
): string | null {
	if (currentCursorColumn > previousCursorColumn || previousRow.text === currentRow.text) {
		return null;
	}
	if (currentCursorColumn < previousCursorColumn) {
		const start = rowOffset(previousRow, currentCursorColumn);
		const end = rowOffset(previousRow, previousCursorColumn);
		const previous = previousRow.text.slice(start, end);
		const current = currentRow.text.slice(
			rowOffset(currentRow, currentCursorColumn),
			rowOffset(currentRow, previousCursorColumn),
		);
		return previous && previous !== current ? previous : null;
	}

	const previousStart = rowOffset(previousRow, currentCursorColumn);
	const currentStart = rowOffset(currentRow, currentCursorColumn);
	const previousTail = previousRow.text.slice(previousStart);
	const currentTail = currentRow.text.slice(currentStart);
	if (!previousTail.trim()) return null;
	if (!currentTail.trim()) return previousTail.trim();
	if (previousTail.startsWith(currentTail)) return null;
	const segment = previousRow.segments.find(
		(candidate) =>
			currentCursorColumn >= candidate.column &&
			currentCursorColumn < candidate.column + candidate.width,
	);
	return segment ? previousRow.text.slice(segment.start, segment.end) : null;
}

/** Preserve UTF-16 and terminal-cell boundaries while producing screen-reader text. */
export function buildAccessibleRow(
	cells: readonly AccessibleCellInput[],
	columnCount: number,
): AccessibleRow {
	let text = '';
	const columnOffsets = Array.from<number>({ length: columnCount + 1 }).fill(0);
	const segments: AccessibleRowSegment[] = [];
	let contentLimit = 0;

	for (let column = 0; column < columnCount; column++) {
		const cell = cells[column];
		if (!cell || cell.width <= 0) continue;
		const value = (cell.text || ' ').replaceAll('\u0000', ' ');
		if (value.trimEnd()) contentLimit = Math.min(columnCount, column + cell.width);
	}

	if (contentLimit === 0) {
		columnOffsets.fill(1, 1);
		return {
			columnCount,
			columnOffsets,
			segments: [{ column: 0, end: 1, start: 0, width: 1 }],
			text: ' ',
		};
	}

	for (let column = 0; column < contentLimit; column++) {
		const cell = cells[column];
		if (!cell || cell.width <= 0) continue;
		const value = (cell.text || ' ').replaceAll('\u0000', ' ');
		const start = text.length;
		text += value;
		const end = text.length;
		const width = Math.min(cell.width, columnCount - column);
		segments.push({ column, end, start, width });
		for (let offset = 0; offset < width; offset++) columnOffsets[column + offset] = start;
		columnOffsets[column + width] = end;
	}

	for (let column = contentLimit; column < columnOffsets.length; column++) {
		columnOffsets[column] = text.length;
	}
	return { columnCount, columnOffsets, segments, text };
}

/** Stable visible-text and terminal-cell mapping used by the off-screen DOM mirror. */
export class AccessibleTextModel {
	#rowStarts: readonly number[] = [];
	#rows: readonly AccessibleRow[] = [];
	#text = '';

	update(rows: readonly AccessibleRow[]): void {
		this.#rows = rows;
		const starts: number[] = [];
		let offset = 0;
		for (const row of rows) {
			starts.push(offset);
			offset += row.text.length + 1;
		}
		this.#rowStarts = starts;
		this.#text = rows.map((row) => row.text).join('\n');
	}

	getText(): string {
		return this.#text;
	}

	positionToOffset(position: AccessibleTextPosition): number {
		const rowIndex = clamp(position.row, 0, Math.max(0, this.#rows.length - 1));
		const row = this.#rows[rowIndex];
		if (!row) return 0;
		const column = clamp(position.column, 0, row.columnCount);
		return (this.#rowStarts[rowIndex] ?? 0) + (row.columnOffsets[column] ?? row.text.length);
	}

	offsetToPosition(
		offset: number,
		bias: 'backward' | 'forward' = 'backward',
	): AccessibleTextPosition {
		if (this.#rows.length === 0) return { column: 0, row: 0 };
		const normalized = clamp(offset, 0, this.#text.length);
		let rowIndex = this.#rows.length - 1;
		for (let index = 0; index < this.#rows.length; index++) {
			const start = this.#rowStarts[index] ?? 0;
			const row = this.#rows[index];
			if (row && normalized <= start + row.text.length) {
				rowIndex = index;
				break;
			}
		}
		const localOffset = normalized - (this.#rowStarts[rowIndex] ?? 0);
		return { column: this.rowOffsetToColumn(rowIndex, localOffset, bias), row: rowIndex };
	}

	rowOffsetToColumn(rowIndex: number, offset: number, edge: 'backward' | 'forward'): number {
		const row = this.#rows[rowIndex];
		if (!row) return 0;
		const normalized = clamp(offset, 0, row.text.length);
		for (const segment of row.segments) {
			if (normalized === segment.start) return segment.column;
			if (normalized > segment.start && normalized < segment.end) {
				return edge === 'forward' ? segment.column + segment.width : segment.column;
			}
			if (normalized === segment.end) return segment.column + segment.width;
		}
		return row.columnCount;
	}

	inclusiveTerminalRangeToOffsets(range: {
		readonly end: AccessibleTextPosition;
		readonly start: AccessibleTextPosition;
	}): AccessibleOffsetRange {
		const start = this.positionToOffset(range.start);
		const endRowIndex = clamp(range.end.row, 0, Math.max(0, this.#rows.length - 1));
		const endRow = this.#rows[endRowIndex];
		const endSegment = endRow?.segments.find(
			(segment) =>
				range.end.column >= segment.column && range.end.column < segment.column + segment.width,
		);
		const end = endSegment
			? (this.#rowStarts[endRowIndex] ?? 0) + endSegment.end
			: this.positionToOffset({ column: range.end.column + 1, row: range.end.row });
		return start <= end ? { end, start } : { end: start, start: end };
	}

	offsetsToInclusiveTerminalRange(range: AccessibleOffsetRange): {
		readonly end: AccessibleTextPosition;
		readonly start: AccessibleTextPosition;
	} {
		const startOffset = Math.min(range.start, range.end);
		const endOffset = Math.max(range.start, range.end);
		const start = this.offsetToPosition(startOffset, 'backward');
		const lastIncludedOffset = Math.max(startOffset, endOffset - 1);
		const end = this.#inclusivePosition(lastIncludedOffset);
		return { end, start };
	}

	#inclusivePosition(offset: number): AccessibleTextPosition {
		const normalized = clamp(offset, 0, this.#text.length);
		for (let rowIndex = 0; rowIndex < this.#rows.length; rowIndex++) {
			const row = this.#rows[rowIndex];
			if (!row) continue;
			const rowStart = this.#rowStarts[rowIndex] ?? 0;
			const rowEnd = rowStart + row.text.length;
			if (normalized < rowEnd) {
				return {
					column: this.rowOffsetToColumn(rowIndex, normalized - rowStart, 'backward'),
					row: rowIndex,
				};
			}
			if (normalized === rowEnd && rowIndex < this.#rows.length - 1) {
				return { column: row.segments.at(-1)?.column ?? 0, row: rowIndex };
			}
		}
		return this.offsetToPosition(normalized, 'backward');
	}
}

function rowOffset(row: AccessibleRow, column: number): number {
	return row.columnOffsets[clamp(column, 0, row.columnCount)] ?? row.text.length;
}

function clamp(value: number, minimum: number, maximum: number): number {
	return Math.min(maximum, Math.max(minimum, value));
}
