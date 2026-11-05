import { describe, expect, it, vi } from 'vitest';

import {
	ActiveScreen,
	CellWidth,
	CursorShape,
	decodeTerminalFrame,
	encodeTerminalFrame,
	inspectTerminalFrameHeader,
	RowPromptClass,
	SemanticContent,
	TerminalColorKind,
	TerminalColorEncoding,
	TerminalFrameKind,
	TerminalFrameSchema,
	UnderlineKind,
} from '../frame';
import { encodeTerminalGraphics, TerminalGraphicsUpdateKind } from '../terminal-graphics';

/** The wire kinds of the frame extensions these cases build, as `schemas/terminal` assigns them. */
const TerminalFrameExtensionKind = {
	commandCompletion: 4,
	kittyGraphics: 2,
	progress: 3,
} as const;

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

function oneCellFrame() {
	return {
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
		themeRevision: 2n,
		title: null,
	} as const;
}

function repeatedCellFrame(grapheme: string, cellCount: number) {
	const source = oneCellFrame();
	const row = source.rowData[0];
	const cell = row?.cells[0];
	if (row === undefined || cell === undefined) throw new Error('Frame fixture is incomplete');
	return {
		...source,
		columns: cellCount,
		rowData: [
			{
				...row,
				cells: Array.from({ length: cellCount }, () => ({ ...cell, grapheme })),
			},
		],
	};
}

it('round trips every ANSI tag without folding inverse, bold or faint into its index', () => {
	for (let index = 0; index < 16; index++) {
		const source = oneCellFrame();
		const encoded = encodeTerminalFrame({
			...source,
			styles: [
				{
					...source.styles[0],
					foreground: { kind: TerminalColorKind.palette, index },
					background: { kind: TerminalColorKind.palette, index: 15 - index },
					underline: { kind: TerminalColorKind.palette, index },
					bold: true,
					faint: true,
					inverse: true,
				},
			],
		});
		if (!encoded.ok) throw new Error(encoded.detail);
		const decoded = decodeTerminalFrame(encoded.value);
		if (!decoded.ok) throw new Error(decoded.detail);
		expect(decoded.value.styles.foregroundColors[0]).toBe(TerminalColorEncoding.palette | index);
		expect(decoded.value.styles.backgroundColors[0]).toBe(
			TerminalColorEncoding.palette | (15 - index),
		);
		expect(decoded.value.styles.underlineColors[0]).toBe(TerminalColorEncoding.palette | index);
		expect(decoded.value.styles.flags[0]).toBe(1 | 4 | 16);
	}
	for (const index of [-1, 16, 255]) {
		const source = oneCellFrame();
		expect(
			TerminalFrameSchema.safeParse({
				...source,
				styles: [{ ...source.styles[0], foreground: { kind: 'palette', index } }],
			}).success,
		).toBe(false);
	}
});

describe('packed terminal frame', () => {
	it('decodes complete rows into typed columns', () => {
		const frame = oneCellFrame();
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
				themeRevision: 2n,
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
			animationTick: 1n,
			images: [
				{
					data: new Uint8Array([1, 2, 3]),
					format: 'rgb',
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
			animationTick: 2n,
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

	it('preserves leading U+FEFF in every encoded frame string role', () => {
		const title = '\uFEFF';
		const currentDirectory = '\uFEFF/';
		const grapheme = '\uFEFFA';
		const hyperlink = '\uFEFFx';
		const source = oneCellFrame();
		const encoded = encodeTerminalFrame({
			...source,
			currentDirectory,
			rowData: [
				{
					...source.rowData[0],
					cells: [{ ...source.rowData[0].cells[0], grapheme }],
				},
			],
			styles: [{ ...source.styles[0], hyperlink }],
			title,
		});
		if (!encoded.ok) throw new Error(encoded.detail);

		const wire = new DataView(
			encoded.value.buffer,
			encoded.value.byteOffset,
			encoded.value.byteLength,
		);
		const stringCount = wire.getUint32(124, true);
		const stringBytes: number[][] = [];
		let offset = 160;
		for (let index = 0; index < stringCount; index += 1) {
			const byteLength = wire.getUint32(offset, true);
			offset += 4;
			stringBytes.push([...encoded.value.subarray(offset, offset + byteLength)]);
			offset += byteLength;
		}
		expect(stringBytes).toEqual([
			[0xef, 0xbb, 0xbf],
			[0xef, 0xbb, 0xbf, 0x2f],
			[0xef, 0xbb, 0xbf, 0x78],
			[0xef, 0xbb, 0xbf, 0x41],
		]);

		const decoded = decodeTerminalFrame(encoded.value);
		if (!decoded.ok) throw new Error(decoded.detail);
		const graphemeIndex = decoded.value.cells.graphemeStringIndices[0];
		const hyperlinkIndex = decoded.value.styles.hyperlinkStringIndices[0];
		expect(decoded.value.title).toBe(title);
		expect(decoded.value.title?.codePointAt(0)).toBe(0xfeff);
		expect(decoded.value.currentDirectory).toBe(currentDirectory);
		expect(decoded.value.currentDirectory?.codePointAt(0)).toBe(0xfeff);
		expect(graphemeIndex).toBeDefined();
		expect(decoded.value.strings[graphemeIndex ?? 0]).toBe(grapheme);
		expect(decoded.value.strings[graphemeIndex ?? 0]?.codePointAt(0)).toBe(0xfeff);
		expect(hyperlinkIndex).toBeDefined();
		expect(decoded.value.strings[hyperlinkIndex ?? 0]).toBe(hyperlink);
		expect(decoded.value.strings[hyperlinkIndex ?? 0]?.codePointAt(0)).toBe(0xfeff);
	});

	it('validates repeated string content once per TerminalFrameSchema parse', () => {
		const repeated = repeatedCellFrame('\uFEFFsame', 32);
		const encodeSpy = vi.spyOn(TextEncoder.prototype, 'encode');
		try {
			expect(TerminalFrameSchema.safeParse(repeated)).toEqual(
				expect.objectContaining({ success: true }),
			);
			expect(encodeSpy).toHaveBeenCalledTimes(1);

			expect(TerminalFrameSchema.safeParse(repeated)).toEqual(
				expect.objectContaining({ success: true }),
			);
			expect(encodeSpy).toHaveBeenCalledTimes(2);
		} finally {
			encodeSpy.mockRestore();
		}
	});

	it('preserves repeated invalid-string issue paths', () => {
		const oversized = TerminalFrameSchema.safeParse(
			repeatedCellFrame('x'.repeat(16 * 1024 + 1), 2),
		);
		expect(oversized.success).toBe(false);
		if (oversized.success) throw new Error('Oversized frame string was accepted');
		expect(oversized.error.issues).toMatchObject([
			{
				message: 'String exceeds 16384 UTF-8 bytes',
				path: ['rowData', 0, 'cells', 0, 'grapheme'],
			},
			{
				message: 'String exceeds 16384 UTF-8 bytes',
				path: ['rowData', 0, 'cells', 1, 'grapheme'],
			},
		]);

		const surrogate = TerminalFrameSchema.safeParse(repeatedCellFrame('\ud800', 2));
		expect(surrogate.success).toBe(false);
		if (surrogate.success) throw new Error('Unpaired surrogate was accepted');
		expect(surrogate.error.issues).toMatchObject([
			{
				message: 'String contains an unpaired UTF-16 surrogate',
				path: ['rowData', 0, 'cells', 0, 'grapheme'],
			},
			{
				message: 'String contains an unpaired UTF-16 surrogate',
				path: ['rowData', 0, 'cells', 1, 'grapheme'],
			},
		]);
	});

	it('keeps null and repeated empty string validation semantics distinct', () => {
		const source = repeatedCellFrame('', 4);
		const encodeSpy = vi.spyOn(TextEncoder.prototype, 'encode');
		try {
			expect(
				TerminalFrameSchema.safeParse({
					...source,
					currentDirectory: '',
					styles: [{ ...source.styles[0], hyperlink: '' }],
					title: null,
				}),
			).toEqual(expect.objectContaining({ success: true }));
			expect(encodeSpy).toHaveBeenCalledTimes(1);
		} finally {
			encodeSpy.mockRestore();
		}
	});
});
