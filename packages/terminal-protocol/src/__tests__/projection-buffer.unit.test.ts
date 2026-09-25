import { runInNewContext } from 'node:vm';

import { describe, expect, it, vi } from 'vitest';

import {
	ActiveScreen,
	CellWidth,
	CursorShape,
	decodeTerminalFrame,
	encodeTerminalFrame,
	RowPromptClass,
	SemanticContent,
	TerminalColorKind,
	TerminalFrameKind,
	TerminalFrameSchema,
	UnderlineKind,
} from '../frame';
import { AttachmentIdSchema } from '../identifiers';
import { PROTOCOL_LIMITS } from '../limits';
import {
	decodeTerminalProjectionBuffer,
	type DecodedTerminalProjectionBuffer,
	encodeTerminalProjectionBuffer,
	encodeTerminalProjectionBufferWithin,
	materializeTerminalProjectionFrame,
	TerminalProjectionSection,
	viewTerminalProjectionBuffer,
} from '../projection-buffer';
import { ProtocolErrorCode } from '../protocol-result';

const frame = {
	activeScreen: ActiveScreen.primary,
	attachmentId: '22222222222222222222222222222222',
	baseRevision: null,
	cellPixelHeight: 16,
	cellPixelWidth: 8,
	columns: 1,
	currentDirectory: '/tmp',
	cursor: {
		blinking: false,
		column: 0,
		row: 0,
		shape: CursorShape.block,
		visible: true,
		wideTail: false,
	},
	defaultColors: {
		backgroundRgb: 0,
		cursorRgb: null,
		foregroundRgb: 0xffffff,
	},
	extensions: [],
	generation: 1n,
	hostEpoch: '11111111111111111111111111111111',
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
	revision: 1n,
	rowData: [
		{
			cells: [
				{
					grapheme: '蜂',
					protected: false,
					semanticContent: SemanticContent.output,
					style: 0,
					width: CellWidth.narrow,
				},
			],
			index: 0,
			promptClass: RowPromptClass.none,
			wrap: false,
			wrapContinuation: false,
		},
	],
	rows: 1,
	scrollbackRows: 0n,
	sequence: 1n,
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
	themeRevision: 2n,
	title: 'projection',
} as const;

const leadingByteOrderMarkProjection = {
	currentDirectory: '\uFEFF/',
	grapheme: '\uFEFFA',
	hyperlink: '\uFEFFx',
	title: '\uFEFF',
} as const;

function encodedProjection(): Uint8Array {
	const encoded = encodeTerminalProjectionBuffer(frame);
	if (!encoded.ok) throw new Error(encoded.detail);
	return encoded.value;
}

function encodedProjectionWithLeadingByteOrderMarks(): Uint8Array {
	const encoded = encodeTerminalProjectionBuffer({
		...frame,
		currentDirectory: leadingByteOrderMarkProjection.currentDirectory,
		rowData: [
			{
				...frame.rowData[0],
				cells: [
					{
						...frame.rowData[0].cells[0],
						grapheme: leadingByteOrderMarkProjection.grapheme,
					},
				],
			},
		],
		styles: [
			{
				...frame.styles[0],
				hyperlink: leadingByteOrderMarkProjection.hyperlink,
			},
		],
		title: leadingByteOrderMarkProjection.title,
	});
	if (!encoded.ok) throw new Error(encoded.detail);
	return encoded.value;
}

function metadataOnlyDeltaProjection(columns: number, rows: number): Uint8Array {
	const encoded = encodeTerminalProjectionBuffer({
		...frame,
		baseRevision: 1n,
		currentDirectory: null,
		kind: TerminalFrameKind.delta,
		revision: 2n,
		rowData: [],
		sequence: 2n,
		title: null,
	});
	if (!encoded.ok) throw new Error(encoded.detail);
	const view = new DataView(
		encoded.value.buffer,
		encoded.value.byteOffset,
		encoded.value.byteLength,
	);
	view.setUint16(14, columns, true);
	view.setUint16(16, rows, true);
	return encoded.value;
}

function repeatedProjection(cellCount: number) {
	const row = frame.rowData[0];
	const cell = row?.cells[0];
	if (row === undefined || cell === undefined) throw new Error('Projection fixture is incomplete');
	return {
		...frame,
		columns: cellCount,
		rowData: [{ ...row, cells: Array.from({ length: cellCount }, () => cell) }],
		styles: [{ ...frame.styles[0], hyperlink: 'https://example.test/link' }],
	};
}

function sectionOffset(bytes: Uint8Array, section: number): number {
	return new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength).getUint32(
		160 + section * 8,
		true,
	);
}

function sectionLength(bytes: Uint8Array, section: number): number {
	return new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength).getUint32(
		160 + section * 8 + 4,
		true,
	);
}

function crossRealmSharedSubview(source: Uint8Array): Uint8Array {
	const byteLength = source.byteLength + 8;
	const buffer: SharedArrayBuffer = runInNewContext('new SharedArrayBuffer(byteLength)', {
		byteLength,
	});
	const storage = new Uint8Array(buffer);
	storage.fill(0xa5);
	const view = storage.subarray(4, 4 + source.byteLength);
	view.set(source);
	return view;
}

function decodedMaterializationSource(
	scrollbackRows: bigint,
	graphemes: readonly string[],
	cellPixelWidth: number = frame.cellPixelWidth,
	hyperlink: string | null = frame.styles[0].hyperlink,
): DecodedTerminalProjectionBuffer {
	const row = frame.rowData[0];
	const cell = row?.cells[0];
	if (row === undefined || cell === undefined || graphemes.length === 0) {
		throw new Error('Projection materialization fixture is incomplete');
	}
	const encoded = encodeTerminalProjectionBuffer({
		...frame,
		cellPixelWidth,
		cursor: { ...frame.cursor, row: graphemes.length - 1 },
		rowData: graphemes.map((grapheme, index) => ({
			...row,
			cells: [{ ...cell, grapheme }],
			index,
		})),
		rows: graphemes.length,
		scrollbackRows,
		styles: [{ ...frame.styles[0], hyperlink }],
	});
	if (!encoded.ok) throw new Error(encoded.detail);
	const decoded = decodeTerminalProjectionBuffer(encoded.value);
	if (!decoded.ok) throw new Error(decoded.detail);
	return decoded.value;
}

function countProjectionSourceReads(sources: readonly DecodedTerminalProjectionBuffer[]): {
	readonly readCount: () => number;
	readonly sources: readonly DecodedTerminalProjectionBuffer[];
} {
	let indexedReads = 0;
	const countedSources = new Proxy(sources, {
		get(target, property, receiver) {
			if (typeof property === 'string') {
				const index = Number(property);
				if (Number.isSafeInteger(index) && index >= 0 && String(index) === property) {
					indexedReads += 1;
				}
			}
			return Reflect.get(target, property, receiver);
		},
	});
	return { readCount: () => indexedReads, sources: countedSources };
}

describe('terminal projection buffer', () => {
	it.each([
		{ boundary: 'strict', decode: decodeTerminalProjectionBuffer },
		{ boundary: 'structural', decode: viewTerminalProjectionBuffer },
	])('preserves leading U+FEFF in $boundary metadata and exact stored bytes', ({ decode }) => {
		const bytes = encodedProjectionWithLeadingByteOrderMarks();
		const decoded = decode(bytes);
		if (!decoded.ok) throw new Error(decoded.detail);
		const projection = decoded.value;
		const header = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
		const titleOffset = header.getUint32(124, true);
		const titleLength = header.getUint32(128, true);
		const directoryOffset = header.getUint32(132, true);
		const directoryLength = header.getUint32(136, true);
		const graphemeOffset = projection.cells.graphemeOffsets[0] ?? 0;
		const graphemeLength = projection.cells.graphemeLengths[0] ?? 0;
		const hyperlinkOffset = projection.cells.hyperlinkOffsets[0] ?? 0;
		const hyperlinkLength = projection.cells.hyperlinkLengths[0] ?? 0;

		expect([...projection.stringBytes.subarray(titleOffset, titleOffset + titleLength)]).toEqual([
			0xef, 0xbb, 0xbf,
		]);
		expect([
			...projection.stringBytes.subarray(directoryOffset, directoryOffset + directoryLength),
		]).toEqual([0xef, 0xbb, 0xbf, 0x2f]);
		expect([
			...projection.stringBytes.subarray(graphemeOffset, graphemeOffset + graphemeLength),
		]).toEqual([0xef, 0xbb, 0xbf, 0x41]);
		expect([
			...projection.stringBytes.subarray(hyperlinkOffset, hyperlinkOffset + hyperlinkLength),
		]).toEqual([0xef, 0xbb, 0xbf, 0x78]);
		expect(projection.title).toBe(leadingByteOrderMarkProjection.title);
		expect(projection.title?.codePointAt(0)).toBe(0xfeff);
		expect(projection.currentDirectory).toBe(leadingByteOrderMarkProjection.currentDirectory);
		expect(projection.currentDirectory?.codePointAt(0)).toBe(0xfeff);
	});

	it('preserves leading U+FEFF while materializing projection strings', () => {
		const decoded = decodeTerminalProjectionBuffer(encodedProjectionWithLeadingByteOrderMarks());
		if (!decoded.ok) throw new Error(decoded.detail);
		const source = {
			...decoded.value,
			currentDirectory: leadingByteOrderMarkProjection.currentDirectory,
			title: leadingByteOrderMarkProjection.title,
		};
		const materialized = materializeTerminalProjectionFrame(source, [source], {
			attachmentId: source.attachmentId,
			revealCursor: true,
			rowCount: 1,
			scrollbackRows: 0n,
			sequence: 2n,
			sourceStartRow: 0n,
		});
		if (!materialized.ok) throw new Error(materialized.detail);

		expect(materialized.value.title).toBe(leadingByteOrderMarkProjection.title);
		expect(materialized.value.title?.codePointAt(0)).toBe(0xfeff);
		expect(materialized.value.currentDirectory).toBe(
			leadingByteOrderMarkProjection.currentDirectory,
		);
		expect(materialized.value.currentDirectory?.codePointAt(0)).toBe(0xfeff);
		expect(materialized.value.rowData[0]?.cells[0]?.grapheme).toBe(
			leadingByteOrderMarkProjection.grapheme,
		);
		expect(materialized.value.rowData[0]?.cells[0]?.grapheme.codePointAt(0)).toBe(0xfeff);
		expect(materialized.value.styles[0]?.hyperlink).toBe(leadingByteOrderMarkProjection.hyperlink);
		expect(materialized.value.styles[0]?.hyperlink?.codePointAt(0)).toBe(0xfeff);
	});

	it('decodes each exact aliased materialization range once', () => {
		const columns = 16;
		const rows = 8;
		const grapheme = 'g'.repeat(PROTOCOL_LIMITS.maxStringBytes);
		const hyperlink = 'h'.repeat(PROTOCOL_LIMITS.maxStringBytes);
		const sourceRow = frame.rowData[0];
		const sourceCell = sourceRow?.cells[0];
		if (sourceRow === undefined || sourceCell === undefined) {
			throw new Error('Aliased projection fixture is incomplete');
		}
		let encodedCell = 0;
		const encoded = encodeTerminalProjectionBuffer({
			...frame,
			columns,
			rowData: Array.from({ length: rows }, (_, rowIndex) => ({
				...sourceRow,
				cells: Array.from({ length: columns }, () => ({
					...sourceCell,
					grapheme: encodedCell++ === 0 ? grapheme : '',
				})),
				index: rowIndex,
			})),
			rows,
			styles: [{ ...frame.styles[0], hyperlink }],
		});
		if (!encoded.ok) throw new Error(encoded.detail);
		expect(encoded.value).toHaveLength(36_974);
		const decoded = decodeTerminalProjectionBuffer(encoded.value);
		if (!decoded.ok) throw new Error(decoded.detail);
		const graphemeOffset = decoded.value.cells.graphemeOffsets[0];
		const hyperlinkOffset = decoded.value.cells.hyperlinkOffsets[0];
		if (graphemeOffset === undefined || hyperlinkOffset === undefined) {
			throw new Error('Aliased projection ranges are incomplete');
		}
		decoded.value.cells.graphemeOffsets.fill(graphemeOffset);
		decoded.value.cells.graphemeLengths.fill(PROTOCOL_LIMITS.maxStringBytes);
		decoded.value.cells.hyperlinkOffsets.fill(hyperlinkOffset);
		decoded.value.cells.hyperlinkLengths.fill(PROTOCOL_LIMITS.maxStringBytes);

		const observed = (() => {
			const decodeSpy = vi.spyOn(TextDecoder.prototype, 'decode');
			const stringifySpy = vi.spyOn(JSON, 'stringify');
			try {
				const result = materializeTerminalProjectionFrame(decoded.value, [decoded.value], {
					attachmentId: decoded.value.attachmentId,
					revealCursor: true,
					rowCount: rows,
					scrollbackRows: 0n,
					sequence: 2n,
					sourceStartRow: 0n,
				});
				return {
					decodeCalls: decodeSpy.mock.calls.length,
					decodedInputBytes: decodeSpy.mock.calls.reduce(
						(total, [input]) => total + (input?.byteLength ?? 0),
						0,
					),
					result,
					styleSerializations: stringifySpy.mock.calls.length,
				};
			} finally {
				decodeSpy.mockRestore();
				stringifySpy.mockRestore();
			}
		})();
		if (!observed.result.ok) throw new Error(observed.result.detail);

		expect(observed.result.value.rowData).toHaveLength(rows);
		expect(observed.result.value.rowData.every((row) => row.cells.length === columns)).toBe(true);
		expect(
			observed.result.value.rowData.every((row) =>
				row.cells.every((cell) => cell.grapheme === grapheme),
			),
		).toBe(true);
		expect(observed.result.value.styles).toHaveLength(1);
		expect(observed.result.value.styles[0]?.hyperlink).toBe(hyperlink);
		expect(observed).toMatchObject({
			decodeCalls: 2,
			decodedInputBytes: 32_768,
			styleSerializations: 1,
		});
	});

	it('keys materialized strings by source identity as well as numeric range', () => {
		const first = decodedMaterializationSource(0n, ['left'], frame.cellPixelWidth, 'link-a');
		const second = decodedMaterializationSource(1n, ['rght'], frame.cellPixelWidth, 'link-b');
		expect(second.cells.graphemeOffsets[0]).toBe(first.cells.graphemeOffsets[0]);
		expect(second.cells.hyperlinkOffsets[0]).toBe(first.cells.hyperlinkOffsets[0]);

		const materialized = materializeTerminalProjectionFrame(first, [first, second], {
			attachmentId: first.attachmentId,
			revealCursor: false,
			rowCount: 2,
			scrollbackRows: 0n,
			sequence: 2n,
			sourceStartRow: 0n,
		});
		if (!materialized.ok) throw new Error(materialized.detail);

		expect(materialized.value.rowData.map((row) => row.cells[0]?.grapheme)).toEqual([
			'left',
			'rght',
		]);
		expect(
			materialized.value.rowData.map((row) => {
				const styleIndex = row.cells[0]?.style;
				return styleIndex === undefined
					? undefined
					: materialized.value.styles[styleIndex]?.hyperlink;
			}),
		).toEqual(['link-a', 'link-b']);
	});

	it('preserves distinct, overlapping, null, and empty materialization ranges', () => {
		const sourceRow = frame.rowData[0];
		const sourceCell = sourceRow?.cells[0];
		if (sourceRow === undefined || sourceCell === undefined) {
			throw new Error('Range projection fixture is incomplete');
		}
		const encoded = encodeTerminalProjectionBuffer({
			...frame,
			columns: 4,
			rowData: [
				{
					...sourceRow,
					cells: [
						{ ...sourceCell, grapheme: 'abcdefabcdef' },
						{ ...sourceCell, grapheme: '' },
						{ ...sourceCell, grapheme: '' },
						{ ...sourceCell, grapheme: '' },
					],
				},
			],
		});
		if (!encoded.ok) throw new Error(encoded.detail);
		const decoded = decodeTerminalProjectionBuffer(encoded.value);
		if (!decoded.ok) throw new Error(decoded.detail);
		const source = decoded.value;
		const baseOffset = source.cells.graphemeOffsets[0];
		if (baseOffset === undefined) throw new Error('Range projection backing is incomplete');
		source.cells.graphemeOffsets.set([baseOffset, baseOffset + 3, baseOffset + 2, baseOffset + 6]);
		source.cells.graphemeLengths.set([3, 3, 4, 0]);
		source.cells.hyperlinkOffsets.set([baseOffset, baseOffset + 6, 0xffff_ffff, baseOffset + 6]);
		source.cells.hyperlinkLengths.set([3, 3, 0, 0]);

		const materialized = materializeTerminalProjectionFrame(source, [source], {
			attachmentId: source.attachmentId,
			revealCursor: false,
			rowCount: 1,
			scrollbackRows: 0n,
			sequence: 2n,
			sourceStartRow: 0n,
		});
		if (!materialized.ok) throw new Error(materialized.detail);
		const cells = materialized.value.rowData[0]?.cells;
		expect(cells?.map((cell) => cell.grapheme)).toEqual(['abc', 'def', 'cdef', '']);
		expect(cells?.map((cell) => materialized.value.styles[cell.style]?.hyperlink)).toEqual([
			'abc',
			'abc',
			null,
			'',
		]);
		expect(cells?.[0]?.style).toBe(cells?.[1]?.style);
		expect(materialized.value.styles).toHaveLength(3);
	});

	it('rejects excessive distinct logical range work before decoding or style construction', () => {
		const rangeBytes = PROTOCOL_LIMITS.maxStringBytes;
		const cellCount = PROTOCOL_LIMITS.maxFrameBytes / rangeBytes + 1;
		const sourceRow = frame.rowData[0];
		const sourceCell = sourceRow?.cells[0];
		if (sourceRow === undefined || sourceCell === undefined) {
			throw new Error('Logical range limit fixture is incomplete');
		}
		const backingGraphemes = ['a'.repeat(rangeBytes), 'b'.repeat(cellCount - 1)];
		const encoded = encodeTerminalProjectionBuffer({
			...frame,
			columns: cellCount,
			rowData: [
				{
					...sourceRow,
					cells: Array.from({ length: cellCount }, (_, index) => ({
						...sourceCell,
						grapheme: backingGraphemes[index] ?? '',
					})),
				},
			],
		});
		if (!encoded.ok) throw new Error(encoded.detail);
		const decoded = decodeTerminalProjectionBuffer(encoded.value);
		if (!decoded.ok) throw new Error(decoded.detail);
		const source = decoded.value;
		const baseOffset = source.cells.graphemeOffsets[0];
		if (baseOffset === undefined) throw new Error('Logical range backing is incomplete');
		for (let index = 0; index < cellCount; index += 1) {
			source.cells.graphemeOffsets[index] = baseOffset + index;
			source.cells.graphemeLengths[index] = rangeBytes;
		}

		const observed = (() => {
			const decodeSpy = vi.spyOn(TextDecoder.prototype, 'decode');
			const stringifySpy = vi.spyOn(JSON, 'stringify');
			try {
				const result = materializeTerminalProjectionFrame(source, [source], {
					attachmentId: source.attachmentId,
					revealCursor: false,
					rowCount: 1,
					scrollbackRows: 0n,
					sequence: 2n,
					sourceStartRow: 0n,
				});
				return {
					decodeCalls: decodeSpy.mock.calls.length,
					result,
					styleSerializations: stringifySpy.mock.calls.length,
				};
			} finally {
				decodeSpy.mockRestore();
				stringifySpy.mockRestore();
			}
		})();

		expect(observed.result).toEqual({
			code: ProtocolErrorCode.limitExceeded,
			detail: 'Projection materialization string ranges exceed the frame byte limit',
			ok: false,
		});
		expect(observed.decodeCalls).toBe(0);
		expect(observed.styleSerializations).toBe(0);
	});

	it('materializes a rebased archive snapshot across contiguous packed sources', () => {
		const archiveAttachmentId = AttachmentIdSchema.parse('33'.repeat(16));
		const historyBytes = encodeTerminalProjectionBuffer({
			...frame,
			cursor: { ...frame.cursor, visible: false },
			rowData: [
				{
					...frame.rowData[0],
					cells: [{ ...frame.rowData[0].cells[0], grapheme: 'old' }],
				},
			],
			scrollbackRows: 5n,
		});
		const viewportBytes = encodeTerminalProjectionBuffer({
			...frame,
			rowData: [
				{
					...frame.rowData[0],
					cells: [{ ...frame.rowData[0].cells[0], grapheme: 'new' }],
				},
			],
			scrollbackRows: 6n,
		});
		if (!historyBytes.ok || !viewportBytes.ok) throw new Error('Archive fixture failed to encode');
		const history = decodeTerminalProjectionBuffer(historyBytes.value);
		const viewport = decodeTerminalProjectionBuffer(viewportBytes.value);
		if (!history.ok || !viewport.ok) throw new Error('Archive fixture failed to decode');

		const materialized = materializeTerminalProjectionFrame(
			viewport.value,
			[history.value, viewport.value],
			{
				attachmentId: archiveAttachmentId,
				revealCursor: true,
				rowCount: 2,
				scrollbackRows: 0n,
				sequence: 7n,
				sourceStartRow: 5n,
			},
		);
		if (!materialized.ok) throw new Error(materialized.detail);

		expect(materialized.value.attachmentId).toBe(archiveAttachmentId);
		expect(materialized.value.rowData.map((row) => row.cells[0]?.grapheme)).toEqual(['old', 'new']);
		expect(materialized.value.cursor).toMatchObject({ row: 1, visible: true });
		expect(materialized.value.scrollbackRows).toBe(0n);
		expect(materialized.value.sequence).toBe(7n);
		expect(materialized.value.extensions).toEqual([]);
	});

	it('materializes contiguous sources with differing legal row counts and origins', () => {
		const historyBytes = encodeTerminalProjectionBuffer({
			...frame,
			cursor: { ...frame.cursor, visible: false },
			rowData: ['history-a', 'history-b'].map((grapheme, index) => ({
				...frame.rowData[0],
				cells: [{ ...frame.rowData[0].cells[0], grapheme }],
				index,
			})),
			rows: 2,
			scrollbackRows: 4n,
		});
		const viewportBytes = encodeTerminalProjectionBuffer({
			...frame,
			rowData: [
				{
					...frame.rowData[0],
					cells: [{ ...frame.rowData[0].cells[0], grapheme: 'viewport' }],
				},
			],
			scrollbackRows: 6n,
		});
		if (!historyBytes.ok || !viewportBytes.ok)
			throw new Error('Projection fixture failed to encode');
		const history = decodeTerminalProjectionBuffer(historyBytes.value);
		const viewport = decodeTerminalProjectionBuffer(viewportBytes.value);
		expect(history).toMatchObject({ ok: true });
		expect(viewport).toMatchObject({ ok: true });
		if (!history.ok || !viewport.ok) throw new Error('Projection fixture failed to decode');
		expect(history.value).toMatchObject({ rows: 2, scrollbackRows: 4n });
		expect(viewport.value).toMatchObject({ rows: 1, scrollbackRows: 6n });

		const materialized = materializeTerminalProjectionFrame(
			viewport.value,
			[history.value, viewport.value],
			{
				attachmentId: viewport.value.attachmentId,
				revealCursor: true,
				rowCount: 3,
				scrollbackRows: 0n,
				sequence: 8n,
				sourceStartRow: 4n,
			},
		);
		if (!materialized.ok) throw new Error(materialized.detail);

		expect(materialized.value.rowData.map((row) => row.cells[0]?.grapheme)).toEqual([
			'history-a',
			'history-b',
			'viewport',
		]);
	});

	it('indexes first-covering sources with linearly bounded source-array reads', () => {
		const sizes = [20, 40, 80] as const;
		const sourceReads = sizes.map((size) => {
			const sources = Array.from({ length: size }, (_, index) =>
				decodedMaterializationSource(BigInt(index), [`row-${index}`]),
			);
			const metadata = sources[0];
			if (metadata === undefined) throw new Error('Projection source fixture is empty');
			const counted = countProjectionSourceReads(sources);
			const materialized = materializeTerminalProjectionFrame(metadata, counted.sources, {
				attachmentId: metadata.attachmentId,
				revealCursor: false,
				rowCount: size,
				scrollbackRows: 0n,
				sequence: 2n,
				sourceStartRow: 0n,
			});
			if (!materialized.ok) throw new Error(materialized.detail);
			expect(materialized.value.rowData).toHaveLength(size);
			expect(materialized.value.rowData.at(-1)?.cells[0]?.grapheme).toBe(`row-${size - 1}`);
			return counted.readCount();
		});

		expect(sourceReads).toEqual(sizes);
	});

	it('uses input-order first-match precedence for overlapping sources', () => {
		const firstMatch = decodedMaterializationSource(1n, ['first-match']);
		const fallback = decodedMaterializationSource(0n, ['fallback-zero', 'fallback-one']);
		const materialized = materializeTerminalProjectionFrame(fallback, [firstMatch, fallback], {
			attachmentId: fallback.attachmentId,
			revealCursor: true,
			rowCount: 2,
			scrollbackRows: 0n,
			sequence: 2n,
			sourceStartRow: 0n,
		});
		if (!materialized.ok) throw new Error(materialized.detail);

		expect(materialized.value.rowData.map((row) => row.cells[0]?.grapheme)).toEqual([
			'fallback-zero',
			'first-match',
		]);
	});

	it('lets an inconsistent first covering source shadow a later matching source', () => {
		const metadata = decodedMaterializationSource(0n, ['metadata']);
		const inconsistent = decodedMaterializationSource(
			0n,
			['inconsistent'],
			frame.cellPixelWidth + 1,
		);
		const matching = decodedMaterializationSource(0n, ['matching']);

		expect(
			materializeTerminalProjectionFrame(metadata, [inconsistent, matching], {
				attachmentId: metadata.attachmentId,
				revealCursor: true,
				rowCount: 1,
				scrollbackRows: 0n,
				sequence: 2n,
				sourceStartRow: 0n,
			}),
		).toEqual(
			expect.objectContaining({
				code: ProtocolErrorCode.invalidField,
				detail: 'Projection materialization is missing source row 0',
				ok: false,
			}),
		);
	});

	it('rejects a missing row between otherwise matching sources', () => {
		const first = decodedMaterializationSource(0n, ['first']);
		const last = decodedMaterializationSource(2n, ['last']);

		expect(
			materializeTerminalProjectionFrame(first, [first, last], {
				attachmentId: first.attachmentId,
				revealCursor: false,
				rowCount: 3,
				scrollbackRows: 0n,
				sequence: 2n,
				sourceStartRow: 0n,
			}),
		).toEqual(
			expect.objectContaining({
				code: ProtocolErrorCode.invalidField,
				detail: 'Projection materialization is missing source row 1',
				ok: false,
			}),
		);
	});

	it.each([
		{
			cellPixelHeight: frame.cellPixelHeight,
			cellPixelWidth: frame.cellPixelWidth + 1,
			dimension: 'cellPixelWidth',
		},
		{
			cellPixelHeight: frame.cellPixelHeight + 1,
			cellPixelWidth: frame.cellPixelWidth,
			dimension: 'cellPixelHeight',
		},
	])('rejects a decoded snapshot source mismatched only in $dimension', (geometry) => {
		const metadataBytes = encodeTerminalProjectionBuffer(frame);
		const sourceBytes = encodeTerminalProjectionBuffer({
			...frame,
			cellPixelHeight: geometry.cellPixelHeight,
			cellPixelWidth: geometry.cellPixelWidth,
		});
		if (!metadataBytes.ok || !sourceBytes.ok)
			throw new Error('Projection fixture failed to encode');
		const metadata = decodeTerminalProjectionBuffer(metadataBytes.value);
		const source = decodeTerminalProjectionBuffer(sourceBytes.value);
		expect(metadata).toMatchObject({ ok: true });
		expect(source).toMatchObject({ ok: true });
		if (!metadata.ok || !source.ok) throw new Error('Projection fixture failed to decode');
		expect(source.value).toMatchObject({
			cellPixelHeight: geometry.cellPixelHeight,
			cellPixelWidth: geometry.cellPixelWidth,
		});

		expect(
			materializeTerminalProjectionFrame(metadata.value, [source.value], {
				attachmentId: metadata.value.attachmentId,
				revealCursor: true,
				rowCount: 1,
				scrollbackRows: 0n,
				sequence: 2n,
				sourceStartRow: 0n,
			}),
		).toEqual(expect.objectContaining({ code: ProtocolErrorCode.invalidField, ok: false }));
	});

	it('round-trips a metadata-only delta with no dirty rows', () => {
		const encoded = encodeTerminalProjectionBuffer({
			...frame,
			baseRevision: 1n,
			currentDirectory: null,
			kind: TerminalFrameKind.delta,
			revision: 2n,
			rowData: [],
			sequence: 2n,
			title: null,
		});
		if (!encoded.ok) throw new Error(encoded.detail);

		expect(encoded.value).toHaveLength(320);
		for (const decode of [viewTerminalProjectionBuffer, decodeTerminalProjectionBuffer]) {
			const decoded = decode(encoded.value);
			if (!decoded.ok) throw new Error(decoded.detail);
			expect(decoded.value.kind).toBe(TerminalFrameKind.delta);
			expect(decoded.value.baseRevision).toBe(1n);
			expect(decoded.value.rowData.indices).toHaveLength(0);
		}
	});

	it('enforces the grid-area cell limit for metadata-only deltas', () => {
		const maximumGrid = metadataOnlyDeltaProjection(512, 512);
		expect(viewTerminalProjectionBuffer(maximumGrid)).toEqual(
			expect.objectContaining({ ok: true }),
		);
		expect(decodeTerminalProjectionBuffer(maximumGrid)).toEqual(
			expect.objectContaining({ ok: true }),
		);

		const oversizedGrid = metadataOnlyDeltaProjection(512, 513);
		expect(viewTerminalProjectionBuffer(oversizedGrid)).toEqual(
			expect.objectContaining({ code: ProtocolErrorCode.invalidField, ok: false }),
		);
		expect(decodeTerminalProjectionBuffer(oversizedGrid)).toEqual(
			expect.objectContaining({ code: ProtocolErrorCode.invalidField, ok: false }),
		);
	});

	it('exposes canonical columns as direct views of the immutable buffer', () => {
		const bytes = encodedProjection();
		const decoded = decodeTerminalProjectionBuffer(bytes);
		if (!decoded.ok) throw new Error(decoded.detail);

		expect(decoded.value.title).toBe('projection');
		expect(decoded.value.currentDirectory).toBe('/tmp');
		expect(decoded.value.cells.widths.buffer).toBe(bytes.buffer);
		expect(decoded.value.rowData.indices.buffer).toBe(bytes.buffer);
		expect(decoded.value.cells.graphemeLengths[0]).toBe(3);
	});

	it('keeps strict decode separate from the trusted local structural view', () => {
		const bytes = encodedProjection();
		bytes[sectionOffset(bytes, TerminalProjectionSection.widths)] = 0xff;

		expect(decodeTerminalProjectionBuffer(bytes)).toEqual(
			expect.objectContaining({ code: ProtocolErrorCode.invalidField, ok: false }),
		);
		expect(viewTerminalProjectionBuffer(bytes)).toEqual(expect.objectContaining({ ok: true }));
	});

	it('validates repeated string ranges with one whole-section UTF-8 scan', () => {
		const cellCount = 8;
		const encoded = encodeTerminalProjectionBuffer(repeatedProjection(cellCount));
		if (!encoded.ok) throw new Error(encoded.detail);
		const decodeSpy = vi.spyOn(TextDecoder.prototype, 'decode');
		try {
			const decoded = decodeTerminalProjectionBuffer(encoded.value);
			expect(decoded).toEqual(expect.objectContaining({ ok: true }));
			expect(decodeSpy).toHaveBeenCalledTimes(2);
		} finally {
			decodeSpy.mockRestore();
		}
	});

	it('encodes strings directly into the bounded output buffer', () => {
		const cellCount = 8;
		const encodeSpy = vi.spyOn(TextEncoder.prototype, 'encode');
		try {
			const encoded = encodeTerminalProjectionBuffer(repeatedProjection(cellCount));
			expect(encoded).toEqual(expect.objectContaining({ ok: true }));
			expect(encodeSpy.mock.calls.length).toBeLessThanOrEqual(cellCount + 4);
		} finally {
			encodeSpy.mockRestore();
		}
	});

	it('prepares a shared hyperlink byte key once per style', () => {
		const cellCount = 256;
		const repeated = repeatedProjection(cellCount);
		const row = repeated.rowData[0];
		const repeatedStyle = repeated.styles[0];
		const sharedHyperlink = 'https://example.test/shared-byte-value';
		if (row === undefined || repeatedStyle === undefined) {
			throw new Error('Repeated projection fixture is incomplete');
		}
		const projection = {
			...repeated,
			rowData: [
				{
					...row,
					cells: row.cells.map((cell, index) => ({ ...cell, style: index % 2 })),
				},
			],
			styles: [
				{ ...repeatedStyle, hyperlink: sharedHyperlink },
				{ ...repeatedStyle, bold: true, hyperlink: sharedHyperlink },
			],
		};
		const encoded = (() => {
			const keyConstructionSpy = vi.spyOn(String, 'fromCharCode');
			try {
				const result = encodeTerminalProjectionBuffer(projection);
				expect(keyConstructionSpy).toHaveBeenCalledTimes(2);
				return result;
			} finally {
				keyConstructionSpy.mockRestore();
			}
		})();
		if (!encoded.ok) throw new Error(encoded.detail);
		const decoded = decodeTerminalProjectionBuffer(encoded.value);
		if (!decoded.ok) throw new Error(decoded.detail);
		const firstOffset = decoded.value.cells.hyperlinkOffsets[0];
		expect(firstOffset).not.toBeUndefined();
		expect([...decoded.value.cells.hyperlinkOffsets]).toEqual(
			Array.from({ length: cellCount }, () => firstOffset),
		);
	});

	it('rejects a caller budget before allocating or encoding the output body', () => {
		const encodeIntoSpy = vi.spyOn(TextEncoder.prototype, 'encodeInto');
		try {
			expect(encodeTerminalProjectionBufferWithin(frame, 320)).toEqual(
				expect.objectContaining({ code: ProtocolErrorCode.limitExceeded, ok: false }),
			);
			expect(encodeIntoSpy).not.toHaveBeenCalled();
		} finally {
			encodeIntoSpy.mockRestore();
		}
	});

	it('strictly decodes the same wire bytes from every storage alignment', () => {
		const bytes = encodedProjection();
		for (const padding of [1, 2, 3]) {
			const storage = new Uint8Array(bytes.byteLength + padding);
			storage.set(bytes, padding);
			const unaligned = storage.subarray(padding);
			expect(viewTerminalProjectionBuffer(unaligned)).toEqual(
				expect.objectContaining({ code: ProtocolErrorCode.invalidField, ok: false }),
			);
			const decoded = decodeTerminalProjectionBuffer(unaligned);
			if (!decoded.ok) throw new Error(decoded.detail);
			expect(decoded.value.bytes.byteOffset % 4).toBe(0);
			expect(decoded.value.title).toBe(frame.title);
		}
	});

	it('keeps shared memory out of the trusted local view', () => {
		const bytes = encodedProjection();
		const sharedBytes = new Uint8Array(new SharedArrayBuffer(bytes.byteLength));
		sharedBytes.set(bytes);
		expect(viewTerminalProjectionBuffer(sharedBytes)).toEqual(
			expect.objectContaining({ code: ProtocolErrorCode.invalidField, ok: false }),
		);
		const decoded = decodeTerminalProjectionBuffer(sharedBytes);
		if (!decoded.ok) throw new Error(decoded.detail);
		expect(decoded.value.bytes.buffer).toBeInstanceOf(ArrayBuffer);
		const widthOffset = sectionOffset(sharedBytes, TerminalProjectionSection.widths);
		sharedBytes[widthOffset] = 0xff;
		expect(decoded.value.cells.widths[0]).toBe(0);
	});

	it('rejects cross-realm shared views and snapshots only the exact strict input view', () => {
		const bytes = encodedProjection();
		const sharedBytes = crossRealmSharedSubview(bytes);
		expect(sharedBytes.buffer).not.toBeInstanceOf(SharedArrayBuffer);
		expect(sharedBytes.byteOffset).toBe(4);
		expect(viewTerminalProjectionBuffer(sharedBytes)).toEqual(
			expect.objectContaining({ code: ProtocolErrorCode.invalidField, ok: false }),
		);

		const decoded = decodeTerminalProjectionBuffer(sharedBytes);
		if (!decoded.ok) throw new Error(decoded.detail);
		expect(decoded.value.bytes.buffer).toBeInstanceOf(ArrayBuffer);
		expect(decoded.value.bytes.buffer.byteLength).toBe(bytes.byteLength);
		expect(decoded.value.bytes.byteOffset).toBe(0);
		const widthOffset = sectionOffset(sharedBytes, TerminalProjectionSection.widths);
		sharedBytes[widthOffset] = 0xff;
		expect(decoded.value.cells.widths[0]).toBe(0);
	});

	it('does not mistake a spoofed ArrayBuffer tag for shared memory', () => {
		const bytes = encodedProjection();
		Object.defineProperty(bytes.buffer, Symbol.toStringTag, {
			configurable: true,
			value: 'SharedArrayBuffer',
		});
		expect(Object.prototype.toString.call(bytes.buffer)).toBe('[object SharedArrayBuffer]');

		const viewed = viewTerminalProjectionBuffer(bytes);
		const decoded = decodeTerminalProjectionBuffer(bytes);
		if (!viewed.ok || !decoded.ok) throw new Error('Spoofed ordinary storage was rejected');
		expect(viewed.value.bytes.buffer).toBe(bytes.buffer);
		expect(decoded.value.bytes.buffer).toBe(bytes.buffer);
	});

	it('maps a shared-memory copy failure to a typed protocol failure', () => {
		const bytes = encodedProjection();
		const sharedBytes = new Uint8Array(new SharedArrayBuffer(bytes.byteLength));
		sharedBytes.set(bytes);
		const setSpy = vi.spyOn(Uint8Array.prototype, 'set').mockImplementationOnce(() => {
			throw new RangeError('controlled shared-memory copy failure');
		});
		try {
			expect(decodeTerminalProjectionBuffer(sharedBytes)).toEqual(
				expect.objectContaining({ code: ProtocolErrorCode.invalidField, ok: false }),
			);
		} finally {
			setSpy.mockRestore();
		}
	});

	it('rejects nonzero canonical alignment padding on the structural path', () => {
		const bytes = encodedProjection();
		const priorSection = TerminalProjectionSection.graphemeLengths;
		const paddingOffset = sectionOffset(bytes, priorSection) + sectionLength(bytes, priorSection);
		expect(paddingOffset).toBeLessThan(
			sectionOffset(bytes, TerminalProjectionSection.hyperlinkOffsets),
		);
		bytes[paddingOffset] = 1;

		expect(viewTerminalProjectionBuffer(bytes)).toEqual(
			expect.objectContaining({ code: ProtocolErrorCode.invalidField, ok: false }),
		);
	});
});

describe('unknown cell-pixel geometry across frame and projection boundaries', () => {
	it('preserves paired zero through schemas, encoders and strict decoders', () => {
		const unknown = { ...frame, cellPixelWidth: 0, cellPixelHeight: 0 };
		expect(TerminalFrameSchema.safeParse(unknown).success).toBe(true);
		const projection = encodeTerminalProjectionBuffer(unknown);
		const terminal = encodeTerminalFrame(unknown);
		expect(projection.ok).toBe(true);
		expect(terminal.ok).toBe(true);
		if (!projection.ok || !terminal.ok) throw new Error('unknown geometry encoding failed');
		const packed = decodeTerminalProjectionBuffer(projection.value);
		const text = decodeTerminalFrame(terminal.value);
		expect(packed.ok).toBe(true);
		expect(text.ok).toBe(true);
		if (!packed.ok || !text.ok) throw new Error('unknown geometry decoding failed');
		expect([packed.value.cellPixelWidth, packed.value.cellPixelHeight]).toEqual([0, 0]);
		expect([text.value.cellPixelWidth, text.value.cellPixelHeight]).toEqual([0, 0]);
		for (const width of [true, false]) {
			const mixed = { ...unknown, ...(width ? { cellPixelWidth: 8 } : { cellPixelHeight: 16 }) };
			expect(TerminalFrameSchema.safeParse(mixed).success).toBe(false);
			for (const [bytes, offset, decode] of [
				[projection.value, 100, decodeTerminalProjectionBuffer],
				[terminal.value, 136, decodeTerminalFrame],
			] as const) {
				const malformed = bytes.slice();
				new DataView(malformed.buffer, malformed.byteOffset, malformed.byteLength).setUint16(
					offset + (width ? 0 : 2),
					8,
					true,
				);
				expect(decode(malformed)).toEqual(
					expect.objectContaining({ ok: false, code: ProtocolErrorCode.invalidField }),
				);
			}
		}
	});
});
