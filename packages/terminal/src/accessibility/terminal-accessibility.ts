/** Cursor position exposed to assistive technology. */
export interface AccessibleCursor {
	readonly column: number;
	readonly row: number;
}

export interface AccessibleTextPosition {
	readonly row: number;
	readonly column: number;
}

/** Terminal-cell range; end is exclusive. */
export interface AccessibleTextRange {
	readonly start: AccessibleTextPosition;
	readonly end: AccessibleTextPosition;
}

export interface AccessibleOffsetRange {
	readonly start: number;
	readonly end: number;
}

export interface AccessibleCellInput {
	readonly text: string;
	readonly width: number;
}

interface AccessibleRowSegment {
	readonly start: number;
	readonly end: number;
	readonly column: number;
	readonly width: number;
}

export interface AccessibleRow {
	readonly text: string;
	readonly columnCount: number;
	readonly columnOffsets: readonly number[];
	readonly segments: readonly AccessibleRowSegment[];
}

export interface TerminalAccessibilityOptions {
	readonly onSelectionChange?: (range: AccessibleTextRange | null) => void;
}

/** Convert terminal rows into stable, non-empty accessible labels. */
export function normalizeAccessibleRows(lines: readonly string[]): readonly string[] {
	return lines.map((line) => line.replaceAll('\u0000', ' ').trimEnd() || ' ');
}

/** Build a live-region update from non-empty rows whose content changed in place. */
export function createAccessibilityAnnouncement(
	rows: readonly string[],
	previousRows: readonly string[],
): string {
	return rows
		.filter((text, row) => text.trim().length > 0 && text !== previousRows[row])
		.join('\n');
}

/** Detect text removed around a stable or left-moving terminal cursor. */
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

function rowOffset(row: AccessibleRow, column: number): number {
	return row.columnOffsets[clamp(column, 0, row.columnCount)] ?? row.text.length;
}

/** Preserve terminal-cell boundaries while producing screen-reader text. */
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
			text: ' ',
			columnCount,
			columnOffsets,
			segments: [{ start: 0, end: 1, column: 0, width: 1 }],
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
		segments.push({ start, end, column, width });
		for (let offset = 0; offset < width; offset++) columnOffsets[column + offset] = start;
		columnOffsets[column + width] = end;
	}

	for (let column = contentLimit; column < columnOffsets.length; column++) {
		columnOffsets[column] = text.length;
	}
	return { text, columnCount, columnOffsets, segments };
}

/** Stable visible-text and cell-offset mapping used by the DOM mirror. */
export class AccessibleTextModel {
	private rows: readonly AccessibleRow[] = [];
	private rowStarts: readonly number[] = [];
	private text = '';

	update(rows: readonly AccessibleRow[]): void {
		this.rows = rows;
		const starts: number[] = [];
		let offset = 0;
		for (const row of rows) {
			starts.push(offset);
			offset += row.text.length + 1;
		}
		this.rowStarts = starts;
		this.text = rows.map((row) => row.text).join('\n');
	}

	getText(): string {
		return this.text;
	}

	getVisibleRange(): AccessibleOffsetRange {
		return { start: 0, end: this.text.length };
	}

	positionToOffset(position: AccessibleTextPosition): number {
		const rowIndex = clamp(position.row, 0, Math.max(0, this.rows.length - 1));
		const row = this.rows[rowIndex];
		if (!row) return 0;
		const column = clamp(position.column, 0, row.columnCount);
		return (this.rowStarts[rowIndex] ?? 0) + (row.columnOffsets[column] ?? row.text.length);
	}

	offsetToPosition(
		offset: number,
		bias: 'backward' | 'forward' = 'backward',
	): AccessibleTextPosition {
		if (this.rows.length === 0) return { row: 0, column: 0 };
		const normalized = clamp(offset, 0, this.text.length);
		let rowIndex = this.rows.length - 1;
		for (let index = 0; index < this.rows.length; index++) {
			const start = this.rowStarts[index] ?? 0;
			const row = this.rows[index];
			if (row && normalized <= start + row.text.length) {
				rowIndex = index;
				break;
			}
		}
		const row = this.rows[rowIndex];
		if (!row) return { row: rowIndex, column: 0 };
		const local = normalized - (this.rowStarts[rowIndex] ?? 0);
		for (const segment of row.segments) {
			if (local === segment.start) return { row: rowIndex, column: segment.column };
			if (local > segment.start && local < segment.end) {
				return {
					row: rowIndex,
					column: bias === 'forward' ? segment.column + segment.width : segment.column,
				};
			}
			if (local === segment.end) {
				return { row: rowIndex, column: segment.column + segment.width };
			}
		}
		return { row: rowIndex, column: row.columnCount };
	}

	terminalRangeToOffsets(range: {
		readonly start: AccessibleTextPosition;
		readonly end: AccessibleTextPosition;
	}): AccessibleOffsetRange {
		const start = this.positionToOffset(range.start);
		const endRow = this.rows[clamp(range.end.row, 0, Math.max(0, this.rows.length - 1))];
		const segment = endRow?.segments.find(
			(candidate) =>
				range.end.column >= candidate.column &&
				range.end.column < candidate.column + candidate.width,
		);
		const endPosition = segment
			? { row: range.end.row, column: segment.column + segment.width }
			: { row: range.end.row, column: range.end.column + 1 };
		const end = this.positionToOffset(endPosition);
		return start <= end ? { start, end } : { start: end, end: start };
	}

	offsetsToTextRange(range: AccessibleOffsetRange): AccessibleTextRange {
		const start = Math.min(range.start, range.end);
		const end = Math.max(range.start, range.end);
		return {
			start: this.offsetToPosition(start, 'backward'),
			end: this.offsetToPosition(end, 'forward'),
		};
	}
}

/** Maintains an off-screen text mirror for canvas terminal accessibility. */
export class TerminalAccessibility {
	private readonly root: HTMLDivElement;
	private readonly screen: HTMLDivElement;
	private readonly liveRegion: HTMLDivElement;
	private readonly model = new AccessibleTextModel();
	private readonly options: TerminalAccessibilityOptions;
	private rowElements: HTMLDivElement[] = [];
	private previousRows: readonly string[] = [];
	private previousAccessibleRows: readonly AccessibleRow[] = [];
	private previousCursor: AccessibleCursor | undefined;
	private synchronizingSelection = false;
	private hadOwnedSelection = false;
	private lastAppliedOffsets: AccessibleOffsetRange | null = null;

	constructor(parent: HTMLElement, rowCount: number, options: TerminalAccessibilityOptions = {}) {
		const document = parent.ownerDocument;
		this.options = options;
		this.root = document.createElement('div');
		applyScreenReaderOnlyStyle(this.root);

		this.screen = document.createElement('div');
		this.screen.setAttribute('role', 'list');
		this.screen.setAttribute('aria-label', 'Terminal screen');
		this.screen.setAttribute('aria-readonly', 'true');
		this.root.appendChild(this.screen);

		this.liveRegion = document.createElement('div');
		this.liveRegion.setAttribute('role', 'status');
		this.liveRegion.setAttribute('aria-live', 'polite');
		this.liveRegion.setAttribute('aria-atomic', 'true');
		this.root.appendChild(this.liveRegion);
		parent.appendChild(this.root);
		document.addEventListener('selectionchange', this.handleSelectionChange);
		this.resize(rowCount);
	}

	update(
		rows: readonly AccessibleRow[],
		cursor: AccessibleCursor,
		selection: string,
		selectionRange?: {
			readonly start: AccessibleTextPosition;
			readonly end: AccessibleTextPosition;
		},
	): void {
		this.model.update(rows);
		this.resize(rows.length);
		const rowTexts = rows.map((row) => row.text);
		const previousCursor = this.previousCursor;
		const deletion =
			previousCursor && previousCursor.row === cursor.row
				? detectAccessibilityDeletion(
						this.previousAccessibleRows[cursor.row] ?? buildAccessibleRow([], 0),
						rows[cursor.row] ?? buildAccessibleRow([], 0),
						previousCursor.column,
						cursor.column,
					)
				: null;
		const changedRows = createAccessibilityAnnouncement(rowTexts, this.previousRows);
		const announcement = deletion ? `Deleted ${deletion}` : changedRows;
		for (let row = 0; row < rows.length; row++) {
			const text = rows[row]?.text ?? ' ';
			const element = this.rowElements[row];
			if (!element) continue;
			if (element.textContent !== text) element.textContent = text;
			element.setAttribute('aria-label', `Row ${row + 1}: ${text}`);
		}
		this.screen.setAttribute(
			'aria-label',
			`Terminal screen, cursor row ${cursor.row + 1}, column ${cursor.column + 1}`,
		);
		this.screen.setAttribute('aria-description', selection ? `Selected text: ${selection}` : '');
		if (announcement && announcement !== this.liveRegion.textContent) {
			this.liveRegion.textContent = announcement;
		}
		this.previousRows = rowTexts;
		this.previousAccessibleRows = rows;
		this.previousCursor = cursor;
		if (selection && selectionRange) this.selectTerminalRange(selectionRange);
		else this.clearOwnedDomSelection();
	}

	getText(): string {
		return this.model.getText();
	}

	getVisibleRange(): AccessibleOffsetRange {
		return this.model.getVisibleRange();
	}

	positionToOffset(position: AccessibleTextPosition): number {
		return this.model.positionToOffset(position);
	}

	offsetToPosition(offset: number): AccessibleTextPosition {
		return this.model.offsetToPosition(offset);
	}

	selectOffsets(range: AccessibleOffsetRange, notify = true): boolean {
		const normalized = {
			start: clamp(Math.min(range.start, range.end), 0, this.model.getText().length),
			end: clamp(Math.max(range.start, range.end), 0, this.model.getText().length),
		};
		const start = this.model.offsetToPosition(normalized.start, 'backward');
		const end = this.model.offsetToPosition(normalized.end, 'forward');
		const domStart = this.domPoint(start, 'start');
		const domEnd = this.domPoint(end, 'end');
		const selection = this.root.ownerDocument.getSelection();
		if (!domStart || !domEnd || !selection) return false;
		const rangeObject = this.root.ownerDocument.createRange();
		rangeObject.setStart(domStart.node, domStart.offset);
		rangeObject.setEnd(domEnd.node, domEnd.offset);
		this.synchronizingSelection = true;
		selection.removeAllRanges();
		selection.addRange(rangeObject);
		this.hadOwnedSelection = true;
		this.lastAppliedOffsets = normalized;
		this.synchronizingSelection = false;
		if (notify) this.options.onSelectionChange?.(this.model.offsetsToTextRange(normalized));
		return true;
	}

	getSelectionOffsets(): AccessibleOffsetRange | null {
		const selection = this.root.ownerDocument.getSelection();
		if (!selection || selection.rangeCount === 0) return null;
		const range = selection.getRangeAt(0);
		const start = this.domOffset(range.startContainer, range.startOffset);
		const end = this.domOffset(range.endContainer, range.endOffset);
		if (start === null || end === null) return null;
		return start <= end ? { start, end } : { start: end, end: start };
	}

	resize(rowCount: number): void {
		while (this.rowElements.length > rowCount) this.rowElements.pop()?.remove();
		while (this.rowElements.length < rowCount) {
			const row = this.root.ownerDocument.createElement('div');
			row.setAttribute('role', 'listitem');
			row.textContent = ' ';
			this.screen.appendChild(row);
			this.rowElements.push(row);
		}
	}

	dispose(): void {
		this.root.ownerDocument.removeEventListener('selectionchange', this.handleSelectionChange);
		this.root.remove();
		this.rowElements = [];
		this.previousRows = [];
		this.previousAccessibleRows = [];
		this.previousCursor = undefined;
		this.model.update([]);
	}

	private selectTerminalRange(range: {
		readonly start: AccessibleTextPosition;
		readonly end: AccessibleTextPosition;
	}): void {
		const offsets = this.model.terminalRangeToOffsets(range);
		const current = this.getSelectionOffsets();
		if (current && current.start === offsets.start && current.end === offsets.end) return;
		this.selectOffsets(offsets, false);
	}

	private clearOwnedDomSelection(): void {
		const selection = this.root.ownerDocument.getSelection();
		if (!selection?.anchorNode || !this.root.contains(selection.anchorNode)) return;
		this.synchronizingSelection = true;
		selection.removeAllRanges();
		this.hadOwnedSelection = false;
		this.synchronizingSelection = false;
	}

	private readonly handleSelectionChange = (): void => {
		if (this.synchronizingSelection || !this.options.onSelectionChange) return;
		const offsets = this.getSelectionOffsets();
		if (
			offsets &&
			this.lastAppliedOffsets?.start === offsets.start &&
			this.lastAppliedOffsets.end === offsets.end
		) {
			this.lastAppliedOffsets = null;
			return;
		}
		this.lastAppliedOffsets = null;
		if (offsets) {
			this.hadOwnedSelection = true;
			this.options.onSelectionChange(this.model.offsetsToTextRange(offsets));
		} else if (this.hadOwnedSelection) {
			this.hadOwnedSelection = false;
			this.options.onSelectionChange(null);
		}
	};

	private domPoint(
		position: AccessibleTextPosition,
		bias: 'start' | 'end',
	): { readonly node: Node; readonly offset: number } | null {
		const row = this.rowElements[position.row];
		const textNode = row?.firstChild;
		if (!row || !textNode) return null;
		const rowStart = this.model.positionToOffset({ row: position.row, column: 0 });
		const offset = this.model.positionToOffset(position) - rowStart;
		return {
			node: textNode,
			offset: clamp(offset, 0, row.textContent?.length ?? (bias === 'start' ? 0 : offset)),
		};
	}

	private domOffset(node: Node, offset: number): number | null {
		const rowIndex = this.rowElements.findIndex((row) => row === node || row.contains(node));
		const row = this.rowElements[rowIndex];
		if (!row || rowIndex < 0) return null;
		const range = this.root.ownerDocument.createRange();
		range.selectNodeContents(row);
		try {
			range.setEnd(node, offset);
		} catch {
			return null;
		}
		return this.model.positionToOffset({ row: rowIndex, column: 0 }) + range.toString().length;
	}
}

function clamp(value: number, minimum: number, maximum: number): number {
	return Math.min(maximum, Math.max(minimum, value));
}

function applyScreenReaderOnlyStyle(element: HTMLElement): void {
	element.style.position = 'absolute';
	element.style.width = '1px';
	element.style.height = '1px';
	element.style.padding = '0';
	element.style.margin = '-1px';
	element.style.overflow = 'hidden';
	element.style.clipPath = 'inset(50%)';
	element.style.whiteSpace = 'nowrap';
	element.style.border = '0';
}
