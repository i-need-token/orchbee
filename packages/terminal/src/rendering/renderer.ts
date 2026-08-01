/**
 * WebGPU Renderer for Terminal Display
 *
 * High-performance WebGPU-based renderer that draws the terminal using
 * Ghostty's WASM terminal emulator. Falls back to Canvas 2D when WebGPU
 * is unavailable.
 *
 * Features:
 * - Glyph atlas texture for efficient text rendering
 * - Instanced rendering for backgrounds, text, cursor, selection, scrollbar
 * - Dirty line optimization for 60 FPS
 * - DPI scaling
 * - All text styles (bold, italic, underline, strikethrough, etc.)
 * - Multiple cursor styles (block, underline, bar) with blinking
 * - Selection overlay rendering
 * - Scrollbar with fade animation
 * - Link underline rendering (OSC8 + regex URLs)
 */

import {
	TerminalCellWidthCode,
	TerminalColorEncoding,
	TerminalStyleFlag,
	TerminalUnderlineKindCode,
} from '@orchbee/terminal-protocol';

import type { SelectionManager } from '../selection';
import { applyFontMetricModifiers, type DetailedFontMetrics } from '../font-metrics';
import {
	cellConstraintWidth,
	cellConstraintWidthFromReader,
	constrainGlyph,
	glyphConstraintForCodepoint,
	type GlyphSize,
} from '../glyph-constraints';
import { drawGhosttySprite, isGhosttySprite, physicalSpriteGeometry } from '../ghostty-sprites';
import { searchMatchContainsCell, type TerminalSearchMatch } from '../search';
import { additionalSurfaceMargins, balanceGridPadding } from '../surface-size';
import {
	buildTextShapingPlan,
	type TextShapingCell,
	type TextShapingPlan,
} from '../buildTextShapingPlan';
import { CellFlags, UnderlineStyle } from '../types';
import type {
	FontMetrics,
	FontMetricModifiers,
	FontWeight,
	GhosttyCell,
	IRenderable,
	IScrollbackProvider,
	ITheme,
	RendererOptions,
	SelectionCoordinates,
	PaddingBalance,
	SurfacePadding,
} from '../types';

export type RendererHealthEvent =
	| { readonly state: 'ready'; readonly backend: 'webgpu' }
	| { readonly state: 'fallback'; readonly backend: 'canvas2d'; readonly reason: string }
	| { readonly state: 'lost'; readonly backend: 'webgpu'; readonly reason: string }
	| { readonly state: 'recovering'; readonly backend: 'webgpu'; readonly attempt: 1 }
	| { readonly state: 'recovery-failed'; readonly backend: 'webgpu'; readonly reason: string };

export type RendererFrameReason =
	| 'full'
	| 'dirty'
	| 'viewport'
	| 'cursor'
	| 'preedit'
	| 'font'
	| 'blink'
	| 'search'
	| 'scrollbar';

export interface RendererFrameTelemetry {
	readonly frameId: number;
	readonly backend: 'webgpu' | 'canvas2d';
	readonly timestamp: number;
	readonly durationMs: number;
	readonly reasons: readonly RendererFrameReason[];
	readonly forceAll: boolean;
	readonly viewportY: number;
	readonly rowsRendered: number;
	readonly cellsVisited: number;
	readonly backgroundInstances: number;
	readonly glyphInstances: number;
	readonly shapingRuns: number;
	readonly overlayInstances: number;
	readonly gpuDrawCalls: number;
	readonly vertexUploadBytes: number;
	readonly atlasUploadBytes: number;
	readonly atlasGlyphsCreated: number;
	readonly atlasGlyphCount: number;
	readonly atlasWidth: number;
	readonly atlasHeight: number;
	readonly atlasPressure: number;
}

export interface PackedRendererRow {
	readonly backgroundColors: Uint32Array;
	readonly dirty: boolean;
	readonly foregroundColors: Uint32Array;
	readonly styleFlags: Uint16Array;
	readonly underlineColors: Uint32Array;
	readonly underlineKinds: Uint8Array;
	readonly widths: Uint8Array;
	grapheme(column: number): string;
}

export interface PackedRendererMetadata {
	readonly columns: number;
	readonly cursor: {
		readonly blinking: boolean;
		readonly column: number;
		readonly row: number;
		readonly shape: 'bar' | 'block' | 'underline';
		readonly visible: boolean;
	};
	readonly defaultColors: {
		readonly backgroundRgb: number;
		readonly cursorRgb: number | null;
		readonly foregroundRgb: number;
	};
	readonly rows: number;
}

/** Packed projection source consumed without reconstructing per-cell objects. */
export interface PackedRendererSource {
	readonly metadata: PackedRendererMetadata | null;
	readonly needsFullRedraw: boolean;
	readonly rows: readonly PackedRendererRow[];
	markRendered(): void;
}

// ============================================================================
// Default Theme
// ============================================================================

export const DEFAULT_THEME: Required<ITheme> = {
	foreground: '#d4d4d4',
	background: '#1e1e1e',
	cursor: '#ffffff',
	cursorAccent: '#1e1e1e',
	selectionBackground: '#d4d4d4',
	selectionForeground: '#1e1e1e',
	searchMatchBackground: '#515c6a',
	searchMatchForeground: '#ffffff',
	searchActiveMatchBackground: '#f5a97f',
	searchActiveMatchForeground: '#1e1e1e',
	black: '#000000',
	red: '#cd3131',
	green: '#0dbc79',
	yellow: '#e5e510',
	blue: '#2472c8',
	magenta: '#bc3fbc',
	cyan: '#11a8cd',
	white: '#e5e5e5',
	brightBlack: '#666666',
	brightRed: '#f14c4c',
	brightGreen: '#23d18b',
	brightYellow: '#f5f543',
	brightBlue: '#3b8eea',
	brightMagenta: '#d670d6',
	brightCyan: '#29b8db',
	brightWhite: '#ffffff',
};

export interface PreeditCell {
	readonly text: string;
	readonly width: number;
}

export interface PreeditSelection {
	readonly start: number;
	readonly end: number;
}

interface PreeditLayout {
	readonly cells: readonly PreeditCell[];
	readonly start: number;
	readonly end: number;
	readonly row: number;
	readonly width: number;
	readonly selectionStart: number;
	readonly selectionEnd: number;
}

interface AtlasDirtyBounds {
	readonly minX: number;
	readonly minY: number;
	readonly maxX: number;
	readonly maxY: number;
}

interface GlyphAtlasEntry {
	readonly x: number;
	readonly y: number;
	readonly w: number;
	readonly h: number;
	readonly offsetX: number;
	readonly offsetY: number;
	readonly renderWidth: number;
	readonly renderHeight: number;
	readonly colored: boolean;
}

interface GlyphRasterLayout {
	readonly left: number;
	readonly ascent: number;
	readonly scaleX: number;
	readonly scaleY: number;
	readonly target: GlyphSize;
	readonly targetTop: number;
}

interface RowTextShaping {
	readonly texts: readonly string[];
	readonly plan: TextShapingPlan;
}

interface PackedRowTextShaping {
	readonly texts: readonly string[];
	readonly plan: TextShapingPlan;
}

type RenderedCursorStyle = 'block' | 'block_hollow' | 'underline' | 'bar';

interface RectangleGeometry {
	readonly x: number;
	readonly y: number;
	readonly width: number;
	readonly height: number;
}

const COLOR_GLYPH_CANDIDATE = /[\p{Extended_Pictographic}\p{Regional_Indicator}\u{FE0F}\u{20E3}]/u;
const ZERO_PADDING: SurfacePadding = { top: 0, bottom: 0, left: 0, right: 0 };

// ============================================================================
// Color Helpers
// ============================================================================

function hexToRGB(color: string): [number, number, number] {
	const hex = color.startsWith('#') ? color.slice(1) : color;
	if (/^[\da-fA-F]{3}$/.test(hex)) {
		return [
			Number.parseInt(hex.charAt(0).repeat(2), 16),
			Number.parseInt(hex.charAt(1).repeat(2), 16),
			Number.parseInt(hex.charAt(2).repeat(2), 16),
		];
	}
	if (/^[\da-fA-F]{6}(?:[\da-fA-F]{2})?$/.test(hex)) {
		return [
			Number.parseInt(hex.slice(0, 2), 16),
			Number.parseInt(hex.slice(2, 4), 16),
			Number.parseInt(hex.slice(4, 6), 16),
		];
	}
	const rgb = color.match(/^rgba?\(\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)/);
	if (rgb) {
		return [
			Math.min(255, Number(rgb[1])),
			Math.min(255, Number(rgb[2])),
			Math.min(255, Number(rgb[3])),
		];
	}
	throw new Error(`Unsupported renderer color: ${color}`);
}

function canvasFont(
	style: 'normal' | 'italic',
	weight: FontWeight,
	size: number,
	family: string,
): string {
	const stylePrefix = style === 'italic' ? 'italic ' : '';
	return `${stylePrefix}${weight} ${size}px ${family}`;
}

function snapToDevicePixel(value: number, devicePixelRatio: number): number {
	return Math.round(value * devicePixelRatio) / devicePixelRatio;
}

function packedColorRgb(value: number, defaultRgb: number): number {
	return (value & 0xff00_0000) === 0 ? value & TerminalColorEncoding.rgbMask : defaultRgb;
}

function packedOptionalColorRgb(value: number, defaultRgb: number): number | null {
	if (value === TerminalColorEncoding.none) return null;
	return packedColorRgb(value, defaultRgb);
}

function rgbChannels(value: number): [number, number, number] {
	return [(value >>> 16) & 0xff, (value >>> 8) & 0xff, value & 0xff];
}

function packedCellWidth(value: number): number {
	if (value === TerminalCellWidthCode.wide) return 2;
	if (value === TerminalCellWidthCode.narrow) return 1;
	return 0;
}

// ============================================================================
// WebGPU Shaders
// ============================================================================

const CELL_BG_SHADER = /* wgsl */ `
struct CellBgInstance {
  @location(0) pos: vec2f,
  @location(1) size: vec2f,
  @location(2) color: vec4f,
};

struct Uniforms {
  viewportSize: vec2f,
  devicePixelRatio: f32,
  _pad: f32,
};

struct VertexOutput {
  @builtin(position) position: vec4f,
  @location(0) color: vec4f,
};

@group(0) @binding(0) var<uniform> uniforms: Uniforms;

@vertex
fn vs(input: CellBgInstance, @builtin(vertex_index) vertexIndex: u32) -> VertexOutput {
  let corners = array<vec2f, 6>(
    vec2f(0.0, 0.0), vec2f(1.0, 0.0), vec2f(0.0, 1.0),
    vec2f(1.0, 0.0), vec2f(1.0, 1.0), vec2f(0.0, 1.0),
  );
  let pixel = (input.pos + corners[vertexIndex] * input.size) * uniforms.devicePixelRatio;
  var output: VertexOutput;
  output.position = vec4f(
    pixel.x / uniforms.viewportSize.x * 2.0 - 1.0,
    1.0 - pixel.y / uniforms.viewportSize.y * 2.0,
    0.0,
    1.0,
  );
  output.color = input.color;
  return output;
}

@fragment
fn fs(input: VertexOutput) -> @location(0) vec4f {
  return input.color;
}
`;

const CELL_TEXT_SHADER = /* wgsl */ `
struct CellTextInstance {
  @location(0) pos: vec2f,
  @location(1) size: vec2f,
  @location(2) uvPos: vec2f,
  @location(3) uvSize: vec2f,
  @location(4) color: vec4f,
  @location(5) colored: f32,
};

struct Uniforms {
  viewportSize: vec2f,
  devicePixelRatio: f32,
  _pad: f32,
};

struct VertexOutput {
  @builtin(position) position: vec4f,
  @location(0) uv: vec2f,
  @location(1) color: vec4f,
  @location(2) colored: f32,
};

@group(0) @binding(0) var<uniform> uniforms: Uniforms;
@group(0) @binding(1) var atlasSampler: sampler;
@group(0) @binding(2) var atlasTexture: texture_2d<f32>;

@vertex
fn vs(input: CellTextInstance, @builtin(vertex_index) vertexIndex: u32) -> VertexOutput {
  let corners = array<vec2f, 6>(
    vec2f(0.0, 0.0), vec2f(1.0, 0.0), vec2f(0.0, 1.0),
    vec2f(1.0, 0.0), vec2f(1.0, 1.0), vec2f(0.0, 1.0),
  );
  let corner = corners[vertexIndex];
  let pixel = (input.pos + corner * input.size) * uniforms.devicePixelRatio;
  var output: VertexOutput;
  output.position = vec4f(
    pixel.x / uniforms.viewportSize.x * 2.0 - 1.0,
    1.0 - pixel.y / uniforms.viewportSize.y * 2.0,
    0.0,
    1.0,
  );
  output.uv = input.uvPos + corner * input.uvSize;
  output.color = input.color;
  output.colored = input.colored;
  return output;
}

@fragment
fn fs(input: VertexOutput) -> @location(0) vec4f {
  let texel = textureSample(atlasTexture, atlasSampler, input.uv);
  let alpha = texel.a * input.color.a;
  let color = select(input.color.rgb, texel.rgb, input.colored > 0.5);
  return vec4f(color * alpha, alpha);
}
`;

// ============================================================================
// WebGPURenderer Class
// ============================================================================

function pushRect(
	target: number[],
	x: number,
	y: number,
	width: number,
	height: number,
	color: readonly [number, number, number],
	alpha = 1,
): void {
	target.push(x, y, width, height, color[0] / 255, color[1] / 255, color[2] / 255, alpha);
}

export class WebGPURenderer {
	private canvas: HTMLCanvasElement;
	private device: GPUDevice | null = null;
	private context: GPUCanvasContext | null = null;
	private format: GPUTextureFormat;

	// Fallback Canvas 2D renderer
	private ctx2d: CanvasRenderingContext2D | null = null;
	private useWebGPU = false;

	// Font
	private fontSize: number;
	private fontFamily: string;
	private fontWeight: FontWeight;
	private fontWeightBold: FontWeight;
	private fontLigatures: boolean;
	private fontMetricModifiers: FontMetricModifiers;
	private metrics: FontMetrics;
	private detailedMetrics: DetailedFontMetrics;
	private paddingBalance: PaddingBalance;

	// Cursor
	private cursorBlink: boolean;
	private cursorVisible = true;
	private focused = false;
	private cursorAppearanceDirty = false;
	private cursorBlinkInterval: ReturnType<typeof setInterval> | undefined;
	private textBlinkVisible = true;
	private textBlinkActive = false;
	private textBlinkDirty = false;
	private textBlinkInterval: ReturnType<typeof setInterval> | undefined;
	private suspended = false;
	private requestRenderCallback: (() => void) | undefined;
	private healthChangeCallback: ((event: RendererHealthEvent) => void) | undefined;
	private telemetryCallback: ((telemetry: RendererFrameTelemetry) => void) | undefined;
	private lastTelemetry: RendererFrameTelemetry | null = null;
	private frameId = 0;
	private frameRowsRendered = 0;
	private frameCellsVisited = 0;
	private frameVertexUploadBytes = 0;
	private frameAtlasUploadBytes = 0;
	private frameAtlasGlyphsCreated = 0;
	private frameBackgroundInstances = 0;
	private frameGlyphInstances = 0;
	private frameShapingRuns = 0;
	private frameOverlayInstances = 0;
	private frameGpuDrawCalls = 0;
	private recoveryInFlight = false;
	private disposed = false;
	private lastCursorPosition: { x: number; y: number } = { x: 0, y: 0 };
	private preeditCells: readonly PreeditCell[] = [];
	private preeditSelection: PreeditSelection = { start: 0, end: 0 };
	private preeditDirty = false;
	private fontDirty = false;

	/** Get the last rendered cursor position (used by Terminal for onCursorMove). */
	getCursorPosition(): { x: number; y: number } {
		return this.lastCursorPosition;
	}

	// Theme
	private theme: Required<ITheme>;
	private palette: string[];

	/** Get the ANSI color palette (16 colors). */
	getPalette(): readonly string[] {
		return this.palette;
	}

	// DPI
	private devicePixelRatio: number;
	private renderedColumns = 0;
	private renderedRows = 0;
	private allowTransparency: boolean;

	// Viewport
	private lastViewportY = 0;
	private lastScrollbarOpacity = 0;

	// Buffer reference for grapheme lookups
	private currentBuffer: IRenderable | null = null;

	// Selection
	private selectionManager: SelectionManager | undefined;
	private currentSelectionCoords: SelectionCoordinates | null = null;
	private projectedSelectionCoords: SelectionCoordinates | null = null;
	private searchMatches: readonly TerminalSearchMatch[] = [];
	private searchMatchesByRow = new Map<number, TerminalSearchMatch[]>();
	private searchMatchesVersion = 0;
	private indexedSearchMatchesVersion = -1;
	private indexedSearchTop = -1;
	private indexedSearchRows = -1;
	private activeSearchMatch: TerminalSearchMatch | null = null;
	private searchDirty = false;
	private currentAbsoluteTop = 0;

	// Link hover
	private hoveredHyperlinkId = 0;
	private previousHoveredHyperlinkId = 0;
	private hoveredLinkRange: { startX: number; startY: number; endX: number; endY: number } | null =
		null;
	private previousHoveredLinkRange: {
		startX: number;
		startY: number;
		endX: number;
		endY: number;
	} | null = null;

	// Glyph atlas (Canvas 2D for rasterization, uploaded as WebGPU texture)
	private atlasCanvas: HTMLCanvasElement;
	private atlasCtx: CanvasRenderingContext2D;
	private atlasTexture: GPUTexture | null = null;
	private atlasSampler: GPUSampler | null = null;
	private glyphMap: Map<string, GlyphAtlasEntry> = new Map();
	private atlasX = 0;
	private atlasY = 0;
	private atlasRowHeight = 0;
	private atlasDirty = false;
	private atlasDirtyBounds: AtlasDirtyBounds | null = null;

	// WebGPU pipeline resources
	private bgPipeline: GPURenderPipeline | null = null;
	private textPipeline: GPURenderPipeline | null = null;
	private uniformBuffer: GPUBuffer | null = null;
	private bgVertexBuffer: GPUBuffer | null = null;
	private bgVertexBufferSize = 0;
	private textVertexBuffer: GPUBuffer | null = null;
	private textVertexBufferSize = 0;
	private overlayVertexBuffer: GPUBuffer | null = null;
	private overlayVertexBufferSize = 0;

	constructor(canvas: HTMLCanvasElement, options: RendererOptions = {}) {
		this.canvas = canvas;
		this.format = 'bgra8unorm';
		this.fontSize = options.fontSize ?? 15;
		this.fontFamily = options.fontFamily ?? 'monospace';
		this.fontWeight = options.fontWeight ?? 'normal';
		this.fontWeightBold = options.fontWeightBold ?? 'bold';
		this.fontLigatures = options.fontLigatures ?? true;
		this.fontMetricModifiers = { ...options.fontMetricModifiers };
		this.paddingBalance = options.paddingBalance ?? false;
		this.cursorBlink = options.cursorBlink ?? false;
		this.theme = { ...DEFAULT_THEME, ...options.theme };
		this.devicePixelRatio = options.devicePixelRatio ?? window.devicePixelRatio ?? 1;
		this.allowTransparency = options.allowTransparency ?? false;

		this.palette = this.buildPalette();

		this.detailedMetrics = this.measureFont();
		this.metrics = publicFontMetrics(this.detailedMetrics);

		// Create atlas canvas for glyph rasterization
		this.atlasCanvas = document.createElement('canvas');
		this.atlasCanvas.width = 2048;
		this.atlasCanvas.height = 2048;
		const atlasCtx = this.atlasCanvas.getContext('2d');
		if (!atlasCtx) throw new Error('Failed to create atlas canvas context');
		this.atlasCtx = atlasCtx;
		this.atlasCtx.imageSmoothingEnabled = false;
		this.atlasCtx.clearRect(0, 0, 2048, 2048);

		if (this.cursorBlink) this.startCursorBlink();
	}

	/** Initialize WebGPU. Call after construction. */
	async initGPU(): Promise<boolean> {
		return await this.initializeGPU(true);
	}

	private async initializeGPU(allowFallback: boolean): Promise<boolean> {
		if (!navigator.gpu) {
			if (!allowFallback) throw new Error('WebGPU became unavailable during recovery');
			return this.activateFallback('webgpu-unavailable');
		}

		try {
			const adapter = await navigator.gpu.requestAdapter();
			if (!adapter) throw new Error('No WebGPU adapter is available');

			this.format = navigator.gpu.getPreferredCanvasFormat();
			const device = await adapter.requestDevice();
			if (this.disposed) {
				device.destroy();
				throw new Error('Renderer was disposed during WebGPU initialization');
			}
			this.device = device;
			void device.lost.then((info) => this.handleDeviceLost(device, info));

			// Validate all device resources before locking this canvas to WebGPU so
			// Canvas 2D remains available if initialization fails.
			device.pushErrorScope('validation');
			await this.createPipelines();
			const pipelineError = await device.popErrorScope();
			if (pipelineError)
				throw new Error(`WebGPU pipeline validation failed: ${pipelineError.message}`);

			// Create atlas texture and sampler
			this.createAtlasTexture();
			this.createAtlasSampler();

			// Create uniform buffer
			this.createUniformBuffer();

			this.context ??= this.canvas.getContext('webgpu');
			if (!this.context) {
				throw new Error('Failed to get WebGPU rendering context');
			}
			this.context.configure({
				device: this.device,
				format: this.format,
				alphaMode: 'premultiplied',
			});

			this.useWebGPU = true;
			this.ctx2d = null;
			this.emitHealth({ state: 'ready', backend: 'webgpu' });
			this.requestRenderCallback?.();
			return true;
		} catch (error) {
			if (!allowFallback || this.context) throw error;
			return this.activateFallback(
				error instanceof Error ? error.message : 'webgpu-initialization-failed',
			);
		}
	}

	private activateFallback(reason: string): false {
		const device = this.device;
		this.releaseGPUResources();
		this.device = null;
		device?.destroy();
		this.initFallback2D();
		this.emitHealth({ state: 'fallback', backend: 'canvas2d', reason });
		return false;
	}

	private handleDeviceLost(device: GPUDevice, info: GPUDeviceLostInfo): void {
		if (this.disposed || info.reason === 'destroyed' || device !== this.device) return;
		this.useWebGPU = false;
		this.emitHealth({
			state: 'lost',
			backend: 'webgpu',
			reason: info.message || info.reason,
		});
		void this.recoverWebGPU();
	}

	private async recoverWebGPU(): Promise<void> {
		if (this.disposed || this.recoveryInFlight) return;
		this.recoveryInFlight = true;
		this.emitHealth({ state: 'recovering', backend: 'webgpu', attempt: 1 });
		this.releaseGPUResources();
		try {
			await this.initializeGPU(false);
		} catch (error) {
			if (!this.disposed) {
				this.releaseGPUResources();
				this.device = null;
				this.emitHealth({
					state: 'recovery-failed',
					backend: 'webgpu',
					reason: error instanceof Error ? error.message : 'webgpu-recovery-failed',
				});
			}
		} finally {
			this.recoveryInFlight = false;
		}
	}

	private releaseGPUResources(): void {
		this.atlasTexture?.destroy();
		this.uniformBuffer?.destroy();
		this.bgVertexBuffer?.destroy();
		this.textVertexBuffer?.destroy();
		this.overlayVertexBuffer?.destroy();
		this.atlasTexture = null;
		this.atlasSampler = null;
		this.uniformBuffer = null;
		this.bgVertexBuffer = null;
		this.textVertexBuffer = null;
		this.overlayVertexBuffer = null;
		this.bgVertexBufferSize = 0;
		this.textVertexBufferSize = 0;
		this.overlayVertexBufferSize = 0;
		this.bgPipeline = null;
		this.textPipeline = null;
		this.markEntireAtlasDirty();
	}

	private initFallback2D(): void {
		const ctx = this.canvas.getContext('2d', { alpha: true });
		if (!ctx) throw new Error('Failed to get 2D rendering context');
		this.ctx2d = ctx;
		this.useWebGPU = false;
	}

	// ==========================================================================
	// Font Metrics
	// ==========================================================================

	private measureFont(): DetailedFontMetrics {
		const canvas = document.createElement('canvas');
		const ctx = canvas.getContext('2d');
		if (!ctx) throw new Error('Failed to create font measurement context');
		ctx.font = canvasFont('normal', this.fontWeight, this.fontSize, this.fontFamily);

		const widthMetrics = ctx.measureText('M');
		const width = Math.max(1, Math.round(widthMetrics.width));
		const ascent =
			widthMetrics.fontBoundingBoxAscent ||
			widthMetrics.actualBoundingBoxAscent ||
			this.fontSize * 0.8;
		const descent =
			widthMetrics.fontBoundingBoxDescent ||
			widthMetrics.actualBoundingBoxDescent ||
			this.fontSize * 0.2;
		const faceHeight = Math.max(1, ascent + descent);
		const height = Math.max(1, Math.round(faceHeight));
		const baselineFromBottom = Math.round(descent - (height - faceHeight) / 2);
		const baseline = Math.min(height, Math.max(0, height - baselineFromBottom));

		const underlineThickness = 1;
		const capHeight = 0.75 * ascent;
		return applyFontMetricModifiers(
			{
				cellWidth: width,
				cellHeight: height,
				cellBaseline: baselineFromBottom,
				underlinePosition: Math.max(0, Math.round(baseline + underlineThickness)),
				underlineThickness,
				strikethroughPosition: Math.max(0, Math.round(height / 2)),
				strikethroughThickness: 1,
				overlinePosition: 0,
				overlineThickness: 1,
				boxThickness: 1,
				cursorThickness: 1 / this.devicePixelRatio,
				cursorHeight: height,
				iconHeight: faceHeight,
				iconHeightSingle: (2 * capHeight + faceHeight) / 3,
				faceWidth: Math.max(1, widthMetrics.width),
				faceHeight,
				faceY: baselineFromBottom - descent,
			},
			this.fontMetricModifiers,
		);
	}

	remeasureFont(): void {
		this.detailedMetrics = this.measureFont();
		this.metrics = publicFontMetrics(this.detailedMetrics);
	}

	private buildPalette(): string[] {
		return [
			this.theme.black,
			this.theme.red,
			this.theme.green,
			this.theme.yellow,
			this.theme.blue,
			this.theme.magenta,
			this.theme.cyan,
			this.theme.white,
			this.theme.brightBlack,
			this.theme.brightRed,
			this.theme.brightGreen,
			this.theme.brightYellow,
			this.theme.brightBlue,
			this.theme.brightMagenta,
			this.theme.brightCyan,
			this.theme.brightWhite,
		];
	}

	// ==========================================================================
	// WebGPU Pipeline Setup
	// ==========================================================================

	private async createPipelines(): Promise<void> {
		if (!this.device) return;

		// Background pipeline
		const bgModule = this.device.createShaderModule({ code: CELL_BG_SHADER });
		this.bgPipeline = this.device.createRenderPipeline({
			layout: 'auto',
			vertex: {
				module: bgModule,
				entryPoint: 'vs',
				buffers: [
					{
						arrayStride: 32, // 8 floats * 4 bytes
						stepMode: 'instance',
						attributes: [
							{ shaderLocation: 0, offset: 0, format: 'float32x2' }, // pos
							{ shaderLocation: 1, offset: 8, format: 'float32x2' }, // cellSize
							{ shaderLocation: 2, offset: 16, format: 'float32x4' }, // color
						],
					},
				],
			},
			fragment: {
				module: bgModule,
				entryPoint: 'fs',
				targets: [
					{
						format: this.format,
						blend: {
							color: { srcFactor: 'src-alpha', dstFactor: 'one-minus-src-alpha' },
							alpha: { srcFactor: 'one', dstFactor: 'one-minus-src-alpha' },
						},
					},
				],
			},
			primitive: { topology: 'triangle-list' },
		});

		// Text pipeline
		const textModule = this.device.createShaderModule({ code: CELL_TEXT_SHADER });
		this.textPipeline = this.device.createRenderPipeline({
			layout: 'auto',
			vertex: {
				module: textModule,
				entryPoint: 'vs',
				buffers: [
					{
						arrayStride: 52, // 13 floats * 4 bytes
						stepMode: 'instance',
						attributes: [
							{ shaderLocation: 0, offset: 0, format: 'float32x2' }, // pos
							{ shaderLocation: 1, offset: 8, format: 'float32x2' }, // cellSize
							{ shaderLocation: 2, offset: 16, format: 'float32x2' }, // uvPos
							{ shaderLocation: 3, offset: 24, format: 'float32x2' }, // uvSize
							{ shaderLocation: 4, offset: 32, format: 'float32x4' }, // color
							{ shaderLocation: 5, offset: 48, format: 'float32' }, // colored glyph
						],
					},
				],
			},
			fragment: {
				module: textModule,
				entryPoint: 'fs',
				targets: [
					{
						format: this.format,
						blend: {
							color: { srcFactor: 'one', dstFactor: 'one-minus-src-alpha' },
							alpha: { srcFactor: 'one', dstFactor: 'one-minus-src-alpha' },
						},
					},
				],
			},
			primitive: { topology: 'triangle-list' },
		});
	}

	private createAtlasTexture(): void {
		if (!this.device) return;
		this.atlasTexture = this.device.createTexture({
			size: [this.atlasCanvas.width, this.atlasCanvas.height],
			format: 'rgba8unorm',
			usage:
				GPUTextureUsage.TEXTURE_BINDING |
				GPUTextureUsage.COPY_DST |
				GPUTextureUsage.RENDER_ATTACHMENT,
		});
		this.markEntireAtlasDirty();
	}

	private createAtlasSampler(): void {
		if (!this.device) return;
		this.atlasSampler = this.device.createSampler({
			magFilter: 'linear',
			minFilter: 'linear',
		});
	}

	private createUniformBuffer(): void {
		if (!this.device) return;
		this.uniformBuffer = this.device.createBuffer({
			size: 16, // vec2f + f32 + f32 = 16 bytes
			usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
		});
	}

	private uploadVertexData(
		kind: 'background' | 'text' | 'overlay',
		data: readonly number[],
	): GPUBuffer | null {
		if (!this.device || data.length === 0) return null;

		const requiredSize = data.length * Float32Array.BYTES_PER_ELEMENT;
		let currentBuffer: GPUBuffer | null;
		let currentSize: number;
		if (kind === 'background') {
			currentBuffer = this.bgVertexBuffer;
			currentSize = this.bgVertexBufferSize;
		} else if (kind === 'text') {
			currentBuffer = this.textVertexBuffer;
			currentSize = this.textVertexBufferSize;
		} else {
			currentBuffer = this.overlayVertexBuffer;
			currentSize = this.overlayVertexBufferSize;
		}

		let buffer = currentBuffer;
		let size = currentSize;
		if (!buffer || size < requiredSize) {
			buffer?.destroy();
			size = 256;
			while (size < requiredSize) size *= 2;
			buffer = this.device.createBuffer({
				size,
				usage: GPUBufferUsage.VERTEX | GPUBufferUsage.COPY_DST,
			});
			if (kind === 'background') {
				this.bgVertexBuffer = buffer;
				this.bgVertexBufferSize = size;
			} else if (kind === 'text') {
				this.textVertexBuffer = buffer;
				this.textVertexBufferSize = size;
			} else {
				this.overlayVertexBuffer = buffer;
				this.overlayVertexBufferSize = size;
			}
		}

		this.device.queue.writeBuffer(buffer, 0, new Float32Array(data));
		this.frameVertexUploadBytes += requiredSize;
		return buffer;
	}

	// ==========================================================================
	// Glyph Atlas
	// ==========================================================================

	private getGlyphKey(
		text: string,
		flags: number,
		width: number,
		codepoint: number,
		constraintWidth: 1 | 2,
	): string {
		const bold = (flags & CellFlags.BOLD) !== 0 ? 'b' : '';
		const italic = (flags & CellFlags.ITALIC) !== 0 ? 'i' : '';
		return JSON.stringify([text, width, codepoint, constraintWidth, `${bold}${italic}`]);
	}

	private ensureGlyph(
		text: string,
		flags: number,
		width: number,
		codepoint = text.codePointAt(0) ?? 0,
		constraintWidth: 1 | 2 = width >= 2 ? 2 : 1,
	): GlyphAtlasEntry | null {
		const key = this.getGlyphKey(text, flags, width, codepoint, constraintWidth);
		const existing = this.glyphMap.get(key);
		if (existing) return existing;

		// Render glyph to atlas
		// The terminal geometry is expressed in CSS pixels, but the atlas is
		// sampled by a physical-pixel WebGPU target. Rasterize glyphs at the same
		// device scale so high-DPI displays never enlarge a low-resolution mask.
		const rasterScale = this.devicePixelRatio;
		const padding = Math.max(2, Math.ceil(2 * rasterScale));

		const fontStyle = (flags & CellFlags.ITALIC) !== 0 ? 'italic' : 'normal';
		const fontWeight = (flags & CellFlags.BOLD) !== 0 ? this.fontWeightBold : this.fontWeight;
		this.atlasCtx.font = canvasFont(
			fontStyle,
			fontWeight,
			this.fontSize * rasterScale,
			this.fontFamily,
		);
		this.atlasCtx.textBaseline = 'alphabetic';
		this.atlasCtx.textAlign = 'left';
		const inkPadding = Math.max(1, Math.ceil(rasterScale));
		const raster = isGhosttySprite(codepoint)
			? {
					kind: 'sprite' as const,
					geometry: physicalSpriteGeometry(this.detailedMetrics, rasterScale, width),
				}
			: {
					kind: 'font' as const,
					layout: this.glyphRasterLayout(
						this.atlasCtx.measureText(text),
						rasterScale,
						codepoint,
						constraintWidth,
					),
				};
		const w =
			raster.kind === 'sprite'
				? raster.geometry.width + inkPadding * 2
				: Math.max(1, Math.ceil(raster.layout.target.width * rasterScale) + inkPadding * 2);
		const h =
			raster.kind === 'sprite'
				? raster.geometry.height + inkPadding * 2
				: Math.max(1, Math.ceil(raster.layout.target.height * rasterScale) + inkPadding * 2);

		// Check if we need a new row
		if (this.atlasX + w + padding > this.atlasCanvas.width) {
			this.atlasX = 0;
			this.atlasY += this.atlasRowHeight;
			this.atlasRowHeight = 0;
		}

		// A single glyph can exceed either axis before normal row packing makes
		// the atlas vertically full, so grow until the complete reservation fits.
		while (
			w + padding > this.atlasCanvas.width ||
			this.atlasY + h + padding > this.atlasCanvas.height
		) {
			if (!this.growAtlas()) return null;
		}

		// Render the full ink bounds with a transparent edge so filtering never
		// clips negative bearings, italic overhang, ascenders, or descenders.
		this.atlasCtx.fillStyle = '#ffffff';
		this.atlasCtx.save();
		if (raster.kind === 'sprite') {
			drawGhosttySprite(
				this.atlasCtx,
				codepoint,
				this.atlasX + inkPadding,
				this.atlasY + inkPadding,
				raster.geometry.width,
				raster.geometry.height,
				raster.geometry.boxThickness,
			);
		} else {
			this.atlasCtx.translate(this.atlasX + inkPadding, this.atlasY + inkPadding);
			this.atlasCtx.scale(raster.layout.scaleX, raster.layout.scaleY);
			this.atlasCtx.fillText(text, raster.layout.left, raster.layout.ascent);
		}
		this.atlasCtx.restore();

		const offsetX =
			raster.kind === 'sprite'
				? -inkPadding / rasterScale
				: raster.layout.target.x - inkPadding / rasterScale;
		const offsetY =
			raster.kind === 'sprite'
				? -inkPadding / rasterScale
				: raster.layout.targetTop - inkPadding / rasterScale;
		const entry: GlyphAtlasEntry = {
			x: this.atlasX,
			y: this.atlasY,
			w,
			h,
			offsetX: snapToDevicePixel(offsetX, rasterScale),
			offsetY: snapToDevicePixel(offsetY, rasterScale),
			renderWidth: w / rasterScale,
			renderHeight: h / rasterScale,
			colored:
				raster.kind === 'sprite' ? false : this.isColorGlyph(text, this.atlasX, this.atlasY, w, h),
		};
		this.glyphMap.set(key, entry);
		this.frameAtlasGlyphsCreated++;

		this.atlasX += w + padding;
		this.atlasRowHeight = Math.max(this.atlasRowHeight, h + padding);
		this.markAtlasDirty(entry.x, entry.y, entry.w, entry.h);

		return entry;
	}

	private glyphRasterLayout(
		measured: TextMetrics,
		rasterScale: number,
		codepoint: number,
		constraintWidth: 1 | 2,
	): GlyphRasterLayout {
		const baseline = this.metrics.baseline * rasterScale;
		const cellHeight = this.metrics.height * rasterScale;
		const measuredAscent = measured.actualBoundingBoxAscent;
		const measuredDescent = measured.actualBoundingBoxDescent;
		const hasVerticalBounds =
			Number.isFinite(measuredAscent) &&
			Number.isFinite(measuredDescent) &&
			measuredAscent + measuredDescent > 0;
		const left = Number.isFinite(measured.actualBoundingBoxLeft)
			? Math.max(0, measured.actualBoundingBoxLeft)
			: 0;
		const measuredRight = measured.actualBoundingBoxRight;
		const right =
			Number.isFinite(measuredRight) && measuredRight > 0
				? measuredRight
				: Math.max(this.metrics.width * rasterScale, measured.width);
		const ascent = hasVerticalBounds ? measuredAscent : baseline;
		const descent = hasVerticalBounds ? measuredDescent : Math.max(0, cellHeight - baseline);
		const original: GlyphSize = {
			width: (left + right) / rasterScale,
			height: (ascent + descent) / rasterScale,
			x: -left / rasterScale,
			y: this.detailedMetrics.cellBaseline - descent / rasterScale,
		};
		const constrained = constrainGlyph(
			original,
			this.detailedMetrics,
			glyphConstraintForCodepoint(codepoint),
			constraintWidth,
		);
		const target: GlyphSize = {
			...constrained,
			x: constrained.x + (this.detailedMetrics.cellWidth - this.detailedMetrics.faceWidth) / 2,
		};
		return {
			left,
			ascent,
			scaleX: target.width / original.width,
			scaleY: target.height / original.height,
			target,
			targetTop: this.detailedMetrics.cellHeight - target.y - target.height,
		};
	}

	private isColorGlyph(text: string, x: number, y: number, width: number, height: number): boolean {
		if (!COLOR_GLYPH_CANDIDATE.test(text)) return false;
		try {
			const pixels = this.atlasCtx.getImageData(x, y, width, height).data;
			for (let index = 0; index < pixels.length; index += 4) {
				const alpha = pixels[index + 3] ?? 0;
				if (alpha === 0) continue;
				const red = pixels[index] ?? 255;
				const green = pixels[index + 1] ?? 255;
				const blue = pixels[index + 2] ?? 255;
				if (red !== 255 || green !== 255 || blue !== 255) return true;
			}
		} catch {
			return false;
		}
		return false;
	}

	private growAtlas(): boolean {
		const maximum = this.device?.limits.maxTextureDimension2D ?? 8192;
		const width = Math.min(this.atlasCanvas.width * 2, maximum);
		const height = Math.min(this.atlasCanvas.height * 2, maximum);
		if (width === this.atlasCanvas.width && height === this.atlasCanvas.height) return false;

		const previousContext = this.atlasCtx;
		const canvas = document.createElement('canvas');
		canvas.width = width;
		canvas.height = height;
		const context = canvas.getContext('2d');
		if (!context) return false;
		context.imageSmoothingEnabled = false;
		context.drawImage(this.atlasCanvas, 0, 0);
		context.font = previousContext.font;
		context.textAlign = previousContext.textAlign;
		context.textBaseline = previousContext.textBaseline;
		context.direction = previousContext.direction;
		this.atlasCanvas = canvas;
		this.atlasCtx = context;
		this.atlasTexture?.destroy();
		this.atlasTexture = null;
		this.createAtlasTexture();
		return true;
	}

	private markAtlasDirty(x: number, y: number, width: number, height: number): void {
		const minX = Math.max(0, Math.floor(x));
		const minY = Math.max(0, Math.floor(y));
		const maxX = Math.min(this.atlasCanvas.width, Math.ceil(x + width));
		const maxY = Math.min(this.atlasCanvas.height, Math.ceil(y + height));
		if (maxX <= minX || maxY <= minY) return;

		const current = this.atlasDirtyBounds;
		this.atlasDirtyBounds = current
			? {
					minX: Math.min(current.minX, minX),
					minY: Math.min(current.minY, minY),
					maxX: Math.max(current.maxX, maxX),
					maxY: Math.max(current.maxY, maxY),
				}
			: { minX, minY, maxX, maxY };
		this.atlasDirty = true;
	}

	private markEntireAtlasDirty(): void {
		this.atlasDirtyBounds = {
			minX: 0,
			minY: 0,
			maxX: this.atlasCanvas.width,
			maxY: this.atlasCanvas.height,
		};
		this.atlasDirty = true;
	}

	private prepareGlyphs(
		buffer: IRenderable,
		rows: number,
		viewportY: number,
		scrollbackProvider: IScrollbackProvider | undefined,
		scrollbackLength: number,
		cursor: { readonly x: number; readonly y: number },
		cursorEnabled: boolean,
		preeditLayout: PreeditLayout | null,
	): void {
		for (let y = 0; y < rows; y++) {
			const line = this.fetchLine(buffer, y, viewportY, scrollbackProvider, scrollbackLength);
			if (!line) continue;
			const shaping = this.buildRowTextShaping(
				buffer,
				line,
				y,
				viewportY,
				scrollbackProvider,
				scrollbackLength,
				cursor,
				cursorEnabled,
				preeditLayout,
			);
			for (let x = 0; x < line.length; x++) {
				const cell = line[x];
				if (!cell || cell.width === 0 || (cell.flags & CellFlags.INVISIBLE) !== 0) continue;
				if ((cell.flags & CellFlags.BLINK) !== 0 && !this.textBlinkVisible) continue;
				const shapingSpan = shaping.plan[x];
				if (shapingSpan && shapingSpan.start !== x) continue;
				const text = shapingSpan?.text ?? shaping.texts[x] ?? '';
				if (text) {
					this.ensureGlyph(
						text,
						cell.flags,
						shapingSpan?.columns ?? cell.width,
						cell.codepoint,
						cellConstraintWidth(line, x, line.length),
					);
				}
			}
		}
	}

	private uploadAtlas(): void {
		if (!this.atlasDirty || !this.device || !this.atlasTexture) return;
		const bounds = this.atlasDirtyBounds ?? {
			minX: 0,
			minY: 0,
			maxX: this.atlasCanvas.width,
			maxY: this.atlasCanvas.height,
		};
		const width = bounds.maxX - bounds.minX;
		const height = bounds.maxY - bounds.minY;
		const uploadsEntireAtlas =
			bounds.minX === 0 &&
			bounds.minY === 0 &&
			width === this.atlasCanvas.width &&
			height === this.atlasCanvas.height;
		if (uploadsEntireAtlas) {
			this.device.queue.copyExternalImageToTexture(
				{ source: this.atlasCanvas },
				{ texture: this.atlasTexture },
				{ width, height },
			);
		} else {
			const pixels = this.atlasCtx.getImageData(bounds.minX, bounds.minY, width, height).data;
			this.device.queue.writeTexture(
				{ texture: this.atlasTexture, origin: { x: bounds.minX, y: bounds.minY } },
				pixels,
				{ bytesPerRow: width * 4, rowsPerImage: height },
				{ width, height },
			);
		}
		this.frameAtlasUploadBytes += width * height * 4;
		this.atlasDirty = false;
		this.atlasDirtyBounds = null;
	}

	private resetAtlas(): void {
		this.glyphMap.clear();
		this.atlasX = 0;
		this.atlasY = 0;
		this.atlasRowHeight = 0;
		this.atlasCtx.clearRect(0, 0, this.atlasCanvas.width, this.atlasCanvas.height);
		this.markEntireAtlasDirty();
	}

	// ==========================================================================
	// Canvas Sizing
	// ==========================================================================

	resize(cols: number, rows: number): void {
		this.renderedColumns = cols;
		this.renderedRows = rows;
		const cssWidth = cols * this.metrics.width;
		const cssHeight = rows * this.metrics.height;

		this.canvas.style.width = `${cssWidth}px`;
		this.canvas.style.height = `${cssHeight}px`;

		if (this.useWebGPU && this.context) {
			// WebGPU: canvas size is set via configure
			this.canvas.width = cssWidth * this.devicePixelRatio;
			this.canvas.height = cssHeight * this.devicePixelRatio;
			const device = this.device;
			if (!device) return;
			this.context.configure({
				device,
				format: this.format,
				alphaMode: 'premultiplied',
			});
		} else if (this.ctx2d) {
			this.canvas.width = cssWidth * this.devicePixelRatio;
			this.canvas.height = cssHeight * this.devicePixelRatio;
			this.ctx2d.setTransform(this.devicePixelRatio, 0, 0, this.devicePixelRatio, 0, 0);
			this.ctx2d.textBaseline = 'alphabetic';
			this.ctx2d.textAlign = 'left';
			this.ctx2d.fillStyle = this.theme.background;
			if (!this.allowTransparency) this.ctx2d.fillRect(0, 0, cssWidth, cssHeight);
		}
		this.syncSurfacePadding();
	}

	/** Recalculate CSS margins used to balance leftover surface pixels. */
	syncSurfacePadding(): void {
		const parent = this.canvas.parentElement;
		if (!parent) return;
		if (this.paddingBalance === false) {
			setCanvasMargins(this.canvas, ZERO_PADDING);
			return;
		}
		if (typeof window.getComputedStyle !== 'function') return;
		const style = window.getComputedStyle(parent);
		const explicit: SurfacePadding = {
			top: cssPixels(style.paddingTop),
			bottom: cssPixels(style.paddingBottom),
			left: cssPixels(style.paddingLeft),
			right: cssPixels(style.paddingRight),
		};
		const balanced = balanceGridPadding(
			{ width: parent.clientWidth, height: parent.clientHeight },
			{ width: this.metrics.width, height: this.metrics.height },
			explicit,
			{ columns: this.renderedColumns, rows: this.renderedRows },
			this.paddingBalance,
		);
		setCanvasMargins(this.canvas, additionalSurfaceMargins(balanced, explicit));
	}

	/** Rebuild physical-pixel resources after browser zoom or monitor changes. */
	setDevicePixelRatio(value: number): boolean {
		const ratio = Number.isFinite(value) && value > 0 ? value : 1;
		if (ratio === this.devicePixelRatio) return false;
		this.devicePixelRatio = ratio;
		this.remeasureFont();
		this.resetAtlas();
		if (this.renderedColumns > 0 && this.renderedRows > 0) {
			this.resize(this.renderedColumns, this.renderedRows);
		}
		this.requestRenderCallback?.();
		return true;
	}

	// ==========================================================================
	// Main Rendering
	// ==========================================================================

	render(
		buffer: IRenderable,
		forceAll = false,
		viewportY = 0,
		scrollbackProvider?: IScrollbackProvider,
		scrollbarOpacity = 1,
	): void {
		const backend = this.getBackend();
		if (backend === 'uninitialized') return;
		const timestamp = rendererNow();
		const reasons = new Set<RendererFrameReason>();
		const cursor = buffer.getCursor();
		if (forceAll || buffer.needsFullRedraw?.()) reasons.add('full');
		if (viewportY !== this.lastViewportY) reasons.add('viewport');
		if (
			this.cursorAppearanceDirty ||
			cursor.x !== this.lastCursorPosition.x ||
			cursor.y !== this.lastCursorPosition.y ||
			cursor.visible !== this.cursorVisible
		)
			reasons.add('cursor');
		if (this.preeditDirty) reasons.add('preedit');
		if (this.fontDirty) reasons.add('font');
		if (this.textBlinkDirty) reasons.add('blink');
		if (this.searchDirty) reasons.add('search');
		if (scrollbarOpacity !== this.lastScrollbarOpacity) reasons.add('scrollbar');
		if (reasons.size === 0) reasons.add('dirty');

		this.resetFrameCounters();

		if (this.useWebGPU) {
			this.renderWebGPU(buffer, forceAll, viewportY, scrollbackProvider, scrollbarOpacity);
		} else {
			this.renderCanvas2D(buffer, forceAll, viewportY, scrollbackProvider, scrollbarOpacity);
		}
		this.fontDirty = false;
		this.cursorAppearanceDirty = false;
		this.completeFrameTelemetry(backend, timestamp, reasons, forceAll, viewportY);
	}

	/** Draw a Host projection directly from its row-partitioned structure-of-arrays store. */
	renderPacked(buffer: PackedRendererSource, forceAll = false): void {
		const backend = this.getBackend();
		const metadata = buffer.metadata;
		if (backend === 'uninitialized' || !metadata) return;
		const timestamp = rendererNow();
		this.currentSelectionCoords = this.projectedSelectionCoords;
		const reasons = new Set<RendererFrameReason>();
		if (forceAll || buffer.needsFullRedraw) reasons.add('full');
		if (
			this.cursorAppearanceDirty ||
			metadata.cursor.column !== this.lastCursorPosition.x ||
			metadata.cursor.row !== this.lastCursorPosition.y
		)
			reasons.add('cursor');
		if (this.preeditDirty) reasons.add('preedit');
		if (this.fontDirty) reasons.add('font');
		if (this.textBlinkDirty) reasons.add('blink');
		if (this.searchDirty) reasons.add('search');
		if (reasons.size === 0) reasons.add('dirty');

		this.resetFrameCounters();
		if (this.useWebGPU) this.renderPackedWebGPU(buffer, metadata);
		else this.renderPackedCanvas2D(buffer, metadata, forceAll);
		this.fontDirty = false;
		this.cursorAppearanceDirty = false;
		this.searchDirty = false;
		this.completeFrameTelemetry(backend, timestamp, reasons, forceAll, 0);
	}

	private resetFrameCounters(): void {
		this.frameRowsRendered = 0;
		this.frameCellsVisited = 0;
		this.frameVertexUploadBytes = 0;
		this.frameAtlasUploadBytes = 0;
		this.frameAtlasGlyphsCreated = 0;
		this.frameBackgroundInstances = 0;
		this.frameGlyphInstances = 0;
		this.frameShapingRuns = 0;
		this.frameOverlayInstances = 0;
		this.frameGpuDrawCalls = 0;
	}

	private completeFrameTelemetry(
		backend: 'webgpu' | 'canvas2d',
		timestamp: number,
		reasons: ReadonlySet<RendererFrameReason>,
		forceAll: boolean,
		viewportY: number,
	): void {
		let atlasUsedPixels = 0;
		for (const glyph of this.glyphMap.values()) atlasUsedPixels += glyph.w * glyph.h;
		const atlasPixels = this.atlasCanvas.width * this.atlasCanvas.height;
		const telemetry: RendererFrameTelemetry = {
			frameId: ++this.frameId,
			backend,
			timestamp,
			durationMs: Math.max(0, rendererNow() - timestamp),
			reasons: [...reasons],
			forceAll,
			viewportY,
			rowsRendered: this.frameRowsRendered,
			cellsVisited: this.frameCellsVisited,
			backgroundInstances: this.frameBackgroundInstances,
			glyphInstances: this.frameGlyphInstances,
			shapingRuns: this.frameShapingRuns,
			overlayInstances: this.frameOverlayInstances,
			gpuDrawCalls: this.frameGpuDrawCalls,
			vertexUploadBytes: this.frameVertexUploadBytes,
			atlasUploadBytes: this.frameAtlasUploadBytes,
			atlasGlyphsCreated: this.frameAtlasGlyphsCreated,
			atlasGlyphCount: this.glyphMap.size,
			atlasWidth: this.atlasCanvas.width,
			atlasHeight: this.atlasCanvas.height,
			atlasPressure: atlasPixels === 0 ? 0 : atlasUsedPixels / atlasPixels,
		};
		this.lastTelemetry = telemetry;
		this.telemetryCallback?.(telemetry);
	}

	private getPreeditLayout(
		cursor: { x: number; y: number },
		columns: number,
	): PreeditLayout | null {
		if (this.preeditCells.length === 0 || columns <= 0) return null;

		let cells = this.preeditCells;
		let width = cells.reduce((total, cell) => total + cell.width, 0);
		let offset = 0;
		let removedWidth = 0;
		while (width > columns && offset < cells.length) {
			const removed = cells[offset]?.width ?? 0;
			width -= removed;
			removedWidth += removed;
			offset++;
		}
		cells = cells.slice(offset);
		if (cells.length === 0 || width <= 0) return null;

		const start = Math.max(0, Math.min(cursor.x, columns - width));
		const selectionStart = clampNumber(this.preeditSelection.start - removedWidth, 0, width);
		const selectionEnd = clampNumber(
			this.preeditSelection.end - removedWidth,
			selectionStart,
			width,
		);
		return {
			cells,
			start,
			end: start + width,
			row: cursor.y,
			width,
			selectionStart,
			selectionEnd,
		};
	}

	// ==========================================================================
	// WebGPU Render Path
	// ==========================================================================

	private buildPackedRowTextShaping(
		row: PackedRendererRow,
		columns: number,
		rowIndex: number,
		cursor: PackedRendererMetadata['cursor'],
		cursorEnabled: boolean,
		preeditLayout: PreeditLayout | null,
	): PackedRowTextShaping & { readonly codepoints: Uint32Array } {
		const texts = Array.from({ length: columns }, (_, column) => row.grapheme(column));
		const codepoints = Uint32Array.from(texts, (text) => text.codePointAt(0) ?? 0);
		const eligible = (column: number): boolean => {
			const text = texts[column] ?? '';
			const flags = row.styleFlags[column] ?? 0;
			const hidden =
				(flags & TerminalStyleFlag.invisible) !== 0 ||
				((flags & TerminalStyleFlag.blink) !== 0 && !this.textBlinkVisible);
			const cursorBoundary = cursorEnabled && cursor.row === rowIndex && cursor.column === column;
			const preeditBoundary =
				preeditLayout !== null &&
				preeditLayout.row === rowIndex &&
				column >= preeditLayout.start &&
				column < preeditLayout.end;
			return (
				packedCellWidth(row.widths[column] ?? TerminalCellWidthCode.spacerTail) === 1 &&
				!hidden &&
				!cursorBoundary &&
				!preeditBoundary &&
				text.length === 1
			);
		};
		const sameStyle = (left: number, right: number): boolean =>
			row.styleFlags[left] === row.styleFlags[right] &&
			row.foregroundColors[left] === row.foregroundColors[right] &&
			row.backgroundColors[left] === row.backgroundColors[right] &&
			row.underlineKinds[left] === row.underlineKinds[right] &&
			row.underlineColors[left] === row.underlineColors[right];
		return {
			codepoints,
			texts,
			plan: this.fontLigatures
				? buildTextShapingPlan({
						length: columns,
						eligible,
						sameStyle,
						text: (column) => texts[column] ?? '',
						width: (column) =>
							packedCellWidth(row.widths[column] ?? TerminalCellWidthCode.spacerTail),
					})
				: Array.from({ length: columns }, () => null),
		};
	}

	private packedConstraintWidth(
		row: PackedRendererRow,
		shaping: PackedRowTextShaping & { readonly codepoints: Uint32Array },
		column: number,
		columns: number,
	): 1 | 2 {
		return cellConstraintWidthFromReader(
			column,
			columns,
			(index) => shaping.codepoints[index] ?? 0,
			(index) => packedCellWidth(row.widths[index] ?? TerminalCellWidthCode.spacerTail),
		);
	}

	private hasPackedBlinkingCells(buffer: PackedRendererSource): boolean {
		for (const row of buffer.rows) {
			for (const flags of row.styleFlags) {
				if ((flags & TerminalStyleFlag.blink) !== 0) return true;
			}
		}
		return false;
	}

	private renderPackedWebGPU(buffer: PackedRendererSource, metadata: PackedRendererMetadata): void {
		if (!this.device || !this.context || !this.bgPipeline || !this.textPipeline) return;
		const { columns, rows } = metadata;
		const cursor = metadata.cursor;
		this.syncCursorBlink(cursor.blinking);
		this.syncTextBlink(this.hasPackedBlinkingCells(buffer));
		const needsResize =
			this.canvas.width !== columns * this.metrics.width * this.devicePixelRatio ||
			this.canvas.height !== rows * this.metrics.height * this.devicePixelRatio;
		if (needsResize) this.resize(columns, rows);

		const preeditLayout = this.getPreeditLayout({ x: cursor.column, y: cursor.row }, columns);
		const renderedCursorStyle: RenderedCursorStyle = this.focused ? cursor.shape : 'block_hollow';
		const cursorEnabled =
			cursor.visible && (!this.focused || this.cursorVisible) && preeditLayout === null;
		const shapings: Array<(PackedRowTextShaping & { readonly codepoints: Uint32Array }) | null> =
			Array.from({ length: rows }, () => null);

		for (let y = 0; y < rows; y++) {
			const row = buffer.rows[y];
			if (!row) continue;
			const shaping = this.buildPackedRowTextShaping(
				row,
				columns,
				y,
				cursor,
				cursorEnabled,
				preeditLayout,
			);
			shapings[y] = shaping;
			for (let x = 0; x < columns; x++) {
				const width = packedCellWidth(row.widths[x] ?? TerminalCellWidthCode.spacerTail);
				const flags = row.styleFlags[x] ?? 0;
				if (
					width === 0 ||
					(flags & TerminalStyleFlag.invisible) !== 0 ||
					((flags & TerminalStyleFlag.blink) !== 0 && !this.textBlinkVisible)
				)
					continue;
				const span = shaping.plan[x];
				if (span && span.start !== x) continue;
				const text = span?.text ?? shaping.texts[x] ?? '';
				if (text.length === 0) continue;
				this.ensureGlyph(
					text,
					flags,
					span?.columns ?? width,
					shaping.codepoints[x] ?? 0,
					this.packedConstraintWidth(row, shaping, x, columns),
				);
			}
		}
		if (preeditLayout) {
			for (const cell of preeditLayout.cells) this.ensureGlyph(cell.text, 0, cell.width);
		}

		const bgData: number[] = [];
		const textData: number[] = [];
		const overlayData: number[] = [];
		const defaultBackground = rgbChannels(metadata.defaultColors.backgroundRgb);
		const defaultForeground = rgbChannels(metadata.defaultColors.foregroundRgb);
		const cursorColor = rgbChannels(
			metadata.defaultColors.cursorRgb ?? metadata.defaultColors.foregroundRgb,
		);
		const cursorAccent = defaultBackground;
		const atlasWidth = this.atlasCanvas.width;
		const atlasHeight = this.atlasCanvas.height;

		for (let y = 0; y < rows; y++) {
			const row = buffer.rows[y];
			const shaping = shapings[y];
			if (!row || !shaping) continue;
			this.frameRowsRendered++;
			for (let x = 0; x < columns; x++) {
				const width = packedCellWidth(row.widths[x] ?? TerminalCellWidthCode.spacerTail);
				if (width === 0) continue;
				this.frameCellsVisited++;
				const flags = row.styleFlags[x] ?? 0;
				const cellX = x * this.metrics.width;
				const cellY = y * this.metrics.height;
				const cellWidth = width * this.metrics.width;
				const isBlockCursor =
					cursorEnabled &&
					renderedCursorStyle === 'block' &&
					cursor.column === x &&
					cursor.row === y;
				const isFullBlockCursor =
					isBlockCursor && this.detailedMetrics.cursorHeight === this.detailedMetrics.cellHeight;
				let foreground = rgbChannels(
					packedColorRgb(
						row.foregroundColors[x] ?? TerminalColorEncoding.default,
						metadata.defaultColors.foregroundRgb,
					),
				);
				let background = rgbChannels(
					packedColorRgb(
						row.backgroundColors[x] ?? TerminalColorEncoding.default,
						metadata.defaultColors.backgroundRgb,
					),
				);
				if ((flags & TerminalStyleFlag.inverse) !== 0) {
					[foreground, background] = [background, foreground];
				}
				const selected = this.isCoordinateInSelection(x, y);
				const search = this.getSearchHighlight(x, y);
				if (selected) {
					background = hexToRGB(this.theme.selectionBackground);
					foreground = hexToRGB(this.theme.selectionForeground);
				} else if (search !== null) {
					background = hexToRGB(
						search === 'active'
							? this.theme.searchActiveMatchBackground
							: this.theme.searchMatchBackground,
					);
					foreground = hexToRGB(
						search === 'active'
							? this.theme.searchActiveMatchForeground
							: this.theme.searchMatchForeground,
					);
				}
				if (isFullBlockCursor) background = cursorColor;
				if (
					background[0] !== defaultBackground[0] ||
					background[1] !== defaultBackground[1] ||
					background[2] !== defaultBackground[2] ||
					isFullBlockCursor ||
					selected ||
					search !== null
				) {
					pushRect(bgData, cellX, cellY, cellWidth, this.metrics.height, background);
				}
				if (isBlockCursor && !isFullBlockCursor) {
					const geometry = cursorBlockGeometry(x, y, this.detailedMetrics);
					pushRect(bgData, geometry.x, geometry.y, geometry.width, geometry.height, cursorColor);
				}
				if (
					preeditLayout &&
					y === preeditLayout.row &&
					x >= preeditLayout.start &&
					x < preeditLayout.end
				)
					continue;
				if ((flags & TerminalStyleFlag.invisible) !== 0) continue;
				if ((flags & TerminalStyleFlag.blink) !== 0 && !this.textBlinkVisible) continue;

				if (isBlockCursor) foreground = cursorAccent;
				const alpha = (flags & TerminalStyleFlag.faint) !== 0 ? 0.5 : 1;
				const span = shaping.plan[x];
				let text = shaping.texts[x] ?? '';
				if (span) text = span.start === x ? span.text : '';
				if (text.length > 0) {
					if (span) this.frameShapingRuns++;
					const glyph = this.ensureGlyph(
						text,
						flags,
						span?.columns ?? width,
						shaping.codepoints[x] ?? 0,
						this.packedConstraintWidth(row, shaping, x, columns),
					);
					if (glyph) {
						textData.push(
							cellX + glyph.offsetX,
							cellY + glyph.offsetY,
							glyph.renderWidth,
							glyph.renderHeight,
							glyph.x / atlasWidth,
							glyph.y / atlasHeight,
							glyph.w / atlasWidth,
							glyph.h / atlasHeight,
							foreground[0] / 255,
							foreground[1] / 255,
							foreground[2] / 255,
							alpha,
							glyph.colored ? 1 : 0,
						);
					}
				}

				const underline = row.underlineKinds[x] ?? TerminalUnderlineKindCode.none;
				const underlineRgb = packedOptionalColorRgb(
					row.underlineColors[x] ?? TerminalColorEncoding.none,
					metadata.defaultColors.foregroundRgb,
				);
				const decorationColor = foreground;
				const underlineColor = underlineRgb === null ? decorationColor : rgbChannels(underlineRgb);
				this.pushPackedDecorations(
					overlayData,
					cellX,
					cellY,
					cellWidth,
					flags,
					underline,
					decorationColor,
					underlineColor,
					alpha,
				);
			}
		}

		if (preeditLayout) {
			const cellY = preeditLayout.row * this.metrics.height;
			const startX = preeditLayout.start * this.metrics.width;
			pushRect(
				bgData,
				startX,
				cellY,
				preeditLayout.width * this.metrics.width,
				this.metrics.height,
				defaultBackground,
			);
			let column = preeditLayout.start;
			for (const cell of preeditLayout.cells) {
				const glyph = this.ensureGlyph(cell.text, 0, cell.width);
				if (glyph) {
					textData.push(
						column * this.metrics.width + glyph.offsetX,
						cellY + glyph.offsetY,
						glyph.renderWidth,
						glyph.renderHeight,
						glyph.x / atlasWidth,
						glyph.y / atlasHeight,
						glyph.w / atlasWidth,
						glyph.h / atlasHeight,
						defaultForeground[0] / 255,
						defaultForeground[1] / 255,
						defaultForeground[2] / 255,
						1,
						glyph.colored ? 1 : 0,
					);
				}
				column += cell.width;
			}
			pushRect(
				overlayData,
				startX,
				cellY + this.detailedMetrics.underlinePosition,
				preeditLayout.width * this.metrics.width,
				this.detailedMetrics.underlineThickness,
				defaultForeground,
			);
		}
		if (cursorEnabled && renderedCursorStyle !== 'block') {
			const geometries =
				renderedCursorStyle === 'block_hollow'
					? cursorOutlineGeometry(cursor.column, cursor.row, this.detailedMetrics)
					: [
							cursorStrokeGeometry(
								renderedCursorStyle,
								cursor.column,
								cursor.row,
								this.detailedMetrics,
							),
						];
			for (const geometry of geometries) {
				pushRect(overlayData, geometry.x, geometry.y, geometry.width, geometry.height, cursorColor);
			}
		}

		this.submitWebGPUFrame(bgData, textData, overlayData, columns, rows, defaultBackground);
		this.lastCursorPosition = { x: cursor.column, y: cursor.row };
		this.preeditDirty = false;
		this.textBlinkDirty = false;
		buffer.markRendered();
	}

	private pushPackedDecorations(
		overlayData: number[],
		cellX: number,
		cellY: number,
		cellWidth: number,
		flags: number,
		underline: number,
		decorationColor: readonly [number, number, number],
		underlineColor: readonly [number, number, number],
		alpha: number,
	): void {
		if (underline !== TerminalUnderlineKindCode.none) {
			const underlineY = cellY + this.detailedMetrics.underlinePosition;
			const thickness = this.detailedMetrics.underlineThickness;
			if (underline === TerminalUnderlineKindCode.double) {
				pushRect(
					overlayData,
					cellX,
					underlineY - thickness,
					cellWidth,
					thickness,
					underlineColor,
					alpha,
				);
				pushRect(
					overlayData,
					cellX,
					underlineY + thickness,
					cellWidth,
					thickness,
					underlineColor,
					alpha,
				);
			} else if (underline === TerminalUnderlineKindCode.curly) {
				for (let offset = 0; offset < cellWidth; offset += 2) {
					pushRect(
						overlayData,
						cellX + offset,
						underlineY + (Math.floor(offset / 2) % 2),
						Math.min(2, cellWidth - offset),
						thickness,
						underlineColor,
						alpha,
					);
				}
			} else if (
				underline === TerminalUnderlineKindCode.dotted ||
				underline === TerminalUnderlineKindCode.dashed
			) {
				const dashWidth = underline === TerminalUnderlineKindCode.dotted ? 1 : 3;
				for (let offset = 0; offset < cellWidth; offset += dashWidth + 2) {
					pushRect(
						overlayData,
						cellX + offset,
						underlineY,
						Math.min(dashWidth, cellWidth - offset),
						thickness,
						underlineColor,
						alpha,
					);
				}
			} else {
				pushRect(overlayData, cellX, underlineY, cellWidth, thickness, underlineColor, alpha);
			}
		}
		if ((flags & TerminalStyleFlag.strikethrough) !== 0) {
			pushRect(
				overlayData,
				cellX,
				cellY + this.detailedMetrics.strikethroughPosition,
				cellWidth,
				this.detailedMetrics.strikethroughThickness,
				decorationColor,
				alpha,
			);
		}
		if ((flags & TerminalStyleFlag.overline) !== 0) {
			pushRect(
				overlayData,
				cellX,
				cellY + this.detailedMetrics.overlinePosition,
				cellWidth,
				this.detailedMetrics.overlineThickness,
				decorationColor,
				alpha,
			);
		}
	}

	private submitWebGPUFrame(
		backgroundData: number[],
		textData: number[],
		overlayData: number[],
		columns: number,
		rows: number,
		clearColor: readonly [number, number, number],
	): void {
		if (!this.device || !this.context || !this.bgPipeline || !this.textPipeline) return;
		this.uploadAtlas();
		this.frameBackgroundInstances = backgroundData.length / 8;
		this.frameGlyphInstances = textData.length / 13;
		this.frameOverlayInstances = overlayData.length / 8;
		if (this.uniformBuffer) {
			this.device.queue.writeBuffer(
				this.uniformBuffer,
				0,
				new Float32Array([
					columns * this.metrics.width * this.devicePixelRatio,
					rows * this.metrics.height * this.devicePixelRatio,
					this.devicePixelRatio,
					0,
				]),
			);
			this.frameVertexUploadBytes += 4 * Float32Array.BYTES_PER_ELEMENT;
		}
		const commandEncoder = this.device.createCommandEncoder();
		const renderPass = commandEncoder.beginRenderPass({
			colorAttachments: [
				{
					view: this.context.getCurrentTexture().createView(),
					clearValue: {
						r: this.allowTransparency ? 0 : clearColor[0] / 255,
						g: this.allowTransparency ? 0 : clearColor[1] / 255,
						b: this.allowTransparency ? 0 : clearColor[2] / 255,
						a: this.allowTransparency ? 0 : 1,
					},
					loadOp: 'clear',
					storeOp: 'store',
				},
			],
		});
		if (this.uniformBuffer) {
			const backgroundBindGroup = this.device.createBindGroup({
				layout: this.bgPipeline.getBindGroupLayout(0),
				entries: [{ binding: 0, resource: { buffer: this.uniformBuffer } }],
			});
			const backgroundBuffer = this.uploadVertexData('background', backgroundData);
			if (backgroundBuffer) {
				renderPass.setPipeline(this.bgPipeline);
				renderPass.setVertexBuffer(0, backgroundBuffer);
				renderPass.setBindGroup(0, backgroundBindGroup);
				renderPass.draw(6, backgroundData.length / 8);
				this.frameGpuDrawCalls++;
			}
			if (textData.length > 0 && this.atlasTexture && this.atlasSampler) {
				const textBuffer = this.uploadVertexData('text', textData);
				if (textBuffer) {
					const textBindGroup = this.device.createBindGroup({
						layout: this.textPipeline.getBindGroupLayout(0),
						entries: [
							{ binding: 0, resource: { buffer: this.uniformBuffer } },
							{ binding: 1, resource: this.atlasSampler },
							{ binding: 2, resource: this.atlasTexture.createView() },
						],
					});
					renderPass.setPipeline(this.textPipeline);
					renderPass.setVertexBuffer(0, textBuffer);
					renderPass.setBindGroup(0, textBindGroup);
					renderPass.draw(6, textData.length / 13);
					this.frameGpuDrawCalls++;
				}
			}
			const overlayBuffer = this.uploadVertexData('overlay', overlayData);
			if (overlayBuffer) {
				renderPass.setPipeline(this.bgPipeline);
				renderPass.setVertexBuffer(0, overlayBuffer);
				renderPass.setBindGroup(0, backgroundBindGroup);
				renderPass.draw(6, overlayData.length / 8);
				this.frameGpuDrawCalls++;
			}
		}
		renderPass.end();
		this.device.queue.submit([commandEncoder.finish()]);
	}

	private renderWebGPU(
		buffer: IRenderable,
		_forceAll: boolean,
		viewportY: number,
		scrollbackProvider: IScrollbackProvider | undefined,
		scrollbarOpacity: number,
	): void {
		if (!this.device || !this.context || !this.bgPipeline || !this.textPipeline) return;

		this.currentBuffer = buffer;
		const cursor = buffer.getCursor();
		this.syncCursorBlink(cursor.blinking);
		const dims = buffer.getDimensions();
		const scrollbackLength = scrollbackProvider ? scrollbackProvider.getScrollbackLength() : 0;
		this.currentAbsoluteTop = scrollbackLength - Math.floor(viewportY);
		this.syncTextBlink(
			this.hasBlinkingCells(buffer, dims.rows, viewportY, scrollbackProvider, scrollbackLength),
		);

		const needsResize =
			this.canvas.width !== dims.cols * this.metrics.width * this.devicePixelRatio ||
			this.canvas.height !== dims.rows * this.metrics.height * this.devicePixelRatio;
		if (needsResize) {
			this.resize(dims.cols, dims.rows);
		}
		this.lastViewportY = viewportY;
		this.lastScrollbarOpacity = scrollbarOpacity;

		const selectionManager = this.selectionManager;
		const hasSelection = selectionManager?.hasSelection() ?? false;
		this.currentSelectionCoords = hasSelection
			? (selectionManager?.getSelectionCoords() ?? null)
			: null;
		if (selectionManager) {
			selectionManager.clearDirtySelectionRows();
		}

		const bgData: number[] = [];
		const textData: number[] = [];
		const overlayData: number[] = [];
		const themeBackground = hexToRGB(this.theme.background);
		const themeForeground = hexToRGB(this.theme.foreground);
		const cursorColor = hexToRGB(this.theme.cursor);
		const cursorAccent = hexToRGB(this.theme.cursorAccent);
		const preeditLayout = viewportY === 0 ? this.getPreeditLayout(cursor, dims.cols) : null;
		const renderedCursorStyle: RenderedCursorStyle = this.focused ? cursor.style : 'block_hollow';
		const cursorEnabled =
			viewportY === 0 &&
			cursor.visible &&
			(!this.focused || this.cursorVisible) &&
			preeditLayout === null;
		this.prepareGlyphs(
			buffer,
			dims.rows,
			viewportY,
			scrollbackProvider,
			scrollbackLength,
			cursor,
			cursorEnabled,
			preeditLayout,
		);
		const atlasW = this.atlasCanvas.width;
		const atlasH = this.atlasCanvas.height;

		for (let y = 0; y < dims.rows; y++) {
			const line = this.fetchLine(buffer, y, viewportY, scrollbackProvider, scrollbackLength);
			if (!line) continue;
			this.frameRowsRendered++;
			const shaping = this.buildRowTextShaping(
				buffer,
				line,
				y,
				viewportY,
				scrollbackProvider,
				scrollbackLength,
				cursor,
				cursorEnabled,
				preeditLayout,
			);

			for (let x = 0; x < line.length; x++) {
				const cell = line[x];
				if (!cell || cell.width === 0) continue;
				this.frameCellsVisited++;

				const cellX = x * this.metrics.width;
				const cellY = y * this.metrics.height;
				const cellW = this.metrics.width * cell.width;
				const cellH = this.metrics.height;
				const isSelected = this.isInSelection(x, y, cell);
				const searchHighlight = this.getSearchHighlight(x, y);
				const isBlockCursor =
					cursorEnabled && renderedCursorStyle === 'block' && cursor.x === x && cursor.y === y;
				const isFullBlockCursor =
					isBlockCursor && this.detailedMetrics.cursorHeight === this.detailedMetrics.cellHeight;
				let bgR: number, bgG: number, bgB: number;

				if (isFullBlockCursor) {
					[bgR, bgG, bgB] = cursorColor;
				} else if (isSelected) {
					[bgR, bgG, bgB] = hexToRGB(this.theme.selectionBackground);
				} else if (searchHighlight === 'active') {
					[bgR, bgG, bgB] = hexToRGB(this.theme.searchActiveMatchBackground);
				} else if (searchHighlight === 'match') {
					[bgR, bgG, bgB] = hexToRGB(this.theme.searchMatchBackground);
				} else {
					bgR = cell.bg_r;
					bgG = cell.bg_g;
					bgB = cell.bg_b;
					if (cell.flags & CellFlags.INVERSE) {
						bgR = cell.fg_r;
						bgG = cell.fg_g;
						bgB = cell.fg_b;
					}
				}

				const isDefaultBg =
					bgR === themeBackground[0] &&
					bgG === themeBackground[1] &&
					bgB === themeBackground[2] &&
					!isSelected &&
					searchHighlight === null &&
					!isFullBlockCursor;
				if (!isDefaultBg) {
					pushRect(bgData, cellX, cellY, cellW, cellH, [bgR, bgG, bgB]);
				}
				if (isBlockCursor && !isFullBlockCursor) {
					const cursorBox = cursorBlockGeometry(x, y, this.detailedMetrics);
					pushRect(
						bgData,
						cursorBox.x,
						cursorBox.y,
						cursorBox.width,
						cursorBox.height,
						cursorColor,
					);
				}

				if (
					preeditLayout &&
					y === preeditLayout.row &&
					x >= preeditLayout.start &&
					x < preeditLayout.end
				) {
					continue;
				}

				if ((cell.flags & CellFlags.INVISIBLE) !== 0) continue;
				if ((cell.flags & CellFlags.BLINK) !== 0 && !this.textBlinkVisible) continue;

				let fgR: number, fgG: number, fgB: number;
				if (isBlockCursor) {
					[fgR, fgG, fgB] = cursorAccent;
				} else if (isSelected) {
					[fgR, fgG, fgB] = hexToRGB(this.theme.selectionForeground);
				} else if (searchHighlight === 'active') {
					[fgR, fgG, fgB] = hexToRGB(this.theme.searchActiveMatchForeground);
				} else if (searchHighlight === 'match') {
					[fgR, fgG, fgB] = hexToRGB(this.theme.searchMatchForeground);
				} else {
					fgR = cell.fg_r;
					fgG = cell.fg_g;
					fgB = cell.fg_b;
					if (cell.flags & CellFlags.INVERSE) {
						fgR = cell.bg_r;
						fgG = cell.bg_g;
						fgB = cell.bg_b;
					}
				}

				const alpha = (cell.flags & CellFlags.FAINT) !== 0 ? 0.5 : 1.0;
				const shapingSpan = shaping.plan[x];
				let text = shaping.texts[x] ?? '';
				if (shapingSpan) text = shapingSpan.start === x ? shapingSpan.text : '';
				const glyphWidth = shapingSpan?.columns ?? cell.width;

				if (text.length > 0) {
					if (shapingSpan) this.frameShapingRuns++;
					const glyph = this.ensureGlyph(
						text,
						cell.flags,
						glyphWidth,
						cell.codepoint,
						cellConstraintWidth(line, x, line.length),
					);
					if (glyph) {
						textData.push(
							cellX + glyph.offsetX,
							cellY + glyph.offsetY,
							glyph.renderWidth,
							glyph.renderHeight,
							glyph.x / atlasW,
							glyph.y / atlasH,
							glyph.w / atlasW,
							glyph.h / atlasH,
							fgR / 255,
							fgG / 255,
							fgB / 255,
							alpha,
							glyph.colored ? 1 : 0,
						);
					}
				}

				const decorationColor: [number, number, number] = [fgR, fgG, fgB];
				const underlineColor: [number, number, number] =
					cell.underline_r >= 0
						? [cell.underline_r, cell.underline_g, cell.underline_b]
						: decorationColor;
				if ((cell.flags & CellFlags.UNDERLINE) !== 0) {
					const style = cell.underline_style || UnderlineStyle.SINGLE;
					const underlineY = cellY + this.detailedMetrics.underlinePosition;
					const underlineThickness = this.detailedMetrics.underlineThickness;
					if (style === UnderlineStyle.DOUBLE) {
						pushRect(
							overlayData,
							cellX,
							underlineY - underlineThickness,
							cellW,
							underlineThickness,
							underlineColor,
							alpha,
						);
						pushRect(
							overlayData,
							cellX,
							underlineY + underlineThickness,
							cellW,
							underlineThickness,
							underlineColor,
							alpha,
						);
					} else if (style === UnderlineStyle.CURLY) {
						for (let offset = 0; offset < cellW; offset += 2) {
							pushRect(
								overlayData,
								cellX + offset,
								underlineY + (Math.floor(offset / 2) % 2),
								Math.min(2, cellW - offset),
								underlineThickness,
								underlineColor,
								alpha,
							);
						}
					} else if (style === UnderlineStyle.DOTTED || style === UnderlineStyle.DASHED) {
						const dashWidth = style === UnderlineStyle.DOTTED ? 1 : 3;
						const gap = style === UnderlineStyle.DOTTED ? 2 : 2;
						for (let offset = 0; offset < cellW; offset += dashWidth + gap) {
							pushRect(
								overlayData,
								cellX + offset,
								underlineY,
								Math.min(dashWidth, cellW - offset),
								underlineThickness,
								underlineColor,
								alpha,
							);
						}
					} else {
						pushRect(
							overlayData,
							cellX,
							underlineY,
							cellW,
							underlineThickness,
							underlineColor,
							alpha,
						);
					}
				}
				if ((cell.flags & CellFlags.STRIKETHROUGH) !== 0) {
					pushRect(
						overlayData,
						cellX,
						cellY + this.detailedMetrics.strikethroughPosition,
						cellW,
						this.detailedMetrics.strikethroughThickness,
						decorationColor,
						alpha,
					);
				}
				if (cell.overline) {
					pushRect(
						overlayData,
						cellX,
						cellY + this.detailedMetrics.overlinePosition,
						cellW,
						this.detailedMetrics.overlineThickness,
						decorationColor,
						alpha,
					);
				}
				if (cell.hyperlink_id > 0 && cell.hyperlink_id === this.hoveredHyperlinkId) {
					pushRect(
						overlayData,
						cellX,
						cellY + this.detailedMetrics.underlinePosition,
						cellW,
						this.detailedMetrics.underlineThickness,
						[74, 144, 226],
					);
				}
				if (this.hoveredLinkRange && isCellInRange(x, y, this.hoveredLinkRange)) {
					pushRect(
						overlayData,
						cellX,
						cellY + this.detailedMetrics.underlinePosition,
						cellW,
						this.detailedMetrics.underlineThickness,
						[74, 144, 226],
					);
				}
			}
		}

		if (preeditLayout) {
			const cellY = preeditLayout.row * this.metrics.height;
			const startX = preeditLayout.start * this.metrics.width;
			pushRect(
				bgData,
				startX,
				cellY,
				preeditLayout.width * this.metrics.width,
				this.metrics.height,
				themeBackground,
			);
			let column = preeditLayout.start;
			for (const cell of preeditLayout.cells) {
				const glyph = this.ensureGlyph(cell.text, 0, cell.width);
				if (glyph) {
					textData.push(
						column * this.metrics.width + glyph.offsetX,
						cellY + glyph.offsetY,
						glyph.renderWidth,
						glyph.renderHeight,
						glyph.x / atlasW,
						glyph.y / atlasH,
						glyph.w / atlasW,
						glyph.h / atlasH,
						themeForeground[0] / 255,
						themeForeground[1] / 255,
						themeForeground[2] / 255,
						1,
						glyph.colored ? 1 : 0,
					);
				}
				column += cell.width;
			}
			pushRect(
				overlayData,
				startX,
				cellY + this.detailedMetrics.underlinePosition,
				preeditLayout.width * this.metrics.width,
				this.detailedMetrics.underlineThickness,
				themeForeground,
			);
		}

		if (cursorEnabled && renderedCursorStyle !== 'block') {
			const geometries =
				renderedCursorStyle === 'block_hollow'
					? cursorOutlineGeometry(cursor.x, cursor.y, this.detailedMetrics)
					: [cursorStrokeGeometry(renderedCursorStyle, cursor.x, cursor.y, this.detailedMetrics)];
			for (const geometry of geometries) {
				pushRect(overlayData, geometry.x, geometry.y, geometry.width, geometry.height, cursorColor);
			}
		}

		if (scrollbackLength > 0 && scrollbarOpacity > 0) {
			const canvasHeight = dims.rows * this.metrics.height;
			const canvasWidth = dims.cols * this.metrics.width;
			const scrollbarWidth = 8;
			const padding = 4;
			const trackHeight = canvasHeight - padding * 2;
			const totalLines = scrollbackLength + dims.rows;
			const thumbHeight = Math.max(20, (dims.rows / totalLines) * trackHeight);
			const thumbY = padding + (trackHeight - thumbHeight) * (1 - viewportY / scrollbackLength);
			pushRect(
				overlayData,
				canvasWidth - scrollbarWidth - padding,
				thumbY,
				scrollbarWidth,
				thumbHeight,
				[128, 128, 128],
				(viewportY > 0 ? 0.5 : 0.3) * scrollbarOpacity,
			);
		}

		this.submitWebGPUFrame(
			bgData,
			textData,
			overlayData,
			dims.cols,
			dims.rows,
			hexToRGB(this.theme.background),
		);
		this.lastCursorPosition = { x: cursor.x, y: cursor.y };
		this.preeditDirty = false;
		this.textBlinkDirty = false;
		this.searchDirty = false;
		buffer.clearDirty();
	}

	// ==========================================================================
	// Canvas 2D Fallback Render Path
	// ==========================================================================

	private renderPackedCanvas2D(
		buffer: PackedRendererSource,
		metadata: PackedRendererMetadata,
		forceAll: boolean,
	): void {
		const ctx = this.ctx2d;
		if (!ctx) return;
		const { columns, rows } = metadata;
		const cursor = metadata.cursor;
		this.syncCursorBlink(cursor.blinking);
		this.syncTextBlink(this.hasPackedBlinkingCells(buffer));
		if (buffer.needsFullRedraw || this.preeditDirty || this.fontDirty || this.textBlinkDirty) {
			forceAll = true;
		}
		const needsResize =
			this.canvas.width !== columns * this.metrics.width * this.devicePixelRatio ||
			this.canvas.height !== rows * this.metrics.height * this.devicePixelRatio;
		if (needsResize) {
			this.resize(columns, rows);
			forceAll = true;
		}
		const preeditLayout = this.getPreeditLayout({ x: cursor.column, y: cursor.row }, columns);
		const renderedCursorStyle: RenderedCursorStyle = this.focused ? cursor.shape : 'block_hollow';
		const cursorEnabled =
			cursor.visible && (!this.focused || this.cursorVisible) && preeditLayout === null;
		const rowsToRender = new Set<number>();
		for (let y = 0; y < rows; y++) {
			if (forceAll || buffer.rows[y]?.dirty === true) {
				rowsToRender.add(y);
				if (y > 0) rowsToRender.add(y - 1);
				if (y + 1 < rows) rowsToRender.add(y + 1);
			}
		}
		if (cursorEnabled) {
			rowsToRender.add(cursor.row);
			rowsToRender.add(this.lastCursorPosition.y);
		}

		const defaultBackground = rgbChannels(metadata.defaultColors.backgroundRgb);
		const defaultForeground = rgbChannels(metadata.defaultColors.foregroundRgb);
		for (const y of rowsToRender) {
			const row = buffer.rows[y];
			if (!row || y < 0 || y >= rows) continue;
			const shaping = this.buildPackedRowTextShaping(
				row,
				columns,
				y,
				cursor,
				cursorEnabled,
				preeditLayout,
			);
			this.frameRowsRendered++;
			const lineY = y * this.metrics.height;
			const lineWidth = columns * this.metrics.width;
			ctx.clearRect(0, lineY, lineWidth, this.metrics.height);
			if (!this.allowTransparency) {
				ctx.fillStyle = `rgb(${defaultBackground[0]}, ${defaultBackground[1]}, ${defaultBackground[2]})`;
				ctx.fillRect(0, lineY, lineWidth, this.metrics.height);
			}

			for (let x = 0; x < columns; x++) {
				const width = packedCellWidth(row.widths[x] ?? TerminalCellWidthCode.spacerTail);
				if (width === 0) continue;
				this.frameCellsVisited++;
				const flags = row.styleFlags[x] ?? 0;
				let background = rgbChannels(
					packedColorRgb(
						row.backgroundColors[x] ?? TerminalColorEncoding.default,
						metadata.defaultColors.backgroundRgb,
					),
				);
				if ((flags & TerminalStyleFlag.inverse) !== 0) {
					background = rgbChannels(
						packedColorRgb(
							row.foregroundColors[x] ?? TerminalColorEncoding.default,
							metadata.defaultColors.foregroundRgb,
						),
					);
				}
				const selected = this.isCoordinateInSelection(x, y);
				const search = this.getSearchHighlight(x, y);
				if (selected) {
					background = hexToRGB(this.theme.selectionBackground);
				} else if (search !== null) {
					background = hexToRGB(
						search === 'active'
							? this.theme.searchActiveMatchBackground
							: this.theme.searchMatchBackground,
					);
				}
				if (
					background[0] !== defaultBackground[0] ||
					background[1] !== defaultBackground[1] ||
					background[2] !== defaultBackground[2] ||
					selected ||
					search !== null
				) {
					ctx.fillStyle = `rgb(${background[0]}, ${background[1]}, ${background[2]})`;
					ctx.fillRect(
						x * this.metrics.width,
						lineY,
						width * this.metrics.width,
						this.metrics.height,
					);
				}
			}

			for (let x = 0; x < columns; x++) {
				const width = packedCellWidth(row.widths[x] ?? TerminalCellWidthCode.spacerTail);
				if (width === 0) continue;
				if (
					preeditLayout &&
					y === preeditLayout.row &&
					x >= preeditLayout.start &&
					x < preeditLayout.end
				)
					continue;
				const span = shaping.plan[x];
				let text = shaping.texts[x] ?? '';
				if (span) text = span.start === x ? span.text : '';
				if (text.length > 0) {
					this.frameGlyphInstances++;
					if (span) this.frameShapingRuns++;
				}
				const selected = this.isCoordinateInSelection(x, y);
				const search = this.getSearchHighlight(x, y);
				let foreground: ReturnType<typeof hexToRGB> | undefined;
				if (selected) {
					foreground = hexToRGB(this.theme.selectionForeground);
				} else if (search === 'active') {
					foreground = hexToRGB(this.theme.searchActiveMatchForeground);
				} else if (search === 'match') {
					foreground = hexToRGB(this.theme.searchMatchForeground);
				}
				this.drawPackedCellText2D(ctx, row, shaping, x, y, width, text, metadata, foreground);
			}
		}

		if (preeditLayout) {
			this.renderPackedPreedit2D(ctx, preeditLayout, defaultForeground, defaultBackground);
		} else if (cursorEnabled) {
			this.renderPackedCursor2D(ctx, buffer, metadata, renderedCursorStyle);
		}
		this.lastCursorPosition = { x: cursor.column, y: cursor.row };
		this.preeditDirty = false;
		this.textBlinkDirty = false;
		buffer.markRendered();
	}

	private drawPackedCellText2D(
		ctx: CanvasRenderingContext2D,
		row: PackedRendererRow,
		shaping: PackedRowTextShaping & { readonly codepoints: Uint32Array },
		column: number,
		rowIndex: number,
		width: number,
		text: string,
		metadata: PackedRendererMetadata,
		colorOverride?: readonly [number, number, number],
		drawDecorations = true,
	): void {
		const flags = row.styleFlags[column] ?? 0;
		if ((flags & TerminalStyleFlag.invisible) !== 0) return;
		if ((flags & TerminalStyleFlag.blink) !== 0 && !this.textBlinkVisible) return;
		const cellX = column * this.metrics.width;
		const cellY = rowIndex * this.metrics.height;
		const cellWidth = width * this.metrics.width;
		const fontStyle = (flags & TerminalStyleFlag.italic) !== 0 ? 'italic' : 'normal';
		const fontWeight =
			(flags & TerminalStyleFlag.bold) !== 0 ? this.fontWeightBold : this.fontWeight;
		ctx.font = canvasFont(fontStyle, fontWeight, this.fontSize, this.fontFamily);
		let foreground = rgbChannels(
			packedColorRgb(
				row.foregroundColors[column] ?? TerminalColorEncoding.default,
				metadata.defaultColors.foregroundRgb,
			),
		);
		if ((flags & TerminalStyleFlag.inverse) !== 0) {
			foreground = rgbChannels(
				packedColorRgb(
					row.backgroundColors[column] ?? TerminalColorEncoding.default,
					metadata.defaultColors.backgroundRgb,
				),
			);
		}
		foreground = colorOverride ? [...colorOverride] : foreground;
		ctx.fillStyle = `rgb(${foreground[0]}, ${foreground[1]}, ${foreground[2]})`;
		if ((flags & TerminalStyleFlag.faint) !== 0) ctx.globalAlpha = 0.5;
		if (text.length > 0) {
			this.drawConstrainedText2D(
				ctx,
				text,
				cellX,
				cellY,
				shaping.codepoints[column] ?? 0,
				this.packedConstraintWidth(row, shaping, column, metadata.columns),
				width,
			);
		}
		if (drawDecorations) {
			const underline = row.underlineKinds[column] ?? TerminalUnderlineKindCode.none;
			if (underline !== TerminalUnderlineKindCode.none) {
				const underlineRgb = packedOptionalColorRgb(
					row.underlineColors[column] ?? TerminalColorEncoding.none,
					metadata.defaultColors.foregroundRgb,
				);
				const underlineColor = underlineRgb === null ? foreground : rgbChannels(underlineRgb);
				ctx.strokeStyle = `rgb(${underlineColor[0]}, ${underlineColor[1]}, ${underlineColor[2]})`;
				ctx.lineWidth = this.detailedMetrics.underlineThickness;
				const underlineY = cellY + this.detailedMetrics.underlinePosition;
				if (underline === TerminalUnderlineKindCode.dotted) ctx.setLineDash([1, 2]);
				else if (underline === TerminalUnderlineKindCode.dashed) ctx.setLineDash([3, 2]);
				else ctx.setLineDash([]);
				ctx.beginPath();
				if (underline === TerminalUnderlineKindCode.curly) {
					ctx.moveTo(cellX, underlineY);
					for (let offset = 2; offset <= cellWidth; offset += 2) {
						ctx.lineTo(cellX + offset, underlineY + (Math.floor(offset / 2) % 2));
					}
				} else {
					ctx.moveTo(cellX, underlineY);
					ctx.lineTo(cellX + cellWidth, underlineY);
					if (underline === TerminalUnderlineKindCode.double) {
						ctx.moveTo(cellX, underlineY - 2);
						ctx.lineTo(cellX + cellWidth, underlineY - 2);
					}
				}
				ctx.stroke();
				ctx.setLineDash([]);
			}
			if ((flags & TerminalStyleFlag.strikethrough) !== 0) {
				ctx.fillRect(
					cellX,
					cellY + this.detailedMetrics.strikethroughPosition,
					cellWidth,
					this.detailedMetrics.strikethroughThickness,
				);
			}
			if ((flags & TerminalStyleFlag.overline) !== 0) {
				ctx.fillRect(
					cellX,
					cellY + this.detailedMetrics.overlinePosition,
					cellWidth,
					this.detailedMetrics.overlineThickness,
				);
			}
		}
		if ((flags & TerminalStyleFlag.faint) !== 0) ctx.globalAlpha = 1;
	}

	private renderPackedCursor2D(
		ctx: CanvasRenderingContext2D,
		buffer: PackedRendererSource,
		metadata: PackedRendererMetadata,
		style: RenderedCursorStyle,
	): void {
		const { column, row } = metadata.cursor;
		const cursorColor = rgbChannels(
			metadata.defaultColors.cursorRgb ?? metadata.defaultColors.foregroundRgb,
		);
		ctx.fillStyle = `rgb(${cursorColor[0]}, ${cursorColor[1]}, ${cursorColor[2]})`;
		if (style === 'block') {
			const geometry = cursorBlockGeometry(column, row, this.detailedMetrics);
			ctx.fillRect(geometry.x, geometry.y, geometry.width, geometry.height);
			const packedRow = buffer.rows[row];
			if (packedRow) {
				const shaping = this.buildPackedRowTextShaping(
					packedRow,
					metadata.columns,
					row,
					metadata.cursor,
					true,
					null,
				);
				const width = packedCellWidth(packedRow.widths[column] ?? TerminalCellWidthCode.spacerTail);
				if (width > 0) {
					ctx.save();
					ctx.beginPath();
					ctx.rect(geometry.x, geometry.y, geometry.width, geometry.height);
					ctx.clip();
					this.drawPackedCellText2D(
						ctx,
						packedRow,
						shaping,
						column,
						row,
						width,
						shaping.texts[column] ?? '',
						metadata,
						rgbChannels(metadata.defaultColors.backgroundRgb),
						false,
					);
					ctx.restore();
				}
			}
			return;
		}
		const geometries =
			style === 'block_hollow'
				? cursorOutlineGeometry(column, row, this.detailedMetrics)
				: [cursorStrokeGeometry(style, column, row, this.detailedMetrics)];
		for (const geometry of geometries) {
			ctx.fillRect(geometry.x, geometry.y, geometry.width, geometry.height);
		}
	}

	private renderPackedPreedit2D(
		ctx: CanvasRenderingContext2D,
		layout: PreeditLayout,
		foreground: readonly [number, number, number],
		background: readonly [number, number, number],
	): void {
		const cellY = layout.row * this.metrics.height;
		const startX = layout.start * this.metrics.width;
		const width = layout.width * this.metrics.width;
		ctx.fillStyle = `rgb(${background[0]}, ${background[1]}, ${background[2]})`;
		ctx.fillRect(startX, cellY, width, this.metrics.height);
		ctx.font = canvasFont('normal', this.fontWeight, this.fontSize, this.fontFamily);
		ctx.fillStyle = `rgb(${foreground[0]}, ${foreground[1]}, ${foreground[2]})`;
		let column = layout.start;
		for (const cell of layout.cells) {
			this.drawConstrainedText2D(
				ctx,
				cell.text,
				column * this.metrics.width,
				cellY,
				cell.text.codePointAt(0) ?? 0,
				cell.width >= 2 ? 2 : 1,
				cell.width,
			);
			column += cell.width;
		}
		ctx.fillRect(
			startX,
			cellY + this.detailedMetrics.underlinePosition,
			width,
			this.detailedMetrics.underlineThickness,
		);
	}

	private renderCanvas2D(
		buffer: IRenderable,
		forceAll: boolean,
		viewportY: number,
		scrollbackProvider: IScrollbackProvider | undefined,
		scrollbarOpacity: number,
	): void {
		if (!this.ctx2d) return;

		this.currentBuffer = buffer;
		const cursor = buffer.getCursor();
		this.syncCursorBlink(cursor.blinking);
		const dims = buffer.getDimensions();
		const preeditLayout = viewportY === 0 ? this.getPreeditLayout(cursor, dims.cols) : null;
		const renderedCursorStyle: RenderedCursorStyle = this.focused ? cursor.style : 'block_hollow';
		const cursorEnabled =
			viewportY === 0 &&
			cursor.visible &&
			(!this.focused || this.cursorVisible) &&
			preeditLayout === null;
		const scrollbackLength = scrollbackProvider ? scrollbackProvider.getScrollbackLength() : 0;
		this.currentAbsoluteTop = scrollbackLength - Math.floor(viewportY);
		this.syncTextBlink(
			this.hasBlinkingCells(buffer, dims.rows, viewportY, scrollbackProvider, scrollbackLength),
		);

		if (buffer.needsFullRedraw?.()) forceAll = true;
		if (this.preeditDirty) forceAll = true;
		if (this.fontDirty) forceAll = true;
		if (this.textBlinkDirty) forceAll = true;
		if (this.searchDirty) forceAll = true;

		const needsResize =
			this.canvas.width !== dims.cols * this.metrics.width * this.devicePixelRatio ||
			this.canvas.height !== dims.rows * this.metrics.height * this.devicePixelRatio;
		if (needsResize) {
			this.resize(dims.cols, dims.rows);
			forceAll = true;
		}

		if (viewportY !== this.lastViewportY) {
			forceAll = true;
			this.lastViewportY = viewportY;
		}
		if (scrollbarOpacity !== this.lastScrollbarOpacity) {
			forceAll = true;
			this.lastScrollbarOpacity = scrollbarOpacity;
		}

		// Selection
		const selectionManager = this.selectionManager;
		const hasSelection = selectionManager?.hasSelection() ?? false;
		const selectionRows = new Set<number>();
		this.currentSelectionCoords = hasSelection
			? (selectionManager?.getSelectionCoords() ?? null)
			: null;
		if (this.currentSelectionCoords) {
			const coords = this.currentSelectionCoords;
			const startRow = Math.max(0, Math.min(coords.startRow, coords.endRow));
			const endRow = Math.min(dims.rows - 1, Math.max(coords.startRow, coords.endRow));
			for (let row = startRow; row <= endRow; row++) selectionRows.add(row);
		}
		if (selectionManager) {
			const dirtyRows = selectionManager.getDirtySelectionRows();
			for (const row of dirtyRows) selectionRows.add(row);
			selectionManager.clearDirtySelectionRows();
		}

		// Hyperlink tracking
		const hyperlinkRows = new Set<number>();
		const hyperlinkChanged = this.hoveredHyperlinkId !== this.previousHoveredHyperlinkId;
		const linkRangeChanged =
			JSON.stringify(this.hoveredLinkRange) !== JSON.stringify(this.previousHoveredLinkRange);

		if (hyperlinkChanged) {
			for (let y = 0; y < dims.rows; y++) {
				const line = this.fetchLine(buffer, y, viewportY, scrollbackProvider, scrollbackLength);
				if (line) {
					for (const cell of line) {
						if (
							cell.hyperlink_id === this.hoveredHyperlinkId ||
							cell.hyperlink_id === this.previousHoveredHyperlinkId
						) {
							hyperlinkRows.add(y);
							break;
						}
					}
				}
			}
			this.previousHoveredHyperlinkId = this.hoveredHyperlinkId;
		}

		if (linkRangeChanged) {
			if (this.previousHoveredLinkRange) {
				for (
					let y = this.previousHoveredLinkRange.startY;
					y <= this.previousHoveredLinkRange.endY;
					y++
				)
					hyperlinkRows.add(y);
			}
			if (this.hoveredLinkRange) {
				for (let y = this.hoveredLinkRange.startY; y <= this.hoveredLinkRange.endY; y++)
					hyperlinkRows.add(y);
			}
			this.previousHoveredLinkRange = this.hoveredLinkRange;
		}

		// Rows to render
		const rowsToRender = new Set<number>();
		for (let y = 0; y < dims.rows; y++) {
			const needsRender =
				viewportY > 0
					? true
					: forceAll || buffer.isRowDirty(y) || selectionRows.has(y) || hyperlinkRows.has(y);
			if (needsRender) {
				rowsToRender.add(y);
				if (y > 0) rowsToRender.add(y - 1);
				if (y < dims.rows - 1) rowsToRender.add(y + 1);
			}
		}
		if (viewportY === 0) {
			rowsToRender.add(cursor.y);
			rowsToRender.add(this.lastCursorPosition.y);
		}

		const ctx = this.ctx2d;

		for (let y = 0; y < dims.rows; y++) {
			if (!rowsToRender.has(y)) continue;

			const line = this.fetchLine(buffer, y, viewportY, scrollbackProvider, scrollbackLength);
			if (!line) continue;
			this.frameRowsRendered++;
			const shaping = this.buildRowTextShaping(
				buffer,
				line,
				y,
				viewportY,
				scrollbackProvider,
				scrollbackLength,
				cursor,
				cursorEnabled,
				preeditLayout,
			);

			const lineY = y * this.metrics.height;
			const lineWidth = dims.cols * this.metrics.width;

			ctx.clearRect(0, lineY, lineWidth, this.metrics.height);
			ctx.fillStyle = this.theme.background;
			if (!this.allowTransparency) ctx.fillRect(0, lineY, lineWidth, this.metrics.height);

			// Pass 1: backgrounds
			for (let x = 0; x < line.length; x++) {
				const cell = line[x];
				if (!cell || cell.width === 0) continue;
				this.frameCellsVisited++;
				this.renderCellBackground2D(ctx, cell, x, y);
			}

			// Pass 2: text + decorations
			for (let x = 0; x < line.length; x++) {
				const cell = line[x];
				if (!cell || cell.width === 0) continue;
				const shapingSpan = shaping.plan[x];
				let text = shaping.texts[x] ?? '';
				if (shapingSpan) text = shapingSpan.start === x ? shapingSpan.text : '';
				if (
					text.length > 0 &&
					(cell.flags & CellFlags.INVISIBLE) === 0 &&
					((cell.flags & CellFlags.BLINK) === 0 || this.textBlinkVisible)
				) {
					this.frameGlyphInstances++;
					if (shapingSpan) this.frameShapingRuns++;
				}
				this.renderCellText2D(
					ctx,
					cell,
					x,
					y,
					undefined,
					text,
					cellConstraintWidth(line, x, line.length),
				);
			}
		}

		if (preeditLayout) {
			this.renderPreedit2D(ctx, preeditLayout);
		} else if (cursorEnabled) {
			this.renderCursor2D(ctx, cursor.x, cursor.y, renderedCursorStyle);
		}
		this.lastCursorPosition = { x: cursor.x, y: cursor.y };

		// Scrollbar
		if (scrollbackProvider && scrollbarOpacity > 0) {
			this.renderScrollbar2D(ctx, viewportY, scrollbackLength, dims.rows, scrollbarOpacity);
		}

		this.preeditDirty = false;
		this.textBlinkDirty = false;
		this.searchDirty = false;
		buffer.clearDirty();
	}

	// ==========================================================================
	// Canvas 2D Cell Rendering (Fallback)
	// ==========================================================================

	private renderCellBackground2D(
		ctx: CanvasRenderingContext2D,
		cell: GhosttyCell,
		x: number,
		y: number,
	): void {
		const cellX = x * this.metrics.width;
		const cellY = y * this.metrics.height;
		const cellWidth = this.metrics.width * cell.width;

		const isSelected = this.isInSelection(x, y, cell);
		const searchHighlight = this.getSearchHighlight(x, y);
		if (isSelected) {
			ctx.fillStyle = this.theme.selectionBackground;
			ctx.fillRect(cellX, cellY, cellWidth, this.metrics.height);
			return;
		}
		if (searchHighlight) {
			ctx.fillStyle =
				searchHighlight === 'active'
					? this.theme.searchActiveMatchBackground
					: this.theme.searchMatchBackground;
			ctx.fillRect(cellX, cellY, cellWidth, this.metrics.height);
			return;
		}

		let bgR = cell.bg_r,
			bgG = cell.bg_g,
			bgB = cell.bg_b;
		if (cell.flags & CellFlags.INVERSE) {
			bgR = cell.fg_r;
			bgG = cell.fg_g;
			bgB = cell.fg_b;
		}

		const [themeR, themeG, themeB] = hexToRGB(this.theme.background);
		if (!(bgR === themeR && bgG === themeG && bgB === themeB)) {
			ctx.fillStyle = `rgb(${bgR}, ${bgG}, ${bgB})`;
			ctx.fillRect(cellX, cellY, cellWidth, this.metrics.height);
		}
	}

	private renderCellText2D(
		ctx: CanvasRenderingContext2D,
		cell: GhosttyCell,
		x: number,
		y: number,
		colorOverride?: string,
		textOverride?: string,
		constraintWidth: 1 | 2 = cell.width >= 2 ? 2 : 1,
	): void {
		const cellX = x * this.metrics.width;
		const cellY = y * this.metrics.height;
		const cellWidth = this.metrics.width * cell.width;

		if (cell.flags & CellFlags.INVISIBLE) return;
		if ((cell.flags & CellFlags.BLINK) !== 0 && !this.textBlinkVisible) return;

		const isSelected = this.isInSelection(x, y, cell);
		const searchHighlight = this.getSearchHighlight(x, y);

		const fontStyle = (cell.flags & CellFlags.ITALIC) !== 0 ? 'italic' : 'normal';
		const fontWeight = (cell.flags & CellFlags.BOLD) !== 0 ? this.fontWeightBold : this.fontWeight;
		ctx.font = canvasFont(fontStyle, fontWeight, this.fontSize, this.fontFamily);

		if (colorOverride) {
			ctx.fillStyle = colorOverride;
		} else if (isSelected) {
			ctx.fillStyle = this.theme.selectionForeground;
		} else if (searchHighlight === 'active') {
			ctx.fillStyle = this.theme.searchActiveMatchForeground;
		} else if (searchHighlight === 'match') {
			ctx.fillStyle = this.theme.searchMatchForeground;
		} else {
			let fgR = cell.fg_r,
				fgG = cell.fg_g,
				fgB = cell.fg_b;
			if (cell.flags & CellFlags.INVERSE) {
				fgR = cell.bg_r;
				fgG = cell.bg_g;
				fgB = cell.bg_b;
			}
			ctx.fillStyle = `rgb(${fgR}, ${fgG}, ${fgB})`;
		}

		if (cell.flags & CellFlags.FAINT) ctx.globalAlpha = 0.5;

		const char =
			textOverride ??
			(cell.grapheme_len > 0 && this.currentBuffer?.getGraphemeString
				? this.currentBuffer.getGraphemeString(y, x)
				: String.fromCodePoint(cell.codepoint || 32));
		this.drawConstrainedText2D(
			ctx,
			char,
			cellX,
			cellY,
			cell.codepoint,
			constraintWidth,
			cell.width,
		);

		// Underline
		if (cell.flags & CellFlags.UNDERLINE) {
			const underlineY = cellY + this.detailedMetrics.underlinePosition;
			ctx.strokeStyle =
				cell.underline_r >= 0
					? `rgb(${cell.underline_r}, ${cell.underline_g}, ${cell.underline_b})`
					: ctx.fillStyle;
			ctx.lineWidth = this.detailedMetrics.underlineThickness;
			const style = cell.underline_style || UnderlineStyle.SINGLE;
			if (style === UnderlineStyle.DOTTED) ctx.setLineDash([1, 2]);
			else if (style === UnderlineStyle.DASHED) ctx.setLineDash([3, 2]);
			else ctx.setLineDash([]);
			ctx.beginPath();
			if (style === UnderlineStyle.CURLY) {
				ctx.moveTo(cellX, underlineY);
				for (let offset = 2; offset <= cellWidth; offset += 2) {
					ctx.lineTo(cellX + offset, underlineY + (Math.floor(offset / 2) % 2));
				}
			} else {
				ctx.moveTo(cellX, underlineY);
				ctx.lineTo(cellX + cellWidth, underlineY);
				if (style === UnderlineStyle.DOUBLE) {
					ctx.moveTo(cellX, underlineY - 2);
					ctx.lineTo(cellX + cellWidth, underlineY - 2);
				}
			}
			ctx.stroke();
			ctx.setLineDash([]);
		}

		// Strikethrough
		if (cell.flags & CellFlags.STRIKETHROUGH) {
			const strikeY = cellY + this.detailedMetrics.strikethroughPosition;
			ctx.strokeStyle = ctx.fillStyle;
			ctx.lineWidth = this.detailedMetrics.strikethroughThickness;
			ctx.beginPath();
			ctx.moveTo(cellX, strikeY);
			ctx.lineTo(cellX + cellWidth, strikeY);
			ctx.stroke();
		}

		if (cell.overline) {
			ctx.strokeStyle = ctx.fillStyle;
			ctx.lineWidth = this.detailedMetrics.overlineThickness;
			ctx.beginPath();
			ctx.moveTo(cellX, cellY + this.detailedMetrics.overlinePosition);
			ctx.lineTo(cellX + cellWidth, cellY + this.detailedMetrics.overlinePosition);
			ctx.stroke();
		}

		if (cell.flags & CellFlags.FAINT) ctx.globalAlpha = 1.0;

		// Hyperlink underline
		if (cell.hyperlink_id > 0 && cell.hyperlink_id === this.hoveredHyperlinkId) {
			const underlineY = cellY + this.detailedMetrics.underlinePosition;
			ctx.strokeStyle = '#4A90E2';
			ctx.lineWidth = this.detailedMetrics.underlineThickness;
			ctx.beginPath();
			ctx.moveTo(cellX, underlineY);
			ctx.lineTo(cellX + cellWidth, underlineY);
			ctx.stroke();
		}

		// Regex link underline
		if (this.hoveredLinkRange) {
			const range = this.hoveredLinkRange;
			const isInRange =
				(y === range.startY && x >= range.startX && (y < range.endY || x <= range.endX)) ||
				(y > range.startY && y < range.endY) ||
				(y === range.endY && x <= range.endX && (y > range.startY || x >= range.startX));
			if (isInRange) {
				const underlineY = cellY + this.detailedMetrics.underlinePosition;
				ctx.strokeStyle = '#4A90E2';
				ctx.lineWidth = this.detailedMetrics.underlineThickness;
				ctx.beginPath();
				ctx.moveTo(cellX, underlineY);
				ctx.lineTo(cellX + cellWidth, underlineY);
				ctx.stroke();
			}
		}
	}

	private drawConstrainedText2D(
		ctx: CanvasRenderingContext2D,
		text: string,
		cellX: number,
		cellY: number,
		codepoint: number,
		constraintWidth: 1 | 2,
		cellWidth: number,
	): void {
		if (
			drawGhosttySprite(
				ctx,
				codepoint,
				cellX,
				cellY,
				this.detailedMetrics.cellWidth * cellWidth,
				this.detailedMetrics.cellHeight,
				this.detailedMetrics.boxThickness,
			)
		) {
			return;
		}
		const layout = this.glyphRasterLayout(ctx.measureText(text), 1, codepoint, constraintWidth);
		ctx.save();
		ctx.translate(cellX + layout.target.x, cellY + layout.targetTop);
		ctx.scale(layout.scaleX, layout.scaleY);
		ctx.fillText(text, layout.left, layout.ascent);
		ctx.restore();
	}

	private renderCursor2D(
		ctx: CanvasRenderingContext2D,
		x: number,
		y: number,
		style: RenderedCursorStyle,
	): void {
		ctx.fillStyle = this.theme.cursor;

		switch (style) {
			case 'block': {
				const geometry = cursorBlockGeometry(x, y, this.detailedMetrics);
				ctx.fillRect(geometry.x, geometry.y, geometry.width, geometry.height);
				const line = this.currentBuffer?.getLine(y);
				const cursorCell = line?.[x];
				if (cursorCell) {
					ctx.save();
					ctx.beginPath();
					ctx.rect(geometry.x, geometry.y, geometry.width, geometry.height);
					ctx.clip();
					this.renderCellText2D(
						ctx,
						cursorCell,
						x,
						y,
						this.theme.cursorAccent,
						undefined,
						line ? cellConstraintWidth(line, x, line.length) : undefined,
					);
					ctx.restore();
				}
				break;
			}
			case 'block_hollow': {
				for (const geometry of cursorOutlineGeometry(x, y, this.detailedMetrics)) {
					ctx.fillRect(geometry.x, geometry.y, geometry.width, geometry.height);
				}
				break;
			}
			case 'underline': {
				const geometry = cursorStrokeGeometry(style, x, y, this.detailedMetrics);
				ctx.fillRect(geometry.x, geometry.y, geometry.width, geometry.height);
				break;
			}
			case 'bar': {
				const geometry = cursorStrokeGeometry(style, x, y, this.detailedMetrics);
				ctx.fillRect(geometry.x, geometry.y, geometry.width, geometry.height);
				break;
			}
		}
	}

	private renderPreedit2D(ctx: CanvasRenderingContext2D, layout: PreeditLayout): void {
		const cellY = layout.row * this.metrics.height;
		const startX = layout.start * this.metrics.width;
		const width = layout.width * this.metrics.width;
		ctx.fillStyle = this.theme.background;
		ctx.fillRect(startX, cellY, width, this.metrics.height);
		ctx.font = canvasFont('normal', this.fontWeight, this.fontSize, this.fontFamily);
		ctx.fillStyle = this.theme.foreground;
		let column = layout.start;
		for (const cell of layout.cells) {
			this.drawConstrainedText2D(
				ctx,
				cell.text,
				column * this.metrics.width,
				cellY,
				cell.text.codePointAt(0) ?? 0,
				cell.width >= 2 ? 2 : 1,
				cell.width,
			);
			column += cell.width;
		}
		ctx.fillStyle = this.theme.foreground;
		ctx.fillRect(
			startX,
			cellY + this.detailedMetrics.underlinePosition,
			width,
			this.detailedMetrics.underlineThickness,
		);
	}

	private renderScrollbar2D(
		ctx: CanvasRenderingContext2D,
		viewportY: number,
		scrollbackLength: number,
		visibleRows: number,
		opacity: number,
	): void {
		const canvasHeight = this.canvas.height / this.devicePixelRatio;
		const canvasWidth = this.canvas.width / this.devicePixelRatio;

		const scrollbarWidth = 8;
		const scrollbarX = canvasWidth - scrollbarWidth - 4;
		const scrollbarPadding = 4;
		const scrollbarTrackHeight = canvasHeight - scrollbarPadding * 2;

		ctx.clearRect(scrollbarX - 2, 0, scrollbarWidth + 6, canvasHeight);
		ctx.fillStyle = this.theme.background;
		if (!this.allowTransparency) {
			ctx.fillRect(scrollbarX - 2, 0, scrollbarWidth + 6, canvasHeight);
		}

		if (opacity <= 0 || scrollbackLength === 0) return;

		const totalLines = scrollbackLength + visibleRows;
		const thumbHeight = Math.max(20, (visibleRows / totalLines) * scrollbarTrackHeight);
		const scrollPosition = viewportY / scrollbackLength;
		const thumbY = scrollbarPadding + (scrollbarTrackHeight - thumbHeight) * (1 - scrollPosition);

		ctx.fillStyle = `rgba(128, 128, 128, ${0.1 * opacity})`;
		ctx.fillRect(scrollbarX, scrollbarPadding, scrollbarWidth, scrollbarTrackHeight);

		const isScrolled = viewportY > 0;
		const baseOpacity = isScrolled ? 0.5 : 0.3;
		ctx.fillStyle = `rgba(128, 128, 128, ${baseOpacity * opacity})`;
		ctx.fillRect(scrollbarX, thumbY, scrollbarWidth, thumbHeight);
	}

	// ==========================================================================
	// Shared Helpers
	// ==========================================================================

	private fetchLine(
		buffer: IRenderable,
		y: number,
		viewportY: number,
		scrollbackProvider: IScrollbackProvider | undefined,
		scrollbackLength: number,
	): GhosttyCell[] | null {
		const offset = Math.floor(viewportY);
		if (offset > 0) {
			if (y < offset && scrollbackProvider) {
				const scrollbackOffset = scrollbackLength - offset + y;
				return scrollbackProvider.getScrollbackLine(scrollbackOffset);
			}
			const screenRow = y - offset;
			return buffer.getLine(screenRow);
		}
		return buffer.getLine(y);
	}

	private resolveCellText(
		buffer: IRenderable,
		cell: GhosttyCell,
		renderRow: number,
		col: number,
		viewportY: number,
		scrollbackProvider: IScrollbackProvider | undefined,
		scrollbackLength: number,
	): string {
		if (cell.grapheme_len === 0) {
			return cell.codepoint === 0 ? '' : String.fromCodePoint(cell.codepoint);
		}
		const offset = Math.floor(viewportY);
		if (offset > 0 && renderRow < offset) {
			const historyRow = scrollbackLength - offset + renderRow;
			return scrollbackProvider?.getScrollbackGraphemeString?.(historyRow, col) ?? '';
		}
		const screenRow = offset > 0 ? renderRow - offset : renderRow;
		return buffer.getGraphemeString?.(screenRow, col) ?? '';
	}

	private buildRowTextShaping(
		buffer: IRenderable,
		line: readonly GhosttyCell[],
		renderRow: number,
		viewportY: number,
		scrollbackProvider: IScrollbackProvider | undefined,
		scrollbackLength: number,
		cursor: { readonly x: number; readonly y: number },
		cursorEnabled: boolean,
		preeditLayout: PreeditLayout | null,
	): RowTextShaping {
		const texts: string[] = [];
		const cells: TextShapingCell[] = [];
		for (let column = 0; column < line.length; column++) {
			const cell = line[column];
			if (!cell) {
				texts.push('');
				cells.push({ text: '', width: 0, styleKey: '', eligible: false });
				continue;
			}

			const text = this.resolveCellText(
				buffer,
				cell,
				renderRow,
				column,
				viewportY,
				scrollbackProvider,
				scrollbackLength,
			);
			texts.push(text);
			const selected = this.isInSelection(column, renderRow, cell);
			const search = this.getSearchHighlight(column, renderRow) ?? 'none';
			const cursorBoundary = cursorEnabled && cursor.y === renderRow && cursor.x === column;
			const preeditBoundary =
				preeditLayout !== null &&
				preeditLayout.row === renderRow &&
				column >= preeditLayout.start &&
				column < preeditLayout.end;
			const hidden =
				(cell.flags & CellFlags.INVISIBLE) !== 0 ||
				((cell.flags & CellFlags.BLINK) !== 0 && !this.textBlinkVisible);
			const inverse = (cell.flags & CellFlags.INVERSE) !== 0;
			const foreground = inverse
				? `${cell.bg_r},${cell.bg_g},${cell.bg_b}`
				: `${cell.fg_r},${cell.fg_g},${cell.fg_b}`;
			const styleKey = [
				cell.flags,
				selected ? 'selection' : search,
				foreground,
				cell.underline_style,
				cell.underline_r,
				cell.underline_g,
				cell.underline_b,
				cell.overline ? 1 : 0,
			].join(':');
			cells.push({
				text,
				width: cell.width,
				styleKey,
				eligible:
					cell.width === 1 && !hidden && !cursorBoundary && !preeditBoundary && text.length === 1,
			});
		}

		return {
			texts,
			plan: this.fontLigatures
				? buildTextShapingPlan(cells)
				: Array.from({ length: cells.length }, () => null),
		};
	}

	private isInSelection(x: number, y: number, cell: GhosttyCell): boolean {
		if (cell.selected) return true;
		return this.isCoordinateInSelection(x, y);
	}

	private isCoordinateInSelection(x: number, y: number): boolean {
		const sel = this.currentSelectionCoords;
		if (!sel) return false;
		const { startCol, startRow, endCol, endRow } = sel;
		if (sel.rectangle) {
			return y >= startRow && y <= endRow && x >= startCol && x <= endCol;
		}
		if (startRow === endRow) return y === startRow && x >= startCol && x <= endCol;
		if (y === startRow) return x >= startCol;
		if (y === endRow) return x <= endCol;
		if (y > startRow && y < endRow) return true;
		return false;
	}

	private getSearchHighlight(x: number, viewportRow: number): 'active' | 'match' | null {
		const row = this.currentAbsoluteTop + viewportRow;
		if (this.activeSearchMatch && searchMatchContainsCell(this.activeSearchMatch, x, row)) {
			return 'active';
		}
		this.indexVisibleSearchMatches();
		return (this.searchMatchesByRow.get(row) ?? []).some((match) =>
			searchMatchContainsCell(match, x, row),
		)
			? 'match'
			: null;
	}

	private indexVisibleSearchMatches(): void {
		const rows = this.currentBuffer?.getDimensions().rows ?? 0;
		if (
			this.indexedSearchMatchesVersion === this.searchMatchesVersion &&
			this.indexedSearchTop === this.currentAbsoluteTop &&
			this.indexedSearchRows === rows
		) {
			return;
		}
		this.searchMatchesByRow.clear();
		const firstRow = this.currentAbsoluteTop;
		const lastRow = firstRow + rows - 1;
		for (const match of this.searchMatches) {
			if (match.end.row < firstRow || match.start.row > lastRow) continue;
			const matchFirstRow = Math.max(firstRow, match.start.row);
			const matchLastRow = Math.min(lastRow, match.end.row);
			for (let row = matchFirstRow; row <= matchLastRow; row++) {
				const existing = this.searchMatchesByRow.get(row);
				if (existing === undefined) {
					this.searchMatchesByRow.set(row, [match]);
				} else {
					existing.push(match);
				}
			}
		}
		this.indexedSearchMatchesVersion = this.searchMatchesVersion;
		this.indexedSearchTop = firstRow;
		this.indexedSearchRows = rows;
	}

	// ==========================================================================
	// Cursor Blinking
	// ==========================================================================

	private startCursorBlink(): void {
		if (this.suspended || this.cursorBlinkInterval !== undefined) return;
		this.cursorBlinkInterval = setInterval(() => {
			this.cursorVisible = !this.cursorVisible;
			this.requestRenderCallback?.();
		}, 530);
	}

	private syncCursorBlink(blinking: boolean): void {
		if (blinking === this.cursorBlink) return;
		this.setCursorBlink(blinking);
	}

	private stopCursorBlink(): void {
		if (this.cursorBlinkInterval !== undefined) {
			clearInterval(this.cursorBlinkInterval);
			this.cursorBlinkInterval = undefined;
		}
		this.cursorVisible = true;
	}

	private hasBlinkingCells(
		buffer: IRenderable,
		rows: number,
		viewportY: number,
		scrollbackProvider: IScrollbackProvider | undefined,
		scrollbackLength: number,
	): boolean {
		for (let row = 0; row < rows; row++) {
			const line = this.fetchLine(buffer, row, viewportY, scrollbackProvider, scrollbackLength);
			if (line?.some((cell) => (cell.flags & CellFlags.BLINK) !== 0)) return true;
		}
		return false;
	}

	private syncTextBlink(active: boolean): void {
		if (active === this.textBlinkActive) return;
		this.textBlinkActive = active;
		if (active) this.startTextBlink();
		else this.stopTextBlink();
	}

	private startTextBlink(): void {
		if (this.suspended || this.textBlinkInterval !== undefined) return;
		this.textBlinkInterval = setInterval(() => {
			this.textBlinkVisible = !this.textBlinkVisible;
			this.textBlinkDirty = true;
			this.requestRenderCallback?.();
		}, 530);
	}

	private stopTextBlink(): void {
		if (this.textBlinkInterval !== undefined) {
			clearInterval(this.textBlinkInterval);
			this.textBlinkInterval = undefined;
		}
		if (!this.textBlinkVisible) this.textBlinkDirty = true;
		this.textBlinkVisible = true;
	}

	// ==========================================================================
	// Public API
	// ==========================================================================

	setTheme(theme: ITheme): void {
		this.theme = { ...DEFAULT_THEME, ...theme };
		this.palette = this.buildPalette();
		this.requestRenderCallback?.();
	}

	setPreedit(cells: readonly PreeditCell[], selection?: PreeditSelection): void {
		this.preeditCells = cells
			.filter((cell) => cell.text.length > 0 && cell.width > 0)
			.map((cell) => ({ text: cell.text, width: cell.width >= 2 ? 2 : 1 }));
		const width = this.preeditCells.reduce((total, cell) => total + cell.width, 0);
		const start = clampNumber(selection?.start ?? width, 0, width);
		this.preeditSelection = {
			start,
			end: clampNumber(selection?.end ?? start, start, width),
		};
		this.preeditDirty = true;
		this.requestRenderCallback?.();
	}

	setFontSize(size: number): void {
		this.fontSize = size;
		this.remeasureFont();
		this.resetAtlas();
		this.fontDirty = true;
		this.requestRenderCallback?.();
	}

	setFontFamily(family: string): void {
		this.fontFamily = family;
		this.remeasureFont();
		this.resetAtlas();
		this.fontDirty = true;
		this.requestRenderCallback?.();
	}

	setFontWeight(weight: FontWeight): void {
		if (weight === this.fontWeight) return;
		this.fontWeight = weight;
		this.remeasureFont();
		this.resetAtlas();
		this.fontDirty = true;
		this.requestRenderCallback?.();
	}

	setFontWeightBold(weight: FontWeight): void {
		if (weight === this.fontWeightBold) return;
		this.fontWeightBold = weight;
		this.resetAtlas();
		this.fontDirty = true;
		this.requestRenderCallback?.();
	}

	setFontLigatures(enabled: boolean): void {
		if (enabled === this.fontLigatures) return;
		this.fontLigatures = enabled;
		this.fontDirty = true;
		this.requestRenderCallback?.();
	}

	setFontMetricModifiers(modifiers: FontMetricModifiers): void {
		this.fontMetricModifiers = { ...modifiers };
		this.remeasureFont();
		this.resetAtlas();
		this.resize(this.renderedColumns, this.renderedRows);
		this.fontDirty = true;
		this.requestRenderCallback?.();
	}

	setPaddingBalance(mode: PaddingBalance): void {
		if (mode === this.paddingBalance) return;
		this.paddingBalance = mode;
		this.syncSurfacePadding();
		this.requestRenderCallback?.();
	}

	setCursorStyle(_style: 'block' | 'underline' | 'bar'): void {
		this.requestRenderCallback?.();
	}

	/** Match Ghostty by replacing an unfocused cursor with a non-blinking hollow block. */
	setFocused(focused: boolean): void {
		if (focused === this.focused) return;
		this.focused = focused;
		this.cursorAppearanceDirty = true;
		this.requestRenderCallback?.();
	}

	setCursorBlink(enabled: boolean): void {
		if (enabled === this.cursorBlink) return;
		this.cursorBlink = enabled;
		if (enabled) this.startCursorBlink();
		else this.stopCursorBlink();
		this.requestRenderCallback?.();
	}

	/** Suspend animation timers while the surface is not visible. */
	setSuspended(suspended: boolean): void {
		if (suspended === this.suspended) return;
		this.suspended = suspended;
		if (suspended) {
			this.stopCursorBlink();
			this.stopTextBlink();
		} else {
			if (this.cursorBlink) this.startCursorBlink();
			if (this.textBlinkActive) this.startTextBlink();
		}
		this.requestRenderCallback?.();
	}

	setAllowTransparency(enabled: boolean): void {
		this.allowTransparency = enabled;
		this.requestRenderCallback?.();
	}

	getMetrics(): FontMetrics {
		return { ...this.metrics };
	}

	/** Measure regular preedit text using the same CSS font as the hidden input. */
	measureTextWidth(text: string): number {
		this.atlasCtx.font = canvasFont('normal', this.fontWeight, this.fontSize, this.fontFamily);
		return this.atlasCtx.measureText(text).width;
	}

	getCanvas(): HTMLCanvasElement {
		return this.canvas;
	}

	getBackend(): 'webgpu' | 'canvas2d' | 'uninitialized' {
		if (this.useWebGPU) return 'webgpu';
		if (this.ctx2d) return 'canvas2d';
		return 'uninitialized';
	}

	setSelectionManager(manager: SelectionManager): void {
		this.selectionManager = manager;
	}

	setProjectedViewportState(absoluteTop: number, selection: SelectionCoordinates | null): void {
		const nextTop = Math.max(0, Math.floor(absoluteTop));
		if (
			this.currentAbsoluteTop === nextTop &&
			JSON.stringify(this.projectedSelectionCoords) === JSON.stringify(selection)
		) {
			return;
		}
		this.currentAbsoluteTop = nextTop;
		this.projectedSelectionCoords = selection;
		this.searchDirty = true;
		this.requestRenderCallback?.();
	}

	setSearchMatches(
		matches: readonly TerminalSearchMatch[],
		active: TerminalSearchMatch | null = null,
	): void {
		this.searchMatches = [...matches];
		this.searchMatchesVersion += 1;
		this.activeSearchMatch = active;
		this.searchDirty = true;
		this.requestRenderCallback?.();
	}

	setRenderRequestCallback(callback: () => void): void {
		this.requestRenderCallback = callback;
	}

	setHealthChangeCallback(callback: (event: RendererHealthEvent) => void): void {
		this.healthChangeCallback = callback;
	}

	setFrameTelemetryCallback(callback: (telemetry: RendererFrameTelemetry) => void): void {
		this.telemetryCallback = callback;
	}

	getLastFrameTelemetry(): RendererFrameTelemetry | null {
		return this.lastTelemetry;
	}

	setHoveredHyperlinkId(id: number): void {
		this.hoveredHyperlinkId = id;
		this.requestRenderCallback?.();
	}

	setHoveredLinkRange(
		range: { startX: number; startY: number; endX: number; endY: number } | null,
	): void {
		this.hoveredLinkRange = range;
		this.requestRenderCallback?.();
	}

	get charWidth(): number {
		return this.metrics.width;
	}

	get charHeight(): number {
		return this.metrics.height;
	}

	clear(): void {
		if (this.ctx2d) {
			this.ctx2d.clearRect(0, 0, this.canvas.width, this.canvas.height);
			this.ctx2d.fillStyle = this.theme.background;
			if (!this.allowTransparency) {
				this.ctx2d.fillRect(0, 0, this.canvas.width, this.canvas.height);
			}
		}
	}

	private emitHealth(event: RendererHealthEvent): void {
		this.healthChangeCallback?.(event);
	}

	dispose(): void {
		if (this.disposed) return;
		this.disposed = true;
		this.stopCursorBlink();
		this.stopTextBlink();
		this.requestRenderCallback = undefined;
		this.healthChangeCallback = undefined;
		this.telemetryCallback = undefined;
		this.releaseGPUResources();
		this.device?.destroy();
		this.device = null;
		this.context = null;
		this.ctx2d = null;
		this.useWebGPU = false;
	}
}

function rendererNow(): number {
	return typeof performance !== 'undefined' && typeof performance.now === 'function'
		? performance.now()
		: Date.now();
}

function isCellInRange(
	x: number,
	y: number,
	range: { startX: number; startY: number; endX: number; endY: number },
): boolean {
	if (y < range.startY || y > range.endY) return false;
	if (range.startY === range.endY) return x >= range.startX && x <= range.endX;
	if (y === range.startY) return x >= range.startX;
	if (y === range.endY) return x <= range.endX;
	return true;
}

function publicFontMetrics(metrics: DetailedFontMetrics): FontMetrics {
	return {
		width: metrics.cellWidth,
		height: metrics.cellHeight,
		baseline: metrics.cellHeight - metrics.cellBaseline,
	};
}

function cssPixels(value: string): number {
	const parsed = Number.parseFloat(value);
	return Number.isFinite(parsed) ? Math.max(0, parsed) : 0;
}

function setCanvasMargins(canvas: HTMLCanvasElement, padding: SurfacePadding): void {
	canvas.style.marginTop = `${padding.top}px`;
	canvas.style.marginBottom = `${padding.bottom}px`;
	canvas.style.marginLeft = `${padding.left}px`;
	canvas.style.marginRight = `${padding.right}px`;
}

function cursorStrokeGeometry(
	style: 'underline' | 'bar',
	column: number,
	row: number,
	metrics: DetailedFontMetrics,
): RectangleGeometry {
	const thickness = metrics.cursorThickness;
	const box = cursorBlockGeometry(column, row, metrics);
	if (style === 'bar') {
		return {
			x: box.x - thickness,
			y: box.y,
			width: thickness,
			height: box.height,
		};
	}
	return {
		x: box.x,
		y: Math.min(
			row * metrics.cellHeight + metrics.underlinePosition,
			box.y + Math.max(0, box.height - thickness),
		),
		width: box.width,
		height: thickness,
	};
}

function cursorBlockGeometry(
	column: number,
	row: number,
	metrics: DetailedFontMetrics,
): RectangleGeometry {
	return {
		x: column * metrics.cellWidth,
		y: row * metrics.cellHeight + (metrics.cellHeight - metrics.cursorHeight) / 2,
		width: metrics.cellWidth,
		height: metrics.cursorHeight,
	};
}

function cursorOutlineGeometry(
	column: number,
	row: number,
	metrics: DetailedFontMetrics,
): readonly RectangleGeometry[] {
	const thickness = metrics.cursorThickness;
	const box = cursorBlockGeometry(column, row, metrics);
	return [
		{ x: box.x, y: box.y, width: box.width, height: thickness },
		{
			x: box.x,
			y: box.y + box.height - thickness,
			width: box.width,
			height: thickness,
		},
		{ x: box.x, y: box.y, width: thickness, height: box.height },
		{
			x: box.x + box.width - thickness,
			y: box.y,
			width: thickness,
			height: box.height,
		},
	];
}

function clampNumber(value: number, minimum: number, maximum: number): number {
	return Math.max(minimum, Math.min(maximum, value));
}
