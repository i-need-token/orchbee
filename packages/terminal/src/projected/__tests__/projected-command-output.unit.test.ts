import {
	PROTOCOL_LIMITS,
	RowPromptClass,
	SemanticContent,
	TERMINAL_ROW_WINDOW_HEADER_BYTES,
} from '@orchbee/terminal-protocol';
import { describe, expect, it, vi } from 'vitest';

import { readProjectedLastCommandOutput } from '../projected-command-output';
import {
	TerminalBufferRowDetail,
	type BufferRowsResult,
	type TerminalBufferRenderCell,
	type TerminalBufferRow,
	type TerminalBufferRowReader,
	type TerminalBufferRowsRequest,
	type TerminalBufferRowsSnapshot,
} from '../../buffer-rows';

const snapshot = {
	generation: 7n,
	revision: 11n,
	totalRows: 6n,
} satisfies TerminalBufferRowsSnapshot;

describe('readProjectedLastCommandOutput', () => {
	it('returns only the latest completed semantic output and rejoins soft-wrapped rows', async () => {
		const rows = [
			row(0n, [{ grapheme: '$', semanticContent: SemanticContent.prompt }], RowPromptClass.prompt),
			row(1n, [{ grapheme: 'old', semanticContent: SemanticContent.output }]),
			row(
				2n,
				[
					{ grapheme: '$', semanticContent: SemanticContent.prompt },
					{ grapheme: 'run', semanticContent: SemanticContent.input },
				],
				RowPromptClass.prompt,
			),
			row(3n, [{ grapheme: 'latest-', semanticContent: SemanticContent.output }], undefined, true),
			row(
				4n,
				[{ grapheme: 'output', semanticContent: SemanticContent.output }],
				undefined,
				false,
				true,
			),
			row(5n, [{ grapheme: '$', semanticContent: SemanticContent.prompt }], RowPromptClass.prompt),
		];

		await expect(readProjectedLastCommandOutput(options(rows))).resolves.toEqual({
			endRow: 4n,
			generation: 7n,
			kind: 'output',
			revision: 11n,
			startRow: 3n,
			text: 'latest-output',
			truncatedStart: false,
		});
	});

	it('returns output written after the newest prompt without waiting for another prompt', async () => {
		const rows = [
			row(0n, [{ grapheme: '$', semanticContent: SemanticContent.prompt }], RowPromptClass.prompt),
			row(1n, [{ grapheme: 'done', semanticContent: SemanticContent.output }]),
			row(2n, [{ grapheme: '$', semanticContent: SemanticContent.prompt }], RowPromptClass.prompt),
			row(3n, [{ grapheme: 'running', semanticContent: SemanticContent.output }]),
		];

		await expect(
			readProjectedLastCommandOutput(
				options(rows, { ...snapshot, totalRows: BigInt(rows.length) }),
			),
		).resolves.toMatchObject({
			endRow: 3n,
			kind: 'output',
			startRow: 3n,
			text: 'running',
			truncatedStart: false,
		});
	});

	it('keeps an empty latest command empty instead of falling back to older output', async () => {
		const rows = [
			row(0n, [{ grapheme: '$', semanticContent: SemanticContent.prompt }], RowPromptClass.prompt),
			row(1n, [{ grapheme: 'older', semanticContent: SemanticContent.output }]),
			row(2n, [{ grapheme: '$', semanticContent: SemanticContent.prompt }], RowPromptClass.prompt),
			row(3n, [{ grapheme: '$', semanticContent: SemanticContent.prompt }], RowPromptClass.prompt),
		];

		await expect(
			readProjectedLastCommandOutput(
				options(rows, { ...snapshot, totalRows: BigInt(rows.length) }),
			),
		).resolves.toEqual({ kind: 'empty', reason: 'no-output' });
	});

	it('requires prompt evidence and marks an evicted lower boundary as truncated', async () => {
		const withoutPrompt = [
			row(0n, [{ grapheme: 'plain', semanticContent: SemanticContent.output }]),
		];
		await expect(
			readProjectedLastCommandOutput(options(withoutPrompt, { ...snapshot, totalRows: 1n })),
		).resolves.toEqual({ kind: 'empty', reason: 'no-shell-integration' });

		const evictedPrompt = [
			row(0n, [{ grapheme: 'retained', semanticContent: SemanticContent.output }]),
			row(1n, [{ grapheme: 'tail', semanticContent: SemanticContent.output }]),
			row(2n, [{ grapheme: '$', semanticContent: SemanticContent.prompt }], RowPromptClass.prompt),
		];
		await expect(
			readProjectedLastCommandOutput(options(evictedPrompt, { ...snapshot, totalRows: 3n })),
		).resolves.toMatchObject({
			kind: 'output',
			startRow: 0n,
			text: 'retained\ntail',
			truncatedStart: true,
		});
	});

	it('reads newest pages first and pins every request to one snapshot', async () => {
		const rows = [
			row(0n, [{ grapheme: '$', semanticContent: SemanticContent.prompt }], RowPromptClass.prompt),
			row(1n, [{ grapheme: 'wanted', semanticContent: SemanticContent.output }]),
			row(2n, [{ grapheme: '$', semanticContent: SemanticContent.prompt }], RowPromptClass.prompt),
			row(3n, []),
			row(4n, []),
			row(5n, []),
		];
		const reader = readerFor(rows, snapshot);

		await expect(
			readProjectedLastCommandOutput({
				maximumPageRows: 2,
				reader,
				signal: new AbortController().signal,
				snapshot,
			}),
		).resolves.toMatchObject({ kind: 'output', text: 'wanted' });
		expect(reader.readBufferRows).toHaveBeenCalledTimes(3);
		expect(reader.readBufferRows).toHaveBeenNthCalledWith(
			1,
			expect.objectContaining({
				detail: TerminalBufferRowDetail.render,
				generation: 7n,
				purpose: 'buffer',
				revision: 11n,
				rowCount: 2,
				startRow: 4n,
			}),
			expect.any(AbortSignal),
		);
		expect(reader.readBufferRows).toHaveBeenNthCalledWith(
			3,
			expect.objectContaining({ rowCount: 2, startRow: 0n }),
			expect.any(AbortSignal),
		);
	});

	it('halves an oversized page and refuses a partial result at the source limit', async () => {
		const rows = [
			row(0n, [{ grapheme: '$', semanticContent: SemanticContent.prompt }], RowPromptClass.prompt),
			row(1n, [{ grapheme: 'output', semanticContent: SemanticContent.output }]),
			row(2n, []),
			row(3n, []),
		];
		const reader = readerFor(rows, { ...snapshot, totalRows: 4n });
		vi.mocked(reader.readBufferRows).mockImplementationOnce(async (request) => ({
			generation: request.generation,
			kind: 'too-large',
			revision: request.revision,
			totalRows: 4n,
		}));
		vi.mocked(reader.readBufferRows).mockImplementationOnce(async (request) => ({
			byteLength: TERMINAL_ROW_WINDOW_HEADER_BYTES * 2,
			detail: request.detail,
			generation: request.generation,
			kind: 'rows',
			revision: request.revision,
			rows: rows.slice(2),
			startRow: request.startRow,
			totalRows: 4n,
		}));

		await expect(
			readProjectedLastCommandOutput({
				maximumPageRows: 4,
				reader,
				signal: new AbortController().signal,
				snapshot: { ...snapshot, totalRows: 4n },
				sourceByteLimit: TERMINAL_ROW_WINDOW_HEADER_BYTES * 2,
			}),
		).resolves.toEqual({ kind: 'unavailable', reason: 'source-limit' });
		expect(reader.readBufferRows).toHaveBeenNthCalledWith(
			2,
			expect.objectContaining({ rowCount: 2, startRow: 2n }),
			expect.any(AbortSignal),
		);
	});

	it.each([
		['cancelled', { kind: 'cancelled' }],
		['stale', { kind: 'unavailable', reason: 'stale' }],
		['evicted', { kind: 'unavailable', reason: 'stale' }],
		['unavailable', { kind: 'unavailable', reason: 'unavailable' }],
	] as const)('maps a %s row result without publishing retained rows', async (kind, expected) => {
		const reader = readerFor([], snapshot);
		vi.mocked(reader.readBufferRows).mockResolvedValueOnce({ ...snapshot, kind });

		await expect(readProjectedLastCommandOutput(options([], snapshot, reader))).resolves.toEqual(
			expected,
		);
	});

	it('cancels before reading and rejects malformed exact-page metadata', async () => {
		const controller = new AbortController();
		controller.abort();
		const cancelledReader = readerFor([], snapshot);
		await expect(
			readProjectedLastCommandOutput({
				maximumPageRows: 10,
				reader: cancelledReader,
				signal: controller.signal,
				snapshot,
			}),
		).resolves.toEqual({ kind: 'cancelled' });
		expect(cancelledReader.readBufferRows).not.toHaveBeenCalled();

		const malformedReader = readerFor([row(0n, [])], { ...snapshot, totalRows: 1n });
		vi.mocked(malformedReader.readBufferRows).mockResolvedValueOnce({
			byteLength: TERMINAL_ROW_WINDOW_HEADER_BYTES,
			detail: TerminalBufferRowDetail.text,
			generation: 7n,
			kind: 'rows',
			revision: 11n,
			rows: [row(0n, [])],
			startRow: 0n,
			totalRows: 1n,
		});
		await expect(
			readProjectedLastCommandOutput(
				options([row(0n, [])], { ...snapshot, totalRows: 1n }, malformedReader),
			),
		).resolves.toEqual({ kind: 'unavailable', reason: 'unavailable' });
	});
});

function options(
	rows: readonly TerminalBufferRow[],
	currentSnapshot: TerminalBufferRowsSnapshot = snapshot,
	reader = readerFor(rows, currentSnapshot),
): Parameters<typeof readProjectedLastCommandOutput>[0] {
	return {
		maximumPageRows: PROTOCOL_LIMITS.maxRowWindowRows,
		reader,
		signal: new AbortController().signal,
		snapshot: currentSnapshot,
	};
}

function readerFor(
	rows: readonly TerminalBufferRow[],
	currentSnapshot: TerminalBufferRowsSnapshot,
): TerminalBufferRowReader {
	return {
		getBufferRowsSnapshot: () => currentSnapshot,
		readBufferRows: vi.fn<(request: TerminalBufferRowsRequest) => Promise<BufferRowsResult>>(
			async (request: TerminalBufferRowsRequest): Promise<BufferRowsResult> => ({
				byteLength: TERMINAL_ROW_WINDOW_HEADER_BYTES,
				detail: request.detail,
				generation: request.generation,
				kind: 'rows',
				revision: request.revision,
				rows: rows.slice(Number(request.startRow), Number(request.startRow) + request.rowCount),
				startRow: request.startRow,
				totalRows: currentSnapshot.totalRows,
			}),
		),
	};
}

function row(
	index: bigint,
	values: readonly Pick<TerminalBufferRenderCell, 'grapheme' | 'semanticContent'>[],
	promptClass: TerminalBufferRow['promptClass'] = RowPromptClass.none,
	wrap = false,
	wrapContinuation = false,
): TerminalBufferRow {
	const cells = values.map((value, column) => ({
		column,
		grapheme: value.grapheme,
		hyperlink: null,
		semanticContent: value.semanticContent,
		width: 1 as const,
	}));
	return {
		cells,
		index,
		mappings: [],
		promptClass,
		text: cells.map((cell) => cell.grapheme).join(''),
		wrap,
		wrapContinuation,
	};
}
