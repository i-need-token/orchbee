import {
	TerminalCellWidthCode,
	TerminalColorEncoding,
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
	textMetrics: Partial<TextMetrics> = {},
	fillText: CanvasRenderingContext2D['fillText'] = () => {},
	fillRect: CanvasRenderingContext2D['fillRect'] = () => {},
	stroke: CanvasRenderingContext2D['stroke'] = () => {},
	clearRect: CanvasRenderingContext2D['clearRect'] = () => {},
	drawImage: AtlasDrawImage = () => {},
): HTMLCanvasElement {
	const context = {
		clearRect,
		drawImage,
		font: '10px sans-serif',
		globalAlpha: 1,
		imageSmoothingEnabled: true,
		measureText: () =>
			({
				width: 7.8,
				actualBoundingBoxAscent: 9.5,
				actualBoundingBoxDescent: 0,
				fontBoundingBoxAscent: 13,
				fontBoundingBoxDescent: 4,
				...textMetrics,
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
		style: {},
		getContext: () => context,
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

describe('projected terminal renderer', () => {
	it('resolves Host terminal colors and application overlay colors without a second palette', () => {
		const theme = {
			...DEFAULT_THEME,
			background: '#abcdef',
			cursor: '#fedcba',
			foreground: '#123456',
			selectionBackground: '#112233',
			selectionForeground: '#ddeeff',
		};
		const colors = resolvePackedPresentationColors(
			{ backgroundRgb: 0x10_14_18, cursorRgb: 0xf5_a9_7f, foregroundRgb: 0xd8_de_e9 },
			theme,
		);

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

	it('draws only the projected scrollbar thumb in Canvas with exact geometry and opacity', async () => {
		vi.stubGlobal('window', { devicePixelRatio: 1 });
		vi.stubGlobal('navigator', {});
		const rectangles: Array<{
			readonly color: string;
			readonly geometry: readonly number[];
		}> = [];
		const clearRect = vi.fn<CanvasRenderingContext2D['clearRect']>();
		const fillRect: CanvasRenderingContext2D['fillRect'] = function (
			this: CanvasRenderingContext2D,
			x,
			y,
			width,
			height,
		) {
			if (String(this.fillStyle).startsWith('rgba(128, 128, 128')) {
				rectangles.push({ color: String(this.fillStyle), geometry: [x, y, width, height] });
			}
		};
		vi.stubGlobal('document', {
			createElement: () => createCanvas(undefined, {}, undefined, fillRect, undefined, clearRect),
		});
		const renderer = new WebGPURenderer(
			createCanvas(undefined, {}, undefined, fillRect, undefined, clearRect),
		);
		await renderer.initGPU();
		const source = createPackedSource(['..........', '..........', '..........', '..........']);
		renderer.setHistoryScrollbar({
			opacity: 1,
			totalRows: 20n,
			visibleRows: 4,
			visibleStartRow: 8n,
		});

		renderer.renderPacked(source);

		expect(rectangles).toEqual([{ color: 'rgba(128, 128, 128, 0.5)', geometry: [68, 24, 8, 20] }]);
		expect(renderer.getLastFrameTelemetry()?.reasons).toContain('scrollbar');
		clearRect.mockClear();
		rectangles.length = 0;
		renderer.renderPacked(source);
		expect(clearRect.mock.calls).toEqual([
			[0, 17, 80, 17],
			[0, 34, 80, 17],
		]);
		expect(rectangles).toEqual([{ color: 'rgba(128, 128, 128, 0.5)', geometry: [68, 24, 8, 20] }]);

		rectangles.length = 0;
		renderer.setHistoryScrollbar({
			opacity: 0.5,
			totalRows: 20n,
			visibleRows: 4,
			visibleStartRow: 16n,
		});
		renderer.renderPacked(source);
		expect(rectangles).toEqual([{ color: 'rgba(128, 128, 128, 0.15)', geometry: [68, 44, 8, 20] }]);
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

		expect(fillText.mock.calls.map(([text]) => text)).toEqual(['中e\u0301', '👩‍💻']);
		expect(renderer.getLastFrameTelemetry()).toMatchObject({
			cellsVisited: 3,
			glyphInstances: 2,
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

	it('shifts a wide preedit left at the right edge without changing its cells', () => {
		vi.stubGlobal('window', { devicePixelRatio: 1 });
		vi.stubGlobal('document', { createElement: () => createCanvas() });
		const renderer = new WebGPURenderer(createCanvas());
		const getPreeditLayout = Reflect.get(renderer, 'getPreeditLayout') as (
			cursor: { x: number; y: number },
			columns: number,
		) => {
			readonly cells: readonly { readonly text: string; readonly width: number }[];
			readonly end: number;
			readonly start: number;
		} | null;
		renderer.setPreedit([{ text: '가', width: 2 }]);

		expect(getPreeditLayout.call(renderer, { x: 9, y: 0 }, 10)).toMatchObject({
			cells: [{ text: '가', width: 2 }],
			end: 10,
			start: 8,
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

	it('uses the same thumb-only scrollbar projection for WebGPU', async () => {
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
		rendering.writeBuffer.mockClear();
		renderer.setHistoryScrollbar({
			opacity: 1,
			totalRows: 20n,
			visibleRows: 4,
			visibleStartRow: 8n,
		});

		renderer.renderPacked(
			createPackedSource(['..........', '..........', '..........', '..........']),
		);

		const uploadedRectangles = rendering.writeBuffer.mock.calls.flatMap((call) => {
			const value = call[2];
			return value instanceof Float32Array && value.length === 8 ? [Array.from(value)] : [];
		});
		expect(uploadedRectangles).toHaveLength(1);
		expect(uploadedRectangles[0]?.slice(0, 4)).toEqual([68, 24, 8, 20]);
		for (const channel of uploadedRectangles[0]?.slice(4, 7) ?? []) {
			expect(channel).toBeCloseTo(128 / 255, 6);
		}
		expect(uploadedRectangles[0]?.[7]).toBe(0.5);
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
