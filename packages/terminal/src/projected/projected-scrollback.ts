import { PROTOCOL_LIMITS, TERMINAL_ROW_WINDOW_HEADER_BYTES } from '@orchbee/terminal-protocol';

import {
	TerminalBufferRowDetail,
	type TerminalBufferRow,
	type TerminalBufferRowReader,
	type TerminalBufferRows,
	type TerminalBufferRowsSnapshot,
} from '../buffer-rows';

const PROJECTED_SCROLLBACK_SOURCE_BYTE_LIMIT = 16 * 1024 * 1024;
export const PROJECTED_SCROLLBACK_PAGE_ROWS = 1024;

export interface ProjectedScrollback {
	readonly endRow: bigint;
	readonly generation: bigint;
	readonly initialTopOffset: number;
	readonly revision: bigint;
	readonly startRow: 0n;
	readonly text: string;
}

export type ProjectedScrollbackReadResult =
	| { readonly kind: 'cancelled' }
	| { readonly kind: 'empty'; readonly reason: 'no-scrollback' }
	| (ProjectedScrollback & { readonly kind: 'scrollback' })
	| {
			readonly kind: 'unavailable';
			readonly reason: 'alternate-screen' | 'source-limit' | 'stale' | 'unavailable';
	  };

export interface ReadProjectedScrollbackOptions {
	readonly initialTopRow: bigint;
	readonly maximumPageRows: number;
	readonly reader: TerminalBufferRowReader;
	readonly signal: AbortSignal;
	readonly snapshot: TerminalBufferRowsSnapshot;
	readonly sourceByteLimit?: number | undefined;
}

/** Read and project one complete generation/revision-pinned retained scrollback snapshot. */
export async function readProjectedScrollback({
	initialTopRow,
	maximumPageRows,
	reader,
	signal,
	snapshot,
	sourceByteLimit = PROJECTED_SCROLLBACK_SOURCE_BYTE_LIMIT,
}: ReadProjectedScrollbackOptions): Promise<ProjectedScrollbackReadResult> {
	if (signal.aborted) return { kind: 'cancelled' };
	if (
		snapshot.generation <= 0n ||
		snapshot.revision < 0n ||
		snapshot.totalRows < 0n ||
		initialTopRow < 0n ||
		!Number.isInteger(maximumPageRows) ||
		maximumPageRows < 1 ||
		!Number.isInteger(sourceByteLimit) ||
		sourceByteLimit < 1
	) {
		return { kind: 'unavailable', reason: 'unavailable' };
	}
	if (snapshot.totalRows === 0n) return { kind: 'empty', reason: 'no-scrollback' };

	const pageRowLimit = Math.min(maximumPageRows, PROTOCOL_LIMITS.maxRowWindowRows);
	let remainingBytes = Math.min(sourceByteLimit, PROJECTED_SCROLLBACK_SOURCE_BYTE_LIMIT);
	let nextPageRows = pageRowLimit;
	let startRow = 0n;
	const collectedRows: TerminalBufferRow[] = [];

	while (startRow < snapshot.totalRows) {
		if (signal.aborted) return { kind: 'cancelled' };
		if (remainingBytes < TERMINAL_ROW_WINDOW_HEADER_BYTES) {
			return { kind: 'unavailable', reason: 'source-limit' };
		}
		const rowCount = boundedPageRowCount(snapshot.totalRows - startRow, nextPageRows);
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
		if (
			result.kind !== 'rows' ||
			!pageMatchesSnapshot(result, snapshot, startRow, rowCount, remainingBytes)
		) {
			return { kind: 'unavailable', reason: 'unavailable' };
		}

		collectedRows.push(...result.rows);
		remainingBytes -= result.byteLength;
		startRow += BigInt(rowCount);
		nextPageRows = pageRowLimit;
	}

	return projectScrollback(collectedRows, initialTopRow, snapshot);
}

function boundedPageRowCount(remainingRows: bigint, maximumRows: number): number {
	return remainingRows > BigInt(maximumRows) ? maximumRows : Number(remainingRows);
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
		page.rows.every((row, offset) => row.index === startRow + BigInt(offset) && row.cells === null)
	);
}

function projectScrollback(
	rows: readonly TerminalBufferRow[],
	initialTopRow: bigint,
	snapshot: TerminalBufferRowsSnapshot,
): ProjectedScrollback & { readonly kind: 'scrollback' } {
	const maximumAnchor = snapshot.totalRows - 1n;
	const anchorRow = initialTopRow > maximumAnchor ? maximumAnchor : initialTopRow;
	let initialTopOffset = 0;
	let text = '';
	for (const row of rows) {
		if (row.index === anchorRow) initialTopOffset = text.length;
		text += row.text;
		if (row.index < maximumAnchor && !row.wrap) text += '\n';
	}
	return {
		endRow: maximumAnchor,
		generation: snapshot.generation,
		initialTopOffset,
		kind: 'scrollback',
		revision: snapshot.revision,
		startRow: 0n,
		text,
	};
}
