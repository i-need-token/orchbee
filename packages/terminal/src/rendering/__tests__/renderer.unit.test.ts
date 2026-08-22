import {
	TerminalCellWidthCode,
	TerminalColorEncoding,
	TerminalGraphicsImageFormat,
	TerminalStyleFlag,
	TerminalUnderlineKindCode,
} from '@orchbee/terminal-protocol';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { DEFAULT_TERMINAL_PRESENTATION_CONFIGURATION } from '../../presentation-configuration';
import {
	DEFAULT_THEME,
	type PackedRendererMetadata,
	type PackedRendererRow,
	type PackedRendererSource,
	type RendererFrameTelemetry,
	resolvePackedPresentationColors,
	WebGPURenderer,
} from '../renderer';

type AtlasDrawImage = (image: CanvasImageSource, dx: number, dy: number) => void;

type TestPackedCursor = PackedRendererMetadata['cursor'] & { readonly wideTail: boolean };

afterEach(() => {
	vi.useRealTimers();
	vi.unstubAllGlobals();
});

function createCanvas(
	getImageData?: CanvasRenderingContext2D['getImageData'],
	textMetrics: Partial<TextMetrics> | ((text: string) => Partial<TextMetrics>) = {},
	fillText: CanvasRenderingContext2D['fillText'] = () => {},
	fillRect: CanvasRenderingContext2D['fillRect'] = () => {},
	stroke: CanvasRenderingContext2D['stroke'] = () => {},
	clearRect: CanvasRenderingContext2D['clearRect'] = () => {},
	drawImage: AtlasDrawImage = () => {},
): HTMLCanvasElement {
	const context = {
		clearRect,
		createImageData: (width: number, height: number) =>
			({ data: new Uint8ClampedArray(width * height * 4), height, width }) as ImageData,
		drawImage,
		font: '10px sans-serif',
		globalAlpha: 1,
		imageSmoothingEnabled: true,
		letterSpacing: '0px',
		textRendering: 'auto',
		measureText: (text: string) =>
			({
				width: 7.8,
				actualBoundingBoxAscent: 9.5,
				actualBoundingBoxDescent: 0,
				fontBoundingBoxAscent: 13,
				fontBoundingBoxDescent: 4,
				...(typeof textMetrics === 'function' ? textMetrics(text) : textMetrics),
			}) as TextMetrics,
		setTransform: () => {},
		save: () => {},
		restore: () => {},
		scale: () => {},
		translate: () => {},
		rect: () => {},
		clip: () => {},
		fillRect,
		fillText,
		beginPath: () => {},
		moveTo: () => {},
		putImageData: () => {},
		lineTo: () => {},
		stroke,
		setLineDash: () => {},
		getImageData:
			getImageData ??
			((_x: number, _y: number, width: number, height: number) =>
				({ data: new Uint8ClampedArray(width * height * 4) }) as ImageData),
	};
	return {
		width: 0,
		height: 0,
		get ownerDocument() {
			return globalThis.document;
		},
		style: {},
		getContext: () => context,
		remove: () => {},
		setAttribute: () => {},
	} as unknown as HTMLCanvasElement;
}

interface TestPackedSource extends PackedRendererSource {
	readonly dirtyRows: Set<number>;
	readonly markRenderedMock: ReturnType<typeof vi.fn<() => void>>;
	readonly styleFlags: readonly Uint16Array[];
}

function createPackedSource(
	lines: readonly string[],
	options: {
		readonly cursor?: Partial<TestPackedCursor>;
		readonly defaultColors?: Partial<PackedRendererMetadata['defaultColors']>;
		readonly full?: boolean;
	} = {},
): TestPackedSource {
	const columns = Math.max(0, ...lines.map((line) => [...line].length));
	const dirtyRows = new Set(lines.map((_line, index) => index));
	const styleFlags = lines.map(() => new Uint16Array(columns));
	let full = options.full ?? true;
	const rows: PackedRendererRow[] = lines.map((line, rowIndex) => {
		const graphemes = [...line.padEnd(columns, ' ')];
		return {
			backgroundColors: new Uint32Array(columns).fill(TerminalColorEncoding.default),
			get dirty() {
				return dirtyRows.has(rowIndex);
			},
			foregroundColors: new Uint32Array(columns).fill(TerminalColorEncoding.default),
			grapheme: (column) => graphemes[column] ?? '',
			styleFlags: styleFlags[rowIndex] ?? new Uint16Array(columns),
			underlineColors: new Uint32Array(columns).fill(TerminalColorEncoding.none),
			underlineKinds: new Uint8Array(columns),
			widths: new Uint8Array(columns).fill(TerminalCellWidthCode.narrow),
		};
	});
	const markRenderedMock = vi.fn<() => void>(() => {
		full = false;
		dirtyRows.clear();
	});
	return {
		dirtyRows,
		markRendered: markRenderedMock,
		markRenderedMock,
		metadata: {
			columns,
			cursor: {
				blinking: false,
				column: 0,
				row: 0,
				shape: 'block',
				visible: false,
				wideTail: false,
				...options.cursor,
			} satisfies TestPackedCursor,
			defaultColors: {
				backgroundRgb: 0x10_14_18,
				cursorRgb: null,
				foregroundRgb: 0xd8_de_e9,
				...options.defaultColors,
			},
			rows: lines.length,
		},
		get needsFullRedraw() {
			return full;
		},
		rows,
		styleFlags,
	};
}

function createPackedGraphemeSource(
	cells: readonly {
		readonly flags?: number;
		readonly text: string;
		readonly underline?: number;
		readonly width: 1 | 2;
	}[],
	options: { readonly cursor?: Partial<TestPackedCursor> } = {},
): TestPackedSource {
	const columns = cells.reduce((total, cell) => total + cell.width, 0);
	const graphemes = Array.from({ length: columns }, () => '');
	const widths = new Uint8Array(columns).fill(TerminalCellWidthCode.spacerTail);
	const flags = new Uint16Array(columns);
	const underlineKinds = new Uint8Array(columns);
	let column = 0;
	for (const cell of cells) {
		graphemes[column] = cell.text;
		widths[column] = cell.width === 2 ? TerminalCellWidthCode.wide : TerminalCellWidthCode.narrow;
		flags[column] = cell.flags ?? 0;
		underlineKinds[column] = cell.underline ?? TerminalUnderlineKindCode.none;
		column += cell.width;
	}
	const dirtyRows = new Set([0]);
	let full = true;
	const markRenderedMock = vi.fn<() => void>(() => {
		full = false;
		dirtyRows.clear();
	});
	const cursor = {
		blinking: false,
		column: 0,
		row: 0,
		shape: 'block',
		visible: false,
		wideTail: false,
		...options.cursor,
	} satisfies TestPackedCursor;
	return {
		dirtyRows,
		markRendered: markRenderedMock,
		markRenderedMock,
		metadata: {
			columns,
			cursor,
			defaultColors: {
				backgroundRgb: 0x10_14_18,
				cursorRgb: null,
				foregroundRgb: 0xd8_de_e9,
			},
			rows: 1,
		},
		get needsFullRedraw() {
			return full;
		},
		rows: [
			{
				backgroundColors: new Uint32Array(columns).fill(TerminalColorEncoding.default),
				get dirty() {
					return dirtyRows.has(0);
				},
				foregroundColors: new Uint32Array(columns).fill(TerminalColorEncoding.default),
				grapheme: (index) => graphemes[index] ?? '',
				styleFlags: flags,
				underlineColors: new Uint32Array(columns).fill(TerminalColorEncoding.none),
				underlineKinds,
				widths,
			},
		],
		styleFlags: [flags],
	};
}

describe('WebGPURenderer recovery retirement', () => {
	it('keeps the last canvas readable while rejecting later draw mutations', async () => {
		vi.stubGlobal('navigator', {});
		vi.stubGlobal('window', { devicePixelRatio: 1 });
		vi.stubGlobal('document', { createElement: () => createCanvas() });
		const fillRect = vi.fn<CanvasRenderingContext2D['fillRect']>();
		const canvas = createCanvas(undefined, {}, undefined, fillRect);
		const renderer = new WebGPURenderer(canvas);
		await renderer.initGPU();
		const first = createPackedSource(['last frame']);
		renderer.renderPacked(first);
		const drawCount = fillRect.mock.calls.length;

		renderer.freeze();
		renderer.renderPacked(createPackedSource(['replacement']));

		expect(fillRect).toHaveBeenCalledTimes(drawCount);
		expect(renderer.setDevicePixelRatio(2)).toBe(false);
		renderer.dispose();
	});
});

describe('projected terminal renderer', () => {
	it('renders decoded Kitty pixels on a clipped sibling overlay and removes it when empty', async () => {
		vi.stubGlobal('navigator', {});
		vi.stubGlobal('window', {
			devicePixelRatio: 1,
			getComputedStyle: () => ({
				paddingBottom: '0px',
				paddingLeft: '0px',
				paddingRight: '0px',
				paddingTop: '0px',
			}),
		});
		const drawImageCalls: unknown[][] = [];
		const drawImage: AtlasDrawImage = (...arguments_) => drawImageCalls.push(arguments_);
		const putImageDataCalls: Array<readonly [ImageData, number, number]> = [];
		const removed: HTMLCanvasElement[] = [];
		const children: HTMLCanvasElement[] = [];
		const ownerDocument = {
			createElement: () => {
				const created = createCanvas(
					undefined,
					{},
					undefined,
					undefined,
					undefined,
					undefined,
					drawImage,
				);
				const context = created.getContext('2d');
				if (context) {
					Reflect.set(context, 'putImageData', (imageData: ImageData, x: number, y: number) => {
						putImageDataCalls.push([imageData, x, y]);
					});
				}
				Object.assign(created, {
					remove: () => removed.push(created),
				});
				return created;
			},
		};
		vi.stubGlobal('document', ownerDocument);
		const parent = {
			appendChild: (child: HTMLCanvasElement) => {
				children.push(child);
				return child;
			},
			clientHeight: 40,
			clientWidth: 80,
		};
		const canvas = createCanvas();
		Object.defineProperties(canvas, {
			ownerDocument: { value: ownerDocument },
			parentElement: { value: parent },
		});
		const renderer = new WebGPURenderer(canvas);
		await renderer.initGPU();
		const base = createPackedSource(['AB']);
		const baseMetadata = base.metadata;
		if (baseMetadata === null) throw new Error('Expected packed-source metadata.');
		const source: PackedRendererSource = {
			...base,
			graphics: {
				images: new Map([
					[
						7,
						{
							data: new Uint8Array([10, 20, 30]),
							format: TerminalGraphicsImageFormat.rgb,
							generation: 1n,
							height: 1,
							id: 7,
							width: 1,
						},
					],
				]),
				placements: [
					{
						column: 1,
						destinationHeight: 16,
						destinationWidth: 16,
						imageId: 7,
						internalId: false,
						offsetX: 2,
						offsetY: 4,
						placementId: 1,
						row: 0,
						sourceHeight: 1,
						sourceWidth: 1,
						sourceX: 0,
						sourceY: 0,
						z: 0,
					},
				],
				resourceRevision: 1n,
			},
			metadata: { ...baseMetadata, cellPixelHeight: 16, cellPixelWidth: 8 },
		};

		renderer.renderPacked(source);

		expect(children).toHaveLength(1);
		expect(children[0]?.getAttribute?.('data-terminal-graphics-overlay')).not.toBeNull();
		expect(putImageDataCalls).toEqual([
			[expect.objectContaining({ data: new Uint8ClampedArray([10, 20, 30, 255]) }), 0, 0],
		]);
		const placementDraw = drawImageCalls.find((call) => call.length === 9);
		expect(placementDraw?.slice(1)).toEqual([0, 0, 1, 1, 10, 4.25, 16, 17]);

		renderer.renderPacked(base);
		expect(removed).toEqual([children[0]]);
		renderer.dispose();
	});

	it('resolves Host terminal colors and application overlay colors without a second palette', () => {
		const hostDefaultColors = {
			backgroundRgb: 0x10_14_18,
			cursorRgb: 0xf5_a9_7f,
			foregroundRgb: 0xd8_de_e9,
		};
		const theme = {
			...DEFAULT_THEME,
			background: '#abcdef',
			cursor: '#fedcba',
			foreground: '#123456',
			selectionBackground: '#112233',
			selectionForeground: '#ddeeff',
		};
		const colors = resolvePackedPresentationColors(hostDefaultColors, theme);

		expect(colors).toMatchObject({
			background: [16, 20, 24],
			cursor: [245, 169, 127],
			cursorAccent: [16, 20, 24],
			foreground: [216, 222, 233],
			selectionBackground: [17, 34, 51],
			selectionForeground: [221, 238, 255],
		});
		expect(
			resolvePackedPresentationColors(
				{ backgroundRgb: 0, cursorRgb: null, foregroundRgb: 0xff_ff_ff },
				theme,
				'#010203',
			).cursorAccent,
		).toEqual([1, 2, 3]);
		const override = resolvePackedPresentationColors(hostDefaultColors, theme, undefined, {
			background: '#102030',
			foreground: '#aabbcc',
		});
		expect(override).toMatchObject({
			background: [16, 32, 48],
			cursor: [245, 169, 127],
			cursorAccent: [16, 20, 24],
			foreground: [170, 187, 204],
		});
		expect(hostDefaultColors).toEqual({
			backgroundRgb: 0x10_14_18,
			cursorRgb: 0xf5_a9_7f,
			foregroundRgb: 0xd8_de_e9,
		});
	});

	it('uses projected style flags for independently configurable font weights', () => {
		vi.stubGlobal('window', { devicePixelRatio: 2 });
		vi.stubGlobal('document', { createElement: () => createCanvas() });
		const renderer = new WebGPURenderer(createCanvas(), {
			fontFamily: 'JetBrains Mono',
			fontSize: 13,
			fontWeight: 225,
			fontWeightBold: 575,
			devicePixelRatio: 2,
		});
		const ensureGlyph = Reflect.get(renderer, 'ensureGlyph') as (
			text: string,
			flags: number,
			width: number,
		) => unknown;
		const atlasContext = Reflect.get(renderer, 'atlasCtx') as CanvasRenderingContext2D;

		ensureGlyph.call(renderer, 'A', 0, 1);
		expect(atlasContext.font).toBe('225 26px JetBrains Mono');
		ensureGlyph.call(renderer, 'B', TerminalStyleFlag.italic, 1);
		expect(atlasContext.font).toBe('italic 225 26px JetBrains Mono');
		ensureGlyph.call(renderer, 'C', TerminalStyleFlag.bold, 1);
		expect(atlasContext.font).toBe('575 26px JetBrains Mono');

		const prepared = renderer.preparePresentationConfiguration({
			...DEFAULT_TERMINAL_PRESENTATION_CONFIGURATION,
			fontFamily: 'JetBrains Mono',
			fontSize: 13,
			fontWeight: 250,
			fontWeightBold: 600,
		});
		renderer.commitPresentationConfiguration(prepared);
		ensureGlyph.call(renderer, 'D', TerminalStyleFlag.bold | TerminalStyleFlag.italic, 1);
		expect(atlasContext.font).toBe('italic 600 26px JetBrains Mono');
		renderer.dispose();
	});

	it('uses geometric-precision rasterization for Ghostty-equivalent unthickened glyphs', () => {
		vi.stubGlobal('window', { devicePixelRatio: 2 });
		vi.stubGlobal('document', { createElement: () => createCanvas() });
		const fallbackCanvas = createCanvas();
		const fallbackContext = fallbackCanvas.getContext('2d');
		if (fallbackContext === null) throw new Error('Canvas fallback context is unavailable.');
		const renderer = new WebGPURenderer(fallbackCanvas, { devicePixelRatio: 2 });
		const atlasContext = Reflect.get(renderer, 'atlasCtx') as CanvasRenderingContext2D;

		expect(Reflect.get(atlasContext, 'textRendering')).toBe('geometricPrecision');
		Reflect.apply(Reflect.get(renderer, 'initFallback2D'), renderer, []);
		expect(Reflect.get(fallbackContext, 'textRendering')).toBe('geometricPrecision');
		renderer.dispose();
	});

	it('rounds sampled font metrics at the physical pixel scale like Ghostty', () => {
		vi.stubGlobal('window', { devicePixelRatio: 2 });
		const measurementCanvas = createCanvas();
		const measurementContext = measurementCanvas.getContext('2d');
		if (measurementContext === null) throw new Error('Canvas measurement context is unavailable.');
		measurementContext.measureText = () => {
			const fontSize = Number.parseFloat(measurementContext.font.match(/([\d.]+)px/u)?.[1] ?? '0');
			const sampled = fontSize >= 1_000;
			return {
				actualBoundingBoxAscent: 0.73 * fontSize,
				actualBoundingBoxDescent: 0,
				fontBoundingBoxAscent: sampled ? 1.02 * fontSize : Math.round(1.02 * fontSize),
				fontBoundingBoxDescent: sampled ? 0.3 * fontSize : Math.round(0.3 * fontSize),
				width: 0.6 * fontSize,
			} as TextMetrics;
		};
		vi.stubGlobal('document', { createElement: () => measurementCanvas });

		const renderer = new WebGPURenderer(createCanvas(), {
			devicePixelRatio: 2,
			fontFamily: 'JetBrains Mono',
			fontSize: 15,
		});

		expect(renderer.getMetrics()).toEqual({ baseline: 15.5, height: 20, width: 9 });
		const detailedMetrics = Reflect.get(renderer, 'detailedMetrics') as {
			readonly cellBaseline: number;
			readonly faceHeight: number;
			readonly faceWidth: number;
		};
		expect(detailedMetrics).toMatchObject({
			cellBaseline: 4.5,
			faceWidth: 9,
		});
		expect(detailedMetrics.faceHeight).toBeCloseTo(19.8, 8);
		renderer.dispose();
	});

	it('preserves signed bearings and descenders in browser glyph raster bounds', () => {
		vi.stubGlobal('window', { devicePixelRatio: 1 });
		vi.stubGlobal('document', { createElement: () => createCanvas() });
		const renderer = new WebGPURenderer(createCanvas());
		const glyphRasterLayout = Reflect.get(renderer, 'glyphRasterLayout') as (
			measured: TextMetrics,
			rasterScale: number,
			codepoint: number,
			constraintWidth: 1 | 2,
		) => {
			readonly left: number;
			readonly target: { readonly x: number; readonly y: number };
			readonly targetTop: number;
		};

		const layout = glyphRasterLayout.call(
			renderer,
			{
				actualBoundingBoxAscent: 9,
				actualBoundingBoxDescent: 2,
				actualBoundingBoxLeft: -2,
				actualBoundingBoxRight: 6,
				width: 8,
			} as TextMetrics,
			1,
			0x41,
			1,
		);

		expect(layout.left).toBe(-2);
		expect(layout.target.x).toBeCloseTo(2.1, 8);

		const rightBearing = glyphRasterLayout.call(
			renderer,
			{
				actualBoundingBoxAscent: 9,
				actualBoundingBoxDescent: 2,
				actualBoundingBoxLeft: 6,
				actualBoundingBoxRight: -2,
				width: 8,
			} as TextMetrics,
			1,
			0x41,
			1,
		);
		expect(rightBearing.target.x).toBeCloseTo(-5.9, 8);

		const descender = glyphRasterLayout.call(
			renderer,
			{
				actualBoundingBoxAscent: 9,
				actualBoundingBoxDescent: 6,
				actualBoundingBoxLeft: 0,
				actualBoundingBoxRight: 8,
				width: 8,
			} as TextMetrics,
			1,
			0x67,
			1,
		);
		expect(descender.target.y).toBe(-2);
		expect(descender.targetTop).toBe(4);
		renderer.dispose();
	});

	it('shapes complex projected graphemes as one forced-LTR browser run', async () => {
		vi.stubGlobal('window', { devicePixelRatio: 1 });
		vi.stubGlobal('navigator', {});
		const textPaints: Array<{ readonly direction: CanvasDirection; readonly text: string }> = [];
		const fillText: CanvasRenderingContext2D['fillText'] = function (
			this: CanvasRenderingContext2D,
			text,
		) {
			textPaints.push({ direction: this.direction, text });
		};
		vi.stubGlobal('document', {
			createElement: () => createCanvas(undefined, {}, fillText),
		});
		const renderer = new WebGPURenderer(createCanvas(undefined, {}, fillText), {
			fontLigatures: false,
		});
		await renderer.initGPU();

		renderer.renderPacked(
			createPackedGraphemeSource([
				{ text: 'ক', width: 1 },
				{ text: '্ষ', width: 1 },
			]),
		);

		expect(textPaints).toEqual([{ direction: 'ltr', text: 'ক্ষ' }]);
		renderer.dispose();
	});

	it('keeps grid-fitted shaping intact across selection boundaries', async () => {
		vi.stubGlobal('window', { devicePixelRatio: 2 });
		vi.stubGlobal('navigator', {});
		const textPaints: Array<{ readonly letterSpacing: number; readonly text: string }> = [];
		const fillText: CanvasRenderingContext2D['fillText'] = function (
			this: CanvasRenderingContext2D,
			text,
		) {
			textPaints.push({ letterSpacing: Number.parseFloat(this.letterSpacing), text });
		};
		vi.stubGlobal('document', {
			createElement: () => createCanvas(undefined, {}, fillText),
		});
		const renderer = new WebGPURenderer(createCanvas(undefined, {}, fillText), {
			devicePixelRatio: 2,
		});
		const ensureGlyph = Reflect.get(renderer, 'ensureGlyph') as (
			text: string,
			flags: number,
			width: number,
			codepoint: number,
			constraintWidth: 1 | 2,
		) => unknown;

		ensureGlyph.call(renderer, 'AB', 0, 2, 0, 1);
		expect(textPaints).toHaveLength(1);
		expect(textPaints[0]?.text).toBe('AB');
		expect(textPaints[0]?.letterSpacing).toBeCloseTo(0.4, 8);

		await renderer.initGPU();
		textPaints.length = 0;
		const source = createPackedSource(['A   B']);
		renderer.renderPacked(source);
		expect(textPaints).toHaveLength(1);
		expect(textPaints[0]?.text).toBe('A   B');
		expect(textPaints[0]?.letterSpacing).toBeCloseTo(0.2, 8);

		textPaints.length = 0;
		renderer.setProjectedViewportState(0, {
			endCol: 4,
			endRow: 0,
			rectangle: false,
			startCol: 4,
			startRow: 0,
		});
		renderer.renderPacked(source, true);
		expect(textPaints.map(({ text }) => text)).toEqual(['A   B', 'A   B']);
		expect(textPaints[0]?.letterSpacing).toBeCloseTo(0.2, 8);
		expect(textPaints[1]?.letterSpacing).toBeCloseTo(0.2, 8);
		renderer.dispose();
	});

	it('keeps animated foreground boundaries out of shaping geometry while preserving paint', async () => {
		vi.stubGlobal('window', { devicePixelRatio: 1 });
		vi.stubGlobal('navigator', {});
		const textPaints: Array<{ readonly color: string; readonly text: string }> = [];
		const fillText: CanvasRenderingContext2D['fillText'] = function (
			this: CanvasRenderingContext2D,
			text,
		) {
			textPaints.push({ color: String(this.fillStyle), text });
		};
		vi.stubGlobal('document', {
			createElement: () => createCanvas(undefined, {}, fillText),
		});
		const renderer = new WebGPURenderer(createCanvas(undefined, {}, fillText));
		await renderer.initGPU();
		const source = createPackedSource(['Added OS']);
		const row = source.rows[0];
		if (row === undefined) throw new Error('Missing animated-color row');
		row.foregroundColors.fill(0x00_00_ff);
		row.foregroundColors.fill(0xff_00_00, 0, 3);

		renderer.renderPacked(source);

		expect(textPaints).toEqual([
			{ color: 'rgb(255, 0, 0)', text: 'Added OS' },
			{ color: 'rgb(0, 0, 255)', text: 'Added OS' },
		]);
		renderer.dispose();
	});

	it('keeps foreground-only blank cells outside shaping geometry', () => {
		vi.stubGlobal('window', { devicePixelRatio: 1 });
		vi.stubGlobal('document', { createElement: () => createCanvas() });
		const renderer = new WebGPURenderer(createCanvas());
		const source = createPackedGraphemeSource([
			{ text: 'A', width: 1 },
			{ text: '', width: 1 },
			{ text: '', width: 1 },
		]);
		const row = source.rows[0];
		if (row === undefined) throw new Error('Missing foreground-only blank row');
		const metadata = source.metadata;
		if (metadata === null) throw new Error('Missing foreground-only blank metadata');
		row.foregroundColors[2] = 0xff_00_00;
		const buildShaping = Reflect.get(renderer, 'buildPackedRowTextShaping') as (
			row: PackedRendererRow,
			columns: number,
			rowIndex: number,
			cursor: TestPackedCursor,
			cursorEnabled: boolean,
			preeditLayout: null,
		) => { readonly plan: readonly ({ readonly columns: number } | null)[] };

		const shaping = buildShaping.call(
			renderer,
			row,
			metadata.columns,
			0,
			metadata.cursor,
			false,
			null,
		);

		expect(shaping.plan[0]).toBeNull();
		renderer.dispose();
	});

	it('reuses one WebGPU shaping raster as an animated foreground boundary moves', async () => {
		vi.stubGlobal('window', { devicePixelRatio: 1 });
		vi.stubGlobal('document', {
			createElement: () => createCanvas(undefined, (text) => ({ width: text.length * 7.8 })),
		});
		vi.stubGlobal('GPUTextureUsage', { TEXTURE_BINDING: 1, COPY_DST: 2, RENDER_ATTACHMENT: 4 });
		vi.stubGlobal('GPUBufferUsage', { UNIFORM: 1, COPY_DST: 2, VERTEX: 4 });
		const rendering = createSubmittingDevice();
		vi.stubGlobal('navigator', {
			gpu: {
				requestAdapter: async () =>
					({ requestDevice: async () => rendering.device }) as unknown as GPUAdapter,
				getPreferredCanvasFormat: () => 'bgra8unorm',
			},
		});
		const context = {
			configure: vi.fn<(configuration: GPUCanvasConfiguration) => void>(),
			getCurrentTexture: () => ({ createView: () => ({}) }),
		} as unknown as GPUCanvasContext;
		const renderer = new WebGPURenderer(createWebGPUCanvas(context));
		await renderer.initGPU();
		const source = createPackedSource(['Added OS']);
		const row = source.rows[0];
		if (row === undefined) throw new Error('Missing animated-color row');
		row.foregroundColors.fill(0x00_00_ff);
		row.foregroundColors.fill(0xff_00_00, 0, 3);

		rendering.writeBuffer.mockClear();
		renderer.renderPacked(source);
		const firstKeys = [...(Reflect.get(renderer, 'glyphMap') as Map<string, unknown>).keys()];
		const firstText = rendering.writeBuffer.mock.calls.flatMap((call) => {
			const value = call[2];
			return value instanceof Float32Array && value.length === 26 ? [Array.from(value)] : [];
		})[0];
		expect(firstText).toBeDefined();
		expect(firstText?.slice(8, 11)).toEqual([1, 0, 0]);
		expect(firstText?.slice(21, 24)).toEqual([0, 0, 1]);
		expect(renderer.getLastFrameTelemetry()?.atlasGlyphsCreated).toBe(1);

		row.foregroundColors.fill(0x00_00_ff);
		row.foregroundColors.fill(0xff_00_00, 0, 5);
		source.dirtyRows.add(0);
		rendering.writeBuffer.mockClear();
		renderer.renderPacked(source, true);
		const secondKeys = [...(Reflect.get(renderer, 'glyphMap') as Map<string, unknown>).keys()];
		const secondText = rendering.writeBuffer.mock.calls.flatMap((call) => {
			const value = call[2];
			return value instanceof Float32Array && value.length === 26 ? [Array.from(value)] : [];
		})[0];
		expect(secondText?.slice(8, 11)).toEqual([1, 0, 0]);
		expect(secondText?.slice(21, 24)).toEqual([0, 0, 1]);
		expect(secondKeys).toEqual(firstKeys);
		expect(renderer.getLastFrameTelemetry()?.atlasGlyphsCreated).toBe(0);
		renderer.dispose();
	});

	it('keeps constrained symbols outside multi-cell browser shaping runs', async () => {
		vi.stubGlobal('window', { devicePixelRatio: 1 });
		vi.stubGlobal('navigator', {});
		const fillText = vi.fn<CanvasRenderingContext2D['fillText']>();
		vi.stubGlobal('document', {
			createElement: () => createCanvas(undefined, {}, fillText),
		});
		const renderer = new WebGPURenderer(createCanvas(undefined, {}, fillText));
		await renderer.initGPU();

		renderer.renderPacked(
			createPackedGraphemeSource([
				{ text: 'A', width: 1 },
				{ text: '\uea61', width: 1 },
				{ text: 'B', width: 1 },
			]),
		);

		expect(fillText.mock.calls.map(([text]) => text)).toEqual(['A', '\uea61', 'B']);
		renderer.dispose();
	});

	it('remeasures and fully repaints Canvas geometry at a fractional device-pixel ratio', async () => {
		vi.stubGlobal('window', {
			devicePixelRatio: 2,
			getComputedStyle: () => ({
				paddingTop: '0px',
				paddingBottom: '0px',
				paddingLeft: '0px',
				paddingRight: '0px',
			}),
		});
		vi.stubGlobal('navigator', {});
		vi.stubGlobal('document', { createElement: () => createCanvas() });
		const canvas = createCanvas();
		Object.defineProperty(canvas, 'parentElement', {
			value: { clientWidth: 85, clientHeight: 39 },
		});
		const renderer = new WebGPURenderer(canvas, {
			fontSize: 13,
			devicePixelRatio: 2,
			paddingBalance: 'equal',
		});
		const telemetry = vi.fn<(frame: RendererFrameTelemetry) => void>();
		const requestRender = vi.fn<() => void>();
		renderer.setFrameTelemetryCallback(telemetry);
		renderer.setRenderRequestCallback(requestRender);
		await renderer.initGPU();
		const source = createPackedSource(['ABCDEFGHIJ', 'KLMNOPQRST']);
		renderer.renderPacked(source);
		const measureFont = vi.spyOn(renderer as unknown as { measureFont(): unknown }, 'measureFont');
		const resetAtlas = vi.spyOn(renderer as unknown as { resetAtlas(): void }, 'resetAtlas');
		const resize = vi.spyOn(renderer, 'resize');
		requestRender.mockClear();
		telemetry.mockClear();

		expect(renderer.getMetrics()).toEqual({ width: 8, height: 17, baseline: 13 });
		expect([canvas.width, canvas.height]).toEqual([160, 68]);
		expect(renderer.setDevicePixelRatio(1.5)).toBe(true);
		expect(measureFont).toHaveBeenCalledOnce();
		expect(resetAtlas).toHaveBeenCalledOnce();
		expect(resize).toHaveBeenCalledOnce();
		expect(requestRender).toHaveBeenCalledOnce();
		expect(canvas.style).toMatchObject({ height: '34px', width: '80px' });
		expect([canvas.width, canvas.height]).toEqual([120, 51]);
		renderer.renderPacked(source);
		expect(telemetry).toHaveBeenCalledWith(
			expect.objectContaining({
				atlasGlyphCount: 0,
				backend: 'canvas2d',
				cellsVisited: 20,
				forceAll: true,
				glyphInstances: 2,
				reasons: ['surface'],
				rowsRendered: 2,
				shapingRuns: 2,
			}),
		);

		telemetry.mockClear();
		renderer.renderPacked(source);
		expect(telemetry).toHaveBeenCalledWith(
			expect.objectContaining({
				backend: 'canvas2d',
				cellsVisited: 0,
				forceAll: false,
				glyphInstances: 0,
				reasons: ['dirty'],
				rowsRendered: 0,
			}),
		);
		renderer.dispose();
	});

	it('stabilizes an integer Canvas backing store at a fractional device-pixel ratio', async () => {
		vi.stubGlobal('window', { devicePixelRatio: 1.5 });
		vi.stubGlobal('navigator', {});
		vi.stubGlobal('document', { createElement: () => createCanvas() });
		const canvas = createCanvas();
		let backingWidth = 0;
		let backingHeight = 0;
		Object.defineProperties(canvas, {
			height: {
				configurable: true,
				get: () => backingHeight,
				set: (value: number) => {
					backingHeight = Math.trunc(value);
				},
			},
			width: {
				configurable: true,
				get: () => backingWidth,
				set: (value: number) => {
					backingWidth = Math.trunc(value);
				},
			},
		});
		const renderer = new WebGPURenderer(canvas, { devicePixelRatio: 1.5 });
		const telemetry = vi.fn<(frame: RendererFrameTelemetry) => void>();
		renderer.setFrameTelemetryCallback(telemetry);
		await renderer.initGPU();
		const source = createPackedSource(['AB']);

		renderer.renderPacked(source);
		expect([canvas.width, canvas.height]).toEqual([24, 26]);
		telemetry.mockClear();
		renderer.renderPacked(source);

		expect(telemetry).toHaveBeenCalledWith(
			expect.objectContaining({ cellsVisited: 0, forceAll: false, rowsRendered: 0 }),
		);
		renderer.dispose();
	});

	it('keeps the pre-frame Canvas transparent and reuses the latest Host background on resize', async () => {
		vi.stubGlobal('window', { devicePixelRatio: 1 });
		vi.stubGlobal('navigator', {});
		const paints: Array<{ readonly color: string; readonly rectangle: readonly number[] }> = [];
		const fillRect: CanvasRenderingContext2D['fillRect'] = function (
			this: CanvasRenderingContext2D,
			x,
			y,
			width,
			height,
		) {
			paints.push({ color: String(this.fillStyle), rectangle: [x, y, width, height] });
		};
		vi.stubGlobal('document', {
			createElement: () => createCanvas(undefined, {}, undefined, fillRect),
		});
		const renderer = new WebGPURenderer(createCanvas(undefined, {}, undefined, fillRect));
		await renderer.initGPU();

		renderer.resize(1, 1);
		expect(paints).toEqual([]);

		renderer.renderPacked(createPackedSource(['A']));
		paints.length = 0;
		renderer.resize(1, 1);
		expect(paints).toEqual([{ color: 'rgb(16, 20, 24)', rectangle: [0, 0, 8, 17] }]);
		renderer.dispose();
	});

	it('applies a default-only display override through Canvas without mutating Host colors', async () => {
		vi.stubGlobal('window', { devicePixelRatio: 1 });
		vi.stubGlobal('navigator', {});
		const fills: Array<{
			readonly alpha: number;
			readonly color: string;
			readonly rectangle: readonly number[];
		}> = [];
		const textPaints: Array<{
			readonly alpha: number;
			readonly color: string;
			readonly text: string;
		}> = [];
		const underlinePaints: Array<{ readonly alpha: number; readonly color: string }> = [];
		const fillRect: CanvasRenderingContext2D['fillRect'] = function (
			this: CanvasRenderingContext2D,
			x,
			y,
			width,
			height,
		) {
			fills.push({
				alpha: this.globalAlpha,
				color: String(this.fillStyle),
				rectangle: [x, y, width, height],
			});
		};
		const fillText: CanvasRenderingContext2D['fillText'] = function (
			this: CanvasRenderingContext2D,
			text,
		) {
			textPaints.push({ alpha: this.globalAlpha, color: String(this.fillStyle), text });
		};
		const stroke: CanvasRenderingContext2D['stroke'] = function (this: CanvasRenderingContext2D) {
			underlinePaints.push({ alpha: this.globalAlpha, color: String(this.strokeStyle) });
		};
		const canvas = createCanvas(undefined, {}, fillText, fillRect, stroke);
		vi.stubGlobal('document', { createElement: () => createCanvas() });
		const renderer = new WebGPURenderer(canvas, {
			allowTransparency: true,
			backgroundOpacity: 0,
			defaultColorOverride: { background: '#102030', foreground: '#ddeeff' },
		});
		await renderer.initGPU();
		renderer.setFocused(true);
		const source = createPackedSource(['ABCDE'], {
			cursor: { column: 4, visible: true },
			defaultColors: {
				backgroundRgb: 0x11_22_33,
				cursorRgb: 0xaa_bb_cc,
				foregroundRgb: 0x22_33_44,
			},
		});
		const row = source.rows[0];
		if (row === undefined || source.metadata === null) {
			throw new Error('default-color fixture did not create one packed row and metadata');
		}
		row.foregroundColors[1] = 0;
		row.backgroundColors[1] = 0x0a_0b_0c;
		row.foregroundColors[2] = TerminalColorEncoding.none;
		row.backgroundColors[2] = TerminalColorEncoding.none;
		row.styleFlags[3] = TerminalStyleFlag.inverse;
		row.underlineKinds[0] = TerminalUnderlineKindCode.single;
		row.underlineColors[0] = TerminalColorEncoding.default;
		const hostDefaultColors = { ...source.metadata.defaultColors };

		renderer.renderPacked(source);

		expect(textPaints).toEqual(
			expect.arrayContaining([
				expect.objectContaining({ alpha: 1, color: 'rgb(221, 238, 255)' }),
				expect.objectContaining({ alpha: 1, color: 'rgb(0, 0, 0)' }),
				expect.objectContaining({ alpha: 1, color: 'rgb(34, 51, 68)' }),
				expect.objectContaining({ alpha: 1, color: 'rgb(16, 32, 48)' }),
			]),
		);
		expect(fills).toEqual(
			expect.arrayContaining([
				expect.objectContaining({
					alpha: 1,
					color: 'rgb(10, 11, 12)',
					rectangle: [8, 0, 8, 17],
				}),
				expect.objectContaining({
					alpha: 1,
					color: 'rgb(221, 238, 255)',
					rectangle: [24, 0, 8, 17],
				}),
				expect.objectContaining({
					alpha: 1,
					color: 'rgb(170, 187, 204)',
					rectangle: [32, 0, 8, 17],
				}),
			]),
		);
		expect(fills.some((paint) => paint.rectangle[0] === 16)).toBe(false);
		expect(underlinePaints).toContainEqual({ alpha: 1, color: 'rgb(221, 238, 255)' });
		expect(source.metadata.defaultColors).toEqual(hostDefaultColors);
		renderer.dispose();
	});

	it('reports the observable Canvas fallback and packed frame telemetry', async () => {
		vi.stubGlobal('window', { devicePixelRatio: 1 });
		vi.stubGlobal('navigator', {});
		const texts: string[] = [];
		const fillText: CanvasRenderingContext2D['fillText'] = (text) => texts.push(text);
		vi.stubGlobal('document', { createElement: () => createCanvas(undefined, {}, fillText) });
		const renderer = new WebGPURenderer(createCanvas(undefined, {}, fillText));
		const health = vi.fn<(event: { readonly state: string; readonly backend: string }) => void>();
		const telemetry = vi.fn<(frame: RendererFrameTelemetry) => void>();
		renderer.setHealthChangeCallback(health);
		renderer.setFrameTelemetryCallback(telemetry);

		await expect(renderer.initGPU()).resolves.toBe(false);
		const source = createPackedSource(['A']);
		renderer.renderPacked(source);

		expect(health).toHaveBeenCalledWith(
			expect.objectContaining({ state: 'fallback', backend: 'canvas2d' }),
		);
		expect(texts).toContain('A');
		expect(source.markRenderedMock).toHaveBeenCalledOnce();
		expect(telemetry).toHaveBeenCalledWith(
			expect.objectContaining({
				backend: 'canvas2d',
				cellsVisited: 1,
				glyphInstances: 1,
				rowsRendered: 1,
			}),
		);
		renderer.dispose();
	});

	it('prepares and publishes one complete Canvas presentation with one full repaint', async () => {
		vi.stubGlobal('window', {
			devicePixelRatio: 2,
			getComputedStyle: () => ({
				paddingBottom: '1px',
				paddingLeft: '1px',
				paddingRight: '1px',
				paddingTop: '1px',
			}),
		});
		vi.stubGlobal('navigator', {});
		const fillRect = vi.fn<CanvasRenderingContext2D['fillRect']>();
		const clearRect = vi.fn<CanvasRenderingContext2D['clearRect']>();
		const canvas = createCanvas(undefined, {}, undefined, fillRect, undefined, clearRect);
		Object.assign(canvas, {
			parentElement: { clientHeight: 52, clientWidth: 37 },
		});
		vi.stubGlobal('document', {
			createElement: () => createCanvas(undefined, {}, undefined, fillRect, undefined, clearRect),
		});
		const renderer = new WebGPURenderer(canvas);
		const telemetry = vi.fn<(frame: RendererFrameTelemetry) => void>();
		const requestRender = vi.fn<() => void>();
		renderer.setFrameTelemetryCallback(telemetry);
		renderer.setRenderRequestCallback(requestRender);
		await renderer.initGPU();
		renderer.resize(2, 1);
		const source = createPackedSource(['AB'], { cursor: { visible: true } });
		renderer.setProjectedViewportState(0, {
			endCol: 1,
			endRow: 0,
			rectangle: false,
			startCol: 0,
			startRow: 0,
		});
		renderer.setSearchMatches(
			[{ end: { column: 2, row: 0 }, start: { column: 0, row: 0 }, text: 'AB' }],
			{ end: { column: 2, row: 0 }, start: { column: 0, row: 0 }, text: 'AB' },
		);
		renderer.setFocused(true);
		renderer.renderPacked(source);
		const measureFont = vi.spyOn(renderer as unknown as { measureFont(): unknown }, 'measureFont');
		const resetAtlas = vi.spyOn(renderer as unknown as { resetAtlas(): void }, 'resetAtlas');
		requestRender.mockClear();
		telemetry.mockClear();
		clearRect.mockClear();

		const prepared = renderer.preparePresentationConfiguration({
			...DEFAULT_TERMINAL_PRESENTATION_CONFIGURATION,
			allowTransparency: true,
			fontFamily: 'Runtime Canvas Mono',
			fontLigatures: false,
			fontMetricModifiers: { cellHeight: 3, cellWidth: 2 },
			fontSize: 18,
			fontWeight: 450,
			fontWeightBold: 700,
			paddingBalance: 'equal',
		});
		expect(requestRender).not.toHaveBeenCalled();
		expect(renderer.getMetrics()).toEqual({ baseline: 13, height: 17, width: 8 });
		renderer.commitPresentationConfiguration(prepared);

		expect(measureFont).toHaveBeenCalledOnce();
		expect(resetAtlas).toHaveBeenCalledOnce();
		expect(requestRender).toHaveBeenCalledOnce();
		expect(renderer.getMetrics()).toEqual({ baseline: 14, height: 20, width: 10 });
		expect(canvas.style).toMatchObject({ height: '20px', width: '20px' });
		expect(canvas.width).toBe(40);
		expect(canvas.height).toBe(40);
		expect(Reflect.get(renderer, 'projectedSelectionCoords')).toEqual({
			endCol: 1,
			endRow: 0,
			rectangle: false,
			startCol: 0,
			startRow: 0,
		});
		expect(Reflect.get(renderer, 'searchMatches')).toHaveLength(1);
		renderer.renderPacked(source);
		expect(telemetry).toHaveBeenCalledWith(
			expect.objectContaining({
				backend: 'canvas2d',
				forceAll: true,
				reasons: expect.arrayContaining(['font', 'surface']),
			}),
		);
		expect(fillRect).toHaveBeenCalled();
		renderer.dispose();
	});

	it('leaves Canvas presentation untouched when measurement preparation fails', async () => {
		vi.stubGlobal('window', { devicePixelRatio: 1 });
		vi.stubGlobal('navigator', {});
		let failMeasurement = false;
		vi.stubGlobal('document', {
			createElement: () =>
				failMeasurement
					? ({ getContext: () => null } as unknown as HTMLCanvasElement)
					: createCanvas(),
		});
		const renderer = new WebGPURenderer(createCanvas());
		await renderer.initGPU();
		const initialMetrics = renderer.getMetrics();
		failMeasurement = true;

		expect(() =>
			renderer.preparePresentationConfiguration({
				...DEFAULT_TERMINAL_PRESENTATION_CONFIGURATION,
				fontFamily: 'Unavailable Runtime Mono',
				fontSize: 22,
			}),
		).toThrow('Failed to create font measurement context');
		expect(renderer.getMetrics()).toEqual(initialMetrics);
		renderer.dispose();
	});

	it('redraws only a dirty packed row and its adjacent shaping rows', async () => {
		vi.stubGlobal('window', { devicePixelRatio: 1 });
		vi.stubGlobal('navigator', {});
		const clearRect = vi.fn<CanvasRenderingContext2D['clearRect']>();
		vi.stubGlobal('document', {
			createElement: () => createCanvas(undefined, {}, undefined, undefined, undefined, clearRect),
		});
		const renderer = new WebGPURenderer(
			createCanvas(undefined, {}, undefined, undefined, undefined, clearRect),
		);
		await renderer.initGPU();
		const source = createPackedSource(['0', '1', '2', '3', '4']);

		renderer.renderPacked(source);
		clearRect.mockClear();
		source.dirtyRows.add(2);
		renderer.renderPacked(source);

		const rows = clearRect.mock.calls
			.map(([, y]) => y / renderer.getMetrics().height)
			.sort((left, right) => left - right);
		expect(rows).toEqual([1, 2, 3]);
		renderer.dispose();
	});

	it('indexes projected search rows without a legacy buffer and redraws selection and link hover', async () => {
		vi.stubGlobal('window', { devicePixelRatio: 1 });
		vi.stubGlobal('navigator', {});
		const paints: string[] = [];
		const strokes: string[] = [];
		const fillRect: CanvasRenderingContext2D['fillRect'] = function (
			this: CanvasRenderingContext2D,
		) {
			paints.push(String(this.fillStyle));
		};
		const stroke: CanvasRenderingContext2D['stroke'] = function (this: CanvasRenderingContext2D) {
			strokes.push(String(this.strokeStyle));
		};
		vi.stubGlobal('document', {
			createElement: () => createCanvas(undefined, {}, undefined, fillRect, stroke),
		});
		const renderer = new WebGPURenderer(createCanvas(undefined, {}, undefined, fillRect, stroke));
		await renderer.initGPU();
		const source = createPackedSource(['A']);

		renderer.setProjectedViewportState(10, null);
		renderer.setSearchMatches([
			{ text: 'A', start: { column: 0, row: 10 }, end: { column: 0, row: 10 } },
		]);
		renderer.renderPacked(source);
		expect(paints).toContain('rgb(81, 92, 106)');

		paints.length = 0;
		renderer.setSearchMatches([]);
		renderer.setProjectedViewportState(10, {
			startCol: 0,
			startRow: 0,
			endCol: 0,
			endRow: 0,
			rectangle: false,
		});
		renderer.renderPacked(source);
		expect(paints).toContain('rgb(212, 212, 212)');

		renderer.setHoveredLinkRange({ startX: 0, startY: 0, endX: 0, endY: 0 });
		renderer.renderPacked(source);
		expect(strokes).toContain('#4A90E2');
		renderer.dispose();
	});

	it('suspends packed blinking-cell animation while hidden', async () => {
		vi.useFakeTimers();
		vi.stubGlobal('window', { devicePixelRatio: 1 });
		vi.stubGlobal('navigator', {});
		vi.stubGlobal('document', { createElement: () => createCanvas() });
		const renderer = new WebGPURenderer(createCanvas());
		await renderer.initGPU();
		const requestRender = vi.fn<() => void>();
		renderer.setRenderRequestCallback(requestRender);
		const source = createPackedSource(['X']);
		const flags = source.styleFlags[0];
		if (!flags) throw new Error('Missing packed style row');
		flags[0] = TerminalStyleFlag.blink;
		renderer.renderPacked(source);

		vi.advanceTimersByTime(530);
		expect(requestRender).toHaveBeenCalled();
		renderer.setSuspended(true);
		requestRender.mockClear();
		vi.advanceTimersByTime(1_060);
		expect(requestRender).not.toHaveBeenCalled();
		renderer.dispose();
	});

	it('renders projected preedit text at the authoritative cursor', async () => {
		vi.stubGlobal('window', { devicePixelRatio: 1 });
		vi.stubGlobal('navigator', {});
		const texts: string[] = [];
		const fillText: CanvasRenderingContext2D['fillText'] = (text) => texts.push(text);
		vi.stubGlobal('document', { createElement: () => createCanvas(undefined, {}, fillText) });
		const renderer = new WebGPURenderer(createCanvas(undefined, {}, fillText));
		await renderer.initGPU();
		const source = createPackedSource(['  '], {
			cursor: { column: 0, row: 0, visible: true },
		});
		renderer.setPreedit([{ text: '你', width: 2 }]);

		renderer.renderPacked(source);

		expect(texts).toContain('你');
		renderer.dispose();
	});

	it('renders CJK, combining, and emoji graphemes once across packed wide-cell tails', async () => {
		vi.stubGlobal('window', { devicePixelRatio: 1 });
		vi.stubGlobal('navigator', {});
		const fillText = vi.fn<CanvasRenderingContext2D['fillText']>();
		vi.stubGlobal('document', {
			createElement: () => createCanvas(undefined, {}, fillText),
		});
		const renderer = new WebGPURenderer(createCanvas(undefined, {}, fillText));
		await renderer.initGPU();
		const source = createPackedGraphemeSource([
			{ text: '中', width: 2 },
			{ text: 'e\u0301', width: 1 },
			{ text: '👩‍💻', width: 2 },
		]);

		renderer.renderPacked(source);

		expect(fillText.mock.calls.map(([text]) => text)).toEqual(['中', 'e\u0301', '👩‍💻']);
		expect(renderer.getLastFrameTelemetry()).toMatchObject({
			cellsVisited: 3,
			glyphInstances: 3,
			shapingRuns: 0,
		});
		renderer.dispose();
	});

	it('anchors wide fallback graphemes to their Host cells and resumes shaping after tails', async () => {
		vi.stubGlobal('window', { devicePixelRatio: 1 });
		vi.stubGlobal('navigator', {});
		const fillText = vi.fn<CanvasRenderingContext2D['fillText']>();
		vi.stubGlobal('document', {
			createElement: () => createCanvas(undefined, {}, fillText),
		});
		const renderer = new WebGPURenderer(createCanvas(undefined, {}, fillText));
		await renderer.initGPU();

		renderer.renderPacked(
			createPackedGraphemeSource([
				{ text: '你', width: 2 },
				{ text: '好', width: 2 },
				{ text: '😀', width: 2 },
				{ text: 'e\u0301', width: 1 },
				{ text: 'A\u030a', width: 1 },
			]),
		);

		expect(fillText.mock.calls.map(([text]) => text)).toEqual(['你', '好', '😀', 'e\u0301A\u030a']);
		expect(renderer.getLastFrameTelemetry()).toMatchObject({
			cellsVisited: 5,
			glyphInstances: 4,
			shapingRuns: 1,
		});
		renderer.dispose();
	});

	it('renders a Canvas block cursor across the wide grapheme when the Host cursor is on its tail', async () => {
		vi.stubGlobal('window', { devicePixelRatio: 1 });
		vi.stubGlobal('navigator', {});
		const rectangles: Array<{
			readonly color: string;
			readonly width: number;
			readonly x: number;
		}> = [];
		const fillRect: CanvasRenderingContext2D['fillRect'] = function (
			this: CanvasRenderingContext2D,
			x,
			_y,
			width,
		) {
			rectangles.push({ color: String(this.fillStyle), width, x });
		};
		vi.stubGlobal('document', {
			createElement: () => createCanvas(undefined, {}, undefined, fillRect),
		});
		const renderer = new WebGPURenderer(createCanvas(undefined, {}, undefined, fillRect));
		await renderer.initGPU();
		renderer.setFocused(true);
		rectangles.length = 0;
		const source = createPackedGraphemeSource([{ text: '中', width: 2 }], {
			cursor: { column: 1, visible: true, wideTail: true },
		});

		renderer.renderPacked(source);

		expect(rectangles).toContainEqual({ color: 'rgb(216, 222, 233)', width: 16, x: 0 });
		renderer.dispose();
	});

	it('keeps a blinking Canvas cursor visible after movement until one idle blink period', async () => {
		vi.useFakeTimers();
		vi.stubGlobal('window', { devicePixelRatio: 1 });
		vi.stubGlobal('navigator', {});
		const rectangles: Array<{
			readonly color: string;
			readonly geometry: readonly number[];
		}> = [];
		const fillRect: CanvasRenderingContext2D['fillRect'] = function (
			this: CanvasRenderingContext2D,
			x,
			y,
			width,
			height,
		) {
			rectangles.push({ color: String(this.fillStyle), geometry: [x, y, width, height] });
		};
		vi.stubGlobal('document', {
			createElement: () => createCanvas(undefined, {}, undefined, fillRect),
		});
		const renderer = new WebGPURenderer(createCanvas(undefined, {}, undefined, fillRect));
		await renderer.initGPU();
		renderer.setFocused(true);
		const renderCursorAt = (column: number): boolean => {
			rectangles.length = 0;
			renderer.renderPacked(
				createPackedSource(['AB'], {
					cursor: { blinking: true, column, visible: true },
				}),
			);
			return rectangles.some(
				({ color, geometry }) =>
					color === 'rgb(216, 222, 233)' && geometry[0] === column * renderer.getMetrics().width,
			);
		};

		expect(renderCursorAt(0)).toBe(true);
		vi.advanceTimersByTime(530);
		expect(renderCursorAt(0)).toBe(false);

		expect(renderCursorAt(1)).toBe(true);
		vi.advanceTimersByTime(529);
		expect(renderCursorAt(1)).toBe(true);
		vi.advanceTimersByTime(1);
		expect(renderCursorAt(1)).toBe(false);
		renderer.dispose();
	});

	it('repaints the cursor row when a Canvas cursor becomes hidden', async () => {
		vi.stubGlobal('window', { devicePixelRatio: 1 });
		vi.stubGlobal('navigator', {});
		const clearRect = vi.fn<CanvasRenderingContext2D['clearRect']>();
		vi.stubGlobal('document', {
			createElement: () => createCanvas(undefined, {}, undefined, undefined, undefined, clearRect),
		});
		const renderer = new WebGPURenderer(
			createCanvas(undefined, {}, undefined, undefined, undefined, clearRect),
		);
		await renderer.initGPU();
		renderer.setFocused(true);
		const source = createPackedSource(['AB'], { cursor: { visible: true } });
		renderer.renderPacked(source);
		clearRect.mockClear();
		const metadata = source.metadata;
		if (metadata === null) throw new Error('packed source metadata is unavailable');
		Reflect.set(metadata.cursor, 'visible', false);

		renderer.renderPacked(source);

		expect(clearRect).toHaveBeenCalledWith(0, 0, 16, 17);
		renderer.dispose();
	});

	it('renders focused bar and underline cursors and an unfocused outline at one physical pixel', async () => {
		vi.stubGlobal('window', { devicePixelRatio: 2 });
		vi.stubGlobal('navigator', {});
		const rectangles: Array<{
			readonly color: string;
			readonly geometry: readonly number[];
		}> = [];
		const fillRect: CanvasRenderingContext2D['fillRect'] = function (
			this: CanvasRenderingContext2D,
			x,
			y,
			width,
			height,
		) {
			rectangles.push({ color: String(this.fillStyle), geometry: [x, y, width, height] });
		};
		vi.stubGlobal('document', {
			createElement: () => createCanvas(undefined, {}, undefined, fillRect),
		});
		const renderer = new WebGPURenderer(createCanvas(undefined, {}, undefined, fillRect), {
			devicePixelRatio: 2,
		});
		await renderer.initGPU();
		renderer.setFocused(true);

		renderer.renderPacked(
			createPackedSource(['AA'], {
				cursor: { column: 1, shape: 'bar', visible: true },
			}),
		);
		expect(rectangles).toContainEqual({
			color: 'rgb(216, 222, 233)',
			geometry: [7.5, 0, 0.5, 17],
		});

		rectangles.length = 0;
		renderer.renderPacked(
			createPackedSource(['AA'], {
				cursor: { column: 1, shape: 'underline', visible: true },
			}),
		);
		expect(rectangles).toContainEqual({
			color: 'rgb(216, 222, 233)',
			geometry: [8, 14, 8, 0.5],
		});

		rectangles.length = 0;
		renderer.setFocused(false);
		renderer.renderPacked(
			createPackedSource(['AA'], {
				cursor: { column: 1, shape: 'underline', visible: true },
			}),
		);
		const cursorRectangles = rectangles.filter((entry) => entry.color === 'rgb(216, 222, 233)');
		expect(cursorRectangles.map(({ geometry }) => geometry)).toEqual(
			expect.arrayContaining([
				[8, 0, 8, 0.5],
				[8, 16.5, 8, 0.5],
				[8, 0, 0.5, 17],
				[15.5, 0, 0.5, 17],
			]),
		);
		renderer.dispose();
	});

	it('repaints block-cursor text with the configured application accent', async () => {
		vi.stubGlobal('window', { devicePixelRatio: 1 });
		vi.stubGlobal('navigator', {});
		const textColors: string[] = [];
		const fillText: CanvasRenderingContext2D['fillText'] = function (
			this: CanvasRenderingContext2D,
		) {
			textColors.push(String(this.fillStyle));
		};
		vi.stubGlobal('document', {
			createElement: () => createCanvas(undefined, {}, fillText),
		});
		const renderer = new WebGPURenderer(createCanvas(undefined, {}, fillText), {
			theme: { cursorAccent: '#010203' },
		});
		await renderer.initGPU();
		renderer.setFocused(true);

		renderer.renderPacked(
			createPackedSource(['A'], {
				cursor: { column: 0, shape: 'block', visible: true },
			}),
		);

		expect(textColors).toContain('rgb(1, 2, 3)');
		renderer.dispose();
	});

	it('wraps a wide preedit at the right edge without changing its cells', () => {
		vi.stubGlobal('window', { devicePixelRatio: 1 });
		vi.stubGlobal('document', { createElement: () => createCanvas() });
		const renderer = new WebGPURenderer(createCanvas());
		const getPreeditLayout = Reflect.get(renderer, 'getPreeditLayout') as (
			cursor: { x: number; y: number },
			columns: number,
			rows: number,
		) => {
			readonly caret: { readonly column: number; readonly row: number };
			readonly rows: readonly {
				readonly cells: readonly { readonly text: string; readonly width: number }[];
				readonly end: number;
				readonly row: number;
				readonly start: number;
			}[];
		} | null;
		renderer.setPreedit([{ text: '가', width: 2 }]);

		expect(getPreeditLayout.call(renderer, { x: 9, y: 0 }, 10, 2)).toMatchObject({
			caret: { column: 2, row: 1 },
			rows: [{ cells: [{ text: '가', width: 2 }], end: 2, row: 1, start: 0 }],
		});
		renderer.dispose();
	});

	it('clips linear and rectangular selection overlays to visible packed cells', async () => {
		vi.stubGlobal('window', { devicePixelRatio: 1 });
		vi.stubGlobal('navigator', {});
		const selected: Array<readonly number[]> = [];
		const fillRect: CanvasRenderingContext2D['fillRect'] = function (
			this: CanvasRenderingContext2D,
			x,
			y,
			width,
			height,
		) {
			if (this.fillStyle === 'rgb(212, 212, 212)') selected.push([x, y, width, height]);
		};
		vi.stubGlobal('document', {
			createElement: () => createCanvas(undefined, {}, undefined, fillRect),
		});
		const renderer = new WebGPURenderer(createCanvas(undefined, {}, undefined, fillRect));
		await renderer.initGPU();
		const source = createPackedSource(['AAAA', 'BBBB', 'CCCC']);
		renderer.setProjectedViewportState(0, {
			startCol: 2,
			startRow: -1,
			endCol: 1,
			endRow: 1,
			rectangle: false,
		});

		renderer.renderPacked(source, true);
		expect(selected).toEqual([
			[0, 0, 8, 17],
			[8, 0, 8, 17],
			[16, 0, 8, 17],
			[24, 0, 8, 17],
			[0, 17, 8, 17],
			[8, 17, 8, 17],
		]);

		selected.length = 0;
		renderer.setProjectedViewportState(0, {
			startCol: 2,
			startRow: 2,
			endCol: 1,
			endRow: 0,
			rectangle: false,
		});
		renderer.renderPacked(source, true);
		expect(selected).toEqual([
			[8, 0, 8, 17],
			[16, 0, 8, 17],
			[24, 0, 8, 17],
			[0, 17, 8, 17],
			[8, 17, 8, 17],
			[16, 17, 8, 17],
			[24, 17, 8, 17],
			[0, 34, 8, 17],
			[8, 34, 8, 17],
			[16, 34, 8, 17],
		]);

		selected.length = 0;
		renderer.setProjectedViewportState(0, {
			startCol: 1,
			startRow: 0,
			endCol: 2,
			endRow: 2,
			rectangle: true,
		});
		source.dirtyRows.add(0);
		source.dirtyRows.add(1);
		source.dirtyRows.add(2);
		renderer.renderPacked(source, true);
		expect(selected).toEqual([
			[8, 0, 8, 17],
			[16, 0, 8, 17],
			[8, 17, 8, 17],
			[16, 17, 8, 17],
			[8, 34, 8, 17],
			[16, 34, 8, 17],
		]);

		selected.length = 0;
		renderer.setProjectedViewportState(0, {
			startCol: 2,
			startRow: 0,
			endCol: 1,
			endRow: 2,
			rectangle: true,
		});
		renderer.renderPacked(source, true);
		expect(selected).toEqual([
			[8, 0, 8, 17],
			[16, 0, 8, 17],
			[8, 17, 8, 17],
			[16, 17, 8, 17],
			[8, 34, 8, 17],
			[16, 34, 8, 17],
		]);
		renderer.dispose();
	});

	it('underlines every visible cell of a multi-row packed link range', async () => {
		vi.stubGlobal('window', { devicePixelRatio: 1 });
		vi.stubGlobal('navigator', {});
		const stroke = vi.fn<() => void>();
		vi.stubGlobal('document', {
			createElement: () => createCanvas(undefined, {}, undefined, undefined, stroke),
		});
		const renderer = new WebGPURenderer(createCanvas(undefined, {}, undefined, undefined, stroke));
		await renderer.initGPU();
		renderer.setHoveredLinkRange({ startX: 1, startY: 0, endX: 1, endY: 1 });

		renderer.renderPacked(createPackedSource(['AAAAA', 'BBBBB']), true);

		expect(stroke).toHaveBeenCalledTimes(6);
		renderer.dispose();
	});

	it('applies packed Ghostty text style and decoration flags together', async () => {
		vi.stubGlobal('window', { devicePixelRatio: 1 });
		vi.stubGlobal('navigator', {});
		const textPaints: Array<{ readonly alpha: number; readonly font: string }> = [];
		const decorationAlphas: number[] = [];
		const decorationFillAlphas: number[] = [];
		const fillText: CanvasRenderingContext2D['fillText'] = function (
			this: CanvasRenderingContext2D,
		) {
			textPaints.push({ alpha: this.globalAlpha, font: this.font });
		};
		const stroke: CanvasRenderingContext2D['stroke'] = function (this: CanvasRenderingContext2D) {
			decorationAlphas.push(this.globalAlpha);
		};
		const fillRect: CanvasRenderingContext2D['fillRect'] = function (
			this: CanvasRenderingContext2D,
		) {
			if (this.globalAlpha === 0.5) decorationFillAlphas.push(this.globalAlpha);
		};
		vi.stubGlobal('document', {
			createElement: () => createCanvas(undefined, {}, fillText, fillRect, stroke),
		});
		const renderer = new WebGPURenderer(createCanvas(undefined, {}, fillText, fillRect, stroke));
		await renderer.initGPU();
		const source = createPackedGraphemeSource([
			{
				flags:
					TerminalStyleFlag.bold |
					TerminalStyleFlag.faint |
					TerminalStyleFlag.italic |
					TerminalStyleFlag.overline |
					TerminalStyleFlag.strikethrough,
				text: 'A',
				underline: TerminalUnderlineKindCode.single,
				width: 1,
			},
		]);

		renderer.renderPacked(source);

		expect(textPaints).toEqual([{ alpha: 0.5, font: 'italic bold 15px monospace' }]);
		expect(decorationAlphas).toEqual([0.5]);
		expect(decorationFillAlphas).toEqual([0.5, 0.5]);
		renderer.dispose();
	});

	it('submits packed structure-of-arrays frames through WebGPU', async () => {
		vi.stubGlobal('window', { devicePixelRatio: 1 });
		vi.stubGlobal('document', { createElement: () => createCanvas() });
		vi.stubGlobal('GPUTextureUsage', { TEXTURE_BINDING: 1, COPY_DST: 2, RENDER_ATTACHMENT: 4 });
		vi.stubGlobal('GPUBufferUsage', { UNIFORM: 1, COPY_DST: 2, VERTEX: 4 });
		const rendering = createSubmittingDevice();
		vi.stubGlobal('navigator', {
			gpu: {
				requestAdapter: async () =>
					({ requestDevice: async () => rendering.device }) as unknown as GPUAdapter,
				getPreferredCanvasFormat: () => 'bgra8unorm',
			},
		});
		const context = {
			configure: vi.fn<(configuration: GPUCanvasConfiguration) => void>(),
			getCurrentTexture: () => ({ createView: () => ({}) }),
		} as unknown as GPUCanvasContext;
		const renderer = new WebGPURenderer(createWebGPUCanvas(context));
		const telemetry = vi.fn<(frame: RendererFrameTelemetry) => void>();
		renderer.setFrameTelemetryCallback(telemetry);
		await expect(renderer.initGPU()).resolves.toBe(true);
		const source = createPackedSource(['A']);

		renderer.renderPacked(source);

		expect(rendering.submit).toHaveBeenCalledOnce();
		expect(rendering.writeBuffer).toHaveBeenCalled();
		expect(telemetry).toHaveBeenCalledWith(
			expect.objectContaining({ backend: 'webgpu', cellsVisited: 1, rowsRendered: 1 }),
		);
		renderer.dispose();
	});

	it('reuses bounded WebGPU vertex staging storage across sustained output frames', async () => {
		vi.stubGlobal('window', { devicePixelRatio: 1 });
		vi.stubGlobal('document', { createElement: () => createCanvas() });
		vi.stubGlobal('GPUTextureUsage', { TEXTURE_BINDING: 1, COPY_DST: 2, RENDER_ATTACHMENT: 4 });
		vi.stubGlobal('GPUBufferUsage', { UNIFORM: 1, COPY_DST: 2, VERTEX: 4 });
		const rendering = createSubmittingDevice();
		vi.stubGlobal('navigator', {
			gpu: {
				requestAdapter: async () =>
					({ requestDevice: async () => rendering.device }) as unknown as GPUAdapter,
				getPreferredCanvasFormat: () => 'bgra8unorm',
			},
		});
		const context = {
			configure: vi.fn<(configuration: GPUCanvasConfiguration) => void>(),
			getCurrentTexture: () => ({ createView: () => ({}) }),
		} as unknown as GPUCanvasContext;
		const renderer = new WebGPURenderer(createWebGPUCanvas(context));
		await renderer.initGPU();
		const source = createPackedSource(['sustained output']);

		for (let frame = 0; frame < 8; frame++) renderer.renderPacked(source, true);

		const textUploads = rendering.writeBuffer.mock.calls.flatMap((call) => {
			const value = call[2];
			return value instanceof Float32Array && value.length > 0 && value.length % 13 === 0
				? [value]
				: [];
		});
		expect(textUploads).toHaveLength(8);
		expect(new Set(textUploads.map((upload) => upload.buffer))).toHaveLength(1);
		renderer.dispose();
	});

	it('reuses one cell-color projection scratch across sustained output cells', () => {
		vi.stubGlobal('window', { devicePixelRatio: 1 });
		vi.stubGlobal('document', { createElement: () => createCanvas() });
		const renderer = new WebGPURenderer(createCanvas());
		const source = createPackedSource(['AB']);
		const row = source.rows[0];
		const metadata = source.metadata;
		if (row === undefined || metadata === null) throw new Error('Missing packed color fixture');
		const presentation = resolvePackedPresentationColors(metadata.defaultColors, DEFAULT_THEME);
		const projectCell = Reflect.get(renderer, 'packedCellPresentation') as (
			row: PackedRendererRow,
			column: number,
			metadata: PackedRendererMetadata,
			presentation: ReturnType<typeof resolvePackedPresentationColors>,
		) => {
			readonly background: readonly [number, number, number];
			readonly foreground: readonly [number, number, number];
		};

		const first = projectCell.call(renderer, row, 0, metadata, presentation);
		const firstBackground = first.background;
		const firstForeground = first.foreground;
		const second = projectCell.call(renderer, row, 1, metadata, presentation);

		expect(second).toBe(first);
		expect(second.background).toBe(firstBackground);
		expect(second.foreground).toBe(firstForeground);
		renderer.dispose();
	});

	it('applies the default-only display override through WebGPU with opaque cell projections', async () => {
		vi.stubGlobal('window', { devicePixelRatio: 1 });
		vi.stubGlobal('document', { createElement: () => createCanvas() });
		vi.stubGlobal('GPUTextureUsage', { TEXTURE_BINDING: 1, COPY_DST: 2, RENDER_ATTACHMENT: 4 });
		vi.stubGlobal('GPUBufferUsage', { UNIFORM: 1, COPY_DST: 2, VERTEX: 4 });
		const rendering = createSubmittingDevice();
		vi.stubGlobal('navigator', {
			gpu: {
				requestAdapter: async () =>
					({ requestDevice: async () => rendering.device }) as unknown as GPUAdapter,
				getPreferredCanvasFormat: () => 'bgra8unorm',
			},
		});
		const context = {
			configure: vi.fn<(configuration: GPUCanvasConfiguration) => void>(),
			getCurrentTexture: () => ({ createView: () => ({}) }),
		} as unknown as GPUCanvasContext;
		const renderer = new WebGPURenderer(createWebGPUCanvas(context), {
			backgroundOpacity: 0,
			defaultColorOverride: { background: '#102030', foreground: '#ddeeff' },
		});
		await renderer.initGPU();
		renderer.setFocused(true);
		const source = createPackedSource(['ABCDE'], {
			cursor: { column: 4, visible: true },
			defaultColors: {
				backgroundRgb: 0x11_22_33,
				cursorRgb: 0xaa_bb_cc,
				foregroundRgb: 0x22_33_44,
			},
		});
		const row = source.rows[0];
		if (row === undefined || source.metadata === null) {
			throw new Error('default-color fixture did not create one packed row and metadata');
		}
		row.foregroundColors[1] = 0;
		row.backgroundColors[1] = 0x0a_0b_0c;
		row.foregroundColors[2] = TerminalColorEncoding.none;
		row.backgroundColors[2] = TerminalColorEncoding.none;
		row.styleFlags[3] = TerminalStyleFlag.inverse;
		row.underlineKinds[0] = TerminalUnderlineKindCode.single;
		row.underlineColors[0] = TerminalColorEncoding.default;
		const hostDefaultColors = { ...source.metadata.defaultColors };

		renderer.renderPacked(source);

		expect(rendering.beginRenderPass.mock.calls.at(-1)?.[0].colorAttachments[0]).toMatchObject({
			clearValue: {
				a: 1,
				b: 48 / 255,
				g: 32 / 255,
				r: 16 / 255,
			},
		});
		const rectangles = rendering.writeBuffer.mock.calls.flatMap((call) => {
			const value = call[2];
			if (!(value instanceof Float32Array) || value.length === 0 || value.length % 8 !== 0) {
				return [];
			}
			return Array.from({ length: value.length / 8 }, (_unused, index) =>
				Array.from(value.subarray(index * 8, (index + 1) * 8)),
			);
		});
		const textInstances = rendering.writeBuffer.mock.calls.flatMap((call) => {
			const value = call[2];
			if (!(value instanceof Float32Array) || value.length === 0 || value.length % 13 !== 0) {
				return [];
			}
			return Array.from({ length: value.length / 13 }, (_unused, index) =>
				Array.from(value.subarray(index * 13, (index + 1) * 13)),
			);
		});
		const includesOpaqueRectangle = (
			x: number,
			color: readonly [number, number, number],
		): boolean =>
			rectangles.some(
				(instance) =>
					instance[0] === x &&
					Math.abs((instance[4] ?? -1) - color[0] / 255) < 0.0001 &&
					Math.abs((instance[5] ?? -1) - color[1] / 255) < 0.0001 &&
					Math.abs((instance[6] ?? -1) - color[2] / 255) < 0.0001 &&
					instance[7] === 1,
			);
		const includesOpaqueText = (color: readonly [number, number, number]): boolean =>
			textInstances.some(
				(instance) =>
					Math.abs((instance[8] ?? -1) - color[0] / 255) < 0.0001 &&
					Math.abs((instance[9] ?? -1) - color[1] / 255) < 0.0001 &&
					Math.abs((instance[10] ?? -1) - color[2] / 255) < 0.0001 &&
					instance[11] === 1,
			);
		expect(includesOpaqueRectangle(0, [221, 238, 255])).toBe(true);
		expect(includesOpaqueRectangle(8, [10, 11, 12])).toBe(true);
		expect(includesOpaqueRectangle(16, [17, 34, 51])).toBe(false);
		expect(includesOpaqueRectangle(24, [221, 238, 255])).toBe(true);
		expect(includesOpaqueRectangle(32, [170, 187, 204])).toBe(true);
		expect(includesOpaqueText([221, 238, 255])).toBe(true);
		expect(includesOpaqueText([0, 0, 0])).toBe(true);
		expect(includesOpaqueText([34, 51, 68])).toBe(true);
		expect(includesOpaqueText([16, 32, 48])).toBe(true);
		expect(source.metadata.defaultColors).toEqual(hostDefaultColors);
		renderer.dispose();
	});

	it('keeps a shaped run at the same horizontal origin when selection recolors it', async () => {
		vi.stubGlobal('window', { devicePixelRatio: 2 });
		vi.stubGlobal('document', {
			createElement: () =>
				createCanvas(undefined, (text) => {
					const width = text.length * 15.6;
					let left = 0;
					if (text === 'Added OS') left = 1.5;
					else if (text === 'Ad') left = 0.5;
					return {
						actualBoundingBoxLeft: left,
						actualBoundingBoxRight: Math.max(1, width - left),
						width,
					};
				}),
		});
		vi.stubGlobal('GPUTextureUsage', { TEXTURE_BINDING: 1, COPY_DST: 2, RENDER_ATTACHMENT: 4 });
		vi.stubGlobal('GPUBufferUsage', { UNIFORM: 1, COPY_DST: 2, VERTEX: 4 });
		const rendering = createSubmittingDevice();
		vi.stubGlobal('navigator', {
			gpu: {
				requestAdapter: async () =>
					({ requestDevice: async () => rendering.device }) as unknown as GPUAdapter,
				getPreferredCanvasFormat: () => 'bgra8unorm',
			},
		});
		const context = {
			configure: vi.fn<(configuration: GPUCanvasConfiguration) => void>(),
			getCurrentTexture: () => ({ createView: () => ({}) }),
		} as unknown as GPUCanvasContext;
		const renderer = new WebGPURenderer(createWebGPUCanvas(context), { devicePixelRatio: 2 });
		await renderer.initGPU();
		const source = createPackedSource(['Added OS']);

		rendering.writeBuffer.mockClear();
		renderer.renderPacked(source);
		const unselectedText = rendering.writeBuffer.mock.calls.flatMap((call) => {
			const value = call[2];
			return value instanceof Float32Array && value.length === 13 ? [Array.from(value)] : [];
		})[0];
		expect(unselectedText).toBeDefined();

		rendering.writeBuffer.mockClear();
		renderer.setProjectedViewportState(0, {
			endCol: 1,
			endRow: 0,
			rectangle: false,
			startCol: 0,
			startRow: 0,
		});
		renderer.renderPacked(source, true);
		const selectedText = rendering.writeBuffer.mock.calls.flatMap((call) => {
			const value = call[2];
			return value instanceof Float32Array && value.length === 26 ? [Array.from(value)] : [];
		})[0];
		expect(selectedText).toBeDefined();
		expect(selectedText?.[0]).toBe(unselectedText?.[0]);
		expect(selectedText?.[4]).toBe(unselectedText?.[4]);
		expect(selectedText?.[8]).toBeCloseTo(30 / 255, 6);
		expect(selectedText?.[21]).toBeCloseTo(216 / 255, 6);
		renderer.dispose();
	});

	it('turns over a saturated shaping-run cache only when a new run is needed', async () => {
		vi.stubGlobal('window', { devicePixelRatio: 1 });
		vi.stubGlobal('document', { createElement: () => createCanvas() });
		vi.stubGlobal('GPUTextureUsage', { TEXTURE_BINDING: 1, COPY_DST: 2, RENDER_ATTACHMENT: 4 });
		vi.stubGlobal('GPUBufferUsage', { UNIFORM: 1, COPY_DST: 2, VERTEX: 4 });
		const rendering = createSubmittingDevice();
		vi.stubGlobal('navigator', {
			gpu: {
				requestAdapter: async () =>
					({ requestDevice: async () => rendering.device }) as unknown as GPUAdapter,
				getPreferredCanvasFormat: () => 'bgra8unorm',
			},
		});
		const context = {
			configure: () => {},
			getCurrentTexture: () => ({ createView: () => ({}) }),
		} as unknown as GPUCanvasContext;
		const renderer = new WebGPURenderer(createWebGPUCanvas(context));
		await renderer.initGPU();
		renderer.renderPacked(createPackedSource(['AB']));
		Reflect.set(renderer, 'shapingRasterPixels', Number.MAX_SAFE_INTEGER);
		const resetAtlas = vi.spyOn(renderer as unknown as { resetAtlas(): void }, 'resetAtlas');

		renderer.renderPacked(createPackedSource(['AB']));
		expect(resetAtlas).not.toHaveBeenCalled();

		renderer.renderPacked(createPackedSource(['CD']));
		expect(resetAtlas).toHaveBeenCalledOnce();
		renderer.dispose();
	});

	it('reconfigures WebGPU and recreates its atlas for a clean device-pixel transition', async () => {
		vi.stubGlobal('window', { devicePixelRatio: 2 });
		vi.stubGlobal('document', { createElement: () => createCanvas() });
		vi.stubGlobal('GPUTextureUsage', { TEXTURE_BINDING: 1, COPY_DST: 2, RENDER_ATTACHMENT: 4 });
		vi.stubGlobal('GPUBufferUsage', { UNIFORM: 1, COPY_DST: 2, VERTEX: 4 });
		const rendering = createSubmittingDevice();
		vi.stubGlobal('navigator', {
			gpu: {
				requestAdapter: async () =>
					({ requestDevice: async () => rendering.device }) as unknown as GPUAdapter,
				getPreferredCanvasFormat: () => 'bgra8unorm',
			},
		});
		const configure = vi.fn<(configuration: GPUCanvasConfiguration) => void>();
		const context = {
			configure,
			getCurrentTexture: () => ({ createView: () => ({}) }),
		} as unknown as GPUCanvasContext;
		const canvas = createWebGPUCanvas(context);
		const renderer = new WebGPURenderer(canvas, { devicePixelRatio: 2 });
		const telemetry = vi.fn<(frame: RendererFrameTelemetry) => void>();
		const requestRender = vi.fn<() => void>();
		renderer.setFrameTelemetryCallback(telemetry);
		renderer.setRenderRequestCallback(requestRender);
		await renderer.initGPU();
		const source = createPackedSource(['AB', 'CD']);
		renderer.renderPacked(source);
		const measureFont = vi.spyOn(renderer as unknown as { measureFont(): unknown }, 'measureFont');
		const resetAtlas = vi.spyOn(renderer as unknown as { resetAtlas(): void }, 'resetAtlas');
		const resize = vi.spyOn(renderer, 'resize');
		configure.mockClear();
		requestRender.mockClear();
		telemetry.mockClear();

		expect([canvas.width, canvas.height]).toEqual([32, 68]);
		expect(renderer.setDevicePixelRatio(1.5)).toBe(true);
		expect(measureFont).toHaveBeenCalledOnce();
		expect(resetAtlas).toHaveBeenCalledOnce();
		expect(resize).toHaveBeenCalledOnce();
		expect(requestRender).toHaveBeenCalledOnce();
		expect(configure).toHaveBeenCalledOnce();
		expect(canvas.style).toMatchObject({ height: '34px', width: '16px' });
		expect([canvas.width, canvas.height]).toEqual([24, 51]);

		renderer.renderPacked(source);
		expect(telemetry).toHaveBeenCalledWith(
			expect.objectContaining({
				atlasGlyphCount: 2,
				atlasGlyphsCreated: 2,
				backend: 'webgpu',
				cellsVisited: 4,
				forceAll: true,
				reasons: ['surface'],
				rowsRendered: 2,
			}),
		);

		measureFont.mockClear();
		resetAtlas.mockClear();
		resize.mockClear();
		configure.mockClear();
		requestRender.mockClear();
		telemetry.mockClear();
		expect(renderer.setDevicePixelRatio(1.5)).toBe(false);
		expect(measureFont).not.toHaveBeenCalled();
		expect(resetAtlas).not.toHaveBeenCalled();
		expect(resize).not.toHaveBeenCalled();
		expect(configure).not.toHaveBeenCalled();
		expect(requestRender).not.toHaveBeenCalled();
		renderer.renderPacked(source);
		expect(telemetry).toHaveBeenCalledWith(
			expect.objectContaining({
				atlasGlyphCount: 2,
				atlasGlyphsCreated: 0,
				backend: 'webgpu',
				forceAll: false,
				reasons: ['dirty'],
			}),
		);
		renderer.dispose();
	});

	it('publishes the same complete runtime presentation through WebGPU', async () => {
		vi.stubGlobal('window', { devicePixelRatio: 2 });
		vi.stubGlobal('document', { createElement: () => createCanvas() });
		vi.stubGlobal('GPUTextureUsage', { TEXTURE_BINDING: 1, COPY_DST: 2, RENDER_ATTACHMENT: 4 });
		vi.stubGlobal('GPUBufferUsage', { UNIFORM: 1, COPY_DST: 2, VERTEX: 4 });
		const rendering = createSubmittingDevice();
		vi.stubGlobal('navigator', {
			gpu: {
				requestAdapter: async () =>
					({ requestDevice: async () => rendering.device }) as unknown as GPUAdapter,
				getPreferredCanvasFormat: () => 'bgra8unorm',
			},
		});
		const configure = vi.fn<(configuration: GPUCanvasConfiguration) => void>();
		const context = {
			configure,
			getCurrentTexture: () => ({ createView: () => ({}) }),
		} as unknown as GPUCanvasContext;
		const canvas = createWebGPUCanvas(context);
		const renderer = new WebGPURenderer(canvas);
		const telemetry = vi.fn<(frame: RendererFrameTelemetry) => void>();
		const requestRender = vi.fn<() => void>();
		renderer.setFrameTelemetryCallback(telemetry);
		renderer.setRenderRequestCallback(requestRender);
		await renderer.initGPU();
		renderer.resize(2, 1);
		const source = createPackedSource(['AB'], { cursor: { visible: true } });
		renderer.setProjectedViewportState(0, {
			endCol: 1,
			endRow: 0,
			rectangle: false,
			startCol: 0,
			startRow: 0,
		});
		renderer.renderPacked(source);
		const measureFont = vi.spyOn(renderer as unknown as { measureFont(): unknown }, 'measureFont');
		const resetAtlas = vi.spyOn(renderer as unknown as { resetAtlas(): void }, 'resetAtlas');
		requestRender.mockClear();
		telemetry.mockClear();
		configure.mockClear();

		const prepared = renderer.preparePresentationConfiguration({
			...DEFAULT_TERMINAL_PRESENTATION_CONFIGURATION,
			allowTransparency: true,
			fontFamily: 'Runtime WebGPU Mono',
			fontLigatures: false,
			fontMetricModifiers: { cellHeight: 3, cellWidth: 2 },
			fontSize: 18,
			fontWeight: 450,
			fontWeightBold: 700,
			paddingBalance: false,
		});
		renderer.commitPresentationConfiguration(prepared);

		expect(measureFont).toHaveBeenCalledOnce();
		expect(resetAtlas).toHaveBeenCalledOnce();
		expect(requestRender).toHaveBeenCalledOnce();
		expect(configure).toHaveBeenCalledOnce();
		expect(canvas.style).toMatchObject({ height: '20px', width: '20px' });
		expect(canvas.width).toBe(40);
		expect(canvas.height).toBe(40);
		expect(Reflect.get(renderer, 'projectedSelectionCoords')).toEqual({
			endCol: 1,
			endRow: 0,
			rectangle: false,
			startCol: 0,
			startRow: 0,
		});
		renderer.renderPacked(source);
		expect(telemetry).toHaveBeenCalledWith(
			expect.objectContaining({
				backend: 'webgpu',
				forceAll: true,
				reasons: expect.arrayContaining(['font', 'surface']),
			}),
		);
		expect(telemetry.mock.calls[0]?.[0].overlayInstances).toBeGreaterThan(0);
		expect(rendering.beginRenderPass.mock.calls.at(-1)?.[0].colorAttachments[0]).toMatchObject({
			clearValue: { a: 0, b: 0, g: 0, r: 0 },
		});
		renderer.dispose();
	});

	it('submits WebGPU cursor geometry for the complete wide grapheme from a tail cursor', async () => {
		vi.stubGlobal('window', { devicePixelRatio: 1 });
		vi.stubGlobal('document', { createElement: () => createCanvas() });
		vi.stubGlobal('GPUTextureUsage', { TEXTURE_BINDING: 1, COPY_DST: 2, RENDER_ATTACHMENT: 4 });
		vi.stubGlobal('GPUBufferUsage', { UNIFORM: 1, COPY_DST: 2, VERTEX: 4 });
		const rendering = createSubmittingDevice();
		vi.stubGlobal('navigator', {
			gpu: {
				requestAdapter: async () =>
					({ requestDevice: async () => rendering.device }) as unknown as GPUAdapter,
				getPreferredCanvasFormat: () => 'bgra8unorm',
			},
		});
		const context = {
			configure: vi.fn<(configuration: GPUCanvasConfiguration) => void>(),
			getCurrentTexture: () => ({ createView: () => ({}) }),
		} as unknown as GPUCanvasContext;
		const renderer = new WebGPURenderer(createWebGPUCanvas(context));
		await renderer.initGPU();
		renderer.setFocused(true);
		rendering.writeBuffer.mockClear();
		const source = createPackedGraphemeSource([{ text: '中', width: 2 }], {
			cursor: { column: 1, visible: true, wideTail: true },
		});

		renderer.renderPacked(source);

		const uploadedRectangles = rendering.writeBuffer.mock.calls.flatMap((call) => {
			const value = call[2];
			return value instanceof Float32Array && value.length === 8 ? [Array.from(value)] : [];
		});
		expect(uploadedRectangles.some((value) => value[0] === 0 && value[2] === 16)).toBe(true);
		renderer.dispose();
	});

	it('reports WebGPU device loss and recovers once', async () => {
		vi.stubGlobal('window', { devicePixelRatio: 1 });
		vi.stubGlobal('document', { createElement: () => createCanvas() });
		vi.stubGlobal('GPUTextureUsage', { TEXTURE_BINDING: 1, COPY_DST: 2, RENDER_ATTACHMENT: 4 });
		vi.stubGlobal('GPUBufferUsage', { UNIFORM: 1, COPY_DST: 2, VERTEX: 4 });
		const first = createControllableDevice();
		const second = createControllableDevice();
		const requestDevice = vi
			.fn<() => Promise<GPUDevice>>()
			.mockResolvedValueOnce(first.device)
			.mockResolvedValueOnce(second.device);
		vi.stubGlobal('navigator', {
			gpu: {
				requestAdapter: async () => ({ requestDevice }) as unknown as GPUAdapter,
				getPreferredCanvasFormat: () => 'bgra8unorm',
			},
		});
		const context = {
			configure: vi.fn<GPUCanvasContext['configure']>(),
		} as unknown as GPUCanvasContext;
		const renderer = new WebGPURenderer(createWebGPUCanvas(context));
		const health = vi.fn<(event: { readonly state: string }) => void>();
		renderer.setHealthChangeCallback(health);
		await renderer.initGPU();

		first.lose({ reason: 'unknown', message: 'simulated device loss' });
		await vi.waitFor(() => expect(requestDevice).toHaveBeenCalledTimes(2));
		expect(health).toHaveBeenCalledWith({ state: 'recovering', backend: 'webgpu', attempt: 1 });
		expect(health.mock.calls.at(-1)?.[0]).toEqual({ state: 'ready', backend: 'webgpu' });
		renderer.dispose();
	});
});

function createWebGPUCanvas(context: GPUCanvasContext): HTMLCanvasElement {
	const canvas = createCanvas();
	const getContext = canvas.getContext.bind(canvas);
	return {
		...canvas,
		getContext: ((kind: string, options?: unknown) =>
			kind === 'webgpu' ? context : getContext(kind, options)) as HTMLCanvasElement['getContext'],
	} as HTMLCanvasElement;
}

function createControllableDevice(): {
	readonly device: GPUDevice;
	lose(info: { readonly reason: GPUDeviceLostReason; readonly message: string }): void;
} {
	let resolveLost: ((info: GPUDeviceLostInfo) => void) | undefined;
	const lost = new Promise<GPUDeviceLostInfo>((resolve) => {
		resolveLost = resolve;
	});
	const destroy = (): void =>
		resolveLost?.({ reason: 'destroyed', message: 'destroyed' } as GPUDeviceLostInfo);
	const device = {
		lost,
		destroy,
		pushErrorScope: () => {},
		popErrorScope: async () => null,
		createShaderModule: () => ({}),
		createRenderPipeline: () => ({ getBindGroupLayout: () => ({}) }),
		createTexture: () => ({ destroy: () => {}, createView: () => ({}) }),
		createSampler: () => ({}),
		createBuffer: () => ({ destroy: () => {} }),
		limits: { maxTextureDimension2D: 8_192 },
		queue: {
			copyExternalImageToTexture: () => {},
			writeTexture: () => {},
			writeBuffer: () => {},
		},
	} as unknown as GPUDevice;
	return {
		device,
		lose: (info) => resolveLost?.(info as GPUDeviceLostInfo),
	};
}

function createSubmittingDevice(): {
	readonly beginRenderPass: ReturnType<
		typeof vi.fn<(descriptor: GPURenderPassDescriptor) => GPURenderPassEncoder>
	>;
	readonly device: GPUDevice;
	readonly submit: ReturnType<typeof vi.fn<GPUQueue['submit']>>;
	readonly writeBuffer: ReturnType<typeof vi.fn<GPUQueue['writeBuffer']>>;
} {
	const submit = vi.fn<GPUQueue['submit']>();
	const writeBuffer = vi.fn<GPUQueue['writeBuffer']>();
	const renderPass = {
		draw: () => {},
		end: () => {},
		setBindGroup: () => {},
		setPipeline: () => {},
		setVertexBuffer: () => {},
	};
	const beginRenderPass = vi.fn<(descriptor: GPURenderPassDescriptor) => GPURenderPassEncoder>(
		() => renderPass as unknown as GPURenderPassEncoder,
	);
	const queue = {
		copyExternalImageToTexture: () => {},
		submit,
		writeBuffer,
		writeTexture: () => {},
	};
	const device = {
		createBindGroup: () => ({}),
		createBuffer: () => ({ destroy: () => {} }),
		createCommandEncoder: () => ({
			beginRenderPass,
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
		queue,
	} as unknown as GPUDevice;
	return { beginRenderPass, device, submit, writeBuffer };
}
