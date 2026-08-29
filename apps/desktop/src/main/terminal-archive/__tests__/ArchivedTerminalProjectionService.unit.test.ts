import { EventEmitter } from 'node:events';

import {
	ActiveScreen,
	AttachmentIdSchema,
	CellWidth,
	CreationIdSchema,
	CursorShape,
	decodeTerminalFrame,
	decodeTerminalGraphics,
	decodeTerminalRowWindow,
	encodeTerminalGraphics,
	encodeTerminalProjectionBuffer,
	encodeTerminalRowWindow,
	HostEpochSchema,
	materializeTerminalProjectionFrame,
	RowPromptClass,
	SemanticContent,
	TerminalColorKind,
	TerminalFrameExtensionKind,
	TerminalFrameKind,
	TerminalGraphicsImageFormat,
	TerminalGraphicsUpdateKind,
	TerminalIdSchema,
	TerminalRowWindowDetail,
	TerminalThemeStateSchema,
	UnderlineKind,
	type AttachmentId,
	type TerminalIdentity,
} from '@orchbee/terminal-protocol';
import {
	ProjectedTerminalEventSchema,
	type ProjectedTerminalEvent,
} from '@orchbee/workbench-client-protocol';
import { terminalPresentationId } from '@orchbee/workbench/terminal-presentation';
import { describe, expect, it, vi } from 'vitest';

import {
	ArchivedTerminalProjectionService,
	type ArchivedTerminalProjectionServiceDependencies,
} from '../ArchivedTerminalProjectionService';
import type { FileTerminalArchiveStore } from '../FileTerminalArchiveStore';
import {
	decodeTerminalArchive,
	encodeTerminalArchive,
	terminalArchiveGraphicsForRows,
	terminalArchiveTextRows,
	type TerminalArchive,
} from '../terminal-archive-document';

const terminalProtocolOperations = vi.hoisted(() => ({
	materializations: 0,
	sourceInputs: 0,
}));

vi.mock(import('@orchbee/terminal-protocol'), async (importOriginal) => {
	const actual = await importOriginal();
	return {
		...actual,
		materializeTerminalProjectionFrame(
			...parameters: Parameters<typeof actual.materializeTerminalProjectionFrame>
		) {
			terminalProtocolOperations.materializations += 1;
			terminalProtocolOperations.sourceInputs += parameters[1].length;
			return actual.materializeTerminalProjectionFrame(...parameters);
		},
	};
});

const presentationId = terminalPresentationId('presentation-archived');
const emptyGraphics = {
	animationTick: 0n,
	images: [],
	kind: TerminalGraphicsUpdateKind.snapshot,
	placements: [],
	resourceRevision: 0n,
} as const;
const encodedEmptyGraphics = encodeTerminalGraphics(emptyGraphics);
if (!encodedEmptyGraphics.ok) throw new Error(encodedEmptyGraphics.detail);
const emptyGraphicsBytes = encodedEmptyGraphics.value;
const attachmentId = '11'.repeat(16);
const identity: TerminalIdentity = {
	creationId: CreationIdSchema.parse('22'.repeat(16)),
	hostEpoch: HostEpochSchema.parse('33'.repeat(16)),
	terminalId: TerminalIdSchema.parse('44'.repeat(16)),
};
const theme = TerminalThemeStateSchema.parse({
	operationId: null,
	revision: '0',
	terminal: {
		colorScheme: 'dark',
		defaultBackgroundRgb: 0,
		defaultCursorRgb: null,
		defaultForegroundRgb: 0xff_ff_ff,
		palette: Array.from({ length: 256 }, (_, index) => index),
	},
});

describe('ArchivedTerminalProjectionService', () => {
	it('serves history, search, selection, and copy without mutable capabilities', async () => {
		const archive = createArchive();
		const channels: TestChannel[] = [];
		const writeClipboardText = vi.fn<(text: string) => Promise<void>>().mockResolvedValue();
		const service = new ArchivedTerminalProjectionService({
			createMessageChannel: () => {
				const channel = createChannel();
				channels.push(channel);
				return channel;
			},
			randomAttachmentId: () => '55'.repeat(16),
			store: {
				load: vi.fn<FileTerminalArchiveStore['load']>().mockResolvedValue(archive),
			} as unknown as FileTerminalArchiveStore,
			writeClipboardText,
		});

		const lease = await service.open(presentationId);
		if (lease === null) throw new Error('Expected an archived projection');
		expect(lease.bootstrap).toMatchObject({ canInput: false, canResize: false });
		await expect(lease.setInputCapability(true)).rejects.toThrow('permanently read-only');
		const port = channels[0]?.port1;
		if (port === undefined) throw new Error('Archive channel was not created');
		port.start();

		const initialFrame = nextEvent(port, (event) => (event.kind === 'frame' ? event : null));
		port.postMessage({
			kind: 'set-projection-demand',
			payload: { attachmentId: lease.bootstrap.attachmentId, demand: 'active' },
		});
		const frameEvent = await initialFrame;
		const frame = decodeTerminalFrame(new Uint8Array(frameEvent.frame));
		if (!frame.ok) throw new Error(frame.detail);
		expect(frame.value.scrollbackRows).toBe(6n);
		expect(frame.value.rowData.indices).toHaveLength(2);

		const renderRows = nextEvent(port, (event) => (event.kind === 'rows' ? event : null));
		port.postMessage({
			kind: 'read-rows',
			payload: {
				attachmentId: lease.bootstrap.attachmentId,
				detail: 'render',
				generation: frame.value.generation.toString(),
				maximumBytes: 2 * 1024 * 1024,
				origin: { kind: 'tail', rowsBefore: 2 },
				purpose: 'viewport',
				requestSequence: '7',
				revision: '0',
				revisionPolicy: 'current-view',
				rowCount: 2,
			},
		});
		const rendered = decodeTerminalRowWindow(new Uint8Array((await renderRows).rows));
		if (!rendered.ok || rendered.value.detail !== TerminalRowWindowDetail.render) {
			throw new Error('Expected archived render rows');
		}
		expect(rendered.value.startRow).toBe(4n);
		expect(rendered.value.revision).toBe(frame.value.revision);

		const scrolledFrameEvent = nextEvent(port, (event) => (event.kind === 'frame' ? event : null));
		port.postMessage({
			kind: 'need-snapshot',
			payload: {
				attachmentId: lease.bootstrap.attachmentId,
				reason: 'store-reset',
				sequence: '1',
			},
		});
		const scrolledFrame = decodeTerminalFrame(new Uint8Array((await scrolledFrameEvent).frame));
		if (!scrolledFrame.ok) throw new Error(scrolledFrame.detail);
		expect(scrolledFrame.value.scrollbackRows).toBe(4n);
		expect(scrolledFrame.value.rowData.indices).toHaveLength(2);

		const searchRows = nextEvent(port, (event) => (event.kind === 'rows' ? event : null));
		port.postMessage({
			kind: 'read-rows',
			payload: {
				attachmentId: lease.bootstrap.attachmentId,
				detail: 'text',
				generation: frame.value.generation.toString(),
				maximumBytes: 2 * 1024 * 1024,
				origin: { kind: 'absolute', row: '0' },
				purpose: 'search',
				requestSequence: '8',
				revision: frame.value.revision.toString(),
				revisionPolicy: 'exact',
				rowCount: 8,
			},
		});
		const searched = decodeTerminalRowWindow(new Uint8Array((await searchRows).rows));
		if (!searched.ok || searched.value.detail !== TerminalRowWindowDetail.text) {
			throw new Error('Expected archived text rows');
		}
		expect(searched.value.rows.map((row) => row.text)).toEqual([
			'history-a',
			'history-b',
			'history-c',
			'history-d',
			'history-e',
			'history-f',
			'viewport-a',
			'viewport-b',
		]);

		const gestureChanged = nextEvent(port, (event) =>
			event.kind === 'selection-changed' ? event : null,
		);
		port.postMessage({
			kind: 'selection-gesture',
			payload: {
				action: 'start',
				attachmentId: lease.bootstrap.attachmentId,
				clickCount: 1,
				generation: frame.value.generation.toString(),
				point: { column: 0, row: 1 },
				position: { x: 0, y: 0 },
				selectOutput: false,
			},
		});
		expect((await gestureChanged).event).toMatchObject({
			end: { column: 0, row: '5' },
			start: { column: 0, row: '5' },
			state: 'active',
		});

		const selectionChanged = nextEvent(port, (event) =>
			event.kind === 'selection-changed' ? event : null,
		);
		port.postMessage({
			kind: 'selection-command',
			payload: {
				attachmentId: lease.bootstrap.attachmentId,
				command: 'select-all',
				generation: frame.value.generation.toString(),
				revision: frame.value.revision.toString(),
			},
		});
		expect((await selectionChanged).event).toMatchObject({
			preview:
				'history-a\nhistory-b\nhistory-c\nhistory-d\nhistory-ehistory-f\nviewport-a\nviewport-b',
			state: 'active',
		});

		const copied = nextEvent(port, (event) => (event.kind === 'selection-copied' ? event : null));
		port.postMessage({
			kind: 'selection-copy',
			payload: { attachmentId: lease.bootstrap.attachmentId, requestSequence: '9' },
		});
		expect((await copied).byteLength).toBeGreaterThan(0);
		expect(writeClipboardText).toHaveBeenCalledWith(
			'history-a\nhistory-b\nhistory-c\nhistory-d\nhistory-ehistory-f\nviewport-a\nviewport-b',
		);

		port.postMessage({
			kind: 'set-projection-demand',
			payload: { attachmentId: lease.bootstrap.attachmentId, demand: 'paused' },
		});
		const reactivatedFrameEvent = nextEvent(port, (event) =>
			event.kind === 'frame' ? event : null,
		);
		port.postMessage({
			kind: 'set-projection-demand',
			payload: { attachmentId: lease.bootstrap.attachmentId, demand: 'active' },
		});
		const reactivatedFrame = decodeTerminalFrame(
			new Uint8Array((await reactivatedFrameEvent).frame),
		);
		if (!reactivatedFrame.ok) throw new Error(reactivatedFrame.detail);
		expect(reactivatedFrame.value.scrollbackRows).toBe(4n);
		expect(reactivatedFrame.value.rowData.indices).toHaveLength(2);

		const rejectedInput = nextEvent(port, (event) => (event.kind === 'failure' ? event : null));
		port.postMessage({
			kind: 'focus-input',
			payload: {
				attachmentId: lease.bootstrap.attachmentId,
				capabilityRevision: '1',
				focused: true,
			},
		});
		expect((await rejectedInput).failure).toMatchObject({
			code: 'terminal-archive-read-only',
			operation: 'input',
			retryable: false,
		});

		await lease.dispose();
		await service.dispose();
	});

	it('preserves pixels and seam-spanning placements in live frames and render rows', async () => {
		const archive = createArchive(true);
		const channel = createChannel();
		const service = new ArchivedTerminalProjectionService({
			createMessageChannel: () => channel,
			randomAttachmentId: () => '55'.repeat(16),
			store: {
				load: vi.fn<FileTerminalArchiveStore['load']>().mockResolvedValue(archive),
			} as unknown as FileTerminalArchiveStore,
			writeClipboardText:
				vi.fn<ArchivedTerminalProjectionServiceDependencies['writeClipboardText']>(),
		});
		const lease = await service.open(presentationId);
		if (lease === null) throw new Error('Expected an archived projection');

		const frameEvent = nextEvent(channel.port1, (event) => (event.kind === 'frame' ? event : null));
		channel.port1.postMessage({
			kind: 'set-projection-demand',
			payload: { attachmentId: lease.bootstrap.attachmentId, demand: 'active' },
		});
		const frame = decodeTerminalFrame(new Uint8Array((await frameEvent).frame));
		if (!frame.ok) throw new Error(frame.detail);
		const extension = frame.value.extensions.find(
			(candidate) => candidate.kind === TerminalFrameExtensionKind.kittyGraphics,
		);
		if (extension === undefined) throw new Error('Expected archived Kitty graphics extension');
		const frameGraphics = decodeTerminalGraphics(extension.data);
		if (!frameGraphics.ok) throw new Error(frameGraphics.detail);
		expect(Array.from(frameGraphics.value.images[0]?.data ?? [])).toEqual(
			Array.from(graphicsPixels),
		);
		expect(frameGraphics.value.placements).toMatchObject([
			{ offsetY: 9, row: -1, z: -0x4000_0000 },
		]);

		const rowsEvent = nextEvent(channel.port1, (event) => (event.kind === 'rows' ? event : null));
		channel.port1.postMessage({
			kind: 'read-rows',
			payload: {
				attachmentId: lease.bootstrap.attachmentId,
				detail: 'render',
				generation: archive.frame.generation.toString(),
				maximumBytes: 2 * 1024 * 1024,
				origin: { kind: 'absolute', row: '4' },
				purpose: 'viewport',
				requestSequence: '73',
				revision: archive.frame.revision.toString(),
				revisionPolicy: 'exact',
				rowCount: 2,
			},
		});
		const rows = decodeTerminalRowWindow(new Uint8Array((await rowsEvent).rows));
		if (!rows.ok || rows.value.detail !== TerminalRowWindowDetail.render) {
			throw new Error('Expected archived render rows');
		}
		expect(Array.from(rows.value.graphics.images[0]?.data ?? [])).toEqual(
			Array.from(graphicsPixels),
		);
		expect(rows.value.graphics.placements).toHaveLength(1);
		expect(rows.value.graphics.placements[0]).toMatchObject({ offsetY: 9, row: 1 });

		await lease.dispose();
		await service.dispose();
	});

	it('falls back to no projection when the local archive is missing', async () => {
		const service = new ArchivedTerminalProjectionService({
			createMessageChannel: () => createChannel(),
			randomAttachmentId: () => '55'.repeat(16),
			store: {
				load: vi.fn<FileTerminalArchiveStore['load']>().mockResolvedValue(null),
			} as unknown as FileTerminalArchiveStore,
			writeClipboardText:
				vi.fn<ArchivedTerminalProjectionServiceDependencies['writeClipboardText']>(),
		});
		expect(await service.open(presentationId)).toBeNull();
	});

	it.each(
		[20, 40, 80].flatMap((sourceCount) =>
			[TerminalRowWindowDetail.render, TerminalRowWindowDetail.text].map((detail) => ({
				detail,
				sourceCount,
			})),
		),
	)(
		'materializes $sourceCount sources once for the maximal $detail row response',
		async ({ detail, sourceCount }) => {
			const archive = createArchiveWithSourceCount(sourceCount);
			const channel = createChannel();
			const projectedAttachmentId = AttachmentIdSchema.parse('55'.repeat(16));
			const service = new ArchivedTerminalProjectionService({
				createMessageChannel: () => channel,
				randomAttachmentId: () => projectedAttachmentId,
				store: {
					load: vi.fn<FileTerminalArchiveStore['load']>().mockResolvedValue(archive),
				} as unknown as FileTerminalArchiveStore,
				writeClipboardText:
					vi.fn<ArchivedTerminalProjectionServiceDependencies['writeClipboardText']>(),
			});
			const lease = await service.open(presentationId);
			if (lease === null) throw new Error('Expected an archived projection');
			const expectedRowCount = sourceCount - 3;
			const requestSequence = 71n;
			const expected = encodeArchivedRows(
				archive,
				projectedAttachmentId,
				detail,
				expectedRowCount,
				requestSequence,
			);
			const firstExcluded = encodeArchivedRows(
				archive,
				projectedAttachmentId,
				detail,
				expectedRowCount + 1,
				requestSequence,
			);
			expect(firstExcluded.byteLength).toBeGreaterThan(expected.byteLength);
			terminalProtocolOperations.materializations = 0;
			terminalProtocolOperations.sourceInputs = 0;
			const rowsEvent = nextEvent(channel.port1, (event) => (event.kind === 'rows' ? event : null));

			channel.port1.postMessage({
				kind: 'read-rows',
				payload: {
					attachmentId: lease.bootstrap.attachmentId,
					detail,
					generation: archive.frame.generation.toString(),
					maximumBytes: expected.byteLength,
					origin: { kind: 'absolute', row: '0' },
					purpose: 'buffer',
					requestSequence: requestSequence.toString(),
					revision: archive.frame.revision.toString(),
					revisionPolicy: 'exact',
					rowCount: sourceCount,
				},
			});
			const encoded = new Uint8Array((await rowsEvent).rows);
			const decoded = decodeTerminalRowWindow(encoded);
			if (!decoded.ok) throw new Error(decoded.detail);
			const returnedRows =
				decoded.value.detail === TerminalRowWindowDetail.render
					? decoded.value.frame.rows
					: decoded.value.rows.length;
			expect(returnedRows).toBe(expectedRowCount);
			expect(encoded).toEqual(expected);
			expect(encoded.byteLength).toBeLessThanOrEqual(expected.byteLength);
			expect(firstExcluded.byteLength).toBeGreaterThan(expected.byteLength);
			expect({
				materializations: terminalProtocolOperations.materializations,
				sourceInputs: terminalProtocolOperations.sourceInputs,
			}).toEqual({ materializations: 1, sourceInputs: sourceCount });

			await lease.dispose();
			await service.dispose();
		},
	);

	it('admits a shared-hyperlink render prefix once and still obeys the final byte budget', async () => {
		const archive = createArchiveWithSharedHyperlink();
		const channel = createChannel();
		const projectedAttachmentId = AttachmentIdSchema.parse('55'.repeat(16));
		const service = new ArchivedTerminalProjectionService({
			createMessageChannel: () => channel,
			randomAttachmentId: () => projectedAttachmentId,
			store: {
				load: vi.fn<FileTerminalArchiveStore['load']>().mockResolvedValue(archive),
			} as unknown as FileTerminalArchiveStore,
			writeClipboardText:
				vi.fn<ArchivedTerminalProjectionServiceDependencies['writeClipboardText']>(),
		});
		const lease = await service.open(presentationId);
		if (lease === null) throw new Error('Expected an archived projection');
		const expected = encodeArchivedRows(
			archive,
			projectedAttachmentId,
			TerminalRowWindowDetail.render,
			24,
			91n,
		);
		const exactResponse = nextEvent(channel.port1, (event) =>
			event.kind === 'rows' || event.kind === 'failure' ? event : null,
		);
		const exactEvent = await (async () => {
			const keyConstructionSpy = vi.spyOn(String, 'fromCharCode');
			try {
				channel.port1.postMessage({
					kind: 'read-rows',
					payload: {
						attachmentId: lease.bootstrap.attachmentId,
						detail: 'render',
						generation: archive.frame.generation.toString(),
						maximumBytes: expected.byteLength,
						origin: { kind: 'absolute', row: '0' },
						purpose: 'buffer',
						requestSequence: '91',
						revision: archive.frame.revision.toString(),
						revisionPolicy: 'exact',
						rowCount: 24,
					},
				});
				const event = await exactResponse;
				expect(keyConstructionSpy).toHaveBeenCalledTimes(4);
				return event;
			} finally {
				keyConstructionSpy.mockRestore();
			}
		})();
		if (exactEvent.kind !== 'rows') {
			throw new Error(`Expected shared-link rows, received ${JSON.stringify(exactEvent.failure)}`);
		}
		const exactBytes = new Uint8Array(exactEvent.rows);
		const exactDecoded = decodeTerminalRowWindow(exactBytes);
		if (!exactDecoded.ok || exactDecoded.value.detail !== TerminalRowWindowDetail.render) {
			throw new Error('Expected archived shared-link render rows');
		}
		expect(exactDecoded.value.frame.rows).toBe(24);
		expect(exactBytes).toEqual(expected);
		expect(exactBytes.byteLength).toBe(expected.byteLength);

		const constrainedResponse = nextEvent(channel.port1, (event) =>
			event.kind === 'rows' || event.kind === 'failure' ? event : null,
		);
		channel.port1.postMessage({
			kind: 'read-rows',
			payload: {
				attachmentId: lease.bootstrap.attachmentId,
				detail: 'render',
				generation: archive.frame.generation.toString(),
				maximumBytes: expected.byteLength - 1,
				origin: { kind: 'absolute', row: '0' },
				purpose: 'buffer',
				requestSequence: '92',
				revision: archive.frame.revision.toString(),
				revisionPolicy: 'exact',
				rowCount: 24,
			},
		});
		const constrainedEvent = await constrainedResponse;
		if (constrainedEvent.kind !== 'rows') {
			throw new Error(
				`Expected constrained shared-link rows, received ${JSON.stringify(constrainedEvent.failure)}`,
			);
		}
		const constrainedBytes = new Uint8Array(constrainedEvent.rows);
		const constrainedDecoded = decodeTerminalRowWindow(constrainedBytes);
		if (
			!constrainedDecoded.ok ||
			constrainedDecoded.value.detail !== TerminalRowWindowDetail.render
		) {
			throw new Error('Expected constrained archived shared-link render rows');
		}
		expect(constrainedDecoded.value.frame.rows).toBeLessThan(24);
		expect(constrainedBytes.byteLength).toBeLessThanOrEqual(expected.byteLength - 1);

		await lease.dispose();
		await service.dispose();
	});
});

interface TestChannel {
	readonly port1: TestPort;
	readonly port2: TestPort;
}

class TestPort extends EventEmitter {
	peer: TestPort | undefined;
	closed = false;

	close(): void {
		if (this.closed) return;
		this.closed = true;
		this.emit('close');
	}

	postMessage(value: unknown): void {
		if (this.closed) throw new Error('Port is closed');
		queueMicrotask(() => this.peer?.emit('message', { data: value }));
	}

	start(): void {}
}

function createChannel(): TestChannel {
	const port1 = new TestPort();
	const port2 = new TestPort();
	port1.peer = port2;
	port2.peer = port1;
	return { port1, port2 };
}

function nextEvent<Event extends ProjectedTerminalEvent>(
	port: TestPort,
	select: (event: ProjectedTerminalEvent) => Event | null,
): Promise<Event> {
	return new Promise((resolve) => {
		const listener = (message: { readonly data: unknown }): void => {
			const parsed = ProjectedTerminalEventSchema.safeParse(message.data);
			if (!parsed.success) return;
			const selected = select(parsed.data);
			if (selected === null) return;
			port.off('message', listener);
			resolve(selected);
		};
		port.on('message', listener);
	});
}

const graphicsPixels = new Uint8Array([
	0xff, 0x00, 0x00, 0xff, 0x00, 0xff, 0x00, 0xff, 0x00, 0x00, 0xff, 0xff, 0xff, 0xff, 0xff, 0x80,
]);

function createGraphicsSnapshot(row: number | null) {
	return {
		animationTick: 17n,
		images: [
			{
				data: graphicsPixels,
				format: TerminalGraphicsImageFormat.rgba,
				generation: 3n,
				height: 2,
				id: 7,
				width: 2,
			},
		],
		kind: TerminalGraphicsUpdateKind.snapshot,
		placements:
			row === null
				? []
				: [
						{
							column: 1,
							destinationHeight: 27,
							destinationWidth: 18,
							imageId: 7,
							internalId: true,
							offsetX: 2,
							offsetY: 9,
							placementId: 13,
							row,
							sourceHeight: 2,
							sourceWidth: 2,
							sourceX: 0,
							sourceY: 0,
							z: -0x4000_0000,
						},
					],
		resourceRevision: 9n,
	} as const;
}

function createArchive(withGraphics = false) {
	const frame = encodeTerminalProjectionBuffer(createFrame(6n, ['viewport-a', 'viewport-b']));
	const viewportGraphics = withGraphics ? createGraphicsSnapshot(-1) : emptyGraphics;
	const histories = [
		{ rows: ['history-a'], sequence: 1n, startRow: 0n },
		{ rows: ['history-b'], sequence: 2n, startRow: 1n },
		{ rows: ['history-c'], sequence: 3n, startRow: 2n },
		{ rows: ['history-d'], sequence: 4n, startRow: 3n },
		{ rows: ['history-e'], sequence: 5n, startRow: 4n },
		{ rows: ['history-f'], sequence: 6n, startRow: 5n },
	].map(({ rows, sequence, startRow }) =>
		encodeTerminalRowWindow({
			attachmentId,
			columns: 2,
			detail: TerminalRowWindowDetail.render,
			frame: createFrame(startRow, rows, sequence, {
				wrap: startRow === 4n,
				wrapContinuation: startRow === 5n,
			}),
			graphics: withGraphics ? createGraphicsSnapshot(startRow === 5n ? 0 : null) : emptyGraphics,
			generation: 3n,
			hostEpoch: identity.hostEpoch,
			revision: 5n,
			sequence,
			startRow,
			totalRows: 8n,
		}),
	);
	const graphics = encodeTerminalGraphics(viewportGraphics);
	if (!frame.ok || !graphics.ok || histories.some((history) => !history.ok)) {
		throw new Error('Archive fixture failed to encode');
	}
	const encoded = encodeTerminalArchive({
		frame: frame.value,
		graphics: graphics.value,
		identity,
		presentationId,
		rowWindows: histories.map((history) => {
			if (!history.ok) throw new Error(history.detail);
			return history.value;
		}),
		theme,
	});
	if (!encoded.ok) throw new Error(encoded.detail);
	const archive = decodeTerminalArchive(encoded.value);
	if (!archive.ok) throw new Error(archive.detail);
	return archive.value;
}

function createArchiveWithSourceCount(sourceCount: number): TerminalArchive {
	const lastRow = sourceCount - 1;
	const frame = encodeTerminalProjectionBuffer(
		createFrame(BigInt(lastRow), [`archive-row-${lastRow.toString().padStart(3, '0')}`]),
	);
	const histories = Array.from({ length: lastRow }, (_, index) => {
		const startRow = BigInt(index);
		return encodeTerminalRowWindow({
			attachmentId,
			columns: 2,
			detail: TerminalRowWindowDetail.render,
			frame: createFrame(
				startRow,
				[`archive-row-${index.toString().padStart(3, '0')}`],
				BigInt(index + 1),
			),
			graphics: emptyGraphics,
			generation: 3n,
			hostEpoch: identity.hostEpoch,
			revision: 5n,
			sequence: BigInt(index + 1),
			startRow,
			totalRows: BigInt(sourceCount),
		});
	});
	if (!frame.ok || histories.some((history) => !history.ok)) {
		throw new Error('Scaling archive fixture failed to encode');
	}
	const encoded = encodeTerminalArchive({
		frame: frame.value,
		graphics: emptyGraphicsBytes,
		identity,
		presentationId,
		rowWindows: histories.map((history) => {
			if (!history.ok) throw new Error(history.detail);
			return history.value;
		}),
		theme,
	});
	if (!encoded.ok) throw new Error(encoded.detail);
	const archive = decodeTerminalArchive(encoded.value);
	if (!archive.ok) throw new Error(archive.detail);
	return archive.value;
}

function createArchiveWithSharedHyperlink(): TerminalArchive {
	const frame = encodeTerminalProjectionBuffer(createSharedHyperlinkFrame());
	if (!frame.ok) throw new Error(`${frame.code}: ${frame.detail}`);
	const encoded = encodeTerminalArchive({
		frame: frame.value,
		graphics: emptyGraphicsBytes,
		identity,
		presentationId,
		theme,
		rowWindows: [],
	});
	if (!encoded.ok) throw new Error(encoded.detail);
	const archive = decodeTerminalArchive(encoded.value);
	if (!archive.ok) throw new Error(archive.detail);
	return archive.value;
}

function createSharedHyperlinkFrame() {
	const base = createFrame(0n, ['']);
	const baseStyle = base.styles[0];
	if (baseStyle === undefined) throw new Error('Expected a base terminal style');
	const columns = 80;
	const rows = 24;
	const prefix = 'https://example.com/';
	const hyperlink = prefix + 'a'.repeat(1_061 - prefix.length);
	return {
		...base,
		columns,
		currentDirectory: null,
		cursor: { ...base.cursor, row: rows - 1 },
		rowData: Array.from({ length: rows }, (_rowValue, row) => ({
			cells: Array.from({ length: columns }, (_cellValue, column) => ({
				grapheme: 'X',
				protected: false,
				semanticContent: SemanticContent.output,
				style: column % 2,
				width: CellWidth.narrow,
			})),
			index: row,
			promptClass: RowPromptClass.none,
			wrap: false,
			wrapContinuation: false,
		})),
		rows,
		styles: [
			{ ...baseStyle, hyperlink },
			{ ...baseStyle, bold: true, hyperlink },
		],
		title: null,
	};
}

function encodeArchivedRows(
	archive: TerminalArchive,
	projectedAttachmentId: AttachmentId,
	detail: TerminalRowWindowDetail,
	rowCount: number,
	sequence: bigint,
): Uint8Array {
	const materialized = materializeTerminalProjectionFrame(
		archive.frame,
		[...archive.rowWindows.map((window) => window.frame), archive.frame],
		{
			attachmentId: projectedAttachmentId,
			revealCursor: false,
			rowCount,
			scrollbackRows: 0n,
			sequence,
			sourceStartRow: archive.sourceStartRow,
		},
	);
	if (!materialized.ok) throw new Error(materialized.detail);
	const common = {
		attachmentId: projectedAttachmentId,
		columns: archive.frame.columns,
		generation: archive.frame.generation,
		hostEpoch: archive.frame.hostEpoch,
		revision: archive.frame.revision,
		sequence,
		startRow: 0n,
		totalRows: BigInt(archive.retainedRows),
	};
	if (detail === TerminalRowWindowDetail.render) {
		const graphics = terminalArchiveGraphicsForRows(archive, 0n, rowCount);
		if (graphics === null) throw new Error('Expected archived graphics to materialize');
		const encoded = encodeTerminalRowWindow({
			...common,
			detail: TerminalRowWindowDetail.render,
			frame: materialized.value,
			graphics,
		});
		if (!encoded.ok) throw new Error(encoded.detail);
		return encoded.value;
	}
	const encoded = encodeTerminalRowWindow({
		...common,
		detail: TerminalRowWindowDetail.text,
		rows: terminalArchiveTextRows(materialized.value, 0n),
	});
	if (!encoded.ok) throw new Error(encoded.detail);
	return encoded.value;
}

function createFrame(
	scrollbackRows: bigint,
	rows: readonly string[],
	sequence = 1n,
	rowFlags: { readonly wrap: boolean; readonly wrapContinuation: boolean } = {
		wrap: false,
		wrapContinuation: false,
	},
) {
	return {
		activeScreen: ActiveScreen.primary,
		attachmentId,
		baseRevision: null,
		cellPixelHeight: 18,
		cellPixelWidth: 9,
		columns: 2,
		currentDirectory: '/tmp',
		cursor: {
			blinking: false,
			column: 0,
			row: rows.length - 1,
			shape: CursorShape.block,
			visible: false,
			wideTail: false,
		},
		defaultColors: { backgroundRgb: 0, cursorRgb: null, foregroundRgb: 0xff_ff_ff },
		extensions: [],
		generation: 3n,
		hostEpoch: identity.hostEpoch,
		interaction: {
			applicationCursorKeys: false,
			applicationKeypad: false,
			bracketedPaste: false,
			focusReporting: false,
			kittyDisambiguateEscapeCodes: false,
			kittyReportAllKeysAsEscapeCodes: false,
			kittyReportAlternateKeys: false,
			kittyReportAssociatedText: false,
			kittyReportEventTypes: false,
			modifyOtherKeys2: false,
			mouseAny: false,
			mouseButton: false,
			mouseNormal: false,
			mouseSgr: false,
			mouseSgrPixels: false,
			mouseUtf8: false,
			mouseUrxvt: false,
			mouseX10: false,
		},
		kind: TerminalFrameKind.snapshot,
		revision: 5n,
		rowData: rows.map((text, index) => ({
			cells: [
				{
					grapheme: text,
					protected: false,
					semanticContent: SemanticContent.output,
					style: 0,
					width: CellWidth.narrow,
				},
				{
					grapheme: '',
					protected: false,
					semanticContent: SemanticContent.output,
					style: 0,
					width: CellWidth.narrow,
				},
			],
			index,
			promptClass: RowPromptClass.none,
			wrap: rowFlags.wrap,
			wrapContinuation: rowFlags.wrapContinuation,
		})),
		rows: rows.length,
		scrollbackRows,
		sequence,
		styles: [
			{
				background: { kind: TerminalColorKind.default },
				blink: false,
				bold: false,
				faint: false,
				foreground: { kind: TerminalColorKind.default },
				hyperlink: null,
				inverse: false,
				invisible: false,
				italic: false,
				overline: false,
				strikethrough: false,
				underline: { kind: TerminalColorKind.none },
				underlineKind: UnderlineKind.none,
			},
		],
		themeRevision: 0n,
		title: 'archived terminal',
	};
}
