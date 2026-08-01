import { RowPromptClass } from '@orchbee/terminal-protocol';
import { afterEach, describe, expect, it, vi } from 'vitest';

import type {
	BufferRowsResult,
	TerminalBufferRow,
	TerminalBufferRowReader,
} from '../../buffer-rows';
import {
	ProjectedSearchController,
	type ProjectedSearchControllerOptions,
} from '../projected-search-controller';
import type {
	ProjectedSearchWorkerLike,
	ProjectedSearchWorkerRequest,
	ProjectedSearchWorkerResponse,
} from '../projected-search-protocol';

afterEach(() => {
	vi.useRealTimers();
});

describe('ProjectedSearchController', () => {
	it('creates the Worker lazily, pages one immutable revision, and reveals the first result', async () => {
		vi.useFakeTimers();
		const worker = createFakeWorker(true);
		const createWorker = vi.fn<ProjectedSearchControllerOptions['createWorker']>(
			() => worker.value,
		);
		const reader = createReader([bufferRow(0n, 'alpha'), bufferRow(1n, 'needle')]);
		const changes = vi.fn<ProjectedSearchControllerOptions['onChange']>();
		const reveal = vi.fn<ProjectedSearchControllerOptions['onReveal']>();
		const controller = new ProjectedSearchController({
			createWorker,
			onChange: changes,
			onReveal: reveal,
			reader,
		});

		controller.update('needle');
		expect(createWorker).not.toHaveBeenCalled();
		expect(changes).toHaveBeenLastCalledWith(expect.objectContaining({ state: 'searching' }));

		await vi.advanceTimersByTimeAsync(200);

		expect(createWorker).toHaveBeenCalledOnce();
		expect(reader.readBufferRows).toHaveBeenCalledOnce();
		expect(worker.messages[0]).toMatchObject({
			generation: '2',
			kind: 'start',
			query: 'needle',
			revision: '7',
		});
		expect(worker.messages[1]).toMatchObject({ final: true, kind: 'page' });
		expect(reveal).toHaveBeenCalledWith(expect.objectContaining({ start: { column: 0, row: 1 } }));
		expect(changes).toHaveBeenLastCalledWith(
			expect.objectContaining({ activeIndex: 0, state: 'complete' }),
		);
		expect(worker.terminated).toBe(true);
	});

	it('retries a bounded page with fewer rows after a too-large result', async () => {
		vi.useFakeTimers();
		const worker = createFakeWorker(true);
		let call = 0;
		const reader = createReader([bufferRow(0n, 'value')], () => {
			call += 1;
			if (call === 1) {
				return { generation: 2n, kind: 'too-large', revision: 7n, totalRows: 1n };
			}
			return rowsResult([bufferRow(0n, 'value')]);
		});
		const controller = new ProjectedSearchController({
			createWorker: () => worker.value,
			onChange: () => {},
			onReveal: () => {},
			reader,
		});

		controller.update('value');
		await vi.advanceTimersByTimeAsync(200);

		expect(reader.readBufferRows).toHaveBeenCalledTimes(2);
		expect(reader.readBufferRows.mock.calls[0]?.[0].rowCount).toBe(1);
		expect(reader.readBufferRows.mock.calls[1]?.[0].rowCount).toBe(1);
	});

	it('adapts later search pages toward the bounded byte budget', async () => {
		vi.useFakeTimers();
		const worker = createFakeWorker(true);
		const rows = Array.from({ length: 5000 }, (_, index) => bufferRow(BigInt(index), 'value'));
		const readBufferRows = vi.fn<TerminalBufferRowReader['readBufferRows']>(async (request) => {
			const start = Number(request.startRow);
			const page = rows.slice(start, start + request.rowCount);
			return {
				byteLength: page.length * 1024,
				detail: 'text',
				generation: 2n,
				kind: 'rows',
				revision: 7n,
				rows: page,
				startRow: request.startRow,
				totalRows: BigInt(rows.length),
			};
		});
		const controller = new ProjectedSearchController({
			createWorker: () => worker.value,
			onChange: () => {},
			onReveal: () => {},
			reader: {
				getBufferRowsSnapshot: () => ({
					generation: 2n,
					revision: 7n,
					totalRows: BigInt(rows.length),
				}),
				readBufferRows,
			},
		});

		controller.update('value');
		await vi.advanceTimersByTimeAsync(200);

		expect(readBufferRows).toHaveBeenCalledTimes(3);
		expect(readBufferRows.mock.calls[0]?.[0].rowCount).toBe(1400);
		expect(readBufferRows.mock.calls[1]?.[0].rowCount).toBe(1945);
		expect(readBufferRows.mock.calls[2]?.[0].rowCount).toBe(1655);
	});

	it('keeps compact ASCII pages below the logical mapping-entry budget', async () => {
		vi.useFakeTimers();
		const worker = createFakeWorker(true);
		const text = 'x'.repeat(113);
		const rows = Array.from({ length: 5000 }, (_, index) => identityBufferRow(BigInt(index), text));
		const readBufferRows = vi.fn<TerminalBufferRowReader['readBufferRows']>(async (request) => {
			const start = Number(request.startRow);
			const page = rows.slice(start, start + request.rowCount);
			return {
				byteLength: page.length * 128,
				detail: 'text',
				generation: 2n,
				kind: 'rows',
				revision: 7n,
				rows: page,
				startRow: request.startRow,
				totalRows: BigInt(rows.length),
			};
		});
		const controller = new ProjectedSearchController({
			createWorker: () => worker.value,
			onChange: () => {},
			onReveal: () => {},
			reader: {
				getBufferRowsSnapshot: () => ({
					generation: 2n,
					revision: 7n,
					totalRows: BigInt(rows.length),
				}),
				readBufferRows,
			},
		});

		controller.update('missing');
		await vi.advanceTimersByTimeAsync(200);

		expect(readBufferRows).toHaveBeenCalledTimes(3);
		expect(readBufferRows.mock.calls[0]?.[0].rowCount).toBe(1400);
		expect(readBufferRows.mock.calls[1]?.[0].rowCount).toBe(2203);
		expect(readBufferRows.mock.calls[2]?.[0].rowCount).toBe(1397);
	});

	it('terminates a stuck Worker at the watchdog boundary', async () => {
		vi.useFakeTimers();
		const worker = createFakeWorker(false);
		const changes = vi.fn<ProjectedSearchControllerOptions['onChange']>();
		const controller = new ProjectedSearchController({
			createWorker: () => worker.value,
			onChange: changes,
			onReveal: () => {},
			reader: createReader([bufferRow(0n, 'value')]),
		});

		controller.update('value');
		await vi.advanceTimersByTimeAsync(200);
		await vi.advanceTimersByTimeAsync(10_000);

		expect(worker.terminated).toBe(true);
		expect(changes).toHaveBeenLastCalledWith(expect.objectContaining({ state: 'timeout' }));
	});
});

function createReader(
	rows: readonly TerminalBufferRow[],
	resultFactory: (() => BufferRowsResult) | undefined = undefined,
): TerminalBufferRowReader & {
	readonly readBufferRows: ReturnType<typeof vi.fn<TerminalBufferRowReader['readBufferRows']>>;
} {
	const readBufferRows = vi.fn<TerminalBufferRowReader['readBufferRows']>(async () =>
		resultFactory ? resultFactory() : rowsResult(rows),
	);
	return {
		getBufferRowsSnapshot: () => ({ generation: 2n, revision: 7n, totalRows: BigInt(rows.length) }),
		readBufferRows,
	};
}

function rowsResult(rows: readonly TerminalBufferRow[]): BufferRowsResult {
	return {
		byteLength: 128,
		detail: 'text',
		generation: 2n,
		kind: 'rows',
		revision: 7n,
		rows,
		startRow: 0n,
		totalRows: BigInt(rows.length),
	};
}

function bufferRow(index: bigint, text: string): TerminalBufferRow {
	return {
		cells: null,
		index,
		mappings: Array.from(text, (_character, column) => ({
			byteOffset: column,
			column,
			utf16Offset: column,
			width: 1 as const,
		})),
		promptClass: RowPromptClass.none,
		text,
		wrap: false,
		wrapContinuation: false,
	};
}

function identityBufferRow(index: bigint, text: string): TerminalBufferRow {
	return {
		cells: null,
		identityAscii: true,
		index,
		mappings: [],
		promptClass: RowPromptClass.none,
		text,
		wrap: false,
		wrapContinuation: false,
	};
}

function createFakeWorker(respond: boolean): {
	readonly messages: ProjectedSearchWorkerRequest[];
	readonly terminated: boolean;
	readonly value: ProjectedSearchWorkerLike;
} {
	const messages: ProjectedSearchWorkerRequest[] = [];
	const messageListeners: Array<(event: MessageEvent<ProjectedSearchWorkerResponse>) => void> = [];
	let terminated = false;
	let start: Extract<ProjectedSearchWorkerRequest, { kind: 'start' }> | undefined;
	const value = {
		addEventListener: (
			type: 'error' | 'message',
			listener:
				| ((event: ErrorEvent) => void)
				| ((event: MessageEvent<ProjectedSearchWorkerResponse>) => void),
		) => {
			if (type === 'message') {
				messageListeners.push(
					listener as (event: MessageEvent<ProjectedSearchWorkerResponse>) => void,
				);
			}
		},
		postMessage: (message: ProjectedSearchWorkerRequest) => {
			messages.push(message);
			if (message.kind === 'start') start = message;
			if (!respond || message.kind !== 'page' || !message.final || start === undefined) return;
			const match = {
				end: { column: 5, row: 1 },
				start: { column: 0, row: 1 },
				text: 'needle',
			};
			const response: ProjectedSearchWorkerResponse = {
				generation: start.generation,
				kind: 'result',
				matches: [match],
				revision: start.revision,
				searchId: start.searchId,
			};
			for (const listener of messageListeners) {
				listener({ data: response } as MessageEvent<ProjectedSearchWorkerResponse>);
			}
		},
		terminate: () => {
			terminated = true;
		},
	} as ProjectedSearchWorkerLike;
	return {
		messages,
		get terminated() {
			return terminated;
		},
		value,
	};
}
