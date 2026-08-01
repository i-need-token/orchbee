import { describe, expect, it } from 'vitest';

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
	UnderlineKind,
} from '../frame';

describe('packed terminal frame', () => {
	it('decodes complete rows into typed columns', () => {
		const encoded = encodeTerminalFrame({
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
				visible: true,
				wideTail: false,
			},
			currentDirectory: null,
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
			revision: 0n,
			rowData: [
				{
					cells: [
						{
							grapheme: 'A',
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
			title: null,
		});
		if (!encoded.ok) throw new Error(encoded.detail);
		const decoded = decodeTerminalFrame(encoded.value);
		if (!decoded.ok) throw new Error(decoded.detail);
		expect(decoded.value.cells.graphemeStringIndices).toBeInstanceOf(Uint32Array);
		expect([...decoded.value.cells.widths]).toEqual([0]);
		expect([...decoded.value.rowData.indices]).toEqual([0]);
	});
});
