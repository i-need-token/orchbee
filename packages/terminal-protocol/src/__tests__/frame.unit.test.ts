import { describe, expect, it } from 'vitest';

import {
	ActiveScreen,
	CellWidth,
	CursorShape,
	decodeTerminalFrame,
	decodeTerminalFrameRoute,
	encodeTerminalFrame,
	inspectTerminalFrameHeader,
	RowPromptClass,
	SemanticContent,
	TerminalColorKind,
	TerminalFrameExtensionKind,
	TerminalFrameKind,
	UnderlineKind,
} from '../frame';
import {
	encodeTerminalGraphics,
	TerminalGraphicsImageFormat,
	TerminalGraphicsUpdateKind,
} from '../terminal-graphics';

function commandCompletionExtension(
	sequence = 7n,
	durationMilliseconds = 5_500n,
	completedAtMilliseconds = 1_786_947_200_000n,
	exitCode = 42,
): Uint8Array {
	const data = new Uint8Array(25);
	const view = new DataView(data.buffer);
	view.setBigUint64(0, sequence, true);
	view.setBigUint64(8, durationMilliseconds, true);
	view.setBigUint64(16, completedAtMilliseconds, true);
	view.setUint8(24, exitCode);
	return data;
}

describe('packed terminal frame', () => {
	it('decodes complete rows into typed columns', () => {
		const frame = {
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
			extensions: [
				{
					data: new Uint8Array([1, 42]),
					kind: TerminalFrameExtensionKind.progress,
					required: false,
				},
				{
					data: commandCompletionExtension(),
					kind: TerminalFrameExtensionKind.commandCompletion,
					required: false,
				},
			],
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
		} as const;
		const encoded = encodeTerminalFrame(frame);
		if (!encoded.ok) throw new Error(encoded.detail);
		const decoded = decodeTerminalFrame(encoded.value);
		if (!decoded.ok) throw new Error(decoded.detail);
		expect(decoded.value.cells.graphemeStringIndices).toBeInstanceOf(Uint32Array);
		expect([...decoded.value.cells.widths]).toEqual([0]);
		expect([...decoded.value.rowData.indices]).toEqual([0]);
		expect(decoded.value.progress).toEqual({ state: 'set', value: 42 });
		expect(decoded.value.commandCompletion).toEqual({
			completedAtMilliseconds: 1_786_947_200_000n,
			durationMilliseconds: 5_500n,
			exitCode: 42,
			sequence: 7n,
		});
		expect(decodeTerminalFrameRoute(encoded.value)).toEqual({
			ok: true,
			value: {
				attachmentId: '22222222222222222222222222222222',
				hostEpoch: '11111111111111111111111111111111',
			},
		});
		expect(inspectTerminalFrameHeader(encoded.value)).toEqual({
			ok: true,
			value: {
				attachmentId: '22222222222222222222222222222222',
				baseRevision: null,
				generation: 1n,
				hostEpoch: '11111111111111111111111111111111',
				kind: TerminalFrameKind.snapshot,
				revision: 0n,
				sequence: 1n,
			},
		});
		expect(encodeTerminalFrame({ ...frame, title: '\ud800' })).toEqual(
			expect.objectContaining({ code: 'invalid-field', ok: false }),
		);
		expect(
			encodeTerminalFrame({
				...frame,
				extensions: [
					{
						data: new Uint8Array([1, 101]),
						kind: TerminalFrameExtensionKind.progress,
						required: false,
					},
				],
			}),
		).toEqual(expect.objectContaining({ code: 'invalid-field', ok: false }));
		expect(
			encodeTerminalFrame({
				...frame,
				extensions: [
					{
						data: new Uint8Array([0, 1]),
						kind: TerminalFrameExtensionKind.progress,
						required: false,
					},
				],
			}),
		).toEqual(expect.objectContaining({ code: 'invalid-field', ok: false }));

		for (const extensions of [
			[
				{
					data: new Uint8Array(24),
					kind: TerminalFrameExtensionKind.commandCompletion,
					required: false,
				},
			],
			[
				{
					data: commandCompletionExtension(0n),
					kind: TerminalFrameExtensionKind.commandCompletion,
					required: false,
				},
			],
			[
				{
					data: commandCompletionExtension(),
					kind: TerminalFrameExtensionKind.commandCompletion,
					required: false,
				},
				{
					data: commandCompletionExtension(8n),
					kind: TerminalFrameExtensionKind.commandCompletion,
					required: false,
				},
			],
		] as const) {
			expect(encodeTerminalFrame({ ...frame, extensions })).toEqual(
				expect.objectContaining({ code: 'invalid-field', ok: false }),
			);
		}

		const graphics = encodeTerminalGraphics({
			images: [
				{
					data: new Uint8Array([1, 2, 3]),
					format: TerminalGraphicsImageFormat.rgb,
					generation: 1n,
					height: 1,
					id: 1,
					width: 1,
				},
			],
			kind: TerminalGraphicsUpdateKind.snapshot,
			placements: [],
			resourceRevision: 1n,
		});
		if (!graphics.ok) throw new Error(graphics.detail);
		const withGraphics = encodeTerminalFrame({
			...frame,
			extensions: [
				{ data: graphics.value, kind: TerminalFrameExtensionKind.kittyGraphics, required: false },
			],
		});
		if (!withGraphics.ok) throw new Error(withGraphics.detail);
		const decodedGraphics = decodeTerminalFrame(withGraphics.value);
		if (!decodedGraphics.ok) throw new Error(decodedGraphics.detail);
		expect(decodedGraphics.value.graphics).toEqual(
			expect.objectContaining({ images: [expect.objectContaining({ id: 1 })] }),
		);

		const placementsOnly = encodeTerminalGraphics({
			images: [],
			kind: TerminalGraphicsUpdateKind.placements,
			placements: [],
			resourceRevision: 1n,
		});
		if (!placementsOnly.ok) throw new Error(placementsOnly.detail);
		expect(
			encodeTerminalFrame({
				...frame,
				extensions: [
					{
						data: placementsOnly.value,
						kind: TerminalFrameExtensionKind.kittyGraphics,
						required: false,
					},
				],
			}),
		).toEqual(expect.objectContaining({ code: 'invalid-field', ok: false }));
	});
});
