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
import {
	decodeTerminalProjectionBuffer,
	encodeTerminalProjectionBuffer,
	encodeTerminalProjectionBufferWithin,
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
		alternateScroll: false,
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
