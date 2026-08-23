import { readFile } from 'node:fs/promises';
import { createRequire } from 'node:module';

import {
	ActiveScreen,
	CursorShape,
	decodeTerminalFrame,
	RowPromptClass,
	SemanticContent,
	TerminalCellWidthCode,
	TerminalColorEncoding,
	TerminalRowPromptClassCode,
	TerminalRowFlag,
	TerminalSemanticContentCode,
	TerminalStyleFlag,
	TerminalUnderlineKindCode,
} from '@orchbee/terminal-protocol';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { AccessibleTextModel, buildAccessibleRow } from '../accessibility';
import type { TerminalBufferRenderCell, TerminalBufferRow } from '../buffer-rows';
import { DEFAULT_TERMINAL_PRESENTATION_CONFIGURATION } from '../presentation-configuration';
import { PackedTerminalStore, projectedCellDisplayWidth } from '../projected';
import { type PackedRendererSource, WebGPURenderer } from '../rendering';
import { findTerminalBufferMatches } from '../search';
import { detectTerminalRowLinks } from '../terminal-row-links';

const moduleRequire = createRequire(import.meta.url);
const surfaceFixturePath = moduleRequire.resolve(
	'@orchbee/terminal-protocol/fixtures/ghostty-surface-v0.0.3.bin',
);
const alternateSurfaceFixturePath = moduleRequire.resolve(
	'@orchbee/terminal-protocol/fixtures/ghostty-alternate-surface-v0.0.3.bin',
);

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
		const backgroundUpload = uploads.find(({ length }) => length === 3 * 8);
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

		const backgroundColors = Array.from({ length: 3 }, (_value, index) =>
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
			backgroundInstances: 3,
			glyphInstances: 25,
			overlayInstances: 15,
			shapingRuns: 0,
		});

		rendering.writeBuffer.mockClear();
		vi.advanceTimersByTime(530);
		renderer.renderPacked(store);
		expect(floatUploads(rendering.writeBuffer).some(({ length }) => length === 24 * 13)).toBe(true);
		expect(renderer.getLastFrameTelemetry()).toMatchObject({
			glyphInstances: 24,
			shapingRuns: 0,
		});

		rendering.writeBuffer.mockClear();
		renderer.setFocused(true);
		renderer.renderPacked(surfaceSourceWithVisibleCursor(store));
		const cursorBackgroundUpload = floatUploads(rendering.writeBuffer).find(
			({ length }) => length === 4 * 8,
		);
		if (cursorBackgroundUpload === undefined) throw new Error('Expected WebGPU cursor background');
		const cursorColors = Array.from({ length: 4 }, (_value, index) =>
			Array.from(cursorBackgroundUpload.slice(index * 8 + 4, index * 8 + 7), (value) =>
				Math.round(value * 255),
			),
		);
		expect(cursorColors).toContainEqual([245, 169, 127]);
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
