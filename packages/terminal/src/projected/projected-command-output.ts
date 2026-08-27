import {
	PROTOCOL_LIMITS,
	RowPromptClass,
	SemanticContent,
	TERMINAL_ROW_WINDOW_HEADER_BYTES,
} from '@orchbee/terminal-protocol';

import {
	TerminalBufferRowDetail,
	type TerminalBufferRow,
	type TerminalBufferRowReader,
	type TerminalBufferRows,
	type TerminalBufferRowsSnapshot,
} from '../buffer-rows';

export const PROJECTED_COMMAND_OUTPUT_SOURCE_BYTE_LIMIT = 16 * 1024 * 1024;
export const PROJECTED_COMMAND_OUTPUT_PAGE_ROWS = 1024;

export interface ProjectedCommandOutput {
	readonly endRow: bigint;
	readonly generation: bigint;
	readonly revision: bigint;
	readonly startRow: bigint;
	readonly text: string;
	readonly truncatedStart: boolean;
}

export type ProjectedCommandOutputReadResult =
	| { readonly kind: 'cancelled' }
	| { readonly kind: 'empty'; readonly reason: 'no-output' | 'no-shell-integration' }
	| (ProjectedCommandOutput & { readonly kind: 'output' })
	| {
			readonly kind: 'unavailable';
			readonly reason: 'alternate-screen' | 'source-limit' | 'stale' | 'unavailable';
	  };

export interface ReadProjectedLastCommandOutputOptions {
	readonly maximumPageRows: number;
	readonly reader: TerminalBufferRowReader;
	readonly signal: AbortSignal;
	readonly snapshot: TerminalBufferRowsSnapshot;
	readonly sourceByteLimit?: number | undefined;
}

type CommandOutputBoundary =
	| { readonly kind: 'completed'; readonly lower: bigint; readonly upper: bigint }
	| { readonly kind: 'current'; readonly lower: bigint; readonly upper: bigint }
	| { readonly kind: 'truncated'; readonly lower: 0n; readonly upper: bigint };

/** Read and extract one generation/revision-pinned last-command output snapshot. */
export async function readProjectedLastCommandOutput({
	maximumPageRows,
	reader,
	signal,
	snapshot,
	sourceByteLimit = PROJECTED_COMMAND_OUTPUT_SOURCE_BYTE_LIMIT,
}: ReadProjectedLastCommandOutputOptions): Promise<ProjectedCommandOutputReadResult> {
	if (signal.aborted) return { kind: 'cancelled' };
	if (
		snapshot.generation <= 0n ||
		snapshot.revision < 0n ||
		snapshot.totalRows < 0n ||
		!Number.isInteger(maximumPageRows) ||
		maximumPageRows < 1 ||
		!Number.isInteger(sourceByteLimit) ||
		sourceByteLimit < 1
	) {
		return { kind: 'unavailable', reason: 'unavailable' };
	}
	if (snapshot.totalRows === 0n) {
		return { kind: 'empty', reason: 'no-shell-integration' };
	}

	const pageRowLimit = Math.min(maximumPageRows, PROTOCOL_LIMITS.maxRowWindowRows);
	let remainingBytes = Math.min(sourceByteLimit, PROJECTED_COMMAND_OUTPUT_SOURCE_BYTE_LIMIT);
	let boundary = snapshot.totalRows;
	let nextPageRows = pageRowLimit;
	let upperPrompt: bigint | null = null;
	let currentOutputSeen = false;
	let resolvedBoundary: CommandOutputBoundary | null = null;
	const collectedRows: TerminalBufferRow[] = [];

	while (boundary > 0n && resolvedBoundary === null) {
		if (signal.aborted) return { kind: 'cancelled' };
		if (remainingBytes < TERMINAL_ROW_WINDOW_HEADER_BYTES) {
			return { kind: 'unavailable', reason: 'source-limit' };
		}
		const rowCount = boundedPageRowCount(boundary, nextPageRows);
		const startRow = boundary - BigInt(rowCount);
		let result: Awaited<ReturnType<TerminalBufferRowReader['readBufferRows']>>;
		try {
			result = await reader.readBufferRows(
				{
					detail: TerminalBufferRowDetail.render,
					generation: snapshot.generation,
					maximumBytes: Math.min(remainingBytes, PROTOCOL_LIMITS.maxRowWindowBytes),
					purpose: 'buffer',
					revision: snapshot.revision,
					rowCount,
					startRow,
				},
				signal,
			);
		} catch {
			return signal.aborted
				? { kind: 'cancelled' }
				: { kind: 'unavailable', reason: 'unavailable' };
		}
		if (signal.aborted || result.kind === 'cancelled') return { kind: 'cancelled' };
		if (result.kind === 'stale' || result.kind === 'evicted') {
			return { kind: 'unavailable', reason: 'stale' };
		}
		if (result.kind === 'unavailable') {
			return { kind: 'unavailable', reason: 'unavailable' };
		}
		if (result.kind === 'too-large') {
			if (rowCount === 1) return { kind: 'unavailable', reason: 'source-limit' };
			nextPageRows = Math.max(1, Math.floor(rowCount / 2));
			continue;
		}
		if (result.kind !== 'rows') {
			return { kind: 'unavailable', reason: 'unavailable' };
		}
		if (!pageMatchesSnapshot(result, snapshot, startRow, rowCount, remainingBytes)) {
			return { kind: 'unavailable', reason: 'unavailable' };
		}

		collectedRows.push(...result.rows);
		for (let index = result.rows.length - 1; index >= 0; index -= 1) {
			const row = result.rows[index];
			if (row === undefined) continue;
			if (upperPrompt === null && hasWrittenOutput(row)) currentOutputSeen = true;
			if (row.promptClass !== RowPromptClass.prompt) continue;
			if (upperPrompt === null && currentOutputSeen) {
				resolvedBoundary = {
					kind: 'current',
					lower: row.index,
					upper: snapshot.totalRows,
				};
				break;
			}
			if (upperPrompt === null) {
				upperPrompt = row.index;
				continue;
			}
			resolvedBoundary = { kind: 'completed', lower: row.index, upper: upperPrompt };
			break;
		}

		remainingBytes -= result.byteLength;
		boundary = startRow;
		nextPageRows = pageRowLimit;
	}

	if (resolvedBoundary === null) {
		if (boundary > 0n) return { kind: 'unavailable', reason: 'source-limit' };
		if (upperPrompt !== null) {
			resolvedBoundary = { kind: 'truncated', lower: 0n, upper: upperPrompt };
		} else {
			return { kind: 'empty', reason: 'no-shell-integration' };
		}
	}

	const output = projectOutput(collectedRows, resolvedBoundary, snapshot);
	return output ?? { kind: 'empty', reason: 'no-output' };
}

function boundedPageRowCount(boundary: bigint, maximumRows: number): number {
	return boundary > BigInt(maximumRows) ? maximumRows : Number(boundary);
}

function pageMatchesSnapshot(
	page: TerminalBufferRows,
	snapshot: TerminalBufferRowsSnapshot,
	startRow: bigint,
	rowCount: number,
	remainingBytes: number,
): boolean {
	return (
		page.detail === TerminalBufferRowDetail.render &&
		page.generation === snapshot.generation &&
		page.revision === snapshot.revision &&
		page.totalRows === snapshot.totalRows &&
		page.startRow === startRow &&
		page.rows.length === rowCount &&
		page.byteLength >= TERMINAL_ROW_WINDOW_HEADER_BYTES &&
		page.byteLength <= remainingBytes &&
		page.rows.every((row, offset) => row.index === startRow + BigInt(offset) && row.cells !== null)
	);
}

function hasWrittenOutput(row: TerminalBufferRow): boolean {
	return (
		row.cells?.some(
			(cell) =>
				cell.width > 0 && cell.semanticContent === SemanticContent.output && cell.grapheme !== '',
		) === true
	);
}

function projectOutput(
	rows: readonly TerminalBufferRow[],
	boundary: CommandOutputBoundary,
	snapshot: TerminalBufferRowsSnapshot,
): (ProjectedCommandOutput & { readonly kind: 'output' }) | null {
	const orderedRows = rows
		.filter((row) => row.index >= boundary.lower && row.index < boundary.upper)
		.slice()
		.sort((left, right) => {
			if (left.index < right.index) return -1;
			if (left.index > right.index) return 1;
			return 0;
		});
	const outputRows = orderedRows.map((row) => ({ row, text: outputText(row) }));
	const first = outputRows.findIndex((value) => value.text !== null);
	if (first < 0) return null;
	let last = outputRows.length - 1;
	while (last > first && outputRows[last]?.text === null) last -= 1;
	const included = outputRows.slice(first, last + 1);
	let text = '';
	for (const [index, value] of included.entries()) {
		text += value.text ?? '';
		if (index < included.length - 1 && !value.row.wrap) text += '\n';
	}
	const firstRow = included[0]?.row;
	const lastRow = included.at(-1)?.row;
	if (firstRow === undefined || lastRow === undefined) return null;
	return {
		endRow: lastRow.index,
		generation: snapshot.generation,
		kind: 'output',
		revision: snapshot.revision,
		startRow: firstRow.index,
		text,
		truncatedStart: boundary.kind === 'truncated',
	};
}

function outputText(row: TerminalBufferRow): string | null {
	const cells = row.cells;
	if (cells === null) return null;
	let first = -1;
	let last = -1;
	for (const [index, cell] of cells.entries()) {
		if (cell.width > 0 && cell.semanticContent === SemanticContent.output && cell.grapheme !== '') {
			if (first < 0) first = index;
			last = index;
		}
	}
	if (first < 0 || last < first) return null;
	let text = '';
	for (let index = first; index <= last; index += 1) {
		const cell = cells[index];
		if (cell === undefined || cell.width === 0) continue;
		if (cell.semanticContent !== SemanticContent.output) continue;
		text += cell.grapheme || ' ';
	}
	return text;
}
