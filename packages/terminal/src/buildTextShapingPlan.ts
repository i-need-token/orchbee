export interface TextShapingCell {
	readonly text: string;
	readonly width: number;
	readonly styleKey: string;
	readonly eligible: boolean;
}

export interface TextShapingSpan {
	readonly start: number;
	readonly end: number;
	readonly columns: number;
	readonly text: string;
}

export type TextShapingPlan = readonly (TextShapingSpan | null)[];

export interface TextShapingReader {
	readonly length: number;
	eligible(index: number): boolean;
	sameStyle(left: number, right: number): boolean;
	text(index: number): string;
	width(index: number): number;
}

export interface TextShapingOptions {
	/** Keep browser shaping for complex scripts when optional Latin ligatures are disabled. */
	readonly allowOptionalLigatures?: boolean;
	/** Bound one shaped raster by terminal columns when the rendering backend has a finite surface. */
	readonly maximumRunColumns?: number;
}

/**
 * Build Ghostty-style, row-bounded shaping runs for the browser text shaper.
 *
 * A run includes empty cells as spaces, retains wide-cell columns, and ends at font-style, cursor,
 * preedit, sprite, or other caller-provided boundaries. Browser font fallback remains the font
 * resolver; this planner only owns terminal-cell run boundaries.
 */
export function buildTextShapingPlan(
	source: readonly TextShapingCell[] | TextShapingReader,
	options: TextShapingOptions = {},
): TextShapingPlan {
	const reader = isTextShapingReader(source)
		? source
		: {
				length: source.length,
				eligible: (index: number) => source[index]?.eligible === true,
				sameStyle: (left: number, right: number) =>
					source[left]?.styleKey === source[right]?.styleKey,
				text: (index: number) => source[index]?.text ?? '',
				width: (index: number) => source[index]?.width ?? 0,
			};
	const maximumRunColumns = options.maximumRunColumns ?? Number.POSITIVE_INFINITY;
	if (
		maximumRunColumns !== Number.POSITIVE_INFINITY &&
		(!Number.isSafeInteger(maximumRunColumns) || maximumRunColumns < 1)
	) {
		throw new RangeError('maximumRunColumns must be a positive safe integer');
	}
	return buildTextShapingPlanFromReader(
		reader,
		options.allowOptionalLigatures !== false,
		maximumRunColumns,
	);
}

function buildTextShapingPlanFromReader(
	reader: TextShapingReader,
	allowOptionalLigatures: boolean,
	maximumRunColumns: number,
): TextShapingPlan {
	const plan: (TextShapingSpan | null)[] = Array.from({ length: reader.length }, () => null);
	let column = 0;
	while (column < reader.length) {
		if (!isRunCell(reader, column)) {
			column++;
			continue;
		}

		const start = column;
		let end = start + 1;
		let runColumns = Math.max(0, reader.width(start));
		while (
			end < reader.length &&
			isRunCell(reader, end) &&
			reader.sameStyle(start, end) &&
			!isRejectedLigatureBoundary(reader.text(end - 1), reader.text(end)) &&
			runColumns + Math.max(0, reader.width(end)) <= maximumRunColumns
		) {
			runColumns += Math.max(0, reader.width(end));
			end++;
		}

		const range = allowOptionalLigatures ? { start, end } : complexShapingRange(reader, start, end);
		const span = range === null ? null : createSpan(reader, range.start, range.end);
		if (span !== null) {
			for (let index = span.start; index < span.end; index++) plan[index] = span;
		}
		column = end;
	}
	return plan;
}

function isTextShapingReader(
	source: readonly TextShapingCell[] | TextShapingReader,
): source is TextShapingReader {
	return 'eligible' in source;
}

function isRunCell(reader: TextShapingReader, index: number): boolean {
	if (!reader.eligible(index)) return false;
	const width = reader.width(index);
	return width >= 0 && width <= 2;
}

function complexShapingRange(
	reader: TextShapingReader,
	start: number,
	end: number,
): { readonly start: number; readonly end: number } | null {
	let complexStart = -1;
	let complexEnd = -1;
	for (let index = start; index < end; index++) {
		if (!containsNonAscii(reader.text(index))) continue;
		if (complexStart < 0) complexStart = index;
		complexEnd = index + 1;
	}
	if (complexStart < 0) return null;
	while (complexEnd < end && reader.width(complexEnd) === 0) complexEnd++;
	return { start: complexStart, end: complexEnd };
}

function containsNonAscii(text: string): boolean {
	for (const character of text) {
		if ((character.codePointAt(0) ?? 0) > 0x7f) return true;
	}
	return false;
}

function isRejectedLigatureBoundary(left: string, right: string): boolean {
	return (left === 'f' && (right === 'i' || right === 'l')) || (left === 's' && right === 't');
}

function createSpan(reader: TextShapingReader, start: number, end: number): TextShapingSpan | null {
	if (end - start < 2) return null;
	let columns = 0;
	let text = '';
	let hasText = false;
	for (let index = start; index < end; index++) {
		const width = reader.width(index);
		columns += width;
		if (width === 0) continue;
		const cellText = reader.text(index);
		if (cellText.length > 0) hasText = true;
		text += cellText || ' ';
	}
	if (columns <= 0 || !hasText) return null;
	return { start, end, columns, text };
}
