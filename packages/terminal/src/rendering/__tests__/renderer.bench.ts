import { TerminalCellWidthCode, TerminalColorEncoding } from '@orchbee/terminal-protocol';
import { afterAll, beforeAll, bench, vi } from 'vitest';

import {
	type PackedRendererMetadata,
	type PackedRendererRow,
	type PackedRendererSource,
	WebGPURenderer,
} from '../renderer';

const BENCHMARK_COLUMNS = 160;
const BENCHMARK_ROWS = 50;
const BENCHMARK_OPTIONS = {
	iterations: 20,
	time: 1_000,
	warmupIterations: 5,
	warmupTime: 250,
} as const;

interface MutableBenchmarkCursor {
	blinking: boolean;
	column: number;
	row: number;
	shape: 'bar' | 'block' | 'underline';
	visible: boolean;
	wideTail: boolean;
}

interface MutableBenchmarkSource extends PackedRendererSource {
	readonly dirtyRows: Set<number>;
	readonly mutableCursor: MutableBenchmarkCursor;
	mutateLastRow(): void;
	moveCursor(): void;
}

interface RendererBenchmarkFixture {
	readonly renderer: WebGPURenderer;
	readonly source: MutableBenchmarkSource;
}

let stableFixture: RendererBenchmarkFixture;
let dirtyRowFixture: RendererBenchmarkFixture;
let cursorFixture: RendererBenchmarkFixture;

beforeAll(async () => {
	installRendererBenchmarkGlobals();
	[stableFixture, dirtyRowFixture, cursorFixture] = await Promise.all([
		createRendererBenchmarkFixture(false),
		createRendererBenchmarkFixture(false),
		createRendererBenchmarkFixture(true),
	]);
});

afterAll(() => {
	stableFixture.renderer.dispose();
	dirtyRowFixture.renderer.dispose();
	cursorFixture.renderer.dispose();
	vi.unstubAllGlobals();
});

bench(
	'prepares an unchanged 160x50 WebGPU frame',
	() => {
		stableFixture.renderer.renderPacked(stableFixture.source);
	},
	BENCHMARK_OPTIONS,
);

bench(
	'prepares one changed row in a 160x50 WebGPU frame',
	() => {
		dirtyRowFixture.source.mutateLastRow();
		dirtyRowFixture.renderer.renderPacked(dirtyRowFixture.source);
	},
	BENCHMARK_OPTIONS,
);

bench(
	'moves a block cursor in a 160x50 WebGPU frame',
	() => {
		cursorFixture.source.moveCursor();
		cursorFixture.renderer.renderPacked(cursorFixture.source);
	},
	BENCHMARK_OPTIONS,
);

async function createRendererBenchmarkFixture(
	cursorVisible: boolean,
): Promise<RendererBenchmarkFixture> {
	const renderer = new WebGPURenderer(document.createElement('canvas'), { devicePixelRatio: 1 });
	const source = createBenchmarkSource(cursorVisible);
	await renderer.initGPU();
	renderer.renderPacked(source);
	return { renderer, source };
}

function createBenchmarkSource(cursorVisible: boolean): MutableBenchmarkSource {
	const dirtyRows = new Set(Array.from({ length: BENCHMARK_ROWS }, (_, row) => row));
	const cells = Array.from({ length: BENCHMARK_ROWS }, (_, row) => benchmarkRowCells(row));
	const mutableCursor: MutableBenchmarkCursor = {
		blinking: false,
		column: 0,
		row: Math.floor(BENCHMARK_ROWS / 2),
		shape: 'block',
		visible: cursorVisible,
		wideTail: false,
	};
	const metadata = {
		columns: BENCHMARK_COLUMNS,
		cursor: mutableCursor,
		defaultColors: {
			backgroundRgb: 0x10_14_18,
			cursorRgb: null,
			foregroundRgb: 0xd8_de_e9,
		},
		rows: BENCHMARK_ROWS,
	} satisfies PackedRendererMetadata;
	const rows = cells.map(
		(rowCells, rowIndex): PackedRendererRow => ({
			backgroundColors: new Uint32Array(BENCHMARK_COLUMNS).fill(TerminalColorEncoding.default),
			get dirty() {
				return dirtyRows.has(rowIndex);
			},
			foregroundColors: new Uint32Array(BENCHMARK_COLUMNS).fill(TerminalColorEncoding.default),
			grapheme: (column) => rowCells[column] ?? '',
			styleFlags: new Uint16Array(BENCHMARK_COLUMNS),
			underlineColors: new Uint32Array(BENCHMARK_COLUMNS).fill(TerminalColorEncoding.none),
			underlineKinds: new Uint8Array(BENCHMARK_COLUMNS),
			widths: new Uint8Array(BENCHMARK_COLUMNS).fill(TerminalCellWidthCode.narrow),
		}),
	);
	let full = true;
	let alternate = false;
	return {
		dirtyRows,
		markRendered: () => {
			full = false;
			dirtyRows.clear();
		},
		metadata,
		mutableCursor,
		mutateLastRow: () => {
			alternate = !alternate;
			const lastRow = cells.at(-1);
			if (lastRow !== undefined) lastRow[BENCHMARK_COLUMNS - 1] = alternate ? 'X' : 'Y';
			dirtyRows.add(BENCHMARK_ROWS - 1);
		},
		moveCursor: () => {
			mutableCursor.column = (mutableCursor.column + 1) % BENCHMARK_COLUMNS;
		},
		get needsFullRedraw() {
			return full;
		},
		rows,
	};
}

function benchmarkRowCells(row: number): string[] {
	const prefix = `${String(row).padStart(2, '0')} `;
	const pattern = `${prefix}The quick brown fox jumps over the lazy dog 0123456789 `;
	return Array.from(
		pattern.repeat(Math.ceil(BENCHMARK_COLUMNS / pattern.length)).slice(0, BENCHMARK_COLUMNS),
	);
}

function installRendererBenchmarkGlobals(): void {
	const device = createBenchmarkDevice();
	vi.stubGlobal('window', { devicePixelRatio: 1 });
	vi.stubGlobal('document', { createElement: () => createCanvas() });
	vi.stubGlobal('GPUTextureUsage', { COPY_DST: 2, RENDER_ATTACHMENT: 4, TEXTURE_BINDING: 1 });
	vi.stubGlobal('GPUBufferUsage', { COPY_DST: 2, UNIFORM: 1, VERTEX: 4 });
	vi.stubGlobal('navigator', {
		gpu: {
			getPreferredCanvasFormat: () => 'bgra8unorm',
			requestAdapter: async () => ({ requestDevice: async () => device }),
		},
	});
}

function createBenchmarkDevice() {
	const renderPass = {
		draw: () => undefined,
		end: () => undefined,
		setBindGroup: () => undefined,
		setPipeline: () => undefined,
		setVertexBuffer: () => undefined,
	};
	return {
		createBindGroup: () => ({}),
		createBuffer: () => ({ destroy: () => undefined }),
		createCommandEncoder: () => ({
			beginRenderPass: () => renderPass,
			finish: () => ({}),
		}),
		createRenderPipeline: () => ({ getBindGroupLayout: () => ({}) }),
		createSampler: () => ({}),
		createShaderModule: () => ({}),
		createTexture: () => ({ createView: () => ({}), destroy: () => undefined }),
		destroy: () => undefined,
		limits: { maxTextureDimension2D: 8_192 },
		lost: new Promise<GPUDeviceLostInfo>(() => undefined),
		popErrorScope: async () => null,
		pushErrorScope: () => undefined,
		queue: {
			copyExternalImageToTexture: () => undefined,
			submit: () => undefined,
			writeBuffer: () => undefined,
			writeTexture: () => undefined,
		},
	};
}

function createCanvas() {
	const context = {
		beginPath: () => undefined,
		clearRect: () => undefined,
		clip: () => undefined,
		createImageData: (width: number, height: number) => ({
			data: new Uint8ClampedArray(width * height * 4),
			height,
			width,
		}),
		drawImage: () => undefined,
		fillRect: () => undefined,
		fillText: () => undefined,
		font: '10px sans-serif',
		getImageData: (_x: number, _y: number, width: number, height: number) => ({
			data: new Uint8ClampedArray(width * height * 4),
		}),
		globalAlpha: 1,
		imageSmoothingEnabled: true,
		letterSpacing: '0px',
		lineTo: () => undefined,
		measureText: (text: string) => ({
			actualBoundingBoxAscent: 9.5,
			actualBoundingBoxDescent: 0,
			fontBoundingBoxAscent: 13,
			fontBoundingBoxDescent: 4,
			width: text.length * 7.8,
		}),
		moveTo: () => undefined,
		putImageData: () => undefined,
		rect: () => undefined,
		restore: () => undefined,
		save: () => undefined,
		scale: () => undefined,
		setLineDash: () => undefined,
		setTransform: () => undefined,
		stroke: () => undefined,
		textRendering: 'auto',
		translate: () => undefined,
	};
	return {
		get ownerDocument() {
			return globalThis.document;
		},
		getContext: (kind: string) =>
			kind === 'webgpu'
				? {
						configure: () => undefined,
						getCurrentTexture: () => ({ createView: () => ({}) }),
					}
				: context,
		height: 0,
		remove: () => undefined,
		setAttribute: () => undefined,
		style: {},
		width: 0,
	};
}
