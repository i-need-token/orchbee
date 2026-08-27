import { RowPromptClass, TERMINAL_ROW_WINDOW_HEADER_BYTES } from '@orchbee/terminal-protocol';
import { describe, expect, it, vi } from 'vitest';

import type {
	BufferRowsResult,
	TerminalBufferRow,
	TerminalBufferRowReader,
	TerminalBufferRowsRequest,
	TerminalBufferRowsSnapshot,
} from '../../buffer-rows';
import { locateProjectedPrompt } from '../projected-prompt-navigation';

const snapshot: TerminalBufferRowsSnapshot = {
	generation: 2n,
	revision: 7n,
	totalRows: 10n,
};

describe('projected prompt navigation', () => {
	it('finds the nearest previous prompt from a strict visible-start anchor', async () => {
		const reader = rowReader([
			row(0n),
			row(1n, RowPromptClass.prompt),
			row(2n),
			row(3n),
			row(4n, RowPromptClass.prompt),
			row(5n),
			row(6n, RowPromptClass.prompt),
			row(7n, RowPromptClass.promptContinuation),
			row(8n, RowPromptClass.prompt),
			row(9n),
		]);

		await expect(
			locateProjectedPrompt({
				anchorRow: 8n,
				direction: 'previous',
				maximumPageRows: 3,
				reader,
				signal: new AbortController().signal,
				snapshot,
			}),
		).resolves.toEqual({ kind: 'target', row: 6n });
		expect(reader.readBufferRows).toHaveBeenCalledOnce();
		expect(reader.readBufferRows).toHaveBeenCalledWith(
			expect.objectContaining({ rowCount: 3, startRow: 5n }),
			expect.any(AbortSignal),
		);
	});

	it('pages outward without gaps and finds the nearest next prompt', async () => {
		const rows = [
			row(0n),
			row(1n, RowPromptClass.prompt),
			row(2n),
			row(3n),
			row(4n, RowPromptClass.prompt),
			row(5n),
			row(6n),
			row(7n),
			row(8n, RowPromptClass.prompt),
			row(9n),
		];
		const previousReader = rowReader(rows);
		const nextReader = rowReader(rows);

		await expect(
			locateProjectedPrompt({
				anchorRow: 8n,
				direction: 'previous',
				maximumPageRows: 3,
				reader: previousReader,
				signal: new AbortController().signal,
				snapshot,
			}),
		).resolves.toEqual({ kind: 'target', row: 4n });
		expect(
			vi.mocked(previousReader.readBufferRows).mock.calls.map(([request]) => request),
		).toMatchObject([
			{ rowCount: 3, startRow: 5n },
			{ rowCount: 3, startRow: 2n },
		]);

		await expect(
			locateProjectedPrompt({
				anchorRow: 4n,
				direction: 'next',
				maximumPageRows: 2,
				reader: nextReader,
				signal: new AbortController().signal,
				snapshot,
			}),
		).resolves.toEqual({ kind: 'target', row: 8n });
		expect(
			vi.mocked(nextReader.readBufferRows).mock.calls.map(([request]) => request),
		).toMatchObject([
			{ rowCount: 2, startRow: 5n },
			{ rowCount: 2, startRow: 7n },
		]);
	});

	it('halves an oversized previous page while retaining the nearest boundary', async () => {
		const rows = Array.from({ length: 10 }, (_, index) =>
			row(BigInt(index), index === 7 ? RowPromptClass.prompt : RowPromptClass.none),
		);
		const reader = rowReader(rows, (request) =>
			request.rowCount === 4
				? {
						generation: snapshot.generation,
						kind: 'too-large',
						revision: snapshot.revision,
						totalRows: snapshot.totalRows,
					}
				: undefined,
		);

		await expect(
			locateProjectedPrompt({
				anchorRow: 8n,
				direction: 'previous',
				maximumPageRows: 4,
				reader,
				signal: new AbortController().signal,
				snapshot,
			}),
		).resolves.toEqual({ kind: 'target', row: 7n });
		expect(vi.mocked(reader.readBufferRows).mock.calls.map(([request]) => request)).toMatchObject([
			{ rowCount: 4, startRow: 4n },
			{ rowCount: 2, startRow: 6n },
		]);
	});

	it('stops before a request cannot fit the remaining encoded-source budget', async () => {
		const reader = rowReader(
			Array.from({ length: 10 }, (_, index) => row(BigInt(index))),
			(request) => rowsResult(request.startRow, request.rowCount, 120),
		);
		const sourceByteLimit = 120 + TERMINAL_ROW_WINDOW_HEADER_BYTES - 1;

		await expect(
			locateProjectedPrompt({
				anchorRow: 0n,
				direction: 'next',
				maximumPageRows: 5,
				reader,
				signal: new AbortController().signal,
				snapshot,
				sourceByteLimit,
			}),
		).resolves.toEqual({ kind: 'unchanged', reason: 'source-limit' });
		expect(reader.readBufferRows).toHaveBeenCalledOnce();
		expect(reader.readBufferRows).toHaveBeenCalledWith(
			expect.objectContaining({ maximumBytes: sourceByteLimit }),
			expect.any(AbortSignal),
		);
	});

	it.each([
		['stale', 'stale'],
		['evicted', 'stale'],
		['unavailable', 'unavailable'],
	] as const)('maps a %s row result to an unchanged %s outcome', async (kind, reason) => {
		const reader = terminalRowReader(async () => ({ ...snapshot, kind }));

		await expect(
			locateProjectedPrompt({
				anchorRow: 5n,
				direction: 'previous',
				maximumPageRows: 2,
				reader,
				signal: new AbortController().signal,
				snapshot,
			}),
		).resolves.toEqual({ kind: 'unchanged', reason });
	});

	it('rejects a noncontiguous page instead of skipping an uninspected gap', async () => {
		const reader = terminalRowReader(async () => rowsResult(2n, 1, 120));

		await expect(
			locateProjectedPrompt({
				anchorRow: 5n,
				direction: 'previous',
				maximumPageRows: 2,
				reader,
				signal: new AbortController().signal,
				snapshot,
			}),
		).resolves.toEqual({ kind: 'unchanged', reason: 'unavailable' });
	});

	it('returns cancellation and boundary outcomes without reading another page', async () => {
		const cancelledReader = terminalRowReader(async () => ({ ...snapshot, kind: 'cancelled' }));
		const boundaryReader = rowReader(Array.from({ length: 10 }, (_, index) => row(BigInt(index))));

		await expect(
			locateProjectedPrompt({
				anchorRow: 5n,
				direction: 'next',
				maximumPageRows: 2,
				reader: cancelledReader,
				signal: new AbortController().signal,
				snapshot,
			}),
		).resolves.toEqual({ kind: 'cancelled' });
		await expect(
			locateProjectedPrompt({
				anchorRow: 0n,
				direction: 'previous',
				maximumPageRows: 2,
				reader: boundaryReader,
				signal: new AbortController().signal,
				snapshot,
			}),
		).resolves.toEqual({ kind: 'unchanged', reason: 'boundary' });
		expect(boundaryReader.readBufferRows).not.toHaveBeenCalled();
	});
});

function terminalRowReader(
	read: (request: TerminalBufferRowsRequest, signal: AbortSignal) => Promise<BufferRowsResult>,
): TerminalBufferRowReader {
	return {
		getBufferRowsSnapshot: () => snapshot,
		readBufferRows: vi.fn<typeof read>(read),
	};
}

function rowReader(
	rows: readonly TerminalBufferRow[],
	override?: (request: TerminalBufferRowsRequest) => BufferRowsResult | undefined,
): TerminalBufferRowReader {
	return terminalRowReader(async (request) => {
		const overridden = override?.(request);
		if (overridden !== undefined) return overridden;
		const start = Number(request.startRow);
		return {
			byteLength: 120,
			detail: request.detail,
			generation: snapshot.generation,
			kind: 'rows',
			revision: snapshot.revision,
			rows: rows.slice(start, start + request.rowCount),
			startRow: request.startRow,
			totalRows: snapshot.totalRows,
		};
	});
}

function rowsResult(startRow: bigint, rowCount: number, byteLength: number): BufferRowsResult {
	return {
		byteLength,
		detail: 'text',
		generation: snapshot.generation,
		kind: 'rows',
		revision: snapshot.revision,
		rows: Array.from({ length: rowCount }, (_, offset) => row(startRow + BigInt(offset))),
		startRow,
		totalRows: snapshot.totalRows,
	};
}

function row(
	index: bigint,
	promptClass: TerminalBufferRow['promptClass'] = RowPromptClass.none,
): TerminalBufferRow {
	return {
		cells: null,
		identityAscii: true,
		index,
		mappings: [],
		promptClass,
		text: '',
		wrap: false,
		wrapContinuation: false,
	};
}
