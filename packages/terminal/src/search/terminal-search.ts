import type { CellData } from '../types';
import type { TerminalBufferRow } from '../buffer-rows';

/** Match-count cap used by VS Code's integrated terminal find widget. */
export const TERMINAL_SEARCH_RESULT_LIMIT = 20_000;

/** Trailing-edge delay used for incremental input and terminal-output refreshes. */
export const TERMINAL_SEARCH_DEBOUNCE_MS = 200;

export interface TerminalSearchOptions {
	readonly caseSensitive?: boolean;
	/** Lower the collection cap below the 20,000-match default. Higher values are clamped. */
	readonly maxResults?: number;
	readonly regex?: boolean;
	/** Allow a query containing newlines to span hard line boundaries. */
	readonly spanLines?: boolean;
	readonly wholeWord?: boolean;
}

export interface TerminalSearchMatch {
	readonly text: string;
	readonly start: { readonly column: number; readonly row: number };
	readonly end: { readonly column: number; readonly row: number };
}

export interface TerminalDirectionalSearchOptions {
	readonly preserveStart?: boolean;
	readonly reverse?: boolean;
	readonly start?: { readonly column: number; readonly row: number };
}

export interface TerminalSearchResultChangeEvent {
	/** Zero-based active result in the bounded collection, or -1 when absent or beyond its cap. */
	readonly resultIndex: number;
	readonly resultCount: number;
	/** Whether the configured match-count cap was reached. */
	readonly resultLimitReached: boolean;
	readonly query: string;
	/** JavaScript regular-expression validation message, or null when valid. */
	readonly regexError: string | null;
}

export function validateTerminalSearchQuery(
	query: string,
	options: TerminalSearchOptions = {},
): string | null {
	if (!query || !options.regex) return null;
	try {
		RegExp(query, options.caseSensitive ? 'gu' : 'giu');
		return null;
	} catch (error) {
		return error instanceof Error ? error.message : 'Invalid regular expression';
	}
}

export function searchMatchContainsCell(
	match: TerminalSearchMatch,
	column: number,
	row: number,
): boolean {
	if (row < match.start.row || row > match.end.row) return false;
	if (match.start.row === match.end.row) {
		return row === match.start.row && column >= match.start.column && column <= match.end.column;
	}
	if (row === match.start.row) return column >= match.start.column;
	if (row === match.end.row) return column <= match.end.column;
	return true;
}

/** Find row-local matches and map JavaScript string offsets back to terminal cells. */
export function findTerminalMatches(
	lines: readonly (readonly CellData[])[],
	query: string,
	options: TerminalSearchOptions = {},
	wrappedRows: readonly boolean[] = [],
): readonly TerminalSearchMatch[] {
	const maxResults = resolveTerminalSearchResultLimit(options.maxResults);
	if (maxResults === 0) return [];
	const matches: TerminalSearchMatch[] = [];
	visitTerminalMatches(lines, query, options, wrappedRows, (match) => {
		matches.push(match);
		return matches.length >= maxResults;
	});
	return matches;
}

/** Search source-neutral bounded rows and preserve their absolute terminal coordinates. */
export function findTerminalBufferMatches(
	rows: readonly TerminalBufferRow[],
	query: string,
	options: TerminalSearchOptions = {},
): readonly TerminalSearchMatch[] {
	const maxResults = resolveTerminalSearchResultLimit(options.maxResults);
	if (maxResults === 0) return [];
	const matches: TerminalSearchMatch[] = [];
	visitTerminalBufferMatches(rows, query, options, (match) => {
		matches.push(match);
		return matches.length >= maxResults;
	});
	return matches;
}

interface BufferSearchRow {
	readonly mappingIndices: Uint16Array | null;
	readonly text: string;
}

interface BufferSearchSegment {
	readonly end: number;
	readonly kind: 'newline' | 'row';
	readonly rowOffset: number;
	readonly searchableRow: BufferSearchRow | null;
	readonly start: number;
}

function visitTerminalBufferMatches(
	rows: readonly TerminalBufferRow[],
	query: string,
	options: TerminalSearchOptions,
	visitor: (match: TerminalSearchMatch) => boolean,
): void {
	if (!query) return;
	const expression = createExpression(query, options);
	if (!expression) return;
	let logicalText = '';
	let segments: BufferSearchSegment[] = [];
	for (let rowOffset = 0; rowOffset < rows.length; rowOffset++) {
		const row = rows[rowOffset];
		if (row === undefined) continue;
		const searchableRow = buildBufferSearchRow(row, !options.regex);
		if (searchableRow.text.length > 0) {
			const start = logicalText.length;
			logicalText += searchableRow.text;
			segments.push({
				end: logicalText.length,
				kind: 'row',
				rowOffset,
				searchableRow,
				start,
			});
		}
		if (row.wrap) continue;
		if (options.spanLines && rowOffset < rows.length - 1) {
			const start = logicalText.length;
			logicalText += '\n';
			segments.push({
				end: logicalText.length,
				kind: 'newline',
				rowOffset,
				searchableRow: null,
				start,
			});
			continue;
		}
		if (visitBufferTextMatches(rows, logicalText, segments, expression, options, visitor)) return;
		logicalText = '';
		segments = [];
	}
	if (logicalText) {
		visitBufferTextMatches(rows, logicalText, segments, expression, options, visitor);
	}
}

function buildBufferSearchRow(
	row: TerminalBufferRow,
	foldCombiningMarks: boolean,
): BufferSearchRow {
	if (row.identityAscii) return { mappingIndices: null, text: row.text };
	if (!foldCombiningMarks) return { mappingIndices: null, text: row.text };
	const folded = foldLiteralSearchText(row.text);
	if (folded === row.text) return { mappingIndices: null, text: row.text };
	let text = '';
	const mappingIndices: number[] = [];
	for (const [mappingIndex, mapping] of row.mappings.entries()) {
		const next = row.mappings[mappingIndex + 1];
		const original = row.text.slice(mapping.utf16Offset, next?.utf16Offset ?? row.text.length);
		const searchable = foldLiteralSearchText(original);
		text += searchable;
		for (let index = 0; index < searchable.length; index++) mappingIndices.push(mappingIndex);
	}
	return { mappingIndices: Uint16Array.from(mappingIndices), text };
}

function visitBufferTextMatches(
	rows: readonly TerminalBufferRow[],
	text: string,
	segments: readonly BufferSearchSegment[],
	expression: RegExp,
	options: TerminalSearchOptions,
	visitor: (match: TerminalSearchMatch) => boolean,
): boolean {
	expression.lastIndex = 0;
	let match: RegExpExecArray | null = expression.exec(text);
	while (match) {
		const value = match[0];
		const index = match.index;
		if (value && isWholeWord(text, index, value, options)) {
			const start = bufferSearchPosition(rows, segments, index);
			const end = bufferSearchPosition(rows, segments, index + value.length - 1);
			if (start !== null && end !== null) {
				const startRow = rows[start.rowOffset]?.index;
				const endRow = rows[end.rowOffset]?.index;
				if (
					startRow !== undefined &&
					endRow !== undefined &&
					startRow <= BigInt(Number.MAX_SAFE_INTEGER) &&
					endRow <= BigInt(Number.MAX_SAFE_INTEGER) &&
					visitor({
						text: originalBufferTextForMatch(rows, segments, index, value.length),
						start: { column: start.column, row: Number(startRow) },
						end: { column: end.column, row: Number(endRow) },
					})
				) {
					return true;
				}
			}
		}
		if (value.length === 0) expression.lastIndex = nextCodePointOffset(text, index);
		match = expression.exec(text);
	}
	return false;
}

function bufferSearchPosition(
	rows: readonly TerminalBufferRow[],
	segments: readonly BufferSearchSegment[],
	offset: number,
): {
	readonly column: number;
	readonly mappingIndex: number | null;
	readonly rowOffset: number;
} | null {
	const segment = bufferSearchSegmentAt(segments, offset);
	if (segment === undefined) return null;
	const row = rows[segment.rowOffset];
	if (row === undefined) return null;
	if (segment.kind === 'newline') {
		return {
			column: row.identityAscii
				? Math.max(0, row.text.length - 1)
				: (row.mappings.at(-1)?.column ?? 0),
			mappingIndex: null,
			rowOffset: segment.rowOffset,
		};
	}
	const searchableRow = segment.searchableRow;
	if (searchableRow === null) return null;
	const localOffset = offset - segment.start;
	const mappingIndex =
		searchableRow.mappingIndices?.[localOffset] ?? mappingIndexAtUtf16Offset(row, localOffset);
	if (row.identityAscii) {
		return { column: mappingIndex, mappingIndex, rowOffset: segment.rowOffset };
	}
	const mapping = row.mappings[mappingIndex];
	return mapping === undefined
		? null
		: { column: mapping.column, mappingIndex, rowOffset: segment.rowOffset };
}

function bufferSearchSegmentAt(
	segments: readonly BufferSearchSegment[],
	offset: number,
): BufferSearchSegment | undefined {
	let low = 0;
	let high = segments.length - 1;
	while (low <= high) {
		const middle = Math.floor((low + high) / 2);
		const segment = segments[middle];
		if (segment === undefined) return undefined;
		if (offset < segment.start) high = middle - 1;
		else if (offset >= segment.end) low = middle + 1;
		else return segment;
	}
	return undefined;
}

function mappingIndexAtUtf16Offset(row: TerminalBufferRow, offset: number): number {
	if (row.identityAscii) return offset;
	let low = 0;
	let high = row.mappings.length - 1;
	while (low <= high) {
		const middle = Math.floor((low + high) / 2);
		const mappingOffset = row.mappings[middle]?.utf16Offset ?? 0;
		if (mappingOffset <= offset) low = middle + 1;
		else high = middle - 1;
	}
	return Math.max(0, high);
}

function originalBufferTextForMatch(
	rows: readonly TerminalBufferRow[],
	segments: readonly BufferSearchSegment[],
	start: number,
	length: number,
): string {
	const end = start + length;
	let text = '';
	for (const segment of segments) {
		if (segment.end <= start) continue;
		if (segment.start >= end) break;
		if (segment.kind === 'newline') {
			text += '\n';
			continue;
		}
		const row = rows[segment.rowOffset];
		if (row === undefined || segment.searchableRow === null) continue;
		const localStart = Math.max(start, segment.start) - segment.start;
		const localEnd = Math.min(end, segment.end) - segment.start - 1;
		if (row.identityAscii) {
			text += row.text.slice(localStart, localEnd + 1);
			continue;
		}
		const firstMapping =
			segment.searchableRow.mappingIndices?.[localStart] ??
			mappingIndexAtUtf16Offset(row, localStart);
		const lastMapping =
			segment.searchableRow.mappingIndices?.[localEnd] ?? mappingIndexAtUtf16Offset(row, localEnd);
		for (let mappingIndex = firstMapping; mappingIndex <= lastMapping; mappingIndex++) {
			const mapping = row.mappings[mappingIndex];
			if (mapping === undefined) continue;
			const next = row.mappings[mappingIndex + 1];
			text += row.text.slice(mapping.utf16Offset, next?.utf16Offset ?? row.text.length);
		}
	}
	return text;
}

/** Find one directional match without applying the bounded decoration/result-count limit. */
export function findDirectionalTerminalMatch(
	lines: readonly (readonly CellData[])[],
	query: string,
	options: TerminalSearchOptions = {},
	wrappedRows: readonly boolean[] = [],
	direction: TerminalDirectionalSearchOptions = {},
): TerminalSearchMatch | null {
	let first: TerminalSearchMatch | null = null;
	let last: TerminalSearchMatch | null = null;
	let candidate: TerminalSearchMatch | null = null;
	visitTerminalMatches(lines, query, options, wrappedRows, (match) => {
		first ??= match;
		last = match;
		const start = direction.start;
		if (!start) {
			if (!direction.reverse) {
				candidate = match;
				return true;
			}
			return false;
		}
		const comparison = compareMatchStart(match, start);
		if (direction.preserveStart && comparison === 0) {
			candidate = match;
			return true;
		}
		if (direction.reverse) {
			if (comparison < 0) candidate = match;
			return false;
		}
		if (comparison > 0) {
			candidate = match;
			return true;
		}
		return false;
	});
	return candidate ?? (direction.reverse ? last : first);
}

function visitTerminalMatches(
	lines: readonly (readonly CellData[])[],
	query: string,
	options: TerminalSearchOptions,
	wrappedRows: readonly boolean[],
	visitor: (match: TerminalSearchMatch) => boolean,
): void {
	if (!query) return;
	const expression = createExpression(query, options);
	if (!expression) return;
	let logicalText = '';
	let logicalPositions: SearchPosition[] = [];
	for (let row = 0; row < lines.length; row++) {
		const searchable = buildSearchableLine(lines[row] ?? [], !options.regex);
		logicalText += searchable.text;
		logicalPositions = logicalPositions.concat(
			searchable.positions.map((position) => ({ ...position, row })),
		);
		if (wrappedRows[row]) continue;
		if (options.spanLines && row < lines.length - 1) {
			logicalText += '\n';
			logicalPositions.push({
				column: searchable.positions.at(-1)?.column ?? 0,
				row,
				text: '\n',
			});
			continue;
		}
		if (visitTextMatches(logicalText, logicalPositions, expression, options, visitor)) {
			return;
		}
		logicalText = '';
		logicalPositions = [];
	}
	if (logicalText) {
		visitTextMatches(logicalText, logicalPositions, expression, options, visitor);
	}
}

interface SearchPosition {
	readonly column: number;
	readonly row: number;
	readonly text: string;
}

function visitTextMatches(
	text: string,
	positions: readonly SearchPosition[],
	expression: RegExp,
	options: TerminalSearchOptions,
	visitor: (match: TerminalSearchMatch) => boolean,
): boolean {
	expression.lastIndex = 0;
	let match: RegExpExecArray | null = expression.exec(text);
	while (match) {
		const value = match[0];
		const index = match.index;
		if (value && isWholeWord(text, index, value, options)) {
			const start = positions[index];
			const end = positions[index + value.length - 1];
			if (start && end) {
				const shouldStop = visitor({
					text: originalTextForMatch(positions, index, value.length),
					start: { column: start.column, row: start.row },
					end: { column: end.column, row: end.row },
				});
				if (shouldStop) return true;
			}
		}
		if (value.length === 0) expression.lastIndex = nextCodePointOffset(text, index);
		match = expression.exec(text);
	}
	return false;
}

function compareMatchStart(
	match: TerminalSearchMatch,
	position: { readonly column: number; readonly row: number },
): number {
	if (match.start.row !== position.row) return match.start.row - position.row;
	return match.start.column - position.column;
}

function originalTextForMatch(
	positions: readonly SearchPosition[],
	index: number,
	length: number,
): string {
	let text = '';
	let previous: SearchPosition | undefined;
	for (const position of positions.slice(index, index + length)) {
		if (
			!previous ||
			position.row !== previous.row ||
			position.column !== previous.column ||
			position.text !== previous.text
		) {
			text += position.text;
		}
		previous = position;
	}
	return text;
}

function nextCodePointOffset(text: string, index: number): number {
	if (index >= text.length) return text.length + 1;
	return index + ((text.codePointAt(index) ?? 0) > 0xffff ? 2 : 1);
}

export function resolveTerminalSearchResultLimit(value: number | undefined): number {
	if (value === undefined || !Number.isFinite(value)) return TERMINAL_SEARCH_RESULT_LIMIT;
	return Math.min(TERMINAL_SEARCH_RESULT_LIMIT, Math.max(0, Math.floor(value)));
}

function createExpression(query: string, options: TerminalSearchOptions): RegExp | null {
	const searchableQuery = options.regex ? query : foldLiteralSearchText(query);
	if (!searchableQuery) return null;
	const source = options.regex ? searchableQuery : escapeRegExp(searchableQuery);
	try {
		return new RegExp(source, options.caseSensitive ? 'gu' : 'giu');
	} catch {
		return null;
	}
}

function buildSearchableLine(
	cells: readonly CellData[],
	foldCombiningMarks: boolean,
): {
	readonly text: string;
	readonly positions: readonly { readonly column: number; readonly text: string }[];
} {
	let lastContentColumn = -1;
	for (let column = cells.length - 1; column >= 0; column--) {
		if ((cells[column]?.graphemeLen ?? 0) > 0) {
			lastContentColumn = column;
			break;
		}
	}
	let text = '';
	const positions: Array<{ readonly column: number; readonly text: string }> = [];
	for (let column = 0; column <= lastContentColumn; column++) {
		const cell = cells[column];
		if (!cell || cell.width === 0) continue;
		const value = cell.graphemeLen > 0 ? cell.text : ' ';
		const searchableValue = foldCombiningMarks ? foldLiteralSearchText(value) : value;
		text += searchableValue;
		for (let index = 0; index < searchableValue.length; index++) {
			positions.push({ column, text: value });
		}
	}
	return { text, positions };
}

function foldLiteralSearchText(value: string): string {
	return value.normalize('NFD').replaceAll(/\p{M}/gu, '');
}

function isWholeWord(
	line: string,
	index: number,
	match: string,
	options: TerminalSearchOptions,
): boolean {
	if (!options.wholeWord) return true;
	const before = index === 0 ? '' : (Array.from(line.slice(0, index)).at(-1) ?? '');
	const after = Array.from(line.slice(index + match.length))[0] ?? '';
	return !isWordCharacter(before) && !isWordCharacter(after);
}

function isWordCharacter(value: string): boolean {
	return value ? /[\p{L}\p{N}_]/u.test(value) : false;
}

function escapeRegExp(value: string): string {
	return value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}
