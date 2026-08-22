import { readFile } from 'node:fs/promises';

import { describe, expect, it, vi } from 'vitest';

import {
	ActiveScreen,
	CellWidth,
	CursorShape,
	RowPromptClass,
	SemanticContent,
	TerminalColorKind,
	TerminalFrameKind,
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

function encodedProjection(): Uint8Array {
	const encoded = encodeTerminalProjectionBuffer(frame);
	if (!encoded.ok) throw new Error(encoded.detail);
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

describe('terminal projection buffer', () => {
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
			expect(encodeSpy.mock.calls.length).toBeLessThanOrEqual(cellCount + 3);
		} finally {
			encodeSpy.mockRestore();
		}
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

	it('classifies every canonical OBPB mutation like the native strict validator', async () => {
		const fixtureUrl = new URL(
			'../../fixtures/terminal-projection-buffer-v0.0.2.bin',
			import.meta.url,
		);
		const bytes = new Uint8Array(await readFile(fixtureUrl));
		const fixtureSectionOffset = (section: number) =>
			new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength).getUint32(
				160 + section * 8,
				true,
			);
		const width = fixtureSectionOffset(TerminalProjectionSection.widths);
		const rowFlags = fixtureSectionOffset(TerminalProjectionSection.rowFlags);
		const rowIndices = fixtureSectionOffset(TerminalProjectionSection.rowIndices);
		const prompt = fixtureSectionOffset(TerminalProjectionSection.rowPromptClasses);
		const stringSection = fixtureSectionOffset(TerminalProjectionSection.stringBytes);
		const graphemeLengths = fixtureSectionOffset(TerminalProjectionSection.graphemeLengths);
		const graphemeOffsets = fixtureSectionOffset(TerminalProjectionSection.graphemeOffsets);
		const foreground = fixtureSectionOffset(TerminalProjectionSection.foregroundColors);
		const styleFlags = fixtureSectionOffset(TerminalProjectionSection.styleFlags);
		const underlineKinds = fixtureSectionOffset(TerminalProjectionSection.underlineKinds);
		const semantic = fixtureSectionOffset(TerminalProjectionSection.semanticContents);
		const protectedCells = fixtureSectionOffset(TerminalProjectionSection.protectedCells);
		const cases = [
			{
				name: 'width-out-of-range',
				offset: width,
				bytes: [0xff],
				code: ProtocolErrorCode.invalidField,
			},
			{
				name: 'row-flag-unknown',
				offset: rowFlags,
				bytes: [0x05],
				code: ProtocolErrorCode.unknownRequiredFlag,
			},
			{
				name: 'row-index-out-of-range',
				offset: rowIndices + 2,
				bytes: [0x02, 0x00],
				code: ProtocolErrorCode.invalidField,
			},
			{
				name: 'prompt-class-out-of-range',
				offset: prompt,
				bytes: [0x03],
				code: ProtocolErrorCode.invalidField,
			},
			{
				name: 'whole-string-invalid-utf8',
				offset: stringSection + 25,
				bytes: [0xff],
				code: ProtocolErrorCode.invalidUtf8,
			},
			{
				name: 'grapheme-length-splits-scalar',
				offset: graphemeLengths,
				bytes: [0x02, 0x00],
				code: ProtocolErrorCode.invalidUtf8,
			},
			{
				name: 'grapheme-offset-null-nonnullable',
				offset: graphemeOffsets,
				bytes: [0xff, 0xff, 0xff, 0xff],
				code: ProtocolErrorCode.invalidField,
			},
			{
				name: 'color-out-of-range',
				offset: foreground,
				bytes: [0x00, 0x00, 0x00, 0x04],
				code: ProtocolErrorCode.invalidField,
			},
			{
				name: 'style-flag-unknown',
				offset: styleFlags,
				bytes: [0x00, 0x01],
				code: ProtocolErrorCode.unknownRequiredFlag,
			},
			{
				name: 'underline-kind-out-of-range',
				offset: underlineKinds,
				bytes: [0x06],
				code: ProtocolErrorCode.invalidField,
			},
			{
				name: 'width-spacer-tail-without-head',
				offset: width,
				bytes: [0x02],
				code: ProtocolErrorCode.invalidField,
			},
			{
				name: 'row-ends-with-wide-cell',
				offset: width + 5,
				bytes: [0x01],
				code: ProtocolErrorCode.invalidField,
			},
			{
				name: 'semantic-out-of-range',
				offset: semantic,
				bytes: [0x03],
				code: ProtocolErrorCode.invalidField,
			},
			{
				name: 'protected-out-of-range',
				offset: protectedCells,
				bytes: [0x02],
				code: ProtocolErrorCode.invalidField,
			},
			{
				name: 'adjacent-wrap-disagreement',
				offset: rowFlags + 1,
				bytes: [0x00],
				code: ProtocolErrorCode.invalidField,
			},
			{
				name: 'interaction-unknown-required-flag',
				offset: 108,
				bytes: [0x8d, 0x62, 0x04, 0x00],
				code: ProtocolErrorCode.unknownRequiredFlag,
			},
			{
				name: 'interaction-mouse-mode-conflict',
				offset: 108,
				bytes: [0x9d, 0x62, 0x00, 0x00],
				code: ProtocolErrorCode.invalidField,
			},
		];
		for (const fixtureCase of cases) {
			const mutated = bytes.slice();
			mutated.set(fixtureCase.bytes, fixtureCase.offset);
			expect({
				name: fixtureCase.name,
				result: decodeTerminalProjectionBuffer(mutated),
			}).toEqual({
				name: fixtureCase.name,
				result: expect.objectContaining({ code: fixtureCase.code, ok: false }),
			});
		}
	});
});
