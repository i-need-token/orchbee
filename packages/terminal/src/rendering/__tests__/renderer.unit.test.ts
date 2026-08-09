import {
	TerminalCellWidthCode,
	TerminalColorEncoding,
	TerminalStyleFlag,
	TerminalUnderlineKindCode,
} from '@orchbee/terminal-protocol';
import { afterEach, describe, expect, it, vi } from 'vitest';

import type { FontWeight } from '../../types';
import {
	type PackedRendererMetadata,
	type PackedRendererRow,
	type PackedRendererSource,
	type RendererFrameTelemetry,
	WebGPURenderer,
} from '../renderer';

type AtlasDrawImage = (image: CanvasImageSource, dx: number, dy: number) => void;

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
		readonly cursor?: Partial<PackedRendererMetadata['cursor']>;
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
				...options.cursor,
			},
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
			},
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

		const regularWeight: FontWeight = 250;
		const boldWeight: FontWeight = 600;
		renderer.setFontWeight(regularWeight);
		renderer.setFontWeightBold(boldWeight);
		ensureGlyph.call(renderer, 'D', TerminalStyleFlag.bold | TerminalStyleFlag.italic, 1);
		expect(atlasContext.font).toBe('italic 600 26px JetBrains Mono');
		renderer.dispose();
	});

	it('measures projected surface geometry and rebuilds for device-pixel changes', async () => {
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
		renderer.resize(10, 2);

		expect(renderer.getMetrics()).toEqual({ width: 8, height: 17, baseline: 13 });
		expect([canvas.width, canvas.height]).toEqual([160, 68]);
		expect(renderer.setDevicePixelRatio(1.5)).toBe(true);
		expect([canvas.width, canvas.height]).toEqual([120, 51]);
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
		});
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
		queue,
	} as unknown as GPUDevice;
	return { device, submit, writeBuffer };
}
