import { describe, expect, it } from 'vitest';

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
	it('round-trips a render window through the nested format-2 snapshot', () => {
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
				scrollbackRows: 9n,
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
		expect(decoded.value.frame.strings).toContain('蜂');
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
		const formatOneZero = encoded.value.slice();
		formatOneZero[5] = 0;
		expect(decodeTerminalRowWindow(formatOneZero)).toEqual(expect.objectContaining({ ok: true }));

		const malformed = encoded.value.slice();
		malformed[7] = 0x80;
		expect(decodeTerminalRowWindow(malformed)).toEqual(
			expect.objectContaining({ code: ProtocolErrorCode.unknownRequiredFlag, ok: false }),
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
		expect(encoded.value[5]).toBe(1);
		expect(encoded.value[112]).toBe(0x04);
		expect(new DataView(encoded.value.buffer).getUint32(92, true)).toBe(3);
		expect(new DataView(encoded.value.buffer).getUint32(120, true)).toBe(0);

		const decoded = decodeTerminalRowWindow(encoded.value);
		if (!decoded.ok) throw new Error(decoded.detail);
		if (decoded.value.detail !== TerminalRowWindowDetail.text) return;
		expect(decoded.value.rows[0]).toEqual(
			expect.objectContaining({ identityAscii: true, text: 'abc' }),
		);
		expect(decoded.value.rows[0]?.columns).toHaveLength(0);

		const versionZero = encoded.value.slice();
		versionZero[5] = 0;
		expect(decodeTerminalRowWindow(versionZero)).toEqual(
			expect.objectContaining({ code: ProtocolErrorCode.unknownRequiredFlag, ok: false }),
		);
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
