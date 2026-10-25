import { readFile } from 'node:fs/promises';

import {
	ActiveScreen,
	CursorShape,
	decodeTerminalFrame,
	decodeTerminalRowWindow,
	type DecodedTerminalProjectionBuffer,
	RowPromptClass,
	SemanticContent,
	TerminalCellWidthCode,
	TerminalColorEncoding,
	TerminalFrameKind,
	TerminalProjectionSection,
	TerminalRowPromptClassCode,
	TerminalRowFlag,
	TerminalRowWindowDetail,
	TerminalSemanticContentCode,
	TerminalStyleFlag,
	TerminalUnderlineKindCode,
	TERMINAL_ROW_WINDOW_HEADER_BYTES,
	viewTerminalRowWindow,
} from '@orchbee/terminal-protocol';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { AccessibleTextModel, buildAccessibleRow } from '../accessibility';
import type { TerminalBufferRenderCell, TerminalBufferRow } from '../buffer-rows';
import { DEFAULT_TERMINAL_PRESENTATION_CONFIGURATION } from '../presentation-configuration';
import { PackedTerminalStore, projectedCellDisplayWidth } from '../projected';
import { type PackedRendererSource, WebGPURenderer } from '../rendering';
import { findTerminalBufferMatches } from '../search';
import { detectTerminalRowLinks } from '../terminal-row-links';

const surfaceFixturePath = new URL(
	'../../../../schemas/terminal/v1/fixtures/ghostty-surface.bin',
	import.meta.url,
);
const alternateSurfaceFixturePath = new URL(
	'../../../../schemas/terminal/v1/fixtures/ghostty-alternate-surface.bin',
	import.meta.url,
);
const renderRowWindowFixturePath = new URL(
	'../../../../schemas/terminal/v1/fixtures/ghostty-render-row-window.bin',
	import.meta.url,
);

const RENDER_ATTACHMENT_ID = '44444444444444444444444444444444';
const RENDER_HOST_EPOCH = '33333333333333333333333333333333';
const RENDER_COLUMNS = 8;
const RENDER_LINK = 'https://example.com';
const RENDER_LINK_OFFSET = 41;
const RENDER_LINK_LENGTH = 19;
const RENDER_PROJECTION_BYTE_LENGTH = 1_891;
const NO_STRING_OFFSET = 0xffff_ffff;
const PROJECTION_DIRECTORY_OFFSET = 160;

interface ExpectedRenderRow {
	readonly backgroundColors: readonly number[];
	readonly foregroundColors: readonly number[];
	readonly graphemeLengths: readonly number[];
	readonly graphemeOffsets: readonly number[];
	readonly graphemes: readonly string[];
	readonly hyperlinkLengths: readonly number[];
	readonly hyperlinkOffsets: readonly number[];
	readonly hyperlinks: readonly (string | null)[];
	readonly index: number;
	readonly promptClass: number;
	readonly protectedCells: readonly number[];
	readonly rowFlags: number;
	readonly semanticContents: readonly number[];
	readonly styleFlags: readonly number[];
	readonly underlineColors: readonly number[];
	readonly underlineKinds: readonly number[];
	readonly widths: readonly number[];
}

function filledRenderRow<Value>(value: Value): Value[] {
	return Array.from({ length: RENDER_COLUMNS }, () => value);
}

const EXPECTED_RENDER_ROWS: readonly ExpectedRenderRow[] = [
	{
		backgroundColors: filledRenderRow(0x10_14_18),
		foregroundColors: filledRenderRow(0xd8_de_e9),
		graphemeLengths: [1, 3, 0, 3, 15, 0, 1, 0],
		graphemeOffsets: [40, 60, 63, 63, 66, 81, 81, 82],
		graphemes: ['A', '中', '', 'e\u0301', '👩🏽‍💻', '', 'x', ''],
		hyperlinkLengths: filledRenderRow(RENDER_LINK_LENGTH),
		hyperlinkOffsets: filledRenderRow(RENDER_LINK_OFFSET),
		hyperlinks: filledRenderRow(RENDER_LINK),
		index: 0,
		promptClass: TerminalRowPromptClassCode.none,
		protectedCells: filledRenderRow(0),
		rowFlags: TerminalRowFlag.wrap,
		semanticContents: filledRenderRow(TerminalSemanticContentCode.output),
		styleFlags: filledRenderRow(0),
		underlineColors: filledRenderRow(TerminalColorEncoding.none),
		underlineKinds: filledRenderRow(TerminalUnderlineKindCode.none),
		widths: [
			TerminalCellWidthCode.narrow,
			TerminalCellWidthCode.wide,
			TerminalCellWidthCode.spacerTail,
			TerminalCellWidthCode.narrow,
			TerminalCellWidthCode.wide,
			TerminalCellWidthCode.spacerTail,
			TerminalCellWidthCode.narrow,
			TerminalCellWidthCode.spacerHead,
		],
	},
	{
		backgroundColors: filledRenderRow(0x10_14_18),
		foregroundColors: filledRenderRow(0xd8_de_e9),
		graphemeLengths: [3, 0, 0, 0, 0, 0, 0, 0],
		graphemeOffsets: [82, 85, 85, 85, 85, 85, 85, 85],
		graphemes: ['界', '', '', '', '', '', '', ''],
		hyperlinkLengths: [RENDER_LINK_LENGTH, RENDER_LINK_LENGTH, 0, 0, 0, 0, 0, 0],
		hyperlinkOffsets: [
			RENDER_LINK_OFFSET,
			RENDER_LINK_OFFSET,
			NO_STRING_OFFSET,
			NO_STRING_OFFSET,
			NO_STRING_OFFSET,
			NO_STRING_OFFSET,
			NO_STRING_OFFSET,
			NO_STRING_OFFSET,
		],
		hyperlinks: [RENDER_LINK, RENDER_LINK, null, null, null, null, null, null],
		index: 1,
		promptClass: TerminalRowPromptClassCode.none,
		protectedCells: filledRenderRow(0),
		rowFlags: TerminalRowFlag.wrapContinuation,
		semanticContents: filledRenderRow(TerminalSemanticContentCode.output),
		styleFlags: filledRenderRow(0),
		underlineColors: filledRenderRow(TerminalColorEncoding.none),
		underlineKinds: filledRenderRow(TerminalUnderlineKindCode.none),
		widths: [
			TerminalCellWidthCode.wide,
			TerminalCellWidthCode.spacerTail,
			TerminalCellWidthCode.narrow,
			TerminalCellWidthCode.narrow,
			TerminalCellWidthCode.narrow,
			TerminalCellWidthCode.narrow,
			TerminalCellWidthCode.narrow,
			TerminalCellWidthCode.narrow,
		],
	},
	{
		backgroundColors: filledRenderRow(0x10_14_18),
		foregroundColors: filledRenderRow(0xd8_de_e9),
		graphemeLengths: [1, 0, 0, 0, 0, 0, 0, 0],
		graphemeOffsets: [85, 86, 86, 86, 86, 86, 86, 86],
		graphemes: ['H', '', '', '', '', '', '', ''],
		hyperlinkLengths: filledRenderRow(0),
		hyperlinkOffsets: filledRenderRow(NO_STRING_OFFSET),
		hyperlinks: filledRenderRow(null),
		index: 2,
		promptClass: TerminalRowPromptClassCode.none,
		protectedCells: filledRenderRow(0),
		rowFlags: 0,
		semanticContents: filledRenderRow(TerminalSemanticContentCode.output),
		styleFlags: filledRenderRow(0),
		underlineColors: filledRenderRow(TerminalColorEncoding.none),
		underlineKinds: filledRenderRow(TerminalUnderlineKindCode.none),
		widths: filledRenderRow(TerminalCellWidthCode.narrow),
	},
	{
		backgroundColors: [
			0x10_14_18, 0, 0x10_14_18, 0x10_14_18, 0x0a_0b_0c, 0x10_14_18, 0x10_14_18, 0x10_14_18,
		],
		foregroundColors: [0x01_02_03, ...filledRenderRow(0xd8_de_e9).slice(1)],
		graphemeLengths: filledRenderRow(1),
		graphemeOffsets: [86, 87, 88, 89, 90, 91, 92, 93],
		graphemes: ['F', 'B', 'I', 'T', 'V', 'K', 'O', 'S'],
		hyperlinkLengths: filledRenderRow(0),
		hyperlinkOffsets: filledRenderRow(NO_STRING_OFFSET),
		hyperlinks: filledRenderRow(null),
		index: 3,
		promptClass: TerminalRowPromptClassCode.none,
		protectedCells: filledRenderRow(0),
		rowFlags: 0,
		semanticContents: filledRenderRow(TerminalSemanticContentCode.output),
		styleFlags: [
			0,
			0,
			TerminalStyleFlag.inverse,
			TerminalStyleFlag.faint,
			TerminalStyleFlag.invisible,
			TerminalStyleFlag.blink,
			TerminalStyleFlag.overline,
			TerminalStyleFlag.strikethrough,
		],
		underlineColors: filledRenderRow(TerminalColorEncoding.none),
		underlineKinds: filledRenderRow(TerminalUnderlineKindCode.none),
		widths: filledRenderRow(TerminalCellWidthCode.narrow),
	},
	{
		backgroundColors: filledRenderRow(0x10_14_18),
		foregroundColors: filledRenderRow(0xd8_de_e9),
		graphemeLengths: filledRenderRow(1),
		graphemeOffsets: [94, 95, 96, 97, 98, 99, 100, 101],
		graphemes: ['1', '2', '3', '4', '5', 'U', 'D', 'E'],
		hyperlinkLengths: filledRenderRow(0),
		hyperlinkOffsets: filledRenderRow(NO_STRING_OFFSET),
		hyperlinks: filledRenderRow(null),
		index: 4,
		promptClass: TerminalRowPromptClassCode.none,
		protectedCells: filledRenderRow(0),
		rowFlags: 0,
		semanticContents: filledRenderRow(TerminalSemanticContentCode.output),
		styleFlags: [
			0,
			0,
			0,
			0,
			0,
			TerminalStyleFlag.faint,
			TerminalStyleFlag.bold,
			TerminalStyleFlag.italic,
		],
		underlineColors: [
			TerminalColorEncoding.none,
			TerminalColorEncoding.none,
			TerminalColorEncoding.none,
			TerminalColorEncoding.none,
			TerminalColorEncoding.none,
			0x07_08_09,
			TerminalColorEncoding.none,
			TerminalColorEncoding.none,
		],
		underlineKinds: [
			TerminalUnderlineKindCode.single,
			TerminalUnderlineKindCode.double,
			TerminalUnderlineKindCode.curly,
			TerminalUnderlineKindCode.dotted,
			TerminalUnderlineKindCode.dashed,
			TerminalUnderlineKindCode.single,
			TerminalUnderlineKindCode.none,
			TerminalUnderlineKindCode.none,
		],
		widths: filledRenderRow(TerminalCellWidthCode.narrow),
	},
	{
		backgroundColors: filledRenderRow(0x10_14_18),
		foregroundColors: [
			0xd8_de_e9, 0xaa_bb_cc, 0xd8_de_e9, 0xd8_de_e9, 0xd8_de_e9, 0xd8_de_e9, 0xd8_de_e9,
			0xd8_de_e9,
		],
		graphemeLengths: [1, 1, 1, 1, 1, 0, 0, 0],
		graphemeOffsets: [102, 103, 104, 105, 106, 107, 107, 107],
		graphemes: ['^', 'P', '.', 'Z', 'u', '', '', ''],
		hyperlinkLengths: filledRenderRow(0),
		hyperlinkOffsets: filledRenderRow(NO_STRING_OFFSET),
		hyperlinks: filledRenderRow(null),
		index: 5,
		promptClass: TerminalRowPromptClassCode.prompt,
		protectedCells: [0, 0, 0, 1, 0, 0, 0, 0],
		rowFlags: 0,
		semanticContents: [
			TerminalSemanticContentCode.prompt,
			TerminalSemanticContentCode.input,
			TerminalSemanticContentCode.output,
			TerminalSemanticContentCode.output,
			TerminalSemanticContentCode.output,
			TerminalSemanticContentCode.output,
			TerminalSemanticContentCode.output,
			TerminalSemanticContentCode.output,
		],
		styleFlags: filledRenderRow(0),
		underlineColors: filledRenderRow(TerminalColorEncoding.none),
		underlineKinds: filledRenderRow(TerminalUnderlineKindCode.none),
		widths: filledRenderRow(TerminalCellWidthCode.narrow),
	},
];

const EXPECTED_RENDER_INTERACTION = {
	alternateScroll: true,
	applicationCursorKeys: true,
	applicationKeypad: true,
	bracketedPaste: true,
	focusReporting: true,
	kittyDisambiguateEscapeCodes: true,
	kittyReportAllKeysAsEscapeCodes: true,
	kittyReportAlternateKeys: true,
	kittyReportAssociatedText: true,
	kittyReportEventTypes: true,
	modifyOtherKeys2: true,
	mouseAny: false,
	mouseButton: true,
	mouseNormal: false,
	mouseSgr: true,
	mouseSgrPixels: false,
	mouseUrxvt: false,
	mouseUtf8: false,
	mouseX10: false,
} as const;

const projectionStringDecoder = new TextDecoder('utf-8', { fatal: true, ignoreBOM: true });

function projectionStrings(
	stringBytes: Uint8Array,
	offsets: Uint32Array,
	lengths: Uint16Array,
	nullable: boolean,
): (string | null)[] {
	return Array.from(offsets, (offset, index) => {
		const length = lengths[index];
		if (length === undefined) throw new Error('Projection string range is incomplete');
		if (offset === NO_STRING_OFFSET) {
			if (nullable && length === 0) return null;
			throw new Error('Projection grapheme range is absent');
		}
		return projectionStringDecoder.decode(stringBytes.subarray(offset, offset + length));
	});
}

function projectionMatrix(frame: DecodedTerminalProjectionBuffer): ExpectedRenderRow[] {
	return Array.from(frame.rowData.indices, (index, rowOffset) => {
		const rowFlags = frame.rowData.flags[rowOffset];
		const promptClass = frame.rowData.promptClasses[rowOffset];
		if (rowFlags === undefined || promptClass === undefined) {
			throw new Error('Projection row metadata is incomplete');
		}
		const cellOffset = rowOffset * frame.columns;
		const cellEnd = cellOffset + frame.columns;
		const graphemeOffsets = frame.cells.graphemeOffsets.subarray(cellOffset, cellEnd);
		const graphemeLengths = frame.cells.graphemeLengths.subarray(cellOffset, cellEnd);
		const hyperlinkOffsets = frame.cells.hyperlinkOffsets.subarray(cellOffset, cellEnd);
		const hyperlinkLengths = frame.cells.hyperlinkLengths.subarray(cellOffset, cellEnd);
		return {
			backgroundColors: Array.from(frame.cells.backgroundColors.subarray(cellOffset, cellEnd)),
			foregroundColors: Array.from(frame.cells.foregroundColors.subarray(cellOffset, cellEnd)),
			graphemeLengths: Array.from(graphemeLengths),
			graphemeOffsets: Array.from(graphemeOffsets),
			graphemes: projectionStrings(frame.stringBytes, graphemeOffsets, graphemeLengths, false).map(
				(value) => value ?? '',
			),
			hyperlinkLengths: Array.from(hyperlinkLengths),
			hyperlinkOffsets: Array.from(hyperlinkOffsets),
			hyperlinks: projectionStrings(frame.stringBytes, hyperlinkOffsets, hyperlinkLengths, true),
			index,
			promptClass,
			protectedCells: Array.from(frame.cells.protected.subarray(cellOffset, cellEnd)),
			rowFlags,
			semanticContents: Array.from(frame.cells.semanticContents.subarray(cellOffset, cellEnd)),
			styleFlags: Array.from(frame.cells.styleFlags.subarray(cellOffset, cellEnd)),
			underlineColors: Array.from(frame.cells.underlineColors.subarray(cellOffset, cellEnd)),
			underlineKinds: Array.from(frame.cells.underlineKinds.subarray(cellOffset, cellEnd)),
			widths: Array.from(frame.cells.widths.subarray(cellOffset, cellEnd)),
		};
	});
}

function storeMatrix(store: PackedTerminalStore) {
	return store.rows.map((row) => ({
		backgroundColors: Array.from(row.backgroundColors),
		foregroundColors: Array.from(row.foregroundColors),
		graphemes: Array.from({ length: RENDER_COLUMNS }, (_value, column) => row.grapheme(column)),
		hyperlinks: Array.from({ length: RENDER_COLUMNS }, (_value, column) => row.hyperlink(column)),
		index: row.index,
		promptClass: row.promptClass,
		protectedCells: Array.from(row.protectedCells),
		rowFlags: row.rowFlags,
		semanticContents: Array.from(row.semanticContents),
		styleFlags: Array.from(row.styleFlags),
		underlineColors: Array.from(row.underlineColors),
		underlineKinds: Array.from(row.underlineKinds),
		widths: Array.from(row.widths),
	}));
}

function expectedStoreMatrix() {
	return EXPECTED_RENDER_ROWS.map((row) => ({
		backgroundColors: row.backgroundColors,
		foregroundColors: row.foregroundColors,
		graphemes: row.graphemes,
		hyperlinks: row.hyperlinks,
		index: row.index,
		promptClass: row.promptClass,
		protectedCells: row.protectedCells,
		rowFlags: row.rowFlags,
		semanticContents: row.semanticContents,
		styleFlags: row.styleFlags,
		underlineColors: row.underlineColors,
		underlineKinds: row.underlineKinds,
		widths: row.widths,
	}));
}

function projectionSectionByteOffset(bytes: Uint8Array, section: number): number {
	const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
	return (
		TERMINAL_ROW_WINDOW_HEADER_BYTES +
		view.getUint32(
			TERMINAL_ROW_WINDOW_HEADER_BYTES + PROJECTION_DIRECTORY_OFFSET + section * 8,
			true,
		)
	);
}

function renderCellIndex(row: number, column: number): number {
	return row * RENDER_COLUMNS + column;
}

interface RenderMutation {
	readonly assertDifference: (rows: readonly ExpectedRenderRow[]) => void;
	readonly mutate: (bytes: Uint8Array) => void;
	readonly name: string;
}

const RENDER_MUTATIONS: readonly RenderMutation[] = [
	{
		assertDifference: (rows) => {
			expect(rows[3]?.foregroundColors[0]).toBe(0x10_14_18);
			expect(rows[3]?.backgroundColors[0]).toBe(0x01_02_03);
		},
		mutate: (bytes) => {
			const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
			const cell = renderCellIndex(3, 0);
			const foreground = projectionSectionByteOffset(
				bytes,
				TerminalProjectionSection.foregroundColors,
			);
			const background = projectionSectionByteOffset(
				bytes,
				TerminalProjectionSection.backgroundColors,
			);
			const foregroundValue = view.getUint32(foreground + cell * 4, true);
			const backgroundValue = view.getUint32(background + cell * 4, true);
			view.setUint32(foreground + cell * 4, backgroundValue, true);
			view.setUint32(background + cell * 4, foregroundValue, true);
		},
		name: 'foreground/background swap',
	},
	{
		assertDifference: (rows) => {
			expect(rows[4]?.styleFlags[6]).toBe(TerminalStyleFlag.italic);
		},
		mutate: (bytes) => {
			const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
			const offset = projectionSectionByteOffset(bytes, TerminalProjectionSection.styleFlags);
			view.setUint16(offset + renderCellIndex(4, 6) * 2, TerminalStyleFlag.italic, true);
		},
		name: 'different supported style flag',
	},
	{
		assertDifference: (rows) => {
			expect(rows[5]?.semanticContents[0]).toBe(TerminalSemanticContentCode.input);
		},
		mutate: (bytes) => {
			const offset = projectionSectionByteOffset(bytes, TerminalProjectionSection.semanticContents);
			bytes[offset + renderCellIndex(5, 0)] = TerminalSemanticContentCode.input;
		},
		name: 'different supported semantic value',
	},
	{
		assertDifference: (rows) => {
			expect(rows[5]?.protectedCells[3]).toBe(0);
		},
		mutate: (bytes) => {
			const offset = projectionSectionByteOffset(bytes, TerminalProjectionSection.protectedCells);
			bytes[offset + renderCellIndex(5, 3)] = 0;
		},
		name: 'cleared protected sentinel',
	},
	{
		assertDifference: (rows) => {
			expect(rows[0]?.hyperlinkOffsets[0]).toBe(NO_STRING_OFFSET);
			expect(rows[0]?.hyperlinkLengths[0]).toBe(0);
			expect(rows[0]?.hyperlinks[0]).toBeNull();
		},
		mutate: (bytes) => {
			const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
			const offsets = projectionSectionByteOffset(
				bytes,
				TerminalProjectionSection.hyperlinkOffsets,
			);
			const lengths = projectionSectionByteOffset(
				bytes,
				TerminalProjectionSection.hyperlinkLengths,
			);
			view.setUint32(offsets, NO_STRING_OFFSET, true);
			view.setUint16(lengths, 0, true);
		},
		name: 'canonically removed hyperlink',
	},
	{
		assertDifference: (rows) => {
			expect(rows[2]?.rowFlags).toBe(TerminalRowFlag.wrap);
			expect(rows[3]?.rowFlags).toBe(TerminalRowFlag.wrapContinuation);
		},
		mutate: (bytes) => {
			const offset = projectionSectionByteOffset(bytes, TerminalProjectionSection.rowFlags);
			bytes[offset + 2] = TerminalRowFlag.wrap;
			bytes[offset + 3] = TerminalRowFlag.wrapContinuation;
		},
		name: 'valid adjacent row-wrap change',
	},
	{
		assertDifference: (rows) => {
			expect(rows[0]?.widths.slice(1, 3)).toEqual([
				TerminalCellWidthCode.narrow,
				TerminalCellWidthCode.narrow,
			]);
		},
		mutate: (bytes) => {
			const offset = projectionSectionByteOffset(bytes, TerminalProjectionSection.widths);
			bytes[offset + renderCellIndex(0, 1)] = TerminalCellWidthCode.narrow;
			bytes[offset + renderCellIndex(0, 2)] = TerminalCellWidthCode.narrow;
		},
		name: 'valid width-topology change',
	},
];

afterEach(() => {
	vi.useRealTimers();
	vi.unstubAllGlobals();
});

function createCanvas(
	fillText: CanvasRenderingContext2D['fillText'] = () => {},
): HTMLCanvasElement {
	const context = {
		beginPath: () => {},
		clearRect: () => {},
		clip: () => {},
		drawImage: () => {},
		fillRect: () => {},
		fillText,
		font: '10px sans-serif',
		getImageData: (_x: number, _y: number, width: number, height: number) =>
			({ data: new Uint8ClampedArray(width * height * 4) }) as ImageData,
		globalAlpha: 1,
		imageSmoothingEnabled: true,
		lineTo: () => {},
		measureText: () =>
			({
				actualBoundingBoxAscent: 9.5,
				actualBoundingBoxDescent: 0,
				fontBoundingBoxAscent: 13,
				fontBoundingBoxDescent: 4,
				width: 7.8,
			}) as TextMetrics,
		moveTo: () => {},
		rect: () => {},
		restore: () => {},
		save: () => {},
		scale: () => {},
		setLineDash: () => {},
		setTransform: () => {},
		stroke: () => {},
		translate: () => {},
	};
	return {
		getContext: () => context,
		height: 0,
		get ownerDocument() {
			return globalThis.document;
		},
		style: {},
		width: 0,
	} as unknown as HTMLCanvasElement;
}

interface CanvasStyleTextPaint {
	readonly alpha: number;
	readonly color: string;
	readonly font: string;
	readonly text: string;
}

interface CanvasStyleRectangle {
	readonly alpha: number;
	readonly color: string;
	readonly height: number;
	readonly width: number;
	readonly x: number;
	readonly y: number;
}

interface CanvasStyleStroke {
	readonly alpha: number;
	readonly color: string;
	readonly dash: readonly number[];
	readonly path: readonly {
		readonly kind: 'line' | 'move';
		readonly x: number;
		readonly y: number;
	}[];
}

function createStyleRecordingCanvas(): {
	readonly canvas: HTMLCanvasElement;
	readonly rectangles: CanvasStyleRectangle[];
	readonly strokes: CanvasStyleStroke[];
	readonly texts: CanvasStyleTextPaint[];
} {
	const rectangles: CanvasStyleRectangle[] = [];
	const strokes: CanvasStyleStroke[] = [];
	const texts: CanvasStyleTextPaint[] = [];
	let dash: readonly number[] = [];
	let path: CanvasStyleStroke['path'] = [];
	const context = {
		beginPath: () => {
			path = [];
		},
		clearRect: () => {},
		clip: () => {},
		drawImage: () => {},
		fillRect: function (
			this: CanvasRenderingContext2D,
			x: number,
			y: number,
			width: number,
			height: number,
		) {
			rectangles.push({
				alpha: this.globalAlpha,
				color: String(this.fillStyle),
				height,
				width,
				x,
				y,
			});
		},
		fillStyle: '#000000',
		fillText: function (this: CanvasRenderingContext2D, text: string) {
			texts.push({
				alpha: this.globalAlpha,
				color: String(this.fillStyle),
				font: this.font,
				text,
			});
		},
		font: '10px sans-serif',
		getImageData: (_x: number, _y: number, width: number, height: number) =>
			({ data: new Uint8ClampedArray(width * height * 4) }) as ImageData,
		globalAlpha: 1,
		imageSmoothingEnabled: true,
		lineTo: (x: number, y: number) => {
			path = [...path, { kind: 'line', x, y }];
		},
		lineWidth: 1,
		measureText: () =>
			({
				actualBoundingBoxAscent: 9.5,
				actualBoundingBoxDescent: 0,
				fontBoundingBoxAscent: 13,
				fontBoundingBoxDescent: 4,
				width: 7.8,
			}) as TextMetrics,
		moveTo: (x: number, y: number) => {
			path = [...path, { kind: 'move', x, y }];
		},
		rect: () => {},
		restore: () => {},
		save: () => {},
		scale: () => {},
		setLineDash: (nextDash: number[]) => {
			dash = [...nextDash];
		},
		setTransform: () => {},
		stroke: function (this: CanvasRenderingContext2D) {
			strokes.push({
				alpha: this.globalAlpha,
				color: String(this.strokeStyle),
				dash,
				path,
			});
		},
		strokeStyle: '#000000',
		translate: () => {},
	};
	return {
		canvas: {
			getContext: () => context,
			height: 0,
			get ownerDocument() {
				return globalThis.document;
			},
			style: {},
			width: 0,
		} as unknown as HTMLCanvasElement,
		rectangles,
		strokes,
		texts,
	};
}

function createWebGPUCanvas(context: GPUCanvasContext): HTMLCanvasElement {
	const canvas = createCanvas();
	const getContext = canvas.getContext.bind(canvas);
	return {
		...canvas,
		getContext: ((kind: string, options?: unknown) =>
			kind === 'webgpu' ? context : getContext(kind, options)) as HTMLCanvasElement['getContext'],
	} as HTMLCanvasElement;
}

function createSubmittingDevice(): {
	readonly device: GPUDevice;
	readonly writeBuffer: ReturnType<typeof vi.fn<GPUQueue['writeBuffer']>>;
} {
	const writeBuffer = vi.fn<GPUQueue['writeBuffer']>();
	const renderPass = {
		draw: () => {},
		end: () => {},
		setBindGroup: () => {},
		setPipeline: () => {},
		setVertexBuffer: () => {},
	};
	const device = {
		createBindGroup: () => ({}),
		createBuffer: () => ({ destroy: () => {} }),
		createCommandEncoder: () => ({
			beginRenderPass: () => renderPass,
			finish: () => ({}),
		}),
		createRenderPipeline: () => ({ getBindGroupLayout: () => ({}) }),
		createSampler: () => ({}),
		createShaderModule: () => ({}),
		createTexture: () => ({ createView: () => ({}), destroy: () => {} }),
		destroy: () => {},
		limits: { maxTextureDimension2D: 8_192 },
		lost: new Promise<GPUDeviceLostInfo>(() => undefined),
		popErrorScope: async () => null,
		pushErrorScope: () => {},
		queue: {
			copyExternalImageToTexture: () => {},
			submit: () => {},
			writeBuffer,
			writeTexture: () => {},
		},
	} as unknown as GPUDevice;
	return { device, writeBuffer };
}

function floatUploads(
	writeBuffer: ReturnType<typeof vi.fn<GPUQueue['writeBuffer']>>,
): readonly Float32Array[] {
	return writeBuffer.mock.calls.flatMap((call) => {
		const value = call[2];
		return value instanceof Float32Array ? [value] : [];
	});
}

async function loadSurfaceStore(): Promise<PackedTerminalStore> {
	const decoded = decodeTerminalFrame(await readFile(surfaceFixturePath));
	expect(decoded.ok).toBe(true);
	if (!decoded.ok) throw new Error(decoded.detail);
	const store = new PackedTerminalStore(decoded.value.hostEpoch, decoded.value.attachmentId);
	expect(store.replaceDecodedSnapshot(decoded.value, 1024 * 1024)).toMatchObject({
		kind: 'committed',
	});
	return store;
}

function surfaceSourceWithVisibleCursor(store: PackedTerminalStore): PackedRendererSource {
	const metadata = store.metadata;
	if (metadata === null) throw new Error('Expected surface fixture metadata');
	return {
		markRendered: () => store.markRendered(),
		metadata: {
			...metadata,
			cursor: {
				...metadata.cursor,
				blinking: false,
				column: 1,
				row: 5,
				shape: CursorShape.block,
				visible: true,
			},
		},
		needsFullRedraw: true,
		rows: store.rows,
	};
}

function projectedSemanticContent(value: number): TerminalBufferRenderCell['semanticContent'] {
	if (value === TerminalSemanticContentCode.prompt) return SemanticContent.prompt;
	if (value === TerminalSemanticContentCode.input) return SemanticContent.input;
	return SemanticContent.output;
}

function projectedPromptClass(value: number): TerminalBufferRow['promptClass'] {
	if (value === TerminalRowPromptClassCode.prompt) return RowPromptClass.prompt;
	if (value === TerminalRowPromptClassCode.promptContinuation) {
		return RowPromptClass.promptContinuation;
	}
	return RowPromptClass.none;
}

function bufferRows(store: PackedTerminalStore): readonly TerminalBufferRow[] {
	const metadata = store.metadata;
	if (metadata === null) throw new Error('Expected surface fixture metadata');
	const encoder = new TextEncoder();
	return store.rows.map((row, rowIndex) => {
		const cells: TerminalBufferRenderCell[] = Array.from(
			{ length: metadata.columns },
			(_value, column) => ({
				column,
				grapheme: row.grapheme(column),
				hyperlink: row.hyperlink(column),
				semanticContent: projectedSemanticContent(row.semanticContents[column] ?? 0),
				width: projectedCellDisplayWidth(row.widths[column] ?? TerminalCellWidthCode.narrow),
			}),
		);
		let lastContentColumn = -1;
		for (let column = cells.length - 1; column >= 0; column -= 1) {
			const cell = cells[column];
			if (
				cell !== undefined &&
				cell.width > 0 &&
				(cell.grapheme !== '' || cell.hyperlink !== null)
			) {
				lastContentColumn = column;
				break;
			}
		}
		let byteOffset = 0;
		let text = '';
		let utf16Offset = 0;
		const mappings: TerminalBufferRow['mappings'][number][] = [];
		for (const cell of cells) {
			if (cell.column > lastContentColumn || cell.width === 0) continue;
			const grapheme = cell.grapheme || ' ';
			mappings.push({
				byteOffset,
				column: cell.column,
				utf16Offset,
				width: cell.width,
			});
			text += grapheme;
			byteOffset += encoder.encode(grapheme).byteLength;
			utf16Offset += grapheme.length;
		}
		return {
			cells,
			index: BigInt(rowIndex),
			mappings,
			promptClass: projectedPromptClass(row.promptClass),
			text,
			wrap: (row.rowFlags & TerminalRowFlag.wrap) !== 0,
			wrapContinuation: (row.rowFlags & TerminalRowFlag.wrapContinuation) !== 0,
		};
	});
}

describe('Ghostty surface parity', () => {
	it('preserves every native Ghostty OBRW projection column through strict, trusted, and store consumers', async () => {
		const fixtureBytes = await readFile(renderRowWindowFixturePath);
		const strict = decodeTerminalRowWindow(fixtureBytes);
		const trusted = viewTerminalRowWindow(fixtureBytes);
		expect(strict.ok).toBe(true);
		expect(trusted.ok).toBe(true);
		if (!strict.ok) throw new Error(strict.detail);
		if (!trusted.ok) throw new Error(trusted.detail);
		expect(strict.value).toMatchObject({
			attachmentId: RENDER_ATTACHMENT_ID,
			columns: RENDER_COLUMNS,
			detail: TerminalRowWindowDetail.render,
			generation: 13n,
			hostEpoch: RENDER_HOST_EPOCH,
			ignoredOptionalFlags: 0,
			revision: 11n,
			sequence: 7n,
			startRow: 0n,
			totalRows: 6n,
		});
		expect(trusted.value).toMatchObject({
			attachmentId: RENDER_ATTACHMENT_ID,
			columns: RENDER_COLUMNS,
			detail: TerminalRowWindowDetail.render,
			generation: 13n,
			hostEpoch: RENDER_HOST_EPOCH,
			ignoredOptionalFlags: 0,
			revision: 11n,
			sequence: 7n,
			startRow: 0n,
			totalRows: 6n,
		});
		if (strict.value.detail !== TerminalRowWindowDetail.render) {
			throw new Error('Expected strict render row window');
		}
		if (trusted.value.detail !== TerminalRowWindowDetail.render) {
			throw new Error('Expected trusted render row window');
		}
		const expectedFrameMetadata = {
			activeScreen: ActiveScreen.primary,
			attachmentId: RENDER_ATTACHMENT_ID,
			baseRevision: null,
			cellPixelHeight: 16,
			cellPixelWidth: 8,
			columns: RENDER_COLUMNS,
			currentDirectory: 'file:///workspace/primary',
			cursor: {
				blinking: false,
				column: 0,
				row: 0,
				shape: CursorShape.block,
				visible: false,
				wideTail: false,
			},
			defaultColors: {
				backgroundRgb: 0x10_14_18,
				cursorRgb: 0xf5_a9_7f,
				foregroundRgb: 0xd8_de_e9,
			},
			generation: 13n,
			hostEpoch: RENDER_HOST_EPOCH,
			ignoredOptionalFlags: 0,
			interaction: EXPECTED_RENDER_INTERACTION,
			kind: TerminalFrameKind.snapshot,
			revision: 11n,
			rows: 6,
			scrollbackRows: 0n,
			sequence: 7n,
			themeRevision: 17n,
			title: 'Orchbee primary',
		};
		expect(strict.value.frame).toMatchObject(expectedFrameMetadata);
		expect(trusted.value.frame).toMatchObject(expectedFrameMetadata);

		const strictMatrix = projectionMatrix(strict.value.frame);
		const trustedMatrix = projectionMatrix(trusted.value.frame);
		expect(strictMatrix).toEqual(EXPECTED_RENDER_ROWS);
		expect(trustedMatrix).toEqual(EXPECTED_RENDER_ROWS);
		expect(trustedMatrix).toEqual(strictMatrix);

		const store = new PackedTerminalStore(
			strict.value.frame.hostEpoch,
			strict.value.frame.attachmentId,
		);
		expect(
			store.replaceProjectionBufferSnapshot(trusted.value.frame, RENDER_PROJECTION_BYTE_LENGTH),
		).toEqual({
			acknowledgement: {
				attachmentId: RENDER_ATTACHMENT_ID,
				generation: '13',
				revision: '11',
				sequence: '7',
			},
			byteLength: RENDER_PROJECTION_BYTE_LENGTH,
			kind: 'committed',
		});
		expect(store.byteLength).toBe(RENDER_PROJECTION_BYTE_LENGTH);
		expect(store.metadata).toEqual({
			activeScreen: ActiveScreen.primary,
			cellPixelHeight: 16,
			cellPixelWidth: 8,
			columns: RENDER_COLUMNS,
			commandCompletion: null,
			closeActivity: 'unknown',
			currentDirectory: 'file:///workspace/primary',
			cursor: expectedFrameMetadata.cursor,
			defaultColors: expectedFrameMetadata.defaultColors,
			generation: 13n,
			interaction: EXPECTED_RENDER_INTERACTION,
			progress: { state: 'remove', value: null },
			revision: 11n,
			rows: 6,
			scrollbackRows: 0n,
			themeRevision: 17n,
			title: 'Orchbee primary',
		});
		expect(storeMatrix(store)).toEqual(expectedStoreMatrix());
	});

	it.each(RENDER_MUTATIONS)(
		'keeps $name wire-valid while the exact native semantic oracle detects it',
		async ({ assertDifference, mutate }) => {
			const mutatedBytes = new Uint8Array(await readFile(renderRowWindowFixturePath));
			mutate(mutatedBytes);
			const decoded = decodeTerminalRowWindow(mutatedBytes);
			expect(decoded.ok).toBe(true);
			if (!decoded.ok) throw new Error(decoded.detail);
			if (decoded.value.detail !== TerminalRowWindowDetail.render) {
				throw new Error('Expected mutated render row window');
			}
			const mutatedMatrix = projectionMatrix(decoded.value.frame);
			assertDifference(mutatedMatrix);
			expect(mutatedMatrix).not.toEqual(EXPECTED_RENDER_ROWS);
		},
	);

	it('preserves native graphemes and wide-cell topology through the packed Canvas surface', async () => {
		const store = await loadSurfaceStore();
		expect(store.metadata).toMatchObject({ columns: 8, rows: 6 });
		expect(store.rows.map((row) => row.rowFlags)).toEqual([1, 2, 0, 0, 0, 0]);
		expect(store.rows[0]?.grapheme(0)).toBe('A');
		expect(store.rows[0]?.grapheme(1)).toBe('中');
		expect(store.rows[0]?.grapheme(3)).toBe('e\u0301');
		expect(store.rows[0]?.grapheme(4)).toBe('👩🏽‍💻');
		expect(store.rows[0]?.grapheme(6)).toBe('x');
		expect(store.rows[1]?.grapheme(0)).toBe('界');
		expect(Array.from(store.rows[0]?.widths ?? [])).toEqual([
			TerminalCellWidthCode.narrow,
			TerminalCellWidthCode.wide,
			TerminalCellWidthCode.spacerTail,
			TerminalCellWidthCode.narrow,
			TerminalCellWidthCode.wide,
			TerminalCellWidthCode.spacerTail,
			TerminalCellWidthCode.narrow,
			TerminalCellWidthCode.spacerHead,
		]);
		expect(Array.from(store.rows[1]?.widths.slice(0, 2) ?? [])).toEqual([
			TerminalCellWidthCode.wide,
			TerminalCellWidthCode.spacerTail,
		]);

		vi.stubGlobal('window', { devicePixelRatio: 1 });
		vi.stubGlobal('navigator', {});
		const fillText = vi.fn<CanvasRenderingContext2D['fillText']>();
		vi.stubGlobal('document', { createElement: () => createCanvas(fillText) });
		const renderer = new WebGPURenderer(createCanvas(fillText));
		await renderer.initGPU();
		renderer.commitPresentationConfiguration(
			renderer.preparePresentationConfiguration({
				...DEFAULT_TERMINAL_PRESENTATION_CONFIGURATION,
				fontLigatures: false,
			}),
		);

		renderer.renderPacked(store);

		const painted = fillText.mock.calls.map(([text]) => text);
		for (const grapheme of [
			'A',
			'x',
			'界',
			'H',
			'F',
			'B',
			'I',
			'T',
			'K',
			'O',
			'S',
			'1',
			'2',
			'3',
			'4',
			'5',
			'U',
			'D',
			'E',
			'^',
			'P',
			'.',
		]) {
			expect(painted.filter((text) => text === grapheme)).toHaveLength(1);
		}
		expect(painted.filter((text) => text === '中')).toHaveLength(1);
		expect(painted.filter((text) => text === 'e\u0301')).toHaveLength(1);
		expect(painted).not.toContain('中e\u0301');
		expect(painted.filter((text) => text === '👩🏽‍💻')).toHaveLength(1);
		expect(painted).not.toContain('V');
		expect(renderer.getLastFrameTelemetry()).toMatchObject({
			glyphInstances: 25,
			shapingRuns: 0,
		});
		renderer.dispose();
	});

	it('preserves every Ghostty color, visibility, and decoration style through Host projection', async () => {
		const store = await loadSurfaceStore();
		const flags = store.rows[3]?.styleFlags;
		const decorations = store.rows[4];
		if (flags === undefined || decorations === undefined) {
			throw new Error('Expected Ghostty style corpus rows');
		}

		expect(store.metadata?.defaultColors).toEqual({
			backgroundRgb: 0x10_14_18,
			cursorRgb: 0xf5_a9_7f,
			foregroundRgb: 0xd8_de_e9,
		});
		expect(store.rows[3]?.foregroundColors[0]).toBe(0x01_02_03);
		expect(store.rows[3]?.backgroundColors[1]).toBe(0);
		expect(store.rows[3]?.backgroundColors[4]).toBe(0x0a_0b_0c);
		expect(Array.from(flags)).toEqual([
			0,
			0,
			TerminalStyleFlag.inverse,
			TerminalStyleFlag.faint,
			TerminalStyleFlag.invisible,
			TerminalStyleFlag.blink,
			TerminalStyleFlag.overline,
			TerminalStyleFlag.strikethrough,
		]);
		expect(Array.from(decorations.underlineKinds)).toEqual([
			TerminalUnderlineKindCode.single,
			TerminalUnderlineKindCode.double,
			TerminalUnderlineKindCode.curly,
			TerminalUnderlineKindCode.dotted,
			TerminalUnderlineKindCode.dashed,
			TerminalUnderlineKindCode.single,
			TerminalUnderlineKindCode.none,
			TerminalUnderlineKindCode.none,
		]);
		expect(Array.from(decorations.underlineColors)).toEqual([
			TerminalColorEncoding.none,
			TerminalColorEncoding.none,
			TerminalColorEncoding.none,
			TerminalColorEncoding.none,
			TerminalColorEncoding.none,
			0x07_08_09,
			TerminalColorEncoding.none,
			TerminalColorEncoding.none,
		]);
		expect(Array.from(decorations.styleFlags)).toEqual([
			0,
			0,
			0,
			0,
			0,
			TerminalStyleFlag.faint,
			TerminalStyleFlag.bold,
			TerminalStyleFlag.italic,
		]);
	});

	it('preserves OSC default, palette, and cursor colors as one Host-owned presentation state', async () => {
		const store = await loadSurfaceStore();
		expect(store.metadata?.defaultColors).toEqual({
			backgroundRgb: 0x10_14_18,
			cursorRgb: 0xf5_a9_7f,
			foregroundRgb: 0xd8_de_e9,
		});
		expect(store.rows[0]?.foregroundColors[0]).toBe(0xd8_de_e9);
		expect(store.rows[0]?.backgroundColors[0]).toBe(0x10_14_18);
		expect(store.rows[2]?.backgroundColors[7]).toBe(0x10_14_18);
		expect(store.rows[5]?.grapheme(1)).toBe('P');
		expect(store.rows[5]?.foregroundColors[1]).toBe(0xaa_bb_cc);
	});

	it('preserves primary-screen metadata and interaction modes from the Host frame', async () => {
		const store = await loadSurfaceStore();
		expect(store.metadata).toMatchObject({
			activeScreen: ActiveScreen.primary,
			currentDirectory: 'file:///workspace/primary',
			cursor: {
				blinking: false,
				shape: CursorShape.underline,
				visible: false,
			},
			interaction: {
				alternateScroll: true,
				applicationCursorKeys: true,
				applicationKeypad: true,
				bracketedPaste: true,
				focusReporting: true,
				kittyDisambiguateEscapeCodes: true,
				kittyReportAllKeysAsEscapeCodes: true,
				kittyReportAlternateKeys: true,
				kittyReportAssociatedText: true,
				kittyReportEventTypes: true,
				modifyOtherKeys2: true,
				mouseAny: false,
				mouseButton: true,
				mouseNormal: false,
				mouseSgr: true,
				mouseSgrPixels: false,
				mouseUtf8: false,
				mouseUrxvt: false,
				mouseX10: false,
			},
			title: 'Orchbee primary',
		});
	});

	it('atomically replaces metadata and rows when pinned Ghostty enters the alternate screen', async () => {
		const [primaryBytes, alternateBytes] = await Promise.all([
			readFile(surfaceFixturePath),
			readFile(alternateSurfaceFixturePath),
		]);
		const primaryFrame = decodeTerminalFrame(primaryBytes);
		expect(primaryFrame.ok).toBe(true);
		if (!primaryFrame.ok) throw new Error(primaryFrame.detail);
		const store = new PackedTerminalStore(
			primaryFrame.value.hostEpoch,
			primaryFrame.value.attachmentId,
		);

		expect(store.apply(primaryBytes)).toMatchObject({ kind: 'committed' });
		const primaryMetadata = store.metadata;
		expect(primaryMetadata).toMatchObject({
			activeScreen: ActiveScreen.primary,
			currentDirectory: 'file:///workspace/primary',
			generation: 1n,
			revision: 1n,
			title: 'Orchbee primary',
		});
		expect(store.rows[0]?.grapheme(1)).toBe('中');

		expect(store.apply(alternateBytes)).toMatchObject({ kind: 'committed' });
		expect(primaryMetadata).toMatchObject({
			activeScreen: ActiveScreen.primary,
			currentDirectory: 'file:///workspace/primary',
			title: 'Orchbee primary',
		});
		expect(store.metadata).not.toBe(primaryMetadata);
		expect(store.metadata).toMatchObject({
			activeScreen: ActiveScreen.alternate,
			currentDirectory: 'file:///workspace/alternate',
			cursor: {
				blinking: true,
				column: 3,
				row: 0,
				shape: CursorShape.bar,
				visible: true,
			},
			generation: 2n,
			interaction: {
				alternateScroll: false,
				applicationCursorKeys: false,
				applicationKeypad: false,
				bracketedPaste: false,
				focusReporting: false,
				kittyDisambiguateEscapeCodes: true,
				kittyReportAllKeysAsEscapeCodes: false,
				kittyReportAlternateKeys: false,
				kittyReportAssociatedText: false,
				kittyReportEventTypes: true,
				modifyOtherKeys2: false,
				mouseAny: true,
				mouseButton: false,
				mouseNormal: false,
				mouseSgr: false,
				mouseSgrPixels: true,
				mouseUtf8: false,
				mouseUrxvt: false,
				mouseX10: false,
			},
			revision: 2n,
			title: 'Orchbee alternate',
		});
		expect([0, 1, 2].map((column) => store.rows[0]?.grapheme(column))).toEqual(['A', 'L', 'T']);
	});

	it('preserves OSC 8 links and OSC 133 semantic boundaries from the same Host frame', async () => {
		const store = await loadSurfaceStore();
		const semanticRow = store.rows[5];
		if (semanticRow === undefined) throw new Error('Expected semantic fixture row');

		expect(semanticRow.promptClass).toBe(TerminalRowPromptClassCode.prompt);
		expect([semanticRow.grapheme(0), semanticRow.grapheme(1), semanticRow.grapheme(2)]).toEqual([
			'^',
			'P',
			'.',
		]);
		expect(Array.from(semanticRow.semanticContents.slice(0, 3))).toEqual([
			TerminalSemanticContentCode.prompt,
			TerminalSemanticContentCode.input,
			TerminalSemanticContentCode.output,
		]);

		const rows = bufferRows(store);
		expect(rows[5]?.promptClass).toBe(RowPromptClass.prompt);
		expect(rows[5]?.cells?.slice(0, 3)).toMatchObject([
			expect.objectContaining({ grapheme: '^', semanticContent: SemanticContent.prompt }),
			expect.objectContaining({ grapheme: 'P', semanticContent: SemanticContent.input }),
			expect.objectContaining({ grapheme: '.', semanticContent: SemanticContent.output }),
		]);
		expect(detectTerminalRowLinks(rows.slice(0, 3))).toEqual([
			expect.objectContaining({
				range: { end: { x: 0, y: 1 }, start: { x: 0, y: 0 } },
				target: { kind: 'url', url: 'https://example.com' },
			}),
		]);
	});

	it('renders every Host-derived style independently in the Canvas fallback', async () => {
		vi.useFakeTimers();
		vi.stubGlobal('window', { devicePixelRatio: 1 });
		vi.stubGlobal('navigator', {});
		vi.stubGlobal('document', { createElement: () => createCanvas() });
		const recording = createStyleRecordingCanvas();
		const renderer = new WebGPURenderer(recording.canvas);
		await renderer.initGPU();
		renderer.commitPresentationConfiguration(
			renderer.preparePresentationConfiguration({
				...DEFAULT_TERMINAL_PRESENTATION_CONFIGURATION,
				fontLigatures: false,
			}),
		);
		const store = await loadSurfaceStore();

		renderer.renderPacked(store);

		expect(recording.texts.find(({ text }) => text === 'F')).toMatchObject({
			alpha: 1,
			color: 'rgb(1, 2, 3)',
		});
		expect(recording.rectangles).toContainEqual({
			alpha: 1,
			color: 'rgb(0, 0, 0)',
			height: 17,
			width: 8,
			x: 8,
			y: 51,
		});
		expect(recording.texts.find(({ text }) => text === 'I')).toMatchObject({
			alpha: 1,
			color: 'rgb(16, 20, 24)',
		});
		expect(recording.rectangles).toContainEqual({
			alpha: 1,
			color: 'rgb(216, 222, 233)',
			height: 17,
			width: 8,
			x: 16,
			y: 51,
		});
		expect(recording.texts.find(({ text }) => text === 'T')).toMatchObject({ alpha: 0.5 });
		expect(recording.texts.some(({ text }) => text === 'V')).toBe(false);
		expect(recording.rectangles).toContainEqual({
			alpha: 1,
			color: 'rgb(10, 11, 12)',
			height: 17,
			width: 8,
			x: 32,
			y: 51,
		});
		expect(recording.texts.find(({ text }) => text === 'K')).toMatchObject({ alpha: 1 });
		expect(recording.texts.find(({ text }) => text === 'D')?.font).toContain('700');
		expect(recording.texts.find(({ text }) => text === 'E')?.font).toMatch(/^italic /);

		expect(recording.rectangles).toContainEqual(
			expect.objectContaining({ color: 'rgb(216, 222, 233)', height: 1, width: 8, x: 48, y: 51 }),
		);
		expect(recording.rectangles).toContainEqual(
			expect.objectContaining({ color: 'rgb(216, 222, 233)', height: 1, width: 8, x: 56 }),
		);
		const underlines = recording.strokes.filter(({ path }) => (path[0]?.y ?? -1) >= 68);
		expect(underlines).toHaveLength(6);
		expect(underlines.find(({ path }) => path[0]?.x === 8)?.path).toHaveLength(4);
		expect(underlines.find(({ path }) => path[0]?.x === 16)?.path.length).toBeGreaterThan(4);
		expect(underlines.find(({ path }) => path[0]?.x === 24)?.dash).toEqual([1, 2]);
		expect(underlines.find(({ path }) => path[0]?.x === 32)?.dash).toEqual([3, 2]);
		expect(underlines.find(({ path }) => path[0]?.x === 40)).toMatchObject({
			alpha: 0.5,
			color: 'rgb(7, 8, 9)',
		});

		recording.rectangles.length = 0;
		renderer.commitPresentationConfiguration(
			renderer.preparePresentationConfiguration({
				...DEFAULT_TERMINAL_PRESENTATION_CONFIGURATION,
				allowTransparency: true,
				fontLigatures: false,
			}),
		);
		renderer.renderPacked(store);
		expect(recording.rectangles).toContainEqual({
			alpha: 1,
			color: 'rgb(0, 0, 0)',
			height: 17,
			width: 8,
			x: 8,
			y: 51,
		});
		expect(
			recording.rectangles.some(
				({ color, height, width }) => color === 'rgb(16, 20, 24)' && height === 17 && width === 64,
			),
		).toBe(false);
		// OSC backgrounds remain explicit on blank cells after transparent clear.
		expect(recording.rectangles).toContainEqual({
			alpha: 1,
			color: 'rgb(16, 20, 24)',
			height: 17,
			width: 8,
			x: 56,
			y: 34,
		});
		expect(renderer.getLastFrameTelemetry()).toMatchObject({
			forceAll: true,
			reasons: expect.arrayContaining(['surface']),
		});

		recording.texts.length = 0;
		vi.advanceTimersByTime(530);
		renderer.renderPacked(store);
		expect(recording.texts.some(({ text }) => text === 'K' || text === 'V')).toBe(false);
		expect(renderer.getLastFrameTelemetry()).toMatchObject({
			glyphInstances: 24,
			shapingRuns: 0,
		});

		recording.rectangles.length = 0;
		renderer.setFocused(true);
		renderer.renderPacked(surfaceSourceWithVisibleCursor(store));
		expect(recording.rectangles).toContainEqual({
			alpha: 1,
			color: 'rgb(245, 169, 127)',
			height: 17,
			width: 8,
			x: 8,
			y: 85,
		});
		renderer.dispose();
	});

	it('submits the same Host-derived styles through WebGPU', async () => {
		vi.useFakeTimers();
		vi.stubGlobal('window', { devicePixelRatio: 1 });
		vi.stubGlobal('GPUTextureUsage', { COPY_DST: 2, RENDER_ATTACHMENT: 4, TEXTURE_BINDING: 1 });
		vi.stubGlobal('GPUBufferUsage', { COPY_DST: 2, UNIFORM: 1, VERTEX: 4 });
		const rendering = createSubmittingDevice();
		vi.stubGlobal('navigator', {
			gpu: {
				getPreferredCanvasFormat: () => 'bgra8unorm',
				requestAdapter: async () =>
					({ requestDevice: async () => rendering.device }) as unknown as GPUAdapter,
			},
		});
		const atlasTexts: Array<{ readonly font: string; readonly text: string }> = [];
		const atlasFillText: CanvasRenderingContext2D['fillText'] = function (
			this: CanvasRenderingContext2D,
			text,
		) {
			atlasTexts.push({ font: this.font, text });
		};
		vi.stubGlobal('document', { createElement: () => createCanvas(atlasFillText) });
		const context = {
			configure: () => {},
			getCurrentTexture: () => ({ createView: () => ({}) }),
		} as unknown as GPUCanvasContext;
		const renderer = new WebGPURenderer(createWebGPUCanvas(context));
		await expect(renderer.initGPU()).resolves.toBe(true);
		renderer.commitPresentationConfiguration(
			renderer.preparePresentationConfiguration({
				...DEFAULT_TERMINAL_PRESENTATION_CONFIGURATION,
				fontLigatures: false,
			}),
		);
		const store = await loadSurfaceStore();
		rendering.writeBuffer.mockClear();

		renderer.renderPacked(store);

		const uploads = floatUploads(rendering.writeBuffer);
		const textUpload = uploads.find(({ length }) => length === 25 * 13);
		const backgroundUpload = uploads.find(({ length }) => length === 44 * 8);
		const overlayUpload = uploads.find(({ length }) => length === 15 * 8);
		if (textUpload === undefined || backgroundUpload === undefined || overlayUpload === undefined) {
			throw new Error('Expected complete WebGPU style uploads');
		}
		const textInstance = (index: number): Float32Array =>
			textUpload.subarray(index * 13, (index + 1) * 13);
		const foreground = textInstance(7);
		expect(foreground[8]).toBeCloseTo(1 / 255);
		expect(foreground[9]).toBeCloseTo(2 / 255);
		expect(foreground[10]).toBeCloseTo(3 / 255);
		expect(foreground[11]).toBe(1);
		const inverse = textInstance(9);
		expect(inverse[8]).toBeCloseTo(16 / 255);
		expect(inverse[9]).toBeCloseTo(20 / 255);
		expect(inverse[10]).toBeCloseTo(24 / 255);
		expect(inverse[11]).toBe(1);
		expect(textInstance(10)[11]).toBe(0.5);
		expect(textInstance(19)[11]).toBe(0.5);
		expect(atlasTexts.some(({ text }) => text === 'V')).toBe(false);
		expect(atlasTexts.find(({ text }) => text === 'D')?.font).toContain('700');
		expect(atlasTexts.find(({ text }) => text === 'E')?.font).toMatch(/^italic /);

		const backgroundColors = Array.from({ length: 44 }, (_value, index) =>
			Array.from(backgroundUpload.slice(index * 8 + 4, index * 8 + 7), (value) =>
				Math.round(value * 255),
			),
		);
		expect(backgroundColors).toEqual(
			expect.arrayContaining([
				[0, 0, 0],
				[10, 11, 12],
				[216, 222, 233],
			]),
		);
		const overlayInstances = Array.from({ length: 15 }, (_value, index) =>
			overlayUpload.slice(index * 8, (index + 1) * 8),
		);
		const coloredFaintUnderline = overlayInstances.find(
			(instance) =>
				Math.round((instance[4] ?? 0) * 255) === 7 &&
				Math.round((instance[5] ?? 0) * 255) === 8 &&
				Math.round((instance[6] ?? 0) * 255) === 9,
		);
		expect(coloredFaintUnderline?.[7]).toBe(0.5);
		expect(renderer.getLastFrameTelemetry()).toMatchObject({
			backgroundInstances: 44,
			glyphInstances: 25,
			overlayInstances: 15,
			shapingRuns: 0,
		});

		rendering.writeBuffer.mockClear();
		vi.advanceTimersByTime(530);
		renderer.renderPacked(store);
		const blinkTelemetry = renderer.getLastFrameTelemetry();
		expect(blinkTelemetry).toMatchObject({
			glyphInstances: 24,
			rowsRendered: 1,
			shapingRuns: 0,
		});
		expect(blinkTelemetry?.vertexUploadBytes).toBeGreaterThan(16);

		rendering.writeBuffer.mockClear();
		renderer.setFocused(true);
		renderer.renderPacked(surfaceSourceWithVisibleCursor(store));
		const cursorColorSubmitted = floatUploads(rendering.writeBuffer).some((upload) => {
			if (upload.length % 8 !== 0) return false;
			for (let offset = 4; offset < upload.length; offset += 8) {
				const color = Array.from(upload.slice(offset, offset + 3), (value) =>
					Math.round(value * 255),
				);
				if (color[0] === 245 && color[1] === 169 && color[2] === 127) return true;
			}
			return false;
		});
		expect(cursorColorSubmitted).toBe(true);
		// The OSC-colored fixture has 48 cells minus 4 spacers; the block cursor recolors one fill.
		expect(renderer.getLastFrameTelemetry()).toMatchObject({ backgroundInstances: 44 });
		renderer.dispose();
	});

	it('preserves one soft wrap and the following hard boundary for every text consumer', async () => {
		const store = await loadSurfaceStore();
		const rows = bufferRows(store).slice(0, 3);

		expect(findTerminalBufferMatches(rows, 'x界')).toEqual([
			{
				end: { column: 0, row: 1 },
				start: { column: 6, row: 0 },
				text: 'x界',
			},
		]);
		expect(findTerminalBufferMatches(rows, '界H')).toEqual([]);
		expect(findTerminalBufferMatches(rows, '界\nH', { spanLines: true })).toEqual([
			{
				end: { column: 0, row: 2 },
				start: { column: 0, row: 1 },
				text: '界\nH',
			},
		]);
		expect(detectTerminalRowLinks(rows)).toEqual([
			expect.objectContaining({
				range: { end: { x: 0, y: 1 }, start: { x: 0, y: 0 } },
				target: { kind: 'url', url: 'https://example.com' },
			}),
		]);

		const metadata = store.metadata;
		if (metadata === null) throw new Error('Expected surface fixture metadata');
		const accessibleRows = store.rows.slice(0, 3).map((row) =>
			buildAccessibleRow(
				Array.from({ length: metadata.columns }, (_value, column) => {
					const width = projectedCellDisplayWidth(
						row.widths[column] ?? TerminalCellWidthCode.narrow,
					);
					return { text: width > 0 ? row.grapheme(column) : '', width };
				}),
				metadata.columns,
			),
		);
		const accessible = new AccessibleTextModel();
		accessible.update(accessibleRows);
		expect(accessible.getText()).toBe('A中e\u0301👩🏽‍💻x\n界\nH');
		const firstAccessibleRow = accessibleRows[0];
		if (firstAccessibleRow === undefined) throw new Error('Expected first accessible row');
		expect(accessible.positionToOffset({ column: 0, row: 1 })).toBe(
			firstAccessibleRow.text.length + 1,
		);
	});
});
