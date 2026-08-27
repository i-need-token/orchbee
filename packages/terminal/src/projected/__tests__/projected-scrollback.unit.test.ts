import {
	PROTOCOL_LIMITS,
	RowPromptClass,
	TERMINAL_ROW_WINDOW_HEADER_BYTES,
} from '@orchbee/terminal-protocol';
import { describe, expect, it, vi } from 'vitest';

import {
	TerminalBufferRowDetail,
	type BufferRowsResult,
	type TerminalBufferRow,
	type TerminalBufferRowReader,
	type TerminalBufferRowsRequest,
	type TerminalBufferRowsSnapshot,
} from '../../buffer-rows';
import { readProjectedScrollback } from '../projected-scrollback';

const snapshot = {
	generation: 7n,
	revision: 11n,
	totalRows: 4n,
} satisfies TerminalBufferRowsSnapshot;

describe('readProjectedScrollback', () => {
	it('projects every retained row in order and anchors the captured visible top', async () => {
		const rows = [
			row(0n, 'old'),
			row(1n, 'soft-', true),
			row(2n, 'wrapped', false, true),
			row(3n, ''),
		];
		const reader = readerFor(rows, snapshot);

		await expect(
			readProjectedScrollback({
				initialTopRow: 2n,
				maximumPageRows: 2,
				reader,
				signal: new AbortController().signal,
				snapshot,
			}),
		).resolves.toEqual({
			endRow: 3n,
			generation: 7n,
			initialTopOffset: 9,
			kind: 'scrollback',
			revision: 11n,
			startRow: 0n,
			text: 'old\nsoft-wrapped\n',
		});
		expect(reader.readBufferRows).toHaveBeenCalledTimes(2);
		expect(reader.readBufferRows).toHaveBeenNthCalledWith(
			1,
			expect.objectContaining({
				detail: TerminalBufferRowDetail.text,
				generation: 7n,
				purpose: 'buffer',
				revision: 11n,
				rowCount: 2,
				startRow: 0n,
			}),
			expect.any(AbortSignal),
		);
		expect(reader.readBufferRows).toHaveBeenNthCalledWith(
			2,
			expect.objectContaining({ rowCount: 2, startRow: 2n }),
			expect.any(AbortSignal),
		);
	});

	it('keeps a retained blank snapshot inspectable and treats zero rows as empty', async () => {
		await expect(
			readProjectedScrollback(options([row(0n, '')], { ...snapshot, totalRows: 1n })),
		).resolves.toMatchObject({ kind: 'scrollback', text: '' });

		const emptySnapshot = { ...snapshot, totalRows: 0n };
		const reader = readerFor([], emptySnapshot);
		await expect(readProjectedScrollback(options([], emptySnapshot, reader))).resolves.toEqual({
			kind: 'empty',
			reason: 'no-scrollback',
		});
		expect(reader.readBufferRows).not.toHaveBeenCalled();
	});

	it('clamps an anchor beyond the retained snapshot to its last row', async () => {
		const currentSnapshot = { ...snapshot, totalRows: 2n };
		await expect(
			readProjectedScrollback({
				...options([row(0n, 'first'), row(1n, 'last')], currentSnapshot),
				initialTopRow: 99n,
			}),
		).resolves.toMatchObject({ initialTopOffset: 6, text: 'first\nlast' });
	});

	it('halves oversized pages and never publishes a source-limited prefix', async () => {
		const rows = [row(0n, 'a'), row(1n, 'b'), row(2n, 'c'), row(3n, 'd')];
		const reader = readerFor(rows, snapshot);
		vi.mocked(reader.readBufferRows).mockImplementationOnce(async (request) => ({
			generation: request.generation,
			kind: 'too-large',
			revision: request.revision,
			totalRows: snapshot.totalRows,
		}));
		vi.mocked(reader.readBufferRows).mockImplementationOnce(async (request) => ({
			byteLength: TERMINAL_ROW_WINDOW_HEADER_BYTES * 2,
			detail: request.detail,
			generation: request.generation,
			kind: 'rows',
			revision: request.revision,
			rows: rows.slice(0, 2),
			startRow: request.startRow,
			totalRows: snapshot.totalRows,
		}));

		await expect(
			readProjectedScrollback({
				...options(rows, snapshot, reader),
				maximumPageRows: 4,
				sourceByteLimit: TERMINAL_ROW_WINDOW_HEADER_BYTES * 2,
			}),
		).resolves.toEqual({ kind: 'unavailable', reason: 'source-limit' });
		expect(reader.readBufferRows).toHaveBeenNthCalledWith(
			2,
			expect.objectContaining({ rowCount: 2, startRow: 0n }),
			expect.any(AbortSignal),
		);
	});

	it.each([
		['cancelled', { kind: 'cancelled' }],
		['stale', { kind: 'unavailable', reason: 'stale' }],
		['evicted', { kind: 'unavailable', reason: 'stale' }],
		['unavailable', { kind: 'unavailable', reason: 'unavailable' }],
	] as const)('maps a %s page without publishing retained rows', async (kind, expected) => {
		const reader = readerFor([], snapshot);
		vi.mocked(reader.readBufferRows).mockResolvedValueOnce({ ...snapshot, kind });

		await expect(readProjectedScrollback(options([], snapshot, reader))).resolves.toEqual(expected);
	});

	it('cancels before reading and rejects malformed exact-page metadata', async () => {
		const controller = new AbortController();
		controller.abort();
		const cancelledReader = readerFor([], snapshot);
		await expect(
			readProjectedScrollback({
				...options([], snapshot, cancelledReader),
				signal: controller.signal,
			}),
		).resolves.toEqual({ kind: 'cancelled' });
		expect(cancelledReader.readBufferRows).not.toHaveBeenCalled();

		const malformedSnapshot = { ...snapshot, totalRows: 1n };
		const malformedReader = readerFor([row(0n, 'bad')], malformedSnapshot);
		vi.mocked(malformedReader.readBufferRows).mockResolvedValueOnce({
			byteLength: TERMINAL_ROW_WINDOW_HEADER_BYTES,
			detail: TerminalBufferRowDetail.render,
			generation: snapshot.generation,
			kind: 'rows',
			revision: snapshot.revision,
			rows: [row(0n, 'bad')],
			startRow: 0n,
			totalRows: 1n,
		});
		await expect(
			readProjectedScrollback(options([row(0n, 'bad')], malformedSnapshot, malformedReader)),
		).resolves.toEqual({ kind: 'unavailable', reason: 'unavailable' });
	});
});

function options(
	rows: readonly TerminalBufferRow[],
	currentSnapshot: TerminalBufferRowsSnapshot = snapshot,
	reader = readerFor(rows, currentSnapshot),
): Parameters<typeof readProjectedScrollback>[0] {
	return {
		initialTopRow: 0n,
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
	text: string,
	wrap = false,
	wrapContinuation = false,
): TerminalBufferRow {
	return {
		cells: null,
		index,
		mappings: [],
		promptClass: RowPromptClass.none,
		text,
		wrap,
		wrapContinuation,
	};
}
