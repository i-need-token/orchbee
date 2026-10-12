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
	it('starts the lazy Worker after a 50-millisecond incremental-input debounce', async () => {
		vi.useFakeTimers();
		const worker = createFakeWorker(true);
		const createWorker = vi.fn<ProjectedSearchControllerOptions['createWorker']>(
			() => worker.value,
		);
		const reader = createReader([bufferRow(0n, 'alpha'), bufferRow(1n, 'needle')]);
		const changes = vi.fn<ProjectedSearchControllerOptions['onChange']>();
		const activate = vi.fn<ProjectedSearchControllerOptions['onActivate']>();
		const controller = new ProjectedSearchController({
			createWorker,
			onActivate: activate,
			onChange: changes,
			reader,
		});

		controller.update('needle');
		expect(createWorker).not.toHaveBeenCalled();
		expect(changes).toHaveBeenLastCalledWith(expect.objectContaining({ state: 'searching' }));

		await vi.advanceTimersByTimeAsync(49);
		expect(createWorker).not.toHaveBeenCalled();
		await vi.advanceTimersByTimeAsync(1);

		expect(createWorker).toHaveBeenCalledOnce();
		expect(reader.readBufferRows).toHaveBeenCalledOnce();
		expect(worker.messages[0]).toMatchObject({
			generation: '2',
			kind: 'start',
			query: 'needle',
			revision: '7',
		});
		expect(worker.messages[1]).toMatchObject({ final: true, kind: 'page' });
		expect(activate).toHaveBeenCalledWith(
			expect.objectContaining({ start: { column: 0, row: 1 } }),
			{
				generation: 2n,
				revision: 7n,
			},
			true,
		);
		expect(changes).toHaveBeenLastCalledWith(
			expect.objectContaining({ activeIndex: 0, state: 'complete' }),
		);
		expect(worker.terminated).toBe(true);
	});

	it('does not let sustained revision invalidation postpone the scheduled query scan', async () => {
		vi.useFakeTimers();
		const worker = createFakeWorker(true);
		let revision = 7n;
		const rows = [bufferRow(0n, 'needle')];
		const readBufferRows = vi.fn<TerminalBufferRowReader['readBufferRows']>(async (request) => ({
			...rowsResult(rows),
			revision: request.revision,
		}));
		const changes = vi.fn<ProjectedSearchControllerOptions['onChange']>();
		const controller = new ProjectedSearchController({
			createWorker: () => worker.value,
			onActivate: () => {},
			onChange: changes,
			reader: {
				getBufferRowsSnapshot: () => ({ generation: 2n, revision, totalRows: 1n }),
				readBufferRows,
			},
		});

		controller.update('needle');
		for (let invalidation = 0; invalidation < 4; invalidation++) {
			await vi.advanceTimersByTimeAsync(10);
			revision += 1n;
			controller.invalidate();
		}
		await vi.advanceTimersByTimeAsync(10);

		expect(readBufferRows).toHaveBeenCalledOnce();
		expect(worker.messages[0]).toMatchObject({ kind: 'start', revision: revision.toString() });
		expect(changes).toHaveBeenLastCalledWith(
			expect.objectContaining({ revision, state: 'complete' }),
		);
	});

	it('settles an admitted stale scan before scheduling one non-resetting refresh', async () => {
		vi.useFakeTimers();
		const staleWorker = createFakeWorker(false);
		const replacementWorker = createFakeWorker(true, 0, [0]);
		const createWorker = vi
			.fn<ProjectedSearchControllerOptions['createWorker']>()
			.mockReturnValueOnce(staleWorker.value)
			.mockReturnValueOnce(replacementWorker.value);
		let revision = 7n;
		const rows = [bufferRow(0n, 'needle')];
		const readBufferRows = vi.fn<TerminalBufferRowReader['readBufferRows']>(async (request) => ({
			...rowsResult(rows),
			revision: request.revision,
		}));
		const changes = vi.fn<ProjectedSearchControllerOptions['onChange']>();
		const controller = new ProjectedSearchController({
			createWorker,
			onActivate: () => {},
			onChange: changes,
			reader: {
				getBufferRowsSnapshot: () => ({ generation: 2n, revision, totalRows: 1n }),
				readBufferRows,
			},
		});

		controller.update('needle');
		await vi.advanceTimersByTimeAsync(50);
		revision = 8n;
		controller.invalidate();
		for (let invalidation = 0; invalidation < 3; invalidation++) {
			await vi.advanceTimersByTimeAsync(50);
			revision += 1n;
			controller.invalidate();
		}
		expect(staleWorker.terminated).toBe(false);

		staleWorker.respond(0, [0]);
		expect(staleWorker.terminated).toBe(true);
		expect(changes).toHaveBeenLastCalledWith(expect.objectContaining({ state: 'searching' }));
		await vi.advanceTimersByTimeAsync(199);
		expect(createWorker).toHaveBeenCalledOnce();
		await vi.advanceTimersByTimeAsync(1);

		expect(createWorker).toHaveBeenCalledTimes(2);
		expect(replacementWorker.messages[0]).toMatchObject({
			kind: 'start',
			revision: revision.toString(),
		});
		expect(changes).toHaveBeenLastCalledWith(
			expect.objectContaining({ revision, state: 'complete' }),
		);
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
			onActivate: () => {},
			onChange: () => {},
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
				byteLength: page.length * 8192,
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
			onActivate: () => {},
			onChange: () => {},
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
			onActivate: () => {},
			onChange: () => {},
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
			onActivate: () => {},
			onChange: changes,
			reader: createReader([bufferRow(0n, 'value')]),
		});

		controller.update('value');
		await vi.advanceTimersByTimeAsync(200);
		await vi.advanceTimersByTimeAsync(10_000);

		expect(worker.terminated).toBe(true);
		expect(changes).toHaveBeenLastCalledWith(expect.objectContaining({ state: 'timeout' }));
	});

	it('retains navigation requested during both debounce and an active scan', async () => {
		vi.useFakeTimers();
		const debounceWorker = createFakeWorker(true, 0, [0, 1]);
		const scanWorker = createFakeWorker(false);
		const createWorker = vi
			.fn<ProjectedSearchControllerOptions['createWorker']>()
			.mockReturnValueOnce(debounceWorker.value)
			.mockReturnValueOnce(scanWorker.value);
		const activate = vi.fn<ProjectedSearchControllerOptions['onActivate']>();
		const controller = new ProjectedSearchController({
			createWorker,
			onActivate: activate,
			onChange: () => {},
			reader: createReader([bufferRow(0n, 'needle'), bufferRow(1n, 'needle')]),
		});

		controller.update('needle');
		expect(controller.next(false)).toBe(true);
		await vi.advanceTimersByTimeAsync(200);
		expect(activate).toHaveBeenLastCalledWith(
			expect.objectContaining({ start: { column: 0, row: 1 } }),
			expect.anything(),
			true,
		);

		controller.update('needle');
		await vi.advanceTimersByTimeAsync(200);
		expect(controller.next(false)).toBe(true);
		scanWorker.respond(0, [0, 1]);
		expect(activate).toHaveBeenLastCalledWith(
			expect.objectContaining({ start: { column: 0, row: 1 } }),
			expect.anything(),
			true,
		);
	});

	it('replays accepted navigation after a same-query revision invalidation', async () => {
		vi.useFakeTimers();
		const staleWorker = createFakeWorker(false);
		const replacementWorker = createFakeWorker(true, 0, [0, 1]);
		const createWorker = vi
			.fn<ProjectedSearchControllerOptions['createWorker']>()
			.mockReturnValueOnce(staleWorker.value)
			.mockReturnValueOnce(replacementWorker.value);
		const activate = vi.fn<ProjectedSearchControllerOptions['onActivate']>();
		const controller = new ProjectedSearchController({
			createWorker,
			onActivate: activate,
			onChange: () => {},
			reader: createReader([bufferRow(0n, 'needle'), bufferRow(1n, 'needle')]),
		});

		controller.update('needle');
		await vi.advanceTimersByTimeAsync(200);
		expect(controller.next(false)).toBe(true);
		controller.invalidate();
		expect(staleWorker.terminated).toBe(false);
		staleWorker.respond(0, [0, 1]);
		await vi.advanceTimersByTimeAsync(200);

		expect(staleWorker.terminated).toBe(true);
		expect(activate).toHaveBeenLastCalledWith(
			expect.objectContaining({ start: { column: 0, row: 1 } }),
			expect.anything(),
			true,
		);
	});

	it('refreshes invalidated results without revealing the active match again', async () => {
		vi.useFakeTimers();
		const initialWorker = createFakeWorker(true, 0, [0, 1]);
		const replacementWorker = createFakeWorker(true, 0, [0, 1]);
		const createWorker = vi
			.fn<ProjectedSearchControllerOptions['createWorker']>()
			.mockReturnValueOnce(initialWorker.value)
			.mockReturnValueOnce(replacementWorker.value);
		const activate = vi.fn<ProjectedSearchControllerOptions['onActivate']>();
		const changes = vi.fn<ProjectedSearchControllerOptions['onChange']>();
		const controller = new ProjectedSearchController({
			createWorker,
			onActivate: activate,
			onChange: changes,
			reader: createReader([bufferRow(0n, 'needle'), bufferRow(1n, 'needle')]),
		});

		controller.update('needle');
		await vi.advanceTimersByTimeAsync(200);
		expect(activate).toHaveBeenCalledOnce();

		activate.mockClear();
		controller.invalidate();
		await vi.advanceTimersByTimeAsync(199);
		expect(createWorker).toHaveBeenCalledOnce();
		await vi.advanceTimersByTimeAsync(1);

		expect(createWorker).toHaveBeenCalledTimes(2);
		expect(activate).toHaveBeenCalledWith(
			expect.objectContaining({ start: { column: 0, row: 0 } }),
			expect.anything(),
			false,
		);
		expect(changes).toHaveBeenLastCalledWith(
			expect.objectContaining({
				activeMatch: expect.objectContaining({ start: { column: 0, row: 0 } }),
				state: 'complete',
			}),
		);
	});

	it('keeps timeout terminal after aborting a stalled row read', async () => {
		vi.useFakeTimers();
		const worker = createFakeWorker(false);
		const changes = vi.fn<ProjectedSearchControllerOptions['onChange']>();
		const readBufferRows = vi.fn<TerminalBufferRowReader['readBufferRows']>(
			async (_request, signal) =>
				await new Promise<BufferRowsResult>((_resolve, reject) => {
					signal.addEventListener('abort', () => reject(new Error('aborted row read')), {
						once: true,
					});
				}),
		);
		const controller = new ProjectedSearchController({
			createWorker: () => worker.value,
			onActivate: () => {},
			onChange: changes,
			reader: {
				getBufferRowsSnapshot: () => ({ generation: 2n, revision: 7n, totalRows: 1n }),
				readBufferRows,
			},
		});

		controller.update('needle');
		await vi.advanceTimersByTimeAsync(200);
		await vi.advanceTimersByTimeAsync(10_000);

		expect(changes).toHaveBeenLastCalledWith(expect.objectContaining({ state: 'timeout' }));
	});

	it('refreshes the inactivity watchdog when a row page makes progress', async () => {
		vi.useFakeTimers();
		const worker = createFakeWorker(false);
		const changes = vi.fn<ProjectedSearchControllerOptions['onChange']>();
		const readBufferRows = vi.fn<TerminalBufferRowReader['readBufferRows']>(
			async () =>
				await new Promise<BufferRowsResult>((resolve) => {
					setTimeout(() => resolve(rowsResult([bufferRow(0n, 'needle')])), 6_000);
				}),
		);
		const controller = new ProjectedSearchController({
			createWorker: () => worker.value,
			onActivate: () => {},
			onChange: changes,
			reader: {
				getBufferRowsSnapshot: () => ({ generation: 2n, revision: 7n, totalRows: 1n }),
				readBufferRows,
			},
		});

		controller.update('needle');
		await vi.advanceTimersByTimeAsync(200);
		await vi.advanceTimersByTimeAsync(10_001);
		expect(changes).toHaveBeenLastCalledWith(expect.objectContaining({ state: 'searching' }));
		await vi.advanceTimersByTimeAsync(5_999);
		expect(changes).toHaveBeenLastCalledWith(expect.objectContaining({ state: 'timeout' }));
	});

	it('prefetches at most one page while the Worker admits the bounded source', async () => {
		vi.useFakeTimers();
		const worker = createFakeWorker(false);
		const rows = Array.from({ length: 10_000 }, (_, index) =>
			identityBufferRow(BigInt(index), 'x'.repeat(113)),
		);
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
			onActivate: () => {},
			onChange: () => {},
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

		expect(readBufferRows).toHaveBeenCalledTimes(2);
		expect(worker.messages.filter((message) => message.kind === 'page')).toHaveLength(1);
	});

	it('rescans the full navigation domain instead of wrapping at the decoration cap', async () => {
		vi.useFakeTimers();
		const firstWorker = createFakeWorker(true, 0, [0]);
		const nextWorker = createFakeWorker(true, 2, [0]);
		const createWorker = vi
			.fn<ProjectedSearchControllerOptions['createWorker']>()
			.mockReturnValueOnce(firstWorker.value)
			.mockReturnValueOnce(nextWorker.value);
		const changes = vi.fn<ProjectedSearchControllerOptions['onChange']>();
		const activate = vi.fn<ProjectedSearchControllerOptions['onActivate']>();
		const controller = new ProjectedSearchController({
			createWorker,
			onActivate: activate,
			onChange: changes,
			reader: createReader([
				bufferRow(0n, 'needle'),
				bufferRow(1n, 'needle'),
				bufferRow(2n, 'needle'),
			]),
		});

		controller.update('needle', { maxResults: 1 }, { column: 0, row: 0 });
		await vi.advanceTimersByTimeAsync(200);
		expect(controller.next(false)).not.toBeNull();
		await vi.advanceTimersByTimeAsync(0);

		expect(createWorker).toHaveBeenCalledTimes(2);
		expect(nextWorker.messages[0]).toMatchObject({
			kind: 'start',
			navigation: {
				preserveStart: false,
				reverse: false,
				start: { column: 0, row: 0 },
			},
		});
		expect(changes).toHaveBeenLastCalledWith(
			expect.objectContaining({
				activeIndex: -1,
				activeMatch: expect.objectContaining({ start: { column: 0, row: 2 } }),
				matches: [expect.objectContaining({ start: { column: 0, row: 0 } })],
				state: 'complete',
			}),
		);
	});
});

describe('ProjectedSearchController synchronous lifecycle callbacks', () => {
	it.each(['debounce', 'scan'] as const)(
		'does not acquire search work after disposal from the %s notification',
		async (phase) => {
			vi.useFakeTimers();
			const worker = createFakeWorker(false);
			const createWorker = vi.fn<ProjectedSearchControllerOptions['createWorker']>(
				() => worker.value,
			);
			const reader = createReader([bufferRow(0n, 'needle')]);
			let notifications = 0;
			const controller = new ProjectedSearchController({
				createWorker,
				onActivate: () => {},
				onChange: (event) => {
					if (event.state !== 'searching') return;
					notifications += 1;
					if (notifications === (phase === 'debounce' ? 1 : 2)) controller.dispose();
				},
				reader,
			});

			controller.update('needle');
			await vi.advanceTimersByTimeAsync(phase === 'debounce' ? 0 : 50);

			expect(vi.getTimerCount()).toBe(0);
			expect(createWorker).not.toHaveBeenCalled();
			expect(reader.readBufferRows).not.toHaveBeenCalled();
		},
	);

	it.each(['debounce', 'scan'] as const)(
		'keeps only the query admitted by a reentrant %s notification',
		async (phase) => {
			vi.useFakeTimers();
			const workers: ReturnType<typeof createFakeWorker>[] = [];
			const createWorker = vi.fn<ProjectedSearchControllerOptions['createWorker']>(() => {
				const worker = createFakeWorker(true, 0);
				workers.push(worker);
				return worker.value;
			});
			const reader = createReader([bufferRow(0n, 'needle replacement')]);
			let notifications = 0;
			const controller = new ProjectedSearchController({
				createWorker,
				onActivate: () => {},
				onChange: (event) => {
					if (event.state !== 'searching' || event.query !== 'needle') return;
					notifications += 1;
					if (notifications === (phase === 'debounce' ? 1 : 2)) {
						controller.update('replacement');
					}
				},
				reader,
			});

			controller.update('needle');
			await vi.advanceTimersByTimeAsync(100);

			expect(createWorker).toHaveBeenCalledOnce();
			expect(reader.readBufferRows).toHaveBeenCalledOnce();
			expect(workers[0]?.messages[0]).toMatchObject({ kind: 'start', query: 'replacement' });
			expect(workers[0]?.terminated).toBe(true);
			expect(controller.event).toMatchObject({ query: 'replacement', state: 'complete' });
			expect(vi.getTimerCount()).toBe(0);
			controller.dispose();
		},
	);

	it('terminates a Worker returned after its factory disposes the search owner', async () => {
		vi.useFakeTimers();
		const worker = createFakeWorker(false);
		const reader = createReader([bufferRow(0n, 'needle')]);
		const controller = new ProjectedSearchController({
			createWorker: () => {
				controller.dispose();
				return worker.value;
			},
			onActivate: () => {},
			onChange: () => {},
			reader,
		});

		controller.update('needle');
		await vi.advanceTimersByTimeAsync(50);

		expect(worker.terminated).toBe(true);
		expect(worker.messages).toEqual([]);
		expect(reader.readBufferRows).not.toHaveBeenCalled();
		expect(vi.getTimerCount()).toBe(0);
	});

	it.each(['result', 'navigation'] as const)(
		'does not complete a replacement query after reentrant %s activation',
		async (phase) => {
			vi.useFakeTimers();
			const firstWorker = createFakeWorker(false, 0);
			const nextWorker = createFakeWorker(true, 0);
			const createWorker = vi
				.fn<ProjectedSearchControllerOptions['createWorker']>()
				.mockReturnValueOnce(firstWorker.value)
				.mockReturnValueOnce(nextWorker.value);
			let replaceOnActivate = false;
			const controller = new ProjectedSearchController({
				createWorker,
				onActivate: () => {
					if (!replaceOnActivate) return;
					replaceOnActivate = false;
					controller.update('replacement');
				},
				onChange: () => {},
				reader: createReader([bufferRow(0n, 'needle replacement')]),
			});
			controller.update('needle');
			await vi.advanceTimersByTimeAsync(50);
			if (phase === 'navigation') firstWorker.respond(0);
			replaceOnActivate = true;

			if (phase === 'result') firstWorker.respond(0);
			else controller.next();

			expect(controller.event).toMatchObject({ query: 'replacement', state: 'searching' });
			await vi.advanceTimersByTimeAsync(50);
			expect(createWorker).toHaveBeenCalledTimes(2);
			expect(controller.event).toMatchObject({ query: 'replacement', state: 'complete' });
			expect(nextWorker.terminated).toBe(true);
			controller.dispose();
		},
	);

	it.each(['update', 'watchdog'] as const)(
		'preserves a query admitted by an abort listener during %s cancellation',
		async (trigger) => {
			vi.useFakeTimers();
			const firstWorker = createFakeWorker(false);
			const nextWorker = createFakeWorker(true, 0);
			const createWorker = vi
				.fn<ProjectedSearchControllerOptions['createWorker']>()
				.mockReturnValueOnce(firstWorker.value)
				.mockReturnValueOnce(nextWorker.value);
			const rows = [bufferRow(0n, 'needle replacement')];
			const readBufferRows = vi
				.fn<TerminalBufferRowReader['readBufferRows']>(async () => rowsResult(rows))
				.mockImplementationOnce(
					async (_request, signal) =>
						await new Promise<BufferRowsResult>((resolve) => {
							signal.addEventListener(
								'abort',
								() => {
									controller.update('replacement');
									resolve(rowsResult(rows));
								},
								{ once: true },
							);
						}),
				);
			const controller = new ProjectedSearchController({
				createWorker,
				onActivate: () => {},
				onChange: () => {},
				reader: {
					getBufferRowsSnapshot: () => ({ generation: 2n, revision: 7n, totalRows: 1n }),
					readBufferRows,
				},
			});
			controller.update('needle');
			await vi.advanceTimersByTimeAsync(50);

			if (trigger === 'update') controller.update('superseded');
			else await vi.advanceTimersByTimeAsync(10_000);

			expect(firstWorker.terminated).toBe(true);
			expect(vi.getTimerCount()).toBe(1);
			expect(controller.event).toMatchObject({ query: 'replacement', state: 'searching' });
			await vi.advanceTimersByTimeAsync(50);
			expect(createWorker).toHaveBeenCalledTimes(2);
			expect(readBufferRows).toHaveBeenCalledTimes(2);
			expect(controller.event).toMatchObject({ query: 'replacement', state: 'complete' });
			controller.dispose();
		},
	);
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

function createFakeWorker(
	respond: boolean,
	activeRow = 1,
	decorationRows: readonly number[] = [activeRow],
): {
	readonly messages: ProjectedSearchWorkerRequest[];
	readonly respond: (activeRow?: number, decorationRows?: readonly number[]) => void;
	readonly terminated: boolean;
	readonly value: ProjectedSearchWorkerLike;
} {
	const messages: ProjectedSearchWorkerRequest[] = [];
	const messageListeners: Array<(event: MessageEvent<ProjectedSearchWorkerResponse>) => void> = [];
	let terminated = false;
	let start: Extract<ProjectedSearchWorkerRequest, { kind: 'start' }> | undefined;
	const emitResult = (
		resultActiveRow = activeRow,
		resultDecorationRows: readonly number[] = decorationRows,
	): void => {
		if (start === undefined) return;
		const activeMatch = {
			end: { column: 5, row: resultActiveRow },
			start: { column: 0, row: resultActiveRow },
			text: 'needle',
		};
		const response: ProjectedSearchWorkerResponse = {
			activeMatch,
			generation: start.generation,
			kind: 'result',
			matches: resultDecorationRows.map((row) => ({
				...activeMatch,
				end: { ...activeMatch.end, row },
				start: { ...activeMatch.start, row },
			})),
			revision: start.revision,
			searchId: start.searchId,
		};
		for (const listener of messageListeners) {
			listener({ data: response } as MessageEvent<ProjectedSearchWorkerResponse>);
		}
	};
	const emitProgress = (page: Extract<ProjectedSearchWorkerRequest, { kind: 'page' }>): void => {
		if (start === undefined) return;
		const response: ProjectedSearchWorkerResponse = {
			generation: start.generation,
			kind: 'progress',
			nextRow: ((page.rows.at(-1)?.index ?? -1n) + 1n).toString(),
			revision: start.revision,
			searchId: start.searchId,
		};
		for (const listener of messageListeners) {
			listener({ data: response } as MessageEvent<ProjectedSearchWorkerResponse>);
		}
	};
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
			if (!respond || message.kind !== 'page' || start === undefined) return;
			if (message.final) emitResult();
			else emitProgress(message);
		},
		terminate: () => {
			terminated = true;
		},
	} as ProjectedSearchWorkerLike;
	return {
		messages,
		respond: emitResult,
		get terminated() {
			return terminated;
		},
		value,
	};
}
