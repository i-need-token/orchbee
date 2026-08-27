import {
	PROTOCOL_LIMITS,
	RowPromptClass,
	TERMINAL_ROW_WINDOW_HEADER_BYTES,
} from '@orchbee/terminal-protocol';

import {
	TerminalBufferRowDetail,
	type TerminalBufferRow,
	type TerminalBufferRowReader,
	type TerminalBufferRows,
	type TerminalBufferRowsSnapshot,
} from '../buffer-rows';

export const PROJECTED_PROMPT_NAVIGATION_SOURCE_BYTE_LIMIT = 16 * 1024 * 1024;
export const PROJECTED_PROMPT_NAVIGATION_PAGE_ROWS = 1024;

export type ProjectedPromptNavigationDirection = 'next' | 'previous';

export type ProjectedPromptNavigationResult =
	| { readonly kind: 'cancelled' }
	| { readonly kind: 'target'; readonly row: bigint }
	| {
			readonly kind: 'unchanged';
			readonly reason: 'boundary' | 'not-found' | 'source-limit' | 'stale' | 'unavailable';
	  };

export interface LocateProjectedPromptOptions {
	readonly anchorRow: bigint;
	readonly direction: ProjectedPromptNavigationDirection;
	readonly maximumPageRows: number;
	readonly reader: TerminalBufferRowReader;
	readonly signal: AbortSignal;
	readonly snapshot: TerminalBufferRowsSnapshot;
	readonly sourceByteLimit?: number | undefined;
}

/** Locate one nearest Host-authored OSC 133 prompt without mutating terminal state. */
export async function locateProjectedPrompt({
	anchorRow,
	direction,
	maximumPageRows,
	reader,
	signal,
	snapshot,
	sourceByteLimit = PROJECTED_PROMPT_NAVIGATION_SOURCE_BYTE_LIMIT,
}: LocateProjectedPromptOptions): Promise<ProjectedPromptNavigationResult> {
	if (signal.aborted) return { kind: 'cancelled' };
	if (
		anchorRow < 0n ||
		snapshot.generation <= 0n ||
		snapshot.revision < 0n ||
		snapshot.totalRows < 0n ||
		!Number.isInteger(maximumPageRows) ||
		maximumPageRows < 1 ||
		!Number.isInteger(sourceByteLimit) ||
		sourceByteLimit < 1
	) {
		return { kind: 'unchanged', reason: 'unavailable' };
	}
	const pageRowLimit = Math.min(maximumPageRows, PROTOCOL_LIMITS.maxRowWindowRows);
	const initial = initialBoundary(direction, anchorRow, snapshot.totalRows);
	if (initial === null) return { kind: 'unchanged', reason: 'boundary' };
	let boundary = initial;
	let remainingBytes = Math.min(sourceByteLimit, PROJECTED_PROMPT_NAVIGATION_SOURCE_BYTE_LIMIT);
	let nextPageRows = pageRowLimit;

	while (hasRemainingDomain(direction, boundary, snapshot.totalRows)) {
		if (signal.aborted) return { kind: 'cancelled' };
		if (remainingBytes < TERMINAL_ROW_WINDOW_HEADER_BYTES) {
			return { kind: 'unchanged', reason: 'source-limit' };
		}
		const rowCount = boundedPageRowCount(direction, boundary, snapshot.totalRows, nextPageRows);
		const startRow = direction === 'previous' ? boundary - BigInt(rowCount) : boundary;
		let result: Awaited<ReturnType<TerminalBufferRowReader['readBufferRows']>>;
		try {
			result = await reader.readBufferRows(
				{
					detail: TerminalBufferRowDetail.text,
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
			return signal.aborted ? { kind: 'cancelled' } : { kind: 'unchanged', reason: 'unavailable' };
		}
		if (signal.aborted || result.kind === 'cancelled') return { kind: 'cancelled' };
		if (result.kind === 'stale' || result.kind === 'evicted') {
			return { kind: 'unchanged', reason: 'stale' };
		}
		if (result.kind === 'unavailable') {
			return { kind: 'unchanged', reason: 'unavailable' };
		}
		if (result.kind === 'too-large') {
			if (rowCount === 1) return { kind: 'unchanged', reason: 'source-limit' };
			nextPageRows = Math.max(1, Math.floor(rowCount / 2));
			continue;
		}
		if (result.kind !== 'rows') return { kind: 'unchanged', reason: 'unavailable' };
		if (!pageMatchesSnapshot(result, snapshot, startRow, rowCount, remainingBytes)) {
			return { kind: 'unchanged', reason: 'unavailable' };
		}
		const target = promptRow(result.rows, direction);
		if (target !== null) return { kind: 'target', row: target };
		remainingBytes -= result.byteLength;
		boundary = direction === 'previous' ? startRow : startRow + BigInt(rowCount);
		nextPageRows = pageRowLimit;
	}

	return { kind: 'unchanged', reason: 'not-found' };
}

function initialBoundary(
	direction: ProjectedPromptNavigationDirection,
	anchorRow: bigint,
	totalRows: bigint,
): bigint | null {
	if (totalRows === 0n) return null;
	if (direction === 'previous') {
		if (anchorRow === 0n) return null;
		return anchorRow < totalRows ? anchorRow : totalRows;
	}
	const firstCandidate = anchorRow + 1n;
	return firstCandidate < totalRows ? firstCandidate : null;
}

function hasRemainingDomain(
	direction: ProjectedPromptNavigationDirection,
	boundary: bigint,
	totalRows: bigint,
): boolean {
	return direction === 'previous' ? boundary > 0n : boundary < totalRows;
}

function boundedPageRowCount(
	direction: ProjectedPromptNavigationDirection,
	boundary: bigint,
	totalRows: bigint,
	maximumRows: number,
): number {
	const available = direction === 'previous' ? boundary : totalRows - boundary;
	return available > BigInt(maximumRows) ? maximumRows : Number(available);
}

function pageMatchesSnapshot(
	page: TerminalBufferRows,
	snapshot: TerminalBufferRowsSnapshot,
	startRow: bigint,
	rowCount: number,
	remainingBytes: number,
): boolean {
	return (
		page.detail === TerminalBufferRowDetail.text &&
		page.generation === snapshot.generation &&
		page.revision === snapshot.revision &&
		page.totalRows === snapshot.totalRows &&
		page.startRow === startRow &&
		page.rows.length === rowCount &&
		page.byteLength >= TERMINAL_ROW_WINDOW_HEADER_BYTES &&
		page.byteLength <= remainingBytes &&
		page.rows.every((row, offset) => row.index === startRow + BigInt(offset))
	);
}

function promptRow(
	rows: readonly TerminalBufferRow[],
	direction: ProjectedPromptNavigationDirection,
): bigint | null {
	if (direction === 'next') {
		for (const row of rows) {
			if (row.promptClass === RowPromptClass.prompt) return row.index;
		}
		return null;
	}
	for (let index = rows.length - 1; index >= 0; index -= 1) {
		const row = rows[index];
		if (row?.promptClass === RowPromptClass.prompt) return row.index;
	}
	return null;
}
