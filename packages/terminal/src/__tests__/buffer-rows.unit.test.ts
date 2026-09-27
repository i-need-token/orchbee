import {
	ActiveScreen,
	CellWidth,
	CursorShape,
	decodeTerminalRowWindow,
	encodeTerminalRowWindow,
	PROTOCOL_LIMITS,
	RowPromptClass,
	SemanticContent,
	TerminalColorKind,
	TerminalFrameKind,
	TerminalGraphicsUpdateKind,
	TerminalRowWindowDetail,
	UnderlineKind,
} from '@orchbee/terminal-protocol';
import { describe, expect, it } from 'vitest';

import {
	bufferRowsFromDecodedWindow,
	TerminalBufferRowDetail,
	validateBufferRowsRequest,
} from '../buffer-rows';

const emptyGraphics = {
	animationTick: 0n,
	images: [],
	kind: TerminalGraphicsUpdateKind.snapshot,
	placements: [],
	resourceRevision: 0n,
} as const;

describe('terminal buffer rows', () => {
	it('converts compact identity-ASCII row windows without allocating explicit mappings', () => {
		const encoded = encodeTerminalRowWindow({
			attachmentId: '22'.repeat(16),
			columns: 3,
			detail: TerminalRowWindowDetail.text,
			generation: 1n,
			hostEpoch: '11'.repeat(16),
			revision: 2n,
			rows: [
				{
					index: 4n,
					mappings: [
						{ byteOffset: 0, column: 0, utf16Offset: 0, width: 1 },
						{ byteOffset: 1, column: 1, utf16Offset: 1, width: 1 },
						{ byteOffset: 2, column: 2, utf16Offset: 2, width: 1 },
					],
					promptClass: RowPromptClass.none,
					text: 'cat',
					wrap: false,
					wrapContinuation: false,
				},
			],
			sequence: 3n,
			startRow: 4n,
			totalRows: 5n,
		});
		if (!encoded.ok) throw new Error(encoded.detail);
		const decoded = decodeTerminalRowWindow(encoded.value);
		if (!decoded.ok) throw new Error(decoded.detail);
		const result = bufferRowsFromDecodedWindow(decoded.value, encoded.value.byteLength);

		expect(result).toMatchObject({
			detail: TerminalBufferRowDetail.text,
			rows: [{ identityAscii: true, mappings: [], text: 'cat' }],
			startRow: 4n,
			totalRows: 5n,
		});
	});

	it('preserves leading U+FEFF in OBPB cells and derived buffer-row mappings', () => {
		const grapheme = '\uFEFFA';
		const hyperlink = '\uFEFFx';
		const baseStyle = {
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
		const encoded = encodeTerminalRowWindow({
			attachmentId: '22'.repeat(16),
			columns: 2,
			detail: TerminalRowWindowDetail.render,
			graphics: emptyGraphics,
			frame: {
				activeScreen: ActiveScreen.primary,
				attachmentId: '22'.repeat(16),
				baseRevision: null,
				cellPixelHeight: 16,
				cellPixelWidth: 8,
				columns: 2,
				currentDirectory: null,
				cursor: {
					blinking: false,
					column: 0,
					row: 0,
					shape: CursorShape.block,
					visible: false,
					wideTail: false,
				},
				defaultColors: {
					backgroundRgb: 0,
					cursorRgb: null,
					foregroundRgb: 0xffffff,
				},
				extensions: [],
				generation: 1n,
				hostEpoch: '11'.repeat(16),
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
					mouseUrxvt: false,
					mouseUtf8: false,
					mouseX10: false,
				},
				kind: TerminalFrameKind.snapshot,
				revision: 2n,
				rowData: [
					{
						cells: [
							{
								grapheme,
								protected: false,
								semanticContent: SemanticContent.output,
								style: 0,
								width: CellWidth.narrow,
							},
							{
								grapheme: 'B',
								protected: false,
								semanticContent: SemanticContent.output,
								style: 1,
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
				styles: [{ ...baseStyle, hyperlink }, baseStyle],
				themeRevision: 0n,
				title: null,
			},
			generation: 1n,
			hostEpoch: '11'.repeat(16),
			revision: 2n,
			sequence: 3n,
			startRow: 4n,
			totalRows: 5n,
		});
		if (!encoded.ok) throw new Error(encoded.detail);
		const decoded = decodeTerminalRowWindow(encoded.value);
		if (!decoded.ok) throw new Error(decoded.detail);
		const result = bufferRowsFromDecodedWindow(decoded.value, encoded.value.byteLength);
		const row = result.rows[0];
		const cells = row?.cells;

		expect(cells?.[0]).toMatchObject({ column: 0, grapheme, hyperlink, width: 1 });
		expect(cells?.[0]?.grapheme.codePointAt(0)).toBe(0xfeff);
		expect(cells?.[0]?.hyperlink?.codePointAt(0)).toBe(0xfeff);
		expect(cells?.[1]).toMatchObject({ column: 1, grapheme: 'B', hyperlink: null, width: 1 });
		expect(row?.text).toBe('\uFEFFAB');
		expect(row?.text.codePointAt(0)).toBe(0xfeff);
		expect(row?.mappings).toEqual([
			{ byteOffset: 0, column: 0, utf16Offset: 0, width: 1 },
			{ byteOffset: 4, column: 1, utf16Offset: 2, width: 1 },
		]);
	});

	it('rejects requests outside the negotiated row and byte bounds', () => {
		const request = {
			detail: TerminalBufferRowDetail.text,
			generation: 1n,
			maximumBytes: PROTOCOL_LIMITS.maxRowWindowBytes,
			revision: 1n,
			rowCount: 1,
			startRow: 0n,
		};
		expect(validateBufferRowsRequest(request)).toBe(true);
		expect(
			validateBufferRowsRequest({
				...request,
				rowCount: PROTOCOL_LIMITS.maxRowWindowRows + 1,
			}),
		).toBe(false);
		expect(validateBufferRowsRequest({ ...request, maximumBytes: 0 })).toBe(false);
	});
});
