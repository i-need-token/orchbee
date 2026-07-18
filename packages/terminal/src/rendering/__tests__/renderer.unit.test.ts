import { afterEach, describe, expect, it, vi } from 'vitest';
import type { FontWeight } from '../../index';
import { CellFlags, type GhosttyCell, type IRenderable } from '../../types';
import { type RendererFrameTelemetry, WebGPURenderer } from '../renderer';

type AtlasDrawImage = (image: CanvasImageSource, dx: number, dy: number) => void;

const failingFillText: CanvasRenderingContext2D['fillText'] = () => {
	throw new Error('simulated raster failure');
};

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
	scale: CanvasRenderingContext2D['scale'] = () => {},
	translate: CanvasRenderingContext2D['translate'] = () => {},
): HTMLCanvasElement {
	const context = {
		clearRect,
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
		scale,
		translate,
		rect: () => {},
		clip: () => {},
		fillRect,
		fillText,
		beginPath: () => {},
		moveTo: () => {},
		lineTo: () => {},
		stroke,
		setLineDash: () => {},
		drawImage,
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

describe('WebGPURenderer font metrics', () => {
	it('uses independently configurable regular and bold font weights', () => {
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
		ensureGlyph.call(renderer, 'B', CellFlags.ITALIC, 1);
		expect(atlasContext.font).toBe('italic 225 26px JetBrains Mono');
		ensureGlyph.call(renderer, 'C', CellFlags.BOLD, 1);
		expect(atlasContext.font).toBe('575 26px JetBrains Mono');

		const regularWeight: FontWeight = 250;
		const boldWeight: FontWeight = 600;
		renderer.setFontWeight(regularWeight);
		ensureGlyph.call(renderer, 'D', 0, 1);
		expect(atlasContext.font).toBe('250 26px JetBrains Mono');
		renderer.setFontWeightBold(boldWeight);
		ensureGlyph.call(renderer, 'E', CellFlags.BOLD | CellFlags.ITALIC, 1);
		expect(atlasContext.font).toBe('italic 600 26px JetBrains Mono');
		renderer.dispose();
	});

	it('uses full font bounds and preserves device pixel ratio', () => {
		vi.stubGlobal('window', { devicePixelRatio: 2 });
		vi.stubGlobal('document', { createElement: () => createCanvas() });
		const renderer = new WebGPURenderer(createCanvas(), {
			fontFamily: 'JetBrains Mono',
			fontSize: 13,
		});
		expect(renderer.getMetrics()).toEqual({ width: 8, height: 17, baseline: 13 });
		expect(renderer.getPalette()).toHaveLength(16);
		expect(renderer.charWidth).toBe(8);
		expect(renderer.charHeight).toBe(17);
		const requestRender = vi.fn<() => void>();
		renderer.setRenderRequestCallback(requestRender);
		renderer.setPreedit([{ text: '你', width: 2 }]);
		expect(requestRender).toHaveBeenCalledOnce();
		requestRender.mockClear();
		renderer.setSearchMatches([
			{ text: 'match', start: { column: 0, row: 0 }, end: { column: 4, row: 0 } },
		]);
		expect(requestRender).toHaveBeenCalledOnce();
		renderer.setFontMetricModifiers({ cellWidth: '25%', cellHeight: 3 });
		expect(renderer.getMetrics()).toEqual({ width: 10, height: 20, baseline: 14 });
		renderer.dispose();
	});

	it('rounds fractional font metrics and centers the baseline like Ghostty', () => {
		const metrics = {
			width: 7.4,
			fontBoundingBoxAscent: 12.2,
			fontBoundingBoxDescent: 3.2,
		} satisfies Partial<TextMetrics>;
		vi.stubGlobal('window', { devicePixelRatio: 1 });
		vi.stubGlobal('document', { createElement: () => createCanvas(undefined, metrics) });

		const renderer = new WebGPURenderer(createCanvas(undefined, metrics), { fontSize: 13 });

		expect(renderer.getMetrics()).toEqual({ width: 7, height: 15, baseline: 12 });
		renderer.dispose();
	});

	it('rebuilds physical-pixel resources when the device pixel ratio changes', async () => {
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
		await renderer.initGPU();
		const requestRender = vi.fn<() => void>();
		renderer.setRenderRequestCallback(requestRender);
		renderer.resize(10, 2);

		expect([canvas.width, canvas.height]).toEqual([160, 68]);
		expect(canvas.style.marginLeft).toBe('2px');
		expect(canvas.style.marginTop).toBe('2px');
		expect(renderer.setDevicePixelRatio(2)).toBe(false);
		expect(renderer.setDevicePixelRatio(1.5)).toBe(true);
		expect(canvas.style.width).toBe('80px');
		expect(canvas.style.height).toBe('34px');
		expect([canvas.width, canvas.height]).toEqual([120, 51]);
		expect(requestRender).toHaveBeenCalledOnce();
		renderer.dispose();
	});

	it('stops cursor animation while suspended and resumes it once visible', () => {
		vi.useFakeTimers();
		vi.stubGlobal('window', { devicePixelRatio: 1 });
		vi.stubGlobal('document', { createElement: () => createCanvas() });
		const renderer = new WebGPURenderer(createCanvas(), { cursorBlink: true });
		const requestRender = vi.fn<() => void>();
		renderer.setRenderRequestCallback(requestRender);

		vi.advanceTimersByTime(530);
		expect(requestRender).toHaveBeenCalledTimes(1);
		renderer.setSuspended(true);
		requestRender.mockClear();
		vi.advanceTimersByTime(1_060);
		expect(requestRender).not.toHaveBeenCalled();

		renderer.setSuspended(false);
		requestRender.mockClear();
		vi.advanceTimersByTime(530);
		expect(requestRender).toHaveBeenCalledTimes(1);
		renderer.dispose();
	});

	it('reports an observable Canvas 2D fallback when WebGPU is unavailable', async () => {
		vi.stubGlobal('window', { devicePixelRatio: 1 });
		vi.stubGlobal('navigator', {});
		vi.stubGlobal('document', { createElement: () => createCanvas() });
		const renderer = new WebGPURenderer(createCanvas());
		const health = vi.fn<(event: { state: string; backend: string; reason?: string }) => void>();
		renderer.setHealthChangeCallback(health);

		await expect(renderer.initGPU()).resolves.toBe(false);
		expect(health).toHaveBeenCalledWith({
			state: 'fallback',
			backend: 'canvas2d',
			reason: 'webgpu-unavailable',
		});
		renderer.dispose();
	});

	it('uses configured regular and bold weights in the Canvas 2D fallback', async () => {
		vi.stubGlobal('window', { devicePixelRatio: 1 });
		vi.stubGlobal('navigator', {});
		vi.stubGlobal('document', { createElement: () => createCanvas() });
		const canvas = createCanvas();
		const context = canvas.getContext('2d');
		if (!context) throw new Error('Missing Canvas 2D context');
		const renderer = new WebGPURenderer(canvas, {
			fontFamily: 'JetBrains Mono',
			fontSize: 13,
			fontWeight: 225,
			fontWeightBold: 575,
		});
		await renderer.initGPU();
		const buffer = createTextBuffer('A');

		renderer.render(buffer, true);
		expect(context.font).toBe('225 13px JetBrains Mono');

		const cell = buffer.getLine(0)?.[0];
		if (!cell) throw new Error('Missing font-weight test cell');
		cell.flags = CellFlags.BOLD | CellFlags.ITALIC;
		renderer.render(buffer, true);
		expect(context.font).toBe('italic 575 13px JetBrains Mono');
		renderer.dispose();
	});

	it('paints explicit black backgrounds over opaque and transparent Canvas surfaces', async () => {
		vi.stubGlobal('window', { devicePixelRatio: 1 });
		vi.stubGlobal('navigator', {});
		const paints: { readonly color: string; readonly rectangle: readonly number[] }[] = [];
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
		const renderer = new WebGPURenderer(createCanvas(undefined, {}, undefined, fillRect), {
			theme: { background: '#123456' },
		});
		await renderer.initGPU();
		renderer.resize(1, 1);
		const buffer = createTextBuffer('A');

		paints.length = 0;
		renderer.render(buffer, true);
		expect(paints).toEqual([
			{ color: '#123456', rectangle: [0, 0, 8, 17] },
			{ color: 'rgb(0, 0, 0)', rectangle: [0, 0, 8, 17] },
		]);

		renderer.setAllowTransparency(true);
		paints.length = 0;
		renderer.render(buffer, true);
		expect(paints).toEqual([{ color: 'rgb(0, 0, 0)', rectangle: [0, 0, 8, 17] }]);
		renderer.dispose();
	});

	it('applies faint alpha and independent colors to Canvas text decorations', async () => {
		vi.stubGlobal('window', { devicePixelRatio: 1 });
		vi.stubGlobal('navigator', {});
		const textAlphas: number[] = [];
		const decorationAlphas: number[] = [];
		const decorationColors: string[] = [];
		const fillText: CanvasRenderingContext2D['fillText'] = function (
			this: CanvasRenderingContext2D,
		) {
			textAlphas.push(this.globalAlpha);
		};
		const stroke: CanvasRenderingContext2D['stroke'] = function (this: CanvasRenderingContext2D) {
			decorationAlphas.push(this.globalAlpha);
			decorationColors.push(String(this.strokeStyle));
		};
		vi.stubGlobal('document', {
			createElement: () => createCanvas(undefined, {}, fillText, undefined, stroke),
		});
		const canvas = createCanvas(undefined, {}, fillText, undefined, stroke);
		const renderer = new WebGPURenderer(canvas);
		await renderer.initGPU();
		const buffer = createTextBuffer('A');
		const cell = buffer.getLine(0)?.[0];
		if (!cell) throw new Error('Missing decoration test cell');
		cell.flags = CellFlags.FAINT | CellFlags.UNDERLINE | CellFlags.STRIKETHROUGH;
		cell.underline_r = 255;
		cell.underline_g = 0;
		cell.underline_b = 0;
		cell.overline = true;

		renderer.render(buffer, true);

		expect(textAlphas).toEqual([0.5]);
		expect(decorationAlphas).toEqual([0.5, 0.5, 0.5]);
		expect(decorationColors).toEqual([
			'rgb(255, 0, 0)',
			'rgb(255, 255, 255)',
			'rgb(255, 255, 255)',
		]);
		expect(canvas.getContext('2d')?.globalAlpha).toBe(1);
		renderer.dispose();
	});

	it('animates blinking terminal text and suspends its timer while hidden', async () => {
		vi.useFakeTimers();
		vi.stubGlobal('window', { devicePixelRatio: 1 });
		vi.stubGlobal('navigator', {});
		vi.stubGlobal('document', { createElement: () => createCanvas() });
		const renderer = new WebGPURenderer(createCanvas());
		await renderer.initGPU();
		const requestRender = vi.fn<() => void>();
		renderer.setRenderRequestCallback(requestRender);
		renderer.render(createBlinkingBuffer(), true);

		vi.advanceTimersByTime(530);
		expect(requestRender).toHaveBeenCalledOnce();
		renderer.setSuspended(true);
		requestRender.mockClear();
		vi.advanceTimersByTime(1_060);
		expect(requestRender).not.toHaveBeenCalled();
		renderer.dispose();
	});

	it('reports frame reasons, dirty work, uploads, and atlas pressure', async () => {
		vi.stubGlobal('window', { devicePixelRatio: 1 });
		vi.stubGlobal('navigator', {});
		vi.stubGlobal('document', { createElement: () => createCanvas() });
		const renderer = new WebGPURenderer(createCanvas());
		await renderer.initGPU();
		const telemetry = vi.fn<(frame: RendererFrameTelemetry) => void>();
		renderer.setFrameTelemetryCallback(telemetry);

		renderer.render(createBlinkingBuffer(), true);

		expect(telemetry).toHaveBeenCalledOnce();
		expect(telemetry.mock.calls[0]?.[0]).toMatchObject({
			frameId: 1,
			backend: 'canvas2d',
			reasons: expect.arrayContaining(['full']),
			forceAll: true,
			rowsRendered: 1,
			cellsVisited: 1,
			glyphInstances: 1,
			vertexUploadBytes: 0,
			atlasUploadBytes: 0,
			atlasGlyphCount: 0,
			atlasPressure: 0,
		});
		expect(renderer.getLastFrameTelemetry()?.durationMs).toBeGreaterThanOrEqual(0);
		renderer.dispose();
	});

	it('preserves untouched rows when incremental content is redrawn', async () => {
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
		const dirtyRows = new Set<number>();
		const base = createGridBuffer(3, 5);
		const buffer: IRenderable = {
			...base,
			isRowDirty: (row) => dirtyRows.has(row),
			clearDirty: () => dirtyRows.clear(),
		};

		renderer.render(buffer, true);
		clearRect.mockClear();
		dirtyRows.add(2);
		renderer.render(buffer);
		const middleRows = clearRect.mock.calls.map(([, y]) => y / renderer.getMetrics().height);
		expect(middleRows).toEqual([0, 1, 2, 3]);
		expect(middleRows).not.toContain(4);

		clearRect.mockClear();
		dirtyRows.add(4);
		renderer.render(buffer);
		const finalRows = clearRect.mock.calls.map(([, y]) => y / renderer.getMetrics().height);
		expect(finalRows).toEqual([0, 3, 4]);
		expect(finalRows).not.toContain(2);
		renderer.dispose();
	});

	it('clips linear and rectangular selections to visible Canvas cells', async () => {
		vi.stubGlobal('window', { devicePixelRatio: 1 });
		vi.stubGlobal('navigator', {});
		const selected: (readonly number[])[] = [];
		const fillRect: CanvasRenderingContext2D['fillRect'] = function (
			this: CanvasRenderingContext2D,
			x,
			y,
			width,
			height,
		) {
			if (this.fillStyle === '#d4d4d4') selected.push([x, y, width, height]);
		};
		vi.stubGlobal('document', {
			createElement: () => createCanvas(undefined, {}, undefined, fillRect),
		});
		const renderer = new WebGPURenderer(createCanvas(undefined, {}, undefined, fillRect));
		await renderer.initGPU();
		let coordinates = {
			startCol: 2,
			startRow: -1,
			endCol: 1,
			endRow: 1,
			rectangle: false,
		};
		Reflect.set(renderer, 'selectionManager', {
			hasSelection: () => true,
			getSelectionCoords: () => coordinates,
			getDirtySelectionRows: () => new Set<number>(),
			clearDirtySelectionRows: () => {},
		});
		const buffer = createGridBuffer(4, 3);

		renderer.render(buffer, true);
		expect(selected).toEqual([
			[0, 0, 8, 17],
			[8, 0, 8, 17],
			[16, 0, 8, 17],
			[24, 0, 8, 17],
			[0, 17, 8, 17],
			[8, 17, 8, 17],
		]);

		coordinates = {
			startCol: 1,
			startRow: 0,
			endCol: 2,
			endRow: 2,
			rectangle: true,
		};
		selected.length = 0;
		renderer.render(buffer, true);
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

	it('renders a zero-sized surface without creating cells or glyphs', async () => {
		vi.stubGlobal('window', { devicePixelRatio: 1 });
		vi.stubGlobal('navigator', {});
		vi.stubGlobal('document', { createElement: () => createCanvas() });
		const renderer = new WebGPURenderer(createCanvas());
		await renderer.initGPU();
		const empty: IRenderable = {
			getLine: () => null,
			getCursor: () => ({
				x: 0,
				y: 0,
				viewportX: 0,
				viewportY: 0,
				visible: false,
				blinking: false,
				style: 'block',
			}),
			getDimensions: () => ({ cols: 0, rows: 0 }),
			isRowDirty: () => false,
			clearDirty: () => {},
		};

		renderer.render(empty, true);

		expect(renderer.getLastFrameTelemetry()).toMatchObject({
			rowsRendered: 0,
			cellsVisited: 0,
			glyphInstances: 0,
		});
		renderer.dispose();
	});

	it('shapes bounded multi-cell runs and redraws individual cells when disabled', async () => {
		vi.stubGlobal('window', { devicePixelRatio: 1 });
		vi.stubGlobal('navigator', {});
		const fillText = vi.fn<CanvasRenderingContext2D['fillText']>();
		vi.stubGlobal('document', {
			createElement: () => createCanvas(undefined, {}, fillText),
		});
		const renderer = new WebGPURenderer(createCanvas(undefined, {}, fillText));
		await renderer.initGPU();
		const buffer = createTextBuffer('=>');

		renderer.render(buffer, true);
		expect(fillText.mock.calls.map(([text]) => text)).toContain('=>');
		expect(renderer.getLastFrameTelemetry()).toMatchObject({
			shapingRuns: 1,
			glyphInstances: 1,
		});

		fillText.mockClear();
		renderer.setFontLigatures(false);
		renderer.render(buffer);
		expect(fillText.mock.calls.map(([text]) => text)).toEqual(expect.arrayContaining(['=', '>']));
		expect(renderer.getLastFrameTelemetry()).toMatchObject({
			reasons: expect.arrayContaining(['font']),
			shapingRuns: 0,
			glyphInstances: 2,
		});
		renderer.dispose();
	});

	it('renders combining and emoji grapheme clusters as single glyphs', async () => {
		vi.stubGlobal('window', { devicePixelRatio: 1 });
		vi.stubGlobal('navigator', {});
		const fillText = vi.fn<CanvasRenderingContext2D['fillText']>();
		vi.stubGlobal('document', {
			createElement: () => createCanvas(undefined, {}, fillText),
		});
		const renderer = new WebGPURenderer(createCanvas(undefined, {}, fillText));
		await renderer.initGPU();

		renderer.render(createGraphemeBuffer('e\u0301', 1), true);
		expect(fillText.mock.calls.map(([text]) => text)).toContain('e\u0301');
		expect(renderer.getLastFrameTelemetry()).toMatchObject({ glyphInstances: 1 });

		fillText.mockClear();
		renderer.render(createGraphemeBuffer('👍🏼', 2), true);
		expect(fillText.mock.calls.map(([text]) => text)).toContain('👍🏼');
		expect(renderer.getLastFrameTelemetry()).toMatchObject({ glyphInstances: 1 });
		renderer.dispose();
	});

	it.each([
		{ name: 'testASCIIUppercase', text: 'A', width: 1 },
		{ name: 'testASCIILowercase', text: 'g', width: 1 },
		{ name: 'testASCIIDigit', text: '0', width: 1 },
		{ name: 'testASCIIPunctuation', text: '@', width: 1 },
		{ name: 'testAccentedLatin', text: 'é', width: 1 },
		{ name: 'testUmlaut', text: 'ü', width: 1 },
		{ name: 'testChineseCharacter', text: '中', width: 2 },
		{ name: 'testJapaneseHiragana', text: 'あ', width: 2 },
		{ name: 'testJapaneseKatakana', text: 'ア', width: 2 },
		{ name: 'testKoreanHangul', text: '한', width: 2 },
		{ name: 'testCombiningMark', text: 'e\u0301', width: 1 },
		{ name: 'testMultipleCombiningMarks', text: 'a\u0300\u0303', width: 1 },
		{ name: 'testCombiningEnclosingCircle', text: 'A\u20dd', width: 1 },
		{ name: 'testBismillah', text: '﷽', width: 1 },
		{ name: 'testArabicCharacter', text: 'ع', width: 1 },
		{ name: 'testHebrewCharacter', text: 'א', width: 1 },
		{ name: 'testThaiCharacter', text: 'ก', width: 1 },
		{ name: 'testThaiWithVowelMark', text: 'ก\u0e34', width: 1 },
		{ name: 'testDevanagariCharacter', text: 'अ', width: 1 },
		{ name: 'testGreekCharacter', text: 'Ω', width: 1 },
		{ name: 'testCyrillicCharacter', text: 'Д', width: 1 },
		{ name: 'testMathSymbol', text: '∑', width: 1 },
		{ name: 'testArrow', text: '→', width: 1 },
		{ name: 'testEmoji', text: '😀', width: 2 },
		{ name: 'testEmojiWithSkinTone', text: '👋🏽', width: 2 },
		{ name: 'testFamilyEmoji', text: '👨‍👩‍👧', width: 2 },
		{ name: 'testFlagEmoji', text: '🇺🇸', width: 2 },
		{ name: 'testZalgoText', text: 'H\u0300\u0301\u0302\u0303\u0304', width: 1 },
		{ name: 'testSurrogatePairEmoji', text: '🎉', width: 2 },
	])('renders iTerm2 CharacterSource case $name as one grapheme glyph', async ({ text, width }) => {
		vi.stubGlobal('window', { devicePixelRatio: 1 });
		vi.stubGlobal('navigator', {});
		const fillText = vi.fn<CanvasRenderingContext2D['fillText']>();
		vi.stubGlobal('document', {
			createElement: () => createCanvas(undefined, {}, fillText),
		});
		const renderer = new WebGPURenderer(createCanvas(undefined, {}, fillText));
		await renderer.initGPU();

		renderer.render(createGraphemeBuffer(text, width), true);

		expect(fillText.mock.calls.map(([rendered]) => rendered)).toContain(text);
		expect(renderer.getLastFrameTelemetry()).toMatchObject({ glyphInstances: 1 });
		renderer.dispose();
	});

	it('shapes Nerd Font icons between spaces and applies their glyph constraint', async () => {
		vi.stubGlobal('window', { devicePixelRatio: 1 });
		vi.stubGlobal('navigator', {});
		const fillText = vi.fn<CanvasRenderingContext2D['fillText']>();
		const scale = vi.fn<CanvasRenderingContext2D['scale']>();
		vi.stubGlobal('document', {
			createElement: () =>
				createCanvas(
					undefined,
					{ actualBoundingBoxRight: 20 },
					fillText,
					undefined,
					undefined,
					undefined,
					undefined,
					scale,
				),
		});
		const renderer = new WebGPURenderer(
			createCanvas(
				undefined,
				{ actualBoundingBoxRight: 20 },
				fillText,
				undefined,
				undefined,
				undefined,
				undefined,
				scale,
			),
		);
		await renderer.initGPU();
		const nerdFontFolder = String.fromCodePoint(0xf024b);

		renderer.render(createTextBuffer(` ${nerdFontFolder} `), true);

		expect(fillText.mock.calls.filter(([text]) => text === nerdFontFolder)).toHaveLength(1);
		expect(scale).toHaveBeenCalledWith(expect.any(Number), expect.any(Number));
		expect(renderer.getLastFrameTelemetry()).toMatchObject({ glyphInstances: 3 });
		renderer.dispose();
	});

	it('keeps background changes in one shaping run and splits foreground changes', async () => {
		vi.stubGlobal('window', { devicePixelRatio: 1 });
		vi.stubGlobal('navigator', {});
		vi.stubGlobal('document', { createElement: () => createCanvas() });
		const renderer = new WebGPURenderer(createCanvas());
		await renderer.initGPU();
		const buffer = createTextBuffer('>=');
		const line = buffer.getLine(0);
		const first = line?.[0];
		const second = line?.[1];
		if (!first || !second) throw new Error('Missing shaping test cells');
		first.bg_r = 10;
		second.bg_r = 20;

		renderer.render(buffer, true);
		expect(renderer.getLastFrameTelemetry()).toMatchObject({ shapingRuns: 1, glyphInstances: 1 });

		second.fg_r = 20;
		renderer.render(buffer, true);
		expect(renderer.getLastFrameTelemetry()).toMatchObject({ shapingRuns: 0, glyphInstances: 2 });
		renderer.dispose();
	});

	it('renders Ghostty-style preedit text without marked-range inversion or a caret', () => {
		vi.stubGlobal('window', { devicePixelRatio: 1 });
		const fillText = vi.fn<CanvasRenderingContext2D['fillText']>();
		const fillRect = vi.fn<CanvasRenderingContext2D['fillRect']>();
		vi.stubGlobal('document', {
			createElement: () => createCanvas(undefined, {}, fillText, fillRect),
		});
		const canvas = createCanvas(undefined, {}, fillText, fillRect);
		const renderer = new WebGPURenderer(canvas);
		const context = canvas.getContext('2d');
		if (!context) throw new Error('Missing Canvas 2D context');
		const renderPreedit = Reflect.get(renderer, 'renderPreedit2D') as (
			context: CanvasRenderingContext2D,
			layout: {
				readonly cells: readonly { readonly text: string; readonly width: number }[];
				readonly start: number;
				readonly end: number;
				readonly row: number;
				readonly width: number;
				readonly selectionStart: number;
				readonly selectionEnd: number;
			},
		) => void;

		renderPreedit.call(renderer, context, {
			cells: [
				{ text: 'c', width: 1 },
				{ text: 'h', width: 1 },
				{ text: 'e', width: 1 },
			],
			start: 0,
			end: 3,
			row: 0,
			width: 3,
			selectionStart: 0,
			selectionEnd: 2,
		});

		expect(fillText.mock.calls.map(([text]) => text)).toEqual(['c', 'h', 'e']);
		expect(fillRect.mock.calls).toEqual([
			[0, 0, 24, 17],
			[0, 14, 24, 1],
		]);
		renderer.dispose();
	});

	it('maps preedit text to its exact cell width', () => {
		vi.stubGlobal('window', { devicePixelRatio: 1 });
		vi.stubGlobal('document', { createElement: () => createCanvas() });
		const renderer = new WebGPURenderer(createCanvas());
		const getPreeditLayout = Reflect.get(renderer, 'getPreeditLayout') as (
			cursor: { x: number; y: number },
			columns: number,
		) => {
			readonly start: number;
			readonly end: number;
			readonly row: number;
			readonly width: number;
		} | null;

		renderer.setPreedit([{ text: 'a', width: 1 }]);
		expect(getPreeditLayout.call(renderer, { x: 2, y: 3 }, 10)).toMatchObject({
			start: 2,
			end: 3,
			row: 3,
			width: 1,
		});
		renderer.setPreedit([{ text: '가', width: 2 }]);
		expect(getPreeditLayout.call(renderer, { x: 2, y: 3 }, 10)).toMatchObject({
			start: 2,
			end: 4,
			row: 3,
			width: 2,
		});
		renderer.dispose();
	});

	it('shifts wide preedit text left at the right edge', () => {
		vi.stubGlobal('window', { devicePixelRatio: 1 });
		vi.stubGlobal('document', { createElement: () => createCanvas() });
		const renderer = new WebGPURenderer(createCanvas());
		const getPreeditLayout = Reflect.get(renderer, 'getPreeditLayout') as (
			cursor: { x: number; y: number },
			columns: number,
		) => {
			readonly start: number;
			readonly end: number;
			readonly cells: readonly { readonly text: string; readonly width: number }[];
		} | null;

		renderer.setPreedit([{ text: '가', width: 2 }]);
		expect(getPreeditLayout.call(renderer, { x: 9, y: 0 }, 10)).toMatchObject({
			start: 8,
			end: 10,
			cells: [{ text: '가', width: 2 }],
		});
		renderer.dispose();
	});

	it('uses Ghostty cursor visibility and focus precedence in the integrated render path', async () => {
		vi.useFakeTimers();
		vi.stubGlobal('window', { devicePixelRatio: 1 });
		vi.stubGlobal('navigator', {});
		vi.stubGlobal('document', { createElement: () => createCanvas() });
		const renderer = new WebGPURenderer(createCanvas());
		await renderer.initGPU();
		renderer.setFocused(true);
		const renderCursor =
			vi.fn<
				(
					context: CanvasRenderingContext2D,
					x: number,
					y: number,
					style: 'block' | 'block_hollow' | 'underline' | 'bar',
				) => void
			>();
		Reflect.set(renderer, 'renderCursor2D', renderCursor);

		renderer.render(createCursorBuffer({ visible: true, blinking: false, style: 'bar' }), true);
		expect(renderCursor).toHaveBeenLastCalledWith(expect.anything(), 0, 0, 'bar');

		renderCursor.mockClear();
		renderer.render(createCursorBuffer({ visible: false, blinking: false, style: 'bar' }), true);
		expect(renderCursor).not.toHaveBeenCalled();

		renderer.setFocused(false);
		renderer.render(
			createCursorBuffer({ visible: true, blinking: true, style: 'underline' }),
			true,
		);
		expect(renderCursor).toHaveBeenLastCalledWith(expect.anything(), 0, 0, 'block_hollow');
		renderer.dispose();
	});

	it('renders neither preedit nor cursor while viewing scrollback', async () => {
		vi.stubGlobal('window', { devicePixelRatio: 1 });
		vi.stubGlobal('navigator', {});
		vi.stubGlobal('document', { createElement: () => createCanvas() });
		const renderer = new WebGPURenderer(createCanvas());
		await renderer.initGPU();
		renderer.setPreedit([{ text: 'a', width: 1 }]);
		const renderPreedit = vi.fn<() => void>();
		const renderCursor = vi.fn<() => void>();
		Reflect.set(renderer, 'renderPreedit2D', renderPreedit);
		Reflect.set(renderer, 'renderCursor2D', renderCursor);

		renderer.render(createCursorBuffer({ visible: true, blinking: false, style: 'bar' }), true, 1);

		expect(renderPreedit).not.toHaveBeenCalled();
		expect(renderCursor).not.toHaveBeenCalled();
		renderer.dispose();
	});

	it('restores a visible cursor when terminal blinking is disabled', async () => {
		vi.useFakeTimers();
		vi.stubGlobal('window', { devicePixelRatio: 1 });
		vi.stubGlobal('navigator', {});
		vi.stubGlobal('document', { createElement: () => createCanvas() });
		const renderer = new WebGPURenderer(createCanvas());
		await renderer.initGPU();
		renderer.setFocused(true);
		const renderCursor =
			vi.fn<
				(
					context: CanvasRenderingContext2D,
					x: number,
					y: number,
					style: 'block' | 'block_hollow' | 'underline' | 'bar',
				) => void
			>();
		Reflect.set(renderer, 'renderCursor2D', renderCursor);

		renderer.render(createCursorBuffer({ visible: true, blinking: true, style: 'bar' }), true);
		renderCursor.mockClear();
		vi.advanceTimersByTime(530);
		renderer.render(createCursorBuffer({ visible: true, blinking: true, style: 'bar' }), true);
		expect(renderCursor).not.toHaveBeenCalled();

		renderer.render(createCursorBuffer({ visible: true, blinking: false, style: 'bar' }), true);
		expect(renderCursor).toHaveBeenLastCalledWith(expect.anything(), 0, 0, 'bar');
		renderer.dispose();
	});

	it('renders bar and underline cursors at one physical pixel', () => {
		vi.stubGlobal('window', { devicePixelRatio: 2 });
		const fillRect = vi.fn<CanvasRenderingContext2D['fillRect']>();
		vi.stubGlobal('document', {
			createElement: () => createCanvas(undefined, {}, undefined, fillRect),
		});
		const canvas = createCanvas(undefined, {}, undefined, fillRect);
		const renderer = new WebGPURenderer(canvas, { devicePixelRatio: 2 });
		const context = canvas.getContext('2d');
		if (!context) throw new Error('Missing Canvas 2D context');
		const renderCursor = Reflect.get(renderer, 'renderCursor2D') as (
			context: CanvasRenderingContext2D,
			x: number,
			y: number,
			style: 'block' | 'block_hollow' | 'underline' | 'bar',
		) => void;

		renderCursor.call(renderer, context, 2, 1, 'bar');
		renderCursor.call(renderer, context, 2, 1, 'underline');

		expect(fillRect.mock.calls).toEqual([
			[15.5, 17, 0.5, 17],
			[16, 31, 8, 0.5],
		]);
		renderer.dispose();
	});

	it('renders the unfocused hollow cursor border at one physical pixel', () => {
		vi.stubGlobal('window', { devicePixelRatio: 2 });
		const fillRect = vi.fn<CanvasRenderingContext2D['fillRect']>();
		vi.stubGlobal('document', {
			createElement: () => createCanvas(undefined, {}, undefined, fillRect),
		});
		const canvas = createCanvas(undefined, {}, undefined, fillRect);
		const renderer = new WebGPURenderer(canvas, { devicePixelRatio: 2 });
		const context = canvas.getContext('2d');
		if (!context) throw new Error('Missing Canvas 2D context');
		const renderCursor = Reflect.get(renderer, 'renderCursor2D') as (
			context: CanvasRenderingContext2D,
			x: number,
			y: number,
			style: 'block' | 'block_hollow' | 'underline' | 'bar',
		) => void;

		renderCursor.call(renderer, context, 2, 1, 'block_hollow');

		expect(fillRect.mock.calls).toEqual([
			[16, 17, 8, 0.5],
			[16, 33.5, 8, 0.5],
			[16, 17, 0.5, 17],
			[23.5, 17, 0.5, 17],
		]);
		renderer.dispose();
	});

	it('requests a redraw only when terminal focus changes', () => {
		vi.stubGlobal('window', { devicePixelRatio: 1 });
		vi.stubGlobal('document', { createElement: () => createCanvas() });
		const renderer = new WebGPURenderer(createCanvas());
		const requestRender = vi.fn<() => void>();
		renderer.setRenderRequestCallback(requestRender);

		renderer.setFocused(false);
		renderer.setFocused(true);
		renderer.setFocused(true);
		renderer.setFocused(false);

		expect(requestRender).toHaveBeenCalledTimes(2);
		renderer.dispose();
	});

	it('underlines every cell in a hovered multi-row link range', async () => {
		vi.stubGlobal('window', { devicePixelRatio: 1 });
		vi.stubGlobal('navigator', {});
		let strokes = 0;
		const stroke: CanvasRenderingContext2D['stroke'] = () => {
			strokes++;
		};
		vi.stubGlobal('document', {
			createElement: () => createCanvas(undefined, {}, undefined, undefined, stroke),
		});
		const renderer = new WebGPURenderer(createCanvas(undefined, {}, undefined, undefined, stroke));
		await renderer.initGPU();
		renderer.setHoveredLinkRange({ startX: 1, startY: 0, endX: 1, endY: 1 });

		renderer.render(createGridBuffer(5, 2), true);

		expect(strokes).toBe(6);
		renderer.dispose();
	});

	it('uploads only the merged glyph-atlas dirty rectangle', () => {
		vi.stubGlobal('window', { devicePixelRatio: 1 });
		const getImageData = vi.fn<CanvasRenderingContext2D['getImageData']>(
			(_x, _y, width, height) => {
				const data = new Uint8ClampedArray(width * height * 4);
				data.set([1, 2, 3, 4]);
				return { data } as ImageData;
			},
		);
		vi.stubGlobal('document', { createElement: () => createCanvas(getImageData) });
		const renderer = new WebGPURenderer(createCanvas(getImageData));
		const writeTexture = vi.fn<GPUQueue['writeTexture']>();
		const device = {
			queue: { writeTexture },
			destroy: () => {},
		} as unknown as GPUDevice;
		const texture = { destroy: () => {} } as unknown as GPUTexture;
		Reflect.set(renderer, 'device', device);
		Reflect.set(renderer, 'atlasTexture', texture);

		const markAtlasDirty = Reflect.get(renderer, 'markAtlasDirty') as (
			x: number,
			y: number,
			width: number,
			height: number,
		) => void;
		const uploadAtlas = Reflect.get(renderer, 'uploadAtlas') as () => void;
		markAtlasDirty.call(renderer, 10, 12, 8, 17);
		markAtlasDirty.call(renderer, 30, 8, 4, 3);
		uploadAtlas.call(renderer);

		expect(writeTexture).toHaveBeenCalledWith(
			{ texture, origin: { x: 10, y: 8 } },
			expect.any(Uint8ClampedArray),
			{ bytesPerRow: 96, rowsPerImage: 21 },
			{ width: 24, height: 21 },
		);
		const uploaded = writeTexture.mock.calls[0]?.[1];
		expect(Array.from(uploaded as Uint8ClampedArray).slice(0, 4)).toEqual([1, 2, 3, 4]);
		expect(Reflect.get(renderer, 'frameAtlasUploadBytes')).toBe(2_016);
		expect(Reflect.get(renderer, 'atlasDirtyBounds')).toBeNull();
		renderer.dispose();
	});

	it('preserves colored emoji while leaving ordinary glyphs tintable', () => {
		vi.stubGlobal('window', { devicePixelRatio: 1 });
		const getImageData = vi.fn<CanvasRenderingContext2D['getImageData']>(() => {
			return { data: new Uint8ClampedArray([255, 90, 20, 255]) } as ImageData;
		});
		vi.stubGlobal('document', { createElement: () => createCanvas(getImageData) });
		const renderer = new WebGPURenderer(createCanvas(getImageData));
		const ensureGlyph = Reflect.get(renderer, 'ensureGlyph') as (
			text: string,
			flags: number,
			width: number,
		) => { readonly colored: boolean } | null;

		expect(ensureGlyph.call(renderer, 'A', 0, 1)?.colored).toBe(false);
		expect(ensureGlyph.call(renderer, '🚀', 0, 2)?.colored).toBe(true);
		expect(getImageData).toHaveBeenCalledOnce();
		renderer.dispose();
	});

	it('rasterizes built-in Ghostty sprites into the physical-pixel atlas', () => {
		vi.stubGlobal('window', { devicePixelRatio: 2 });
		const fillText = vi.fn<CanvasRenderingContext2D['fillText']>();
		const fillRect = vi.fn<CanvasRenderingContext2D['fillRect']>();
		vi.stubGlobal('document', {
			createElement: () => createCanvas(undefined, {}, fillText, fillRect),
		});
		const renderer = new WebGPURenderer(createCanvas(undefined, {}, fillText, fillRect), {
			devicePixelRatio: 2,
		});
		const ensureGlyph = Reflect.get(renderer, 'ensureGlyph') as (
			text: string,
			flags: number,
			width: number,
			codepoint: number,
			constraintWidth: 1 | 2,
		) => {
			readonly w: number;
			readonly h: number;
			readonly offsetX: number;
			readonly offsetY: number;
		} | null;

		expect(ensureGlyph.call(renderer, '─', 0, 1, 0x2500, 1)).toMatchObject({
			w: 20,
			h: 38,
			offsetX: -1,
			offsetY: -1,
		});
		expect(fillRect.mock.calls).toEqual([
			[9, 18, 9, 2],
			[2, 18, 9, 2],
		]);
		expect(fillText).not.toHaveBeenCalled();
		renderer.dispose();
	});

	it('draws built-in Ghostty sprites in the Canvas 2D fallback', () => {
		vi.stubGlobal('window', { devicePixelRatio: 1 });
		const fillText = vi.fn<CanvasRenderingContext2D['fillText']>();
		const fillRect = vi.fn<CanvasRenderingContext2D['fillRect']>();
		vi.stubGlobal('document', {
			createElement: () => createCanvas(undefined, {}, fillText, fillRect),
		});
		const renderer = new WebGPURenderer(createCanvas(undefined, {}, fillText, fillRect));
		const drawConstrainedText2D = Reflect.get(renderer, 'drawConstrainedText2D') as (
			context: CanvasRenderingContext2D,
			text: string,
			x: number,
			y: number,
			codepoint: number,
			constraintWidth: 1 | 2,
			cellWidth: number,
		) => void;
		const context = createCanvas(undefined, {}, fillText, fillRect).getContext('2d');
		if (!context) throw new Error('Expected a Canvas 2D context');

		drawConstrainedText2D.call(renderer, context, '─', 10, 20, 0x2500, 1, 1);

		expect(fillRect.mock.calls).toEqual([
			[13, 28, 5, 1],
			[10, 28, 4, 1],
		]);
		expect(fillText).not.toHaveBeenCalled();
		renderer.dispose();
	});

	it('caches glyphs by text, cell width, and raster style', () => {
		vi.stubGlobal('window', { devicePixelRatio: 1 });
		vi.stubGlobal('document', { createElement: () => createCanvas() });
		const renderer = new WebGPURenderer(createCanvas());
		const ensureGlyph = Reflect.get(renderer, 'ensureGlyph') as (
			text: string,
			flags: number,
			width: number,
		) => object | null;

		const regular = ensureGlyph.call(renderer, 'A', 0, 1);
		expect(ensureGlyph.call(renderer, 'A', 0, 1)).toBe(regular);
		expect(ensureGlyph.call(renderer, 'A', CellFlags.BOLD, 1)).not.toBe(regular);
		expect(ensureGlyph.call(renderer, 'A', 0, 2)).not.toBe(regular);
		expect(Reflect.get(renderer, 'glyphMap')).toHaveProperty('size', 3);
		renderer.dispose();
	});

	it('does not retain a cache entry when glyph rasterization fails', () => {
		vi.stubGlobal('window', { devicePixelRatio: 1 });
		vi.stubGlobal('document', {
			createElement: () => createCanvas(undefined, {}, failingFillText),
		});
		const renderer = new WebGPURenderer(createCanvas(undefined, {}, failingFillText));
		const ensureGlyph = Reflect.get(renderer, 'ensureGlyph') as (
			text: string,
			flags: number,
			width: number,
		) => object | null;

		expect(() => ensureGlyph.call(renderer, 'A', 0, 1)).toThrow('simulated raster failure');
		expect(Reflect.get(renderer, 'glyphMap')).toHaveProperty('size', 0);
		renderer.dispose();
	});

	it('invalidates the glyph cache when font size or family changes', () => {
		vi.stubGlobal('window', { devicePixelRatio: 1 });
		vi.stubGlobal('document', { createElement: () => createCanvas() });
		const renderer = new WebGPURenderer(createCanvas());
		const ensureGlyph = Reflect.get(renderer, 'ensureGlyph') as (
			text: string,
			flags: number,
			width: number,
		) => object | null;
		const glyphMap = Reflect.get(renderer, 'glyphMap') as Map<string, object>;

		expect(ensureGlyph.call(renderer, 'A', 0, 1)).not.toBeNull();
		expect(glyphMap.size).toBe(1);
		renderer.setFontSize(15);
		expect(glyphMap.size).toBe(0);
		expect(ensureGlyph.call(renderer, 'A', 0, 1)).not.toBeNull();
		renderer.setFontFamily('JetBrains Mono');
		expect(glyphMap.size).toBe(0);
		renderer.dispose();
	});

	it('accepts a glyph whose reserved rectangle exactly fits the atlas row', () => {
		vi.stubGlobal('window', { devicePixelRatio: 1 });
		vi.stubGlobal('document', { createElement: () => createCanvas() });
		const renderer = new WebGPURenderer(createCanvas());
		const ensureGlyph = Reflect.get(renderer, 'ensureGlyph') as (
			text: string,
			flags: number,
			width: number,
		) => { readonly x: number; readonly w: number } | null;

		const measured = ensureGlyph.call(renderer, 'A', 0, 1);
		if (!measured) throw new Error('Expected a measured atlas glyph');
		const atlas = Reflect.get(renderer, 'atlasCanvas') as HTMLCanvasElement;
		const padding = 2;
		Reflect.get(renderer, 'glyphMap').clear();
		Reflect.set(renderer, 'atlasX', atlas.width - measured.w - padding);
		Reflect.set(renderer, 'atlasY', 0);
		Reflect.set(renderer, 'atlasRowHeight', measured.w + padding);

		expect(ensureGlyph.call(renderer, 'B', 0, 1)?.x).toBe(atlas.width - measured.w - padding);
		renderer.dispose();
	});

	it('rejects a glyph whose reserved rectangle exceeds the texture limit', () => {
		const metrics = {
			actualBoundingBoxRight: 2_048,
			actualBoundingBoxAscent: 10,
			actualBoundingBoxDescent: 2,
		} satisfies Partial<TextMetrics>;
		vi.stubGlobal('window', { devicePixelRatio: 1 });
		vi.stubGlobal('document', { createElement: () => createCanvas(undefined, metrics) });
		const renderer = new WebGPURenderer(createCanvas(undefined, metrics));
		const atlas = Reflect.get(renderer, 'atlasCanvas') as HTMLCanvasElement;
		Reflect.set(renderer, 'device', {
			limits: { maxTextureDimension2D: atlas.width },
			destroy: () => {},
		} as unknown as GPUDevice);
		const ensureGlyph = Reflect.get(renderer, 'ensureGlyph') as (
			text: string,
			flags: number,
			width: number,
		) => object | null;

		expect(ensureGlyph.call(renderer, 'W', 0, 1)).toBeNull();
		expect(Reflect.get(renderer, 'glyphMap')).toHaveProperty('size', 0);
		renderer.dispose();
	});

	it('packs multiple exact-fit glyphs before reporting a full atlas', () => {
		vi.stubGlobal('window', { devicePixelRatio: 1 });
		vi.stubGlobal('document', { createElement: () => createCanvas() });
		const renderer = new WebGPURenderer(createCanvas());
		const atlas = Reflect.get(renderer, 'atlasCanvas') as HTMLCanvasElement;
		atlas.width = 24;
		atlas.height = 24;
		Reflect.set(renderer, 'device', {
			limits: { maxTextureDimension2D: 24 },
			destroy: () => {},
		} as unknown as GPUDevice);
		const ensureGlyph = Reflect.get(renderer, 'ensureGlyph') as (
			text: string,
			flags: number,
			width: number,
		) => { readonly x: number } | null;

		expect(ensureGlyph.call(renderer, 'A', 0, 1)?.x).toBe(0);
		expect(ensureGlyph.call(renderer, 'B', 0, 1)?.x).toBe(12);
		expect(ensureGlyph.call(renderer, 'C', 0, 1)).toBeNull();
		renderer.dispose();
	});

	it('stops growing the atlas at the WebGPU texture limit', () => {
		vi.stubGlobal('window', { devicePixelRatio: 1 });
		vi.stubGlobal('document', { createElement: () => createCanvas() });
		const renderer = new WebGPURenderer(createCanvas());
		const atlas = Reflect.get(renderer, 'atlasCanvas') as HTMLCanvasElement;
		Reflect.set(renderer, 'device', {
			limits: { maxTextureDimension2D: atlas.width },
			destroy: () => {},
		} as unknown as GPUDevice);
		const growAtlas = Reflect.get(renderer, 'growAtlas') as () => boolean;

		expect(growAtlas.call(renderer)).toBe(false);
		expect([atlas.width, atlas.height]).toEqual([2048, 2048]);
		renderer.dispose();
	});

	it('preserves glyph ink bounds and snaps atlas quads to physical pixels', () => {
		const metrics = {
			actualBoundingBoxLeft: 2,
			actualBoundingBoxRight: 9,
			actualBoundingBoxAscent: 11,
			actualBoundingBoxDescent: 3,
		} satisfies Partial<TextMetrics>;
		vi.stubGlobal('window', { devicePixelRatio: 2 });
		vi.stubGlobal('document', { createElement: () => createCanvas(undefined, metrics) });
		const renderer = new WebGPURenderer(createCanvas(undefined, metrics), {
			fontFamily: 'Menlo',
			fontSize: 13,
			devicePixelRatio: 2,
		});
		const ensureGlyph = Reflect.get(renderer, 'ensureGlyph') as (
			text: string,
			flags: number,
			width: number,
		) => {
			readonly w: number;
			readonly h: number;
			readonly offsetX: number;
			readonly offsetY: number;
			readonly renderWidth: number;
			readonly renderHeight: number;
		} | null;

		const glyph = ensureGlyph.call(renderer, 'j', CellFlags.ITALIC, 1);
		expect(glyph).toMatchObject({
			w: 15,
			h: 18,
			offsetX: -2,
			offsetY: 6.5,
			renderWidth: 7.5,
			renderHeight: 9,
		});
		expect(Number.isInteger((glyph?.offsetX ?? 0) * 2)).toBe(true);
		expect(Number.isInteger((glyph?.offsetY ?? 0) * 2)).toBe(true);
		renderer.dispose();
	});

	it('preserves rasterization state when the glyph atlas grows', () => {
		vi.stubGlobal('window', { devicePixelRatio: 1 });
		vi.stubGlobal('document', { createElement: () => createCanvas() });
		const renderer = new WebGPURenderer(createCanvas(), {
			fontFamily: 'Menlo',
			fontSize: 13,
		});
		const atlasCanvas = Reflect.get(renderer, 'atlasCanvas') as HTMLCanvasElement;
		const previousContext = atlasCanvas.getContext('2d');
		if (!previousContext) throw new Error('Missing initial atlas context');
		previousContext.font = 'italic bold 13px Menlo';
		previousContext.textAlign = 'left';
		previousContext.textBaseline = 'alphabetic';
		previousContext.direction = 'rtl';
		const growAtlas = Reflect.get(renderer, 'growAtlas') as () => boolean;

		expect(growAtlas.call(renderer)).toBe(true);
		const grownCanvas = Reflect.get(renderer, 'atlasCanvas') as HTMLCanvasElement;
		const grownContext = grownCanvas.getContext('2d');
		expect(grownContext).toMatchObject({
			font: 'italic bold 13px Menlo',
			textAlign: 'left',
			textBaseline: 'alphabetic',
			direction: 'rtl',
		});
		renderer.dispose();
	});

	it('copies existing atlas pixels when the atlas grows', () => {
		vi.stubGlobal('window', { devicePixelRatio: 1 });
		const drawImage = vi.fn<AtlasDrawImage>();
		vi.stubGlobal('document', {
			createElement: () =>
				createCanvas(undefined, {}, undefined, undefined, undefined, undefined, drawImage),
		});
		const renderer = new WebGPURenderer(
			createCanvas(undefined, {}, undefined, undefined, undefined, undefined, drawImage),
		);
		const original = Reflect.get(renderer, 'atlasCanvas') as HTMLCanvasElement;
		const growAtlas = Reflect.get(renderer, 'growAtlas') as () => boolean;

		expect(growAtlas.call(renderer)).toBe(true);
		expect(drawImage).toHaveBeenCalledWith(original, 0, 0);
		expect(Reflect.get(renderer, 'atlasCanvas')).not.toBe(original);
		renderer.dispose();
	});

	it('rebuilds WebGPU resources after a device-loss event', async () => {
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
		const requestAdapter = vi.fn<() => Promise<GPUAdapter>>(async () =>
			Promise.resolve({ requestDevice } as unknown as GPUAdapter),
		);
		vi.stubGlobal('navigator', {
			gpu: { requestAdapter, getPreferredCanvasFormat: () => 'bgra8unorm' },
		});
		const context = {
			configure: vi.fn<(configuration: GPUCanvasConfiguration) => void>(),
		} as unknown as GPUCanvasContext;
		const renderer = new WebGPURenderer(createWebGPUCanvas(context));
		const health = vi.fn<(event: { state: string }) => void>();
		renderer.setHealthChangeCallback(health);

		await expect(renderer.initGPU()).resolves.toBe(true);
		first.lose({ reason: 'unknown', message: 'simulated device loss' });
		await vi.waitFor(() => expect(requestDevice).toHaveBeenCalledTimes(2));
		expect(renderer.getBackend()).toBe('webgpu');

		expect(health.mock.calls.map(([event]) => event.state)).toEqual([
			'ready',
			'lost',
			'recovering',
			'ready',
		]);
		expect(context.configure).toHaveBeenCalledTimes(2);
		renderer.dispose();
	});

	it('reports a failed WebGPU recovery without attempting an invalid Canvas fallback', async () => {
		vi.stubGlobal('window', { devicePixelRatio: 1 });
		vi.stubGlobal('document', { createElement: () => createCanvas() });
		vi.stubGlobal('GPUTextureUsage', { TEXTURE_BINDING: 1, COPY_DST: 2, RENDER_ATTACHMENT: 4 });
		vi.stubGlobal('GPUBufferUsage', { UNIFORM: 1, COPY_DST: 2, VERTEX: 4 });
		const first = createControllableDevice();
		const adapter = { requestDevice: async () => first.device } as unknown as GPUAdapter;
		const requestAdapter = vi
			.fn<() => Promise<GPUAdapter | null>>()
			.mockResolvedValueOnce(adapter)
			.mockResolvedValueOnce(null);
		vi.stubGlobal('navigator', {
			gpu: { requestAdapter, getPreferredCanvasFormat: () => 'bgra8unorm' },
		});
		const context = {
			configure: vi.fn<(configuration: GPUCanvasConfiguration) => void>(),
		} as unknown as GPUCanvasContext;
		const renderer = new WebGPURenderer(createWebGPUCanvas(context));
		const health = vi.fn<(event: { state: string; reason?: string }) => void>();
		renderer.setHealthChangeCallback(health);

		await renderer.initGPU();
		first.lose({ reason: 'unknown', message: 'simulated device loss' });
		await vi.waitFor(() =>
			expect(health.mock.calls.at(-1)?.[0]).toMatchObject({
				state: 'recovery-failed',
				reason: 'No WebGPU adapter is available',
			}),
		);

		expect(renderer.getBackend()).toBe('uninitialized');
		expect(context.configure).toHaveBeenCalledOnce();
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
	} as unknown as GPUDevice;
	return {
		device,
		lose: (info) => resolveLost?.(info as GPUDeviceLostInfo),
	};
}

function createBlinkingBuffer(): IRenderable {
	const cell: GhosttyCell = {
		codepoint: 88,
		fg_r: 255,
		fg_g: 255,
		fg_b: 255,
		bg_r: 0,
		bg_g: 0,
		bg_b: 0,
		flags: CellFlags.BLINK,
		width: 1,
		hyperlink_id: 0,
		grapheme_len: 0,
		selected: false,
		underline_style: 0,
		underline_r: -1,
		underline_g: -1,
		underline_b: -1,
		overline: false,
	};
	return {
		getLine: () => [cell],
		getCursor: () => ({
			x: 0,
			y: 0,
			viewportX: 0,
			viewportY: 0,
			visible: false,
			blinking: false,
			style: 'block',
		}),
		getDimensions: () => ({ cols: 1, rows: 1 }),
		isRowDirty: () => true,
		clearDirty: () => {},
	};
}

function createTextBuffer(text: string): IRenderable {
	const cells = [...text].map(
		(character): GhosttyCell => ({
			codepoint: character.codePointAt(0) ?? 0,
			fg_r: 255,
			fg_g: 255,
			fg_b: 255,
			bg_r: 0,
			bg_g: 0,
			bg_b: 0,
			flags: 0,
			width: 1,
			hyperlink_id: 0,
			grapheme_len: 0,
			selected: false,
			underline_style: 0,
			underline_r: -1,
			underline_g: -1,
			underline_b: -1,
			overline: false,
		}),
	);
	return {
		getLine: (row) => (row === 0 ? cells : null),
		getCursor: () => ({
			x: 0,
			y: 0,
			viewportX: 0,
			viewportY: 0,
			visible: false,
			blinking: false,
			style: 'block',
		}),
		getDimensions: () => ({ cols: cells.length, rows: 1 }),
		isRowDirty: () => true,
		clearDirty: () => {},
	};
}

function createGraphemeBuffer(text: string, width: number): IRenderable {
	const first: GhosttyCell = {
		codepoint: text.codePointAt(0) ?? 0,
		fg_r: 255,
		fg_g: 255,
		fg_b: 255,
		bg_r: 0,
		bg_g: 0,
		bg_b: 0,
		flags: 0,
		width,
		hyperlink_id: 0,
		grapheme_len: Math.max(1, [...text].length - 1),
		selected: false,
		underline_style: 0,
		underline_r: -1,
		underline_g: -1,
		underline_b: -1,
		overline: false,
	};
	const continuation: GhosttyCell = { ...first, codepoint: 0, width: 0, grapheme_len: 0 };
	const line = width === 2 ? [first, continuation] : [first];
	return {
		getLine: (row) => (row === 0 ? line : null),
		getGraphemeString: () => text,
		getCursor: () => ({
			x: 0,
			y: 0,
			viewportX: 0,
			viewportY: 0,
			visible: false,
			blinking: false,
			style: 'block',
		}),
		getDimensions: () => ({ cols: width, rows: 1 }),
		isRowDirty: () => true,
		clearDirty: () => {},
	};
}

function createCursorBuffer(cursor: {
	readonly visible: boolean;
	readonly blinking: boolean;
	readonly style: 'block' | 'underline' | 'bar';
}): IRenderable {
	const buffer = createTextBuffer('A');
	return {
		...buffer,
		getCursor: () => ({
			x: 0,
			y: 0,
			viewportX: 0,
			viewportY: 0,
			visible: cursor.visible,
			blinking: cursor.blinking,
			style: cursor.style,
		}),
	};
}

function createGridBuffer(columns: number, rows: number): IRenderable {
	const lines = Array.from({ length: rows }, (_, row) =>
		createTextBuffer(String(row).padEnd(columns, 'A')).getLine(0),
	);
	return {
		getLine: (row) => lines[row] ?? null,
		getCursor: () => ({
			x: 0,
			y: 0,
			viewportX: 0,
			viewportY: 0,
			visible: false,
			blinking: false,
			style: 'block',
		}),
		getDimensions: () => ({ cols: columns, rows }),
		isRowDirty: () => true,
		clearDirty: () => {},
	};
}
