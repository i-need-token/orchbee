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
import { ProtocolErrorCode } from '../protocol-result';
import {
	decodeTerminalRowWindow,
	encodeTerminalRowWindow,
	TerminalRowWindowDetail,
	viewTerminalRowWindow,
} from '../row-window';
import { decodeSelectionExportChunk, encodeSelectionExportChunk } from '../selection';

const interaction = {
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
} as const;

const style = {
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
} as const;

describe('packed terminal row windows', () => {
	it('round-trips a render window through the nested OBPB 0.0.1 projection buffer', () => {
		const encoded = encodeTerminalRowWindow({
			attachmentId: '22222222222222222222222222222222',
			columns: 1,
			detail: TerminalRowWindowDetail.render,
			frame: {
				activeScreen: ActiveScreen.primary,
				attachmentId: '22222222222222222222222222222222',
				baseRevision: null,
				cellPixelHeight: 16,
				cellPixelWidth: 8,
				columns: 1,
				cursor: {
					blinking: false,
					column: 0,
					row: 0,
					shape: CursorShape.block,
					visible: false,
					wideTail: false,
				},
				currentDirectory: null,
				defaultColors: {
					backgroundRgb: 0,
					cursorRgb: null,
					foregroundRgb: 0xffffff,
				},
				extensions: [],
				generation: 2n,
				hostEpoch: '11111111111111111111111111111111',
				interaction,
				kind: TerminalFrameKind.snapshot,
				revision: 7n,
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
				scrollbackRows: 4n,
				sequence: 3n,
				styles: [style],
				title: null,
			},
			generation: 2n,
			hostEpoch: '11111111111111111111111111111111',
			revision: 7n,
			sequence: 3n,
			startRow: 4n,
			totalRows: 10n,
		});
		if (!encoded.ok) throw new Error(encoded.detail);
		const decoded = decodeTerminalRowWindow(encoded.value);
		if (!decoded.ok) throw new Error(decoded.detail);
		expect(decoded.value.detail).toBe(TerminalRowWindowDetail.render);
		if (decoded.value.detail !== TerminalRowWindowDetail.render) return;
		expect(decoded.value.startRow).toBe(4n);
		expect(decoded.value.frame.bytes.buffer).toBe(encoded.value.buffer);
		expect(decoded.value.frame.cells.widths.buffer).toBe(encoded.value.buffer);
		const graphemeOffset = decoded.value.frame.cells.graphemeOffsets[0] ?? 0;
		const graphemeLength = decoded.value.frame.cells.graphemeLengths[0] ?? 0;
		expect(
			new TextDecoder().decode(
				decoded.value.frame.stringBytes.subarray(graphemeOffset, graphemeOffset + graphemeLength),
			),
		).toBe('蜂');
		expect(decoded.value).not.toHaveProperty('frameBytes');

		const mismatchedHistoryOrigin = encoded.value.slice();
		new DataView(mismatchedHistoryOrigin.buffer).setBigUint64(80, 5n, true);
		expect(decodeTerminalRowWindow(mismatchedHistoryOrigin)).toEqual(
			expect.objectContaining({ code: ProtocolErrorCode.invalidField, ok: false }),
		);

		const noncanonicalDirectory = encoded.value.slice();
		const projectionDirectory = 112 + 160;
		new DataView(noncanonicalDirectory.buffer).setUint32(projectionDirectory, 0, true);
		expect(decodeTerminalRowWindow(noncanonicalDirectory)).toEqual(
			expect.objectContaining({ code: ProtocolErrorCode.invalidField, ok: false }),
		);

		const invalidTrustedCell = encoded.value.slice();
		const invalidView = new DataView(invalidTrustedCell.buffer);
		const widthDirectoryEntry = projectionDirectory + 12 * 8;
		const widthOffset = 112 + invalidView.getUint32(widthDirectoryEntry, true);
		invalidTrustedCell[widthOffset] = 0xff;
		expect(decodeTerminalRowWindow(invalidTrustedCell)).toEqual(
			expect.objectContaining({ code: ProtocolErrorCode.invalidField, ok: false }),
		);
		expect(viewTerminalRowWindow(invalidTrustedCell)).toEqual(
			expect.objectContaining({ ok: true }),
		);
	});

	it('round-trips compact text and rejects reserved flags atomically', () => {
		const encoded = encodeTerminalRowWindow({
			attachmentId: '22222222222222222222222222222222',
			columns: 4,
			detail: TerminalRowWindowDetail.text,
			generation: 1n,
			hostEpoch: '11111111111111111111111111111111',
			revision: 9n,
			rows: [
				{
					index: 11n,
					mappings: [
						{ byteOffset: 0, column: 0, utf16Offset: 0, width: 1 },
						{ byteOffset: 1, column: 1, utf16Offset: 1, width: 2 },
					],
					promptClass: RowPromptClass.prompt,
					text: 'A蜂',
					wrap: true,
					wrapContinuation: false,
				},
			],
			sequence: 4n,
			startRow: 11n,
			totalRows: 20n,
		});
		if (!encoded.ok) throw new Error(encoded.detail);
		const decoded = decodeTerminalRowWindow(encoded.value);
		if (!decoded.ok) throw new Error(decoded.detail);
		expect(decoded.value.detail).toBe(TerminalRowWindowDetail.text);
		if (decoded.value.detail !== TerminalRowWindowDetail.text) return;
		expect(decoded.value.rows[0]?.text).toBe('A蜂');
		expect(decoded.value.rows[0]?.identityAscii).toBe(false);
		expect([...(decoded.value.rows[0]?.columns ?? [])]).toEqual([0, 1]);
		const differentFormat = encoded.value.slice();
		differentFormat[5] = 1;
		expect(decodeTerminalRowWindow(differentFormat)).toEqual(
			expect.objectContaining({ code: ProtocolErrorCode.unsupportedVersion, ok: false }),
		);

		const malformed = encoded.value.slice();
		malformed[7] = 0x80;
		expect(decodeTerminalRowWindow(malformed)).toEqual(
			expect.objectContaining({ code: ProtocolErrorCode.unknownRequiredFlag, ok: false }),
		);
	});

	it('encodes each text row once after bounded preflight and rejects shared local views', () => {
		const window = {
			attachmentId: '22222222222222222222222222222222',
			columns: 2,
			detail: TerminalRowWindowDetail.text,
			generation: 1n,
			hostEpoch: '11111111111111111111111111111111',
			revision: 0n,
			rows: [
				{
					index: 4n,
					mappings: [
						{ byteOffset: 0, column: 0, utf16Offset: 0, width: 1 },
						{ byteOffset: 1, column: 1, utf16Offset: 1, width: 1 },
					],
					promptClass: RowPromptClass.none,
					text: 'ab',
					wrap: false,
					wrapContinuation: false,
				},
				{
					index: 5n,
					mappings: [
						{ byteOffset: 0, column: 0, utf16Offset: 0, width: 1 },
						{ byteOffset: 1, column: 1, utf16Offset: 1, width: 1 },
					],
					promptClass: RowPromptClass.none,
					text: 'cd',
					wrap: false,
					wrapContinuation: false,
				},
			],
			sequence: 1n,
			startRow: 4n,
			totalRows: 6n,
		} as const;
		const encodeSpy = vi.spyOn(TextEncoder.prototype, 'encode');
		let encoded: ReturnType<typeof encodeTerminalRowWindow>;
		try {
			encoded = encodeTerminalRowWindow(window);
			expect(encoded).toEqual(expect.objectContaining({ ok: true }));
			expect(encodeSpy).toHaveBeenCalledTimes(window.rows.length);
		} finally {
			encodeSpy.mockRestore();
		}
		if (!encoded.ok) throw new Error(encoded.detail);

		const sharedBytes = new Uint8Array(new SharedArrayBuffer(encoded.value.byteLength));
		sharedBytes.set(encoded.value);
		expect(viewTerminalRowWindow(sharedBytes)).toEqual(
			expect.objectContaining({ code: ProtocolErrorCode.invalidField, ok: false }),
		);
		expect(decodeTerminalRowWindow(sharedBytes)).toEqual(expect.objectContaining({ ok: true }));
	});

	it('keeps supplementary UTF-8 and UTF-16 mapping boundaries aligned', () => {
		const window = {
			attachmentId: '22222222222222222222222222222222',
			columns: 4,
			detail: TerminalRowWindowDetail.text,
			generation: 1n,
			hostEpoch: '11111111111111111111111111111111',
			revision: 9n,
			rows: [
				{
					index: 11n,
					mappings: [
						{ byteOffset: 0, column: 0, utf16Offset: 0, width: 1 },
						{ byteOffset: 1, column: 1, utf16Offset: 1, width: 2 },
						{ byteOffset: 5, column: 3, utf16Offset: 3, width: 1 },
					],
					promptClass: RowPromptClass.none,
					text: 'A🐝B',
					wrap: false,
					wrapContinuation: false,
				},
			],
			sequence: 4n,
			startRow: 11n,
			totalRows: 20n,
		} as const;
		const encoded = encodeTerminalRowWindow(window);
		if (!encoded.ok) throw new Error(encoded.detail);
		const decoded = decodeTerminalRowWindow(encoded.value);
		if (!decoded.ok) throw new Error(decoded.detail);
		if (decoded.value.detail !== TerminalRowWindowDetail.text) return;
		expect([...(decoded.value.rows[0]?.byteOffsets ?? [])]).toEqual([0, 1, 5]);
		expect([...(decoded.value.rows[0]?.utf16Offsets ?? [])]).toEqual([0, 1, 3]);

		expect(
			encodeTerminalRowWindow({
				...window,
				rows: [
					{
						...window.rows[0],
						mappings: [
							window.rows[0].mappings[0],
							window.rows[0].mappings[1],
							{ ...window.rows[0].mappings[2], utf16Offset: 2 },
						],
					},
				],
			}),
		).toEqual(expect.objectContaining({ code: ProtocolErrorCode.invalidField, ok: false }));

		const invalidByteBoundary = encoded.value.slice();
		const view = new DataView(
			invalidByteBoundary.buffer,
			invalidByteBoundary.byteOffset,
			invalidByteBoundary.byteLength,
		);
		const textLength = view.getUint32(116, true);
		const secondMapping = 124 + textLength + 12;
		view.setUint32(secondMapping, 2, true);
		expect(decodeTerminalRowWindow(invalidByteBoundary)).toEqual(
			expect.objectContaining({ code: ProtocolErrorCode.invalidField, ok: false }),
		);
	});

	it('omits identity mappings for narrow ASCII rows while preserving logical entries', () => {
		const encoded = encodeTerminalRowWindow({
			attachmentId: '22222222222222222222222222222222',
			columns: 3,
			detail: TerminalRowWindowDetail.text,
			generation: 1n,
			hostEpoch: '11111111111111111111111111111111',
			revision: 9n,
			rows: [
				{
					index: 11n,
					mappings: [
						{ byteOffset: 0, column: 0, utf16Offset: 0, width: 1 },
						{ byteOffset: 1, column: 1, utf16Offset: 1, width: 1 },
						{ byteOffset: 2, column: 2, utf16Offset: 2, width: 1 },
					],
					promptClass: RowPromptClass.none,
					text: 'abc',
					wrap: false,
					wrapContinuation: false,
				},
			],
			sequence: 4n,
			startRow: 11n,
			totalRows: 20n,
		});
		if (!encoded.ok) throw new Error(encoded.detail);
		expect(encoded.value[5]).toBe(0);
		expect(encoded.value[112]).toBe(0x04);
		expect(new DataView(encoded.value.buffer).getUint32(92, true)).toBe(3);
		expect(new DataView(encoded.value.buffer).getUint32(120, true)).toBe(0);

		const decoded = decodeTerminalRowWindow(encoded.value);
		if (!decoded.ok) throw new Error(decoded.detail);
		if (decoded.value.detail !== TerminalRowWindowDetail.text) return;
		expect(decoded.value.rows[0]).toEqual(
			expect.objectContaining({ identityAscii: true, text: 'abc' }),
		);
		expect(decoded.value.rows[0]?.byteOffsets).toHaveLength(0);
		expect(decoded.value.rows[0]?.columns).toHaveLength(0);
		expect(decoded.value.rows[0]?.utf16Offsets).toHaveLength(0);
		expect(decoded.value.rows[0]?.widths).toHaveLength(0);
	});

	it('rejects incomplete, overlapping, and Unicode-incoherent text mappings', () => {
		const base = {
			attachmentId: '22222222222222222222222222222222',
			columns: 4,
			detail: TerminalRowWindowDetail.text,
			generation: 1n,
			hostEpoch: '11111111111111111111111111111111',
			revision: 9n,
			rows: [
				{
					index: 11n,
					mappings: [
						{ byteOffset: 0, column: 0, utf16Offset: 0, width: 1 },
						{ byteOffset: 1, column: 1, utf16Offset: 1, width: 2 },
					],
					promptClass: RowPromptClass.none,
					text: 'A蜂',
					wrap: false,
					wrapContinuation: false,
				},
			],
			sequence: 4n,
			startRow: 11n,
			totalRows: 20n,
		} as const;

		expect(
			encodeTerminalRowWindow({
				...base,
				rows: [{ ...base.rows[0], text: '\ud800' }],
			}),
		).toEqual(expect.objectContaining({ code: ProtocolErrorCode.invalidField, ok: false }));

		for (const rows of [
			[{ ...base.rows[0], mappings: [] }],
			[
				{
					...base.rows[0],
					mappings: [base.rows[0].mappings[0], { ...base.rows[0].mappings[1], byteOffset: 2 }],
				},
			],
			[
				{
					...base.rows[0],
					mappings: [{ ...base.rows[0].mappings[0], width: 2 }, base.rows[0].mappings[1]],
				},
			],
		]) {
			expect(encodeTerminalRowWindow({ ...base, rows })).toEqual(
				expect.objectContaining({ code: ProtocolErrorCode.invalidField, ok: false }),
			);
		}
	});

	it('round-trips one bounded selection-export chunk', () => {
		const encoded = encodeSelectionExportChunk({
			attachmentId: '22222222222222222222222222222222',
			bytes: new TextEncoder().encode('selected'),
			chunkSequence: 0,
			exportSequence: 2n,
			final: true,
		});
		if (!encoded.ok) throw new Error(encoded.detail);
		const decoded = decodeSelectionExportChunk(encoded.value);
		if (!decoded.ok) throw new Error(decoded.detail);
		expect(new TextDecoder().decode(decoded.value.bytes)).toBe('selected');
		expect(decoded.value.final).toBe(true);
	});
});
