/**
 * WebGPU Renderer for Terminal Display
 *
 * High-performance WebGPU-based renderer for packed Terminal Host projections.
 * Falls back to Canvas 2D when WebGPU is unavailable.
 *
 * Features:
 * - Glyph atlas texture for efficient text rendering
 * - Instanced rendering for backgrounds, text, cursor, and selection
 * - Dirty line optimization for 60 FPS
 * - DPI scaling
 * - All text styles (bold, italic, underline, strikethrough, etc.)
 * - Multiple cursor styles (block, underline, bar) with blinking
 * - Selection overlay rendering
 * - Link underline rendering (OSC8 + regex URLs)
 */

import {
	TerminalCellWidthCode,
	TerminalColorEncoding,
	TerminalStyleFlag,
	TerminalUnderlineKindCode,
	type TerminalGraphicsImage,
	type TerminalGraphicsPlacement,
} from '@orchbee/terminal-protocol';

import { measureTerminalFont, type DetailedFontMetrics } from '../font-metrics';
import {
	cellConstraintWidthFromReader,
	constrainGlyph,
	glyphConstraintForCodepoint,
	isSymbolCodepoint,
	type GlyphSize,
} from '../glyph-constraints';
import {
	drawGhosttySprite,
	ghosttySpriteRasterPadding,
	isGhosttySprite,
	physicalSpriteGeometry,
} from '../ghostty-sprites';
import {
	projectPreeditLayout,
	type PreeditCellData,
	type PreeditLayout,
	type PreeditLayoutRow,
} from '../input';
import { searchMatchContainsCell, type TerminalSearchMatch } from '../search';
import { additionalSurfaceMargins, balanceGridPadding } from '../surface-size';
import { buildTextShapingPlan, type TextShapingPlan } from '../buildTextShapingPlan';
import {
	normalizeTerminalPresentationConfiguration,
	presentationConfigurationFromRendererOptions,
	type TerminalPresentationConfiguration,
} from '../presentation-configuration';
import type {
	FontMetrics,
	FontWeight,
	ITheme,
	RendererOptions,
	SelectionCoordinates,
	SurfacePadding,
} from '../types';
import { resolvePackedCursorPresentation } from './resolvePackedCursorPresentation';

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
	| 'graphics'
	| 'font'
	| 'blink'
	| 'search'
	| 'surface';

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
	readonly cellPixelHeight?: number;
	readonly cellPixelWidth?: number;
	readonly columns: number;
	readonly cursor: {
		readonly blinking: boolean;
		readonly column: number;
		readonly row: number;
		readonly shape: 'bar' | 'block' | 'underline';
		readonly visible: boolean;
		readonly wideTail: boolean;
	};
	readonly defaultColors: {
		readonly backgroundRgb: number;
		readonly cursorRgb: number | null;
		readonly foregroundRgb: number;
	};
	readonly rows: number;
}

export interface PackedRendererGraphics {
	readonly images: ReadonlyMap<number, TerminalGraphicsImage>;
	readonly placements: readonly TerminalGraphicsPlacement[];
	readonly resourceRevision: bigint;
}

/** Packed projection source consumed without reconstructing per-cell objects. */
export interface PackedRendererSource {
	readonly graphics?: PackedRendererGraphics;
	readonly metadata: PackedRendererMetadata | null;
	readonly needsFullRedraw: boolean;
	readonly rows: readonly PackedRendererRow[];
	markRendered(): void;
}

/** Font measurements prepared before one complete presentation configuration commit. */
export interface PreparedTerminalPresentationConfiguration {
	readonly configuration: TerminalPresentationConfiguration;
	readonly detailedMetrics: DetailedFontMetrics;
	readonly metrics: FontMetrics;
}

/** Complete client-owned colors that accompany one Host theme revision. */
export interface TerminalThemeOverlayPresentation {
	readonly cursorAccentRgb: number;
	readonly searchActiveMatchBackgroundRgb: number;
	readonly searchActiveMatchForegroundRgb: number;
	readonly searchMatchBackgroundRgb: number;
	readonly searchMatchForegroundRgb: number;
	readonly selectionBackgroundRgb: number;
	readonly selectionForegroundRgb: number;
}

/** Validated, mutation-free renderer input for one atomic theme publication. */
export interface PreparedTerminalThemePresentation {
	readonly cursorAccent: string;
	readonly theme: Required<ITheme>;
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

export type PreeditCell = PreeditCellData;

export interface PreeditSelection {
	readonly start: number;
	readonly end: number;
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

interface PackedTextPaintSegment {
	endColumn: number;
	readonly foreground: readonly [number, number, number];
	readonly foregroundRgb: number;
	readonly startColumn: number;
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
const CURSOR_BLINK_INTERVAL_MS = 530;
const MAX_CACHED_SHAPING_RASTER_PIXELS = 8 * 1024 * 1024;
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

function configureGhosttyTextRasterization(context: CanvasRenderingContext2D): void {
	// Chromium's default legibility hinting thickens small glyphs. Geometric precision matches
	// Ghostty's unthickened CoreText coverage while retaining the selected variable-font outline.
	Reflect.set(context, 'textRendering', 'geometricPrecision');
}

function snapToDevicePixel(value: number, devicePixelRatio: number): number {
	return Math.round(value * devicePixelRatio) / devicePixelRatio;
}

function packedColorRgb(
	value: number,
	hostDefaultRgb: number,
	effectiveDefaultRgb: number,
): number {
	// Only the tagged default encoding selects a client-effective default. Explicit RGB (including
	// black) and `none` retain their Host-projected behavior.
	if (value === TerminalColorEncoding.default) return effectiveDefaultRgb;
	if (value <= TerminalColorEncoding.rgbMask) return value;
	return hostDefaultRgb;
}

function packedOptionalColorRgb(
	value: number,
	hostDefaultRgb: number,
	effectiveDefaultRgb: number,
): number | null {
	if (value === TerminalColorEncoding.none) return null;
	return packedColorRgb(value, hostDefaultRgb, effectiveDefaultRgb);
}

function rgbChannels(value: number): [number, number, number] {
	return [(value >>> 16) & 0xff, (value >>> 8) & 0xff, value & 0xff];
}

function writeRgbChannels(value: number, target: [number, number, number]): void {
	target[0] = (value >>> 16) & 0xff;
	target[1] = (value >>> 8) & 0xff;
	target[2] = value & 0xff;
}

function rgbValue(channels: readonly [number, number, number]): number {
	return (channels[0] << 16) | (channels[1] << 8) | channels[2];
}

function rgbHex(value: number): string {
	if (!Number.isInteger(value) || value < 0 || value > 0xff_ff_ff) {
		throw new TypeError('Terminal theme colors must be unsigned 24-bit integers');
	}
	return `#${value.toString(16).padStart(6, '0')}`;
}

export interface PackedPresentationColors {
	readonly background: readonly [number, number, number];
	readonly cursor: readonly [number, number, number];
	readonly cursorAccent: readonly [number, number, number];
	readonly foreground: readonly [number, number, number];
	readonly searchActiveBackground: readonly [number, number, number];
	readonly searchActiveForeground: readonly [number, number, number];
	readonly searchBackground: readonly [number, number, number];
	readonly searchForeground: readonly [number, number, number];
	readonly selectionBackground: readonly [number, number, number];
	readonly selectionForeground: readonly [number, number, number];
}

interface PackedCellPresentation {
	readonly background: readonly [number, number, number];
	readonly foreground: readonly [number, number, number];
	readonly requiresBackgroundFill: boolean;
}

/** Resolve Host metadata with one client-only default-color display override and overlay colors. */
export function resolvePackedPresentationColors(
	defaultColors: PackedRendererMetadata['defaultColors'],
	theme: Required<ITheme>,
	cursorAccentOverride?: string,
	defaultColorOverride: TerminalPresentationConfiguration['defaultColorOverride'] = null,
): PackedPresentationColors {
	const hostBackground = rgbChannels(defaultColors.backgroundRgb);
	const background =
		defaultColorOverride === null ? hostBackground : hexToRGB(defaultColorOverride.background);
	const foreground =
		defaultColorOverride === null
			? rgbChannels(defaultColors.foregroundRgb)
			: hexToRGB(defaultColorOverride.foreground);
	return {
		background,
		cursor: rgbChannels(defaultColors.cursorRgb ?? defaultColors.foregroundRgb),
		cursorAccent:
			cursorAccentOverride === undefined ? hostBackground : hexToRGB(cursorAccentOverride),
		foreground,
		searchActiveBackground: hexToRGB(theme.searchActiveMatchBackground),
		searchActiveForeground: hexToRGB(theme.searchActiveMatchForeground),
		searchBackground: hexToRGB(theme.searchMatchBackground),
		searchForeground: hexToRGB(theme.searchMatchForeground),
		selectionBackground: hexToRGB(theme.selectionBackground),
		selectionForeground: hexToRGB(theme.selectionForeground),
	};
}

function packedCellWidth(value: number): number {
	if (value === TerminalCellWidthCode.wide) return 2;
	if (value === TerminalCellWidthCode.narrow) return 1;
	return 0;
}

function normalizeSelectionCoordinates(
	selection: SelectionCoordinates | null,
): SelectionCoordinates | null {
	if (selection === null) return null;
	const { startCol, startRow, endCol, endRow } = selection;
	if (selection.rectangle) {
		const normalized = {
			startCol: Math.min(startCol, endCol),
			startRow: Math.min(startRow, endRow),
			endCol: Math.max(startCol, endCol),
			endRow: Math.max(startRow, endRow),
			rectangle: true,
		} satisfies SelectionCoordinates;
		return normalized.startCol === startCol && normalized.startRow === startRow
			? selection
			: normalized;
	}
	if (startRow < endRow || (startRow === endRow && startCol <= endCol)) return selection;
	return {
		startCol: endCol,
		startRow: endRow,
		endCol: startCol,
		endRow: startRow,
		rectangle: false,
	};
}

function writeGraphicsPixels(target: Uint8ClampedArray, image: TerminalGraphicsImage): void {
	if (image.format === 'rgba') {
		target.set(image.data);
		return;
	}
	let channels = 1;
	if (image.format === 'rgb') channels = 3;
	else if (image.format === 'gray-alpha') channels = 2;
	for (let source = 0, destination = 0; source < image.data.length; source += channels) {
		const gray = image.data[source] ?? 0;
		if (image.format === 'rgb') {
			target[destination] = gray;
			target[destination + 1] = image.data[source + 1] ?? 0;
			target[destination + 2] = image.data[source + 2] ?? 0;
			target[destination + 3] = 255;
		} else {
			target[destination] = gray;
			target[destination + 1] = gray;
			target[destination + 2] = gray;
			target[destination + 3] = image.format === 'gray-alpha' ? (image.data[source + 1] ?? 0) : 255;
		}
		destination += 4;
	}
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

function pushGlyph(
	target: number[],
	glyph: GlyphAtlasEntry,
	x: number,
	y: number,
	atlasWidth: number,
	atlasHeight: number,
	color: readonly [number, number, number],
	alpha: number,
	clipStart = x,
	clipEnd = x + glyph.renderWidth,
): boolean {
	const clippedStart = Math.max(x, clipStart);
	const clippedEnd = Math.min(x + glyph.renderWidth, clipEnd);
	if (clippedEnd <= clippedStart) return false;

	const sourceStart = (clippedStart - x) / glyph.renderWidth;
	const sourceWidth = (clippedEnd - clippedStart) / glyph.renderWidth;
	target.push(
		clippedStart,
		y,
		clippedEnd - clippedStart,
		glyph.renderHeight,
		(glyph.x + sourceStart * glyph.w) / atlasWidth,
		glyph.y / atlasHeight,
		(sourceWidth * glyph.w) / atlasWidth,
		glyph.h / atlasHeight,
		color[0] / 255,
		color[1] / 255,
		color[2] / 255,
		alpha,
		glyph.colored ? 1 : 0,
	);
	return true;
}

export class WebGPURenderer {
	private canvas: HTMLCanvasElement;
	private device: GPUDevice | null = null;
	private context: GPUCanvasContext | null = null;
	private format: GPUTextureFormat;

	// Fallback Canvas 2D renderer
	private ctx2d: CanvasRenderingContext2D | null = null;
	private graphicsCanvas: HTMLCanvasElement | null = null;
	private graphicsContext: CanvasRenderingContext2D | null = null;
	private graphicsResourceCanvases = new Map<
		number,
		{ readonly canvas: HTMLCanvasElement; readonly generation: bigint }
	>();
	private useWebGPU = false;

	// Font and surface presentation
	private presentationConfiguration: TerminalPresentationConfiguration;
	private metrics: FontMetrics;
	private detailedMetrics: DetailedFontMetrics;

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
	private frozen = false;
	private lastCursorPosition: { x: number; y: number } = { x: 0, y: 0 };
	private lastCursorEnabled = false;
	private preeditCells: readonly PreeditCell[] = [];
	private preeditSelection: PreeditSelection = { start: 0, end: 0 };
	private preeditDirty = false;
	private fontDirty = false;
	private surfaceDirty = false;

	// Theme
	private theme: Required<ITheme>;
	private cursorAccentOverride: string | undefined;

	// DPI
	private devicePixelRatio: number;
	private renderedColumns = 0;
	private renderedRows = 0;
	/** Raw Host metadata retained for opaque resize and clear after a client display override. */
	private packedHostDefaultBackgroundRgb: number | null = null;

	// Selection
	private currentSelectionCoords: SelectionCoordinates | null = null;
	private projectedSelectionCoords: SelectionCoordinates | null = null;
	private currentVisibleRows = 0;
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
	private hoveredLinkRange: { startX: number; startY: number; endX: number; endY: number } | null =
		null;

	// Glyph atlas (Canvas 2D for rasterization, uploaded as WebGPU texture)
	private atlasCanvas: HTMLCanvasElement;
	private atlasCtx: CanvasRenderingContext2D;
	private atlasTexture: GPUTexture | null = null;
	private atlasSampler: GPUSampler | null = null;
	private glyphMap: Map<string, GlyphAtlasEntry> = new Map();
	private shapingGlyphKeys = new Set<string>();
	private shapingRasterPixels = 0;
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
	private backgroundVertexData: number[] = [];
	private textVertexData: number[] = [];
	private overlayVertexData: number[] = [];
	private backgroundVertexUpload = new Float32Array(0);
	private textVertexUpload = new Float32Array(0);
	private overlayVertexUpload = new Float32Array(0);
	private readonly uniformUpload = new Float32Array(4);
	private readonly cellPresentationScratch: {
		background: [number, number, number];
		foreground: [number, number, number];
		requiresBackgroundFill: boolean;
	} = {
		background: [0, 0, 0],
		foreground: [0, 0, 0],
		requiresBackgroundFill: false,
	};
	private readonly cellTextForegroundScratch: [number, number, number] = [0, 0, 0];
	private readonly drawnTextForegroundScratch: [number, number, number] = [0, 0, 0];

	constructor(canvas: HTMLCanvasElement, options: RendererOptions = {}) {
		this.canvas = canvas;
		this.format = 'bgra8unorm';
		this.presentationConfiguration = presentationConfigurationFromRendererOptions(options);
		this.cursorBlink = options.cursorBlink ?? false;
		this.theme = { ...DEFAULT_THEME, ...options.theme };
		this.cursorAccentOverride = options.theme?.cursorAccent;
		this.devicePixelRatio = options.devicePixelRatio ?? window.devicePixelRatio ?? 1;

		this.detailedMetrics = this.measureFont();
		this.metrics = publicFontMetrics(this.detailedMetrics);

		// Create atlas canvas for glyph rasterization
		this.atlasCanvas = document.createElement('canvas');
		this.atlasCanvas.width = 2048;
		this.atlasCanvas.height = 2048;
		const atlasCtx = this.atlasCanvas.getContext('2d');
		if (!atlasCtx) throw new Error('Failed to create atlas canvas context');
		this.atlasCtx = atlasCtx;
		configureGhosttyTextRasterization(this.atlasCtx);
		this.atlasCtx.direction = 'ltr';
		this.atlasCtx.imageSmoothingEnabled = false;
		this.atlasCtx.clearRect(0, 0, 2048, 2048);

		if (this.cursorBlink) this.startCursorBlink();
	}

	/** Initialize WebGPU. Call after construction. */
	async initGPU(): Promise<boolean> {
		if (this.frozen || this.disposed) return false;
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
			if (this.disposed || this.frozen) {
				device.destroy();
				throw new Error('Renderer was retired during WebGPU initialization');
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
		if (this.disposed || this.frozen || info.reason === 'destroyed' || device !== this.device)
			return;
		this.useWebGPU = false;
		this.emitHealth({
			state: 'lost',
			backend: 'webgpu',
			reason: info.message || info.reason,
		});
		void this.recoverWebGPU();
	}

	private async recoverWebGPU(): Promise<void> {
		if (this.disposed || this.frozen || this.recoveryInFlight) return;
		this.recoveryInFlight = true;
		this.emitHealth({ state: 'recovering', backend: 'webgpu', attempt: 1 });
		this.releaseGPUResources();
		try {
			await this.initializeGPU(false);
		} catch (error) {
			if (!this.disposed && !this.frozen) {
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
		ctx.direction = 'ltr';
		configureGhosttyTextRasterization(ctx);
		ctx.textAlign = 'left';
		ctx.textBaseline = 'alphabetic';
		this.ctx2d = ctx;
		this.useWebGPU = false;
	}

	// ==========================================================================
	// Font Metrics
	// ==========================================================================

	private measureFont(configuration = this.presentationConfiguration): DetailedFontMetrics {
		return measureTerminalFont(this.canvas.ownerDocument, configuration, this.devicePixelRatio);
	}

	private remeasureFont(): void {
		this.detailedMetrics = this.measureFont();
		this.metrics = publicFontMetrics(this.detailedMetrics);
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
		let upload: Float32Array<ArrayBuffer>;
		if (kind === 'background') {
			currentBuffer = this.bgVertexBuffer;
			currentSize = this.bgVertexBufferSize;
			upload = this.backgroundVertexUpload;
		} else if (kind === 'text') {
			currentBuffer = this.textVertexBuffer;
			currentSize = this.textVertexBufferSize;
			upload = this.textVertexUpload;
		} else {
			currentBuffer = this.overlayVertexBuffer;
			currentSize = this.overlayVertexBufferSize;
			upload = this.overlayVertexUpload;
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

		if (upload.length < data.length) {
			let capacity = 64;
			while (capacity < data.length) capacity *= 2;
			upload = new Float32Array(capacity);
			if (kind === 'background') this.backgroundVertexUpload = upload;
			else if (kind === 'text') this.textVertexUpload = upload;
			else this.overlayVertexUpload = upload;
		}
		upload.set(data);
		// WebGPU snapshots writeBuffer input before returning, so the backing store is reusable on the
		// next frame without extending its lifetime into the GPU queue.
		this.device.queue.writeBuffer(buffer, 0, upload.subarray(0, data.length));
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
		const bold = (flags & TerminalStyleFlag.bold) !== 0 ? 'b' : '';
		const italic = (flags & TerminalStyleFlag.italic) !== 0 ? 'i' : '';
		return JSON.stringify([text, width, codepoint, constraintWidth, `${bold}${italic}`]);
	}

	private configureTextRunAdvance(
		context: CanvasRenderingContext2D,
		shaped: boolean,
		rasterScale: number,
	): void {
		// Terminal cells use the rounded grid width, while Canvas shapes with the face's
		// fractional natural advance. Fill that per-cell difference inside a shaped run so
		// splitting the run at a selection or style boundary cannot move later glyphs.
		const letterSpacing = shaped
			? (this.detailedMetrics.cellWidth - this.detailedMetrics.faceWidth) * rasterScale
			: 0;
		context.letterSpacing = `${letterSpacing}px`;
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

		const fontStyle = (flags & TerminalStyleFlag.italic) !== 0 ? 'italic' : 'normal';
		const fontWeight =
			(flags & TerminalStyleFlag.bold) !== 0
				? this.presentationConfiguration.fontWeightBold
				: this.presentationConfiguration.fontWeight;
		this.atlasCtx.font = canvasFont(
			fontStyle,
			fontWeight,
			this.presentationConfiguration.fontSize * rasterScale,
			this.presentationConfiguration.fontFamily,
		);
		this.configureTextRunAdvance(this.atlasCtx, codepoint === 0, rasterScale);
		this.atlasCtx.textBaseline = 'alphabetic';
		this.atlasCtx.textAlign = 'left';
		const inkPadding = Math.max(1, Math.ceil(rasterScale));
		const spriteGeometry = isGhosttySprite(codepoint)
			? physicalSpriteGeometry(this.detailedMetrics, rasterScale, width)
			: null;
		const raster =
			spriteGeometry === null
				? {
						kind: 'font' as const,
						layout: this.glyphRasterLayout(
							this.atlasCtx.measureText(text),
							rasterScale,
							codepoint,
							constraintWidth,
						),
					}
				: {
						kind: 'sprite' as const,
						geometry: spriteGeometry,
						padding: ghosttySpriteRasterPadding(spriteGeometry.width, spriteGeometry.height),
					};
		const rawOffsetX =
			raster.kind === 'sprite'
				? -(inkPadding + raster.padding.x) / rasterScale
				: raster.layout.target.x - inkPadding / rasterScale;
		const offsetX = snapToDevicePixel(rawOffsetX, rasterScale);
		// Keep the textured quad on physical pixels without discarding the subpixel crop remainder.
		// Rasterizing that remainder into the atlas leaves every natural glyph baseline on the same
		// cell anchor even when a cursor boundary changes the first glyph and its ink bearing.
		const fontRasterShiftX = raster.kind === 'font' ? (rawOffsetX - offsetX) * rasterScale : 0;
		const w =
			raster.kind === 'sprite'
				? raster.geometry.width + raster.padding.x * 2 + inkPadding * 2
				: Math.max(
						1,
						Math.ceil(raster.layout.target.width * rasterScale + fontRasterShiftX) + inkPadding * 2,
					);
		const h =
			raster.kind === 'sprite'
				? raster.geometry.height + raster.padding.y * 2 + inkPadding * 2
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
				this.atlasX + inkPadding + raster.padding.x,
				this.atlasY + inkPadding + raster.padding.y,
				raster.geometry.width,
				raster.geometry.height,
				raster.geometry.boxThickness,
			);
		} else {
			this.atlasCtx.translate(
				this.atlasX + inkPadding + fontRasterShiftX,
				this.atlasY + inkPadding,
			);
			this.atlasCtx.scale(raster.layout.scaleX, raster.layout.scaleY);
			this.atlasCtx.fillText(text, raster.layout.left, raster.layout.ascent);
		}
		this.atlasCtx.restore();

		const offsetY =
			raster.kind === 'sprite'
				? -(inkPadding + raster.padding.y) / rasterScale
				: raster.layout.targetTop - inkPadding / rasterScale;
		const entry: GlyphAtlasEntry = {
			x: this.atlasX,
			y: this.atlasY,
			w,
			h,
			offsetX,
			offsetY: snapToDevicePixel(offsetY, rasterScale),
			renderWidth: w / rasterScale,
			renderHeight: h / rasterScale,
			colored:
				raster.kind === 'sprite' ? false : this.isColorGlyph(text, this.atlasX, this.atlasY, w, h),
		};
		this.glyphMap.set(key, entry);
		if (codepoint === 0) {
			this.shapingGlyphKeys.add(key);
			this.shapingRasterPixels += w * h;
		}
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
			? measured.actualBoundingBoxLeft
			: 0;
		const measuredRight = measured.actualBoundingBoxRight;
		const right = Number.isFinite(measuredRight)
			? measuredRight
			: Math.max(this.metrics.width * rasterScale, measured.width);
		const ascent = hasVerticalBounds ? measuredAscent : baseline;
		const descent = hasVerticalBounds ? measuredDescent : Math.max(0, cellHeight - baseline);
		const inkWidth = left + right;
		const original: GlyphSize = {
			width:
				(Number.isFinite(inkWidth) && inkWidth > 0
					? inkWidth
					: Math.max(this.metrics.width * rasterScale, measured.width)) / rasterScale,
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
		configureGhosttyTextRasterization(context);
		context.imageSmoothingEnabled = false;
		context.drawImage(this.atlasCanvas, 0, 0);
		context.font = previousContext.font;
		context.textAlign = previousContext.textAlign;
		context.textBaseline = previousContext.textBaseline;
		context.direction = previousContext.direction;
		context.letterSpacing = previousContext.letterSpacing;
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
		this.shapingGlyphKeys.clear();
		this.shapingRasterPixels = 0;
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
		const physicalWidth = Math.round(cssWidth * this.devicePixelRatio);
		const physicalHeight = Math.round(cssHeight * this.devicePixelRatio);

		this.canvas.style.width = `${cssWidth}px`;
		this.canvas.style.height = `${cssHeight}px`;

		if (this.useWebGPU && this.context) {
			// WebGPU: canvas size is set via configure
			this.canvas.width = physicalWidth;
			this.canvas.height = physicalHeight;
			const device = this.device;
			if (!device) return;
			this.context.configure({
				device,
				format: this.format,
				alphaMode: 'premultiplied',
			});
		} else if (this.ctx2d) {
			this.canvas.width = physicalWidth;
			this.canvas.height = physicalHeight;
			this.ctx2d.setTransform(this.devicePixelRatio, 0, 0, this.devicePixelRatio, 0, 0);
			this.ctx2d.textBaseline = 'alphabetic';
			this.ctx2d.textAlign = 'left';
			const defaultBackgroundRgb = this.effectiveDefaultBackgroundRgb();
			if (!this.presentationConfiguration.allowTransparency && defaultBackgroundRgb !== null) {
				const [red, green, blue] = rgbChannels(defaultBackgroundRgb);
				this.ctx2d.fillStyle = `rgb(${red}, ${green}, ${blue})`;
				this.ctx2d.fillRect(0, 0, cssWidth, cssHeight);
			}
		}
		this.syncSurfacePadding();
		this.syncGraphicsOverlayGeometry();
	}

	private effectiveDefaultBackgroundRgb(): number | null {
		const hostBackgroundRgb = this.packedHostDefaultBackgroundRgb;
		if (hostBackgroundRgb === null) return null;
		const defaultColorOverride = this.presentationConfiguration.defaultColorOverride;
		return defaultColorOverride === null
			? hostBackgroundRgb
			: rgbValue(hexToRGB(defaultColorOverride.background));
	}

	/** Recalculate CSS margins used to balance leftover surface pixels. */
	syncSurfacePadding(): void {
		const parent = this.canvas.parentElement;
		if (!parent) return;
		if (this.presentationConfiguration.paddingBalance === false) {
			this.setSurfaceMargins(ZERO_PADDING);
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
			this.presentationConfiguration.paddingBalance,
		);
		this.setSurfaceMargins(additionalSurfaceMargins(balanced, explicit));
	}

	private setSurfaceMargins(padding: SurfacePadding): void {
		setCanvasMargins(this.canvas, padding);
		if (this.graphicsCanvas) setCanvasMargins(this.graphicsCanvas, padding);
	}

	private syncGraphicsOverlayGeometry(): void {
		const canvas = this.graphicsCanvas;
		const context = this.graphicsContext;
		if (!canvas || !context) return;
		const cssWidth = this.renderedColumns * this.metrics.width;
		const cssHeight = this.renderedRows * this.metrics.height;
		canvas.style.width = `${cssWidth}px`;
		canvas.style.height = `${cssHeight}px`;
		canvas.style.marginTop = this.canvas.style.marginTop;
		canvas.style.marginBottom = this.canvas.style.marginBottom;
		canvas.style.marginLeft = this.canvas.style.marginLeft;
		canvas.style.marginRight = this.canvas.style.marginRight;
		const physicalWidth = Math.round(cssWidth * this.devicePixelRatio);
		const physicalHeight = Math.round(cssHeight * this.devicePixelRatio);
		if (canvas.width !== physicalWidth || canvas.height !== physicalHeight) {
			canvas.width = physicalWidth;
			canvas.height = physicalHeight;
		}
		context.setTransform(this.devicePixelRatio, 0, 0, this.devicePixelRatio, 0, 0);
		context.imageSmoothingEnabled = true;
	}

	/** Rebuild physical-pixel resources after browser zoom or monitor changes. */
	setDevicePixelRatio(value: number): boolean {
		if (this.frozen || this.disposed) return false;
		const ratio = Number.isFinite(value) && value > 0 ? value : 1;
		if (ratio === this.devicePixelRatio) return false;
		this.devicePixelRatio = ratio;
		this.remeasureFont();
		this.resetAtlas();
		if (this.renderedColumns > 0 && this.renderedRows > 0) {
			this.resize(this.renderedColumns, this.renderedRows);
		}
		this.surfaceDirty = true;
		this.requestRenderCallback?.();
		return true;
	}

	// ==========================================================================
	// Main Rendering
	// ==========================================================================

	/** Draw a Host projection directly from its row-partitioned structure-of-arrays store. */
	renderPacked(buffer: PackedRendererSource, forceAll = false): void {
		if (this.frozen || this.disposed) return;
		const backend = this.getBackend();
		const metadata = buffer.metadata;
		if (backend === 'uninitialized' || !metadata) return;
		this.packedHostDefaultBackgroundRgb = metadata.defaultColors.backgroundRgb;
		const cursorMoved =
			metadata.cursor.column !== this.lastCursorPosition.x ||
			metadata.cursor.row !== this.lastCursorPosition.y;
		this.syncCursorBlink(metadata.cursor.blinking);
		if (cursorMoved) this.restartCursorBlink();
		const timestamp = rendererNow();
		this.currentSelectionCoords = this.projectedSelectionCoords;
		this.currentVisibleRows = metadata.rows;
		const reasons = new Set<RendererFrameReason>();
		if (forceAll || buffer.needsFullRedraw) reasons.add('full');
		if ((buffer.graphics?.placements.length ?? 0) > 0) reasons.add('graphics');
		if (this.cursorAppearanceDirty || cursorMoved) reasons.add('cursor');
		if (this.preeditDirty) reasons.add('preedit');
		if (this.fontDirty) reasons.add('font');
		if (this.textBlinkDirty) reasons.add('blink');
		if (this.searchDirty) reasons.add('search');
		if (this.surfaceDirty) reasons.add('surface');
		if (reasons.size === 0) reasons.add('dirty');

		const redrawAll = forceAll || this.surfaceDirty;
		this.resetFrameCounters();
		if (this.useWebGPU) this.renderPackedWebGPU(buffer, metadata);
		else this.renderPackedCanvas2D(buffer, metadata, redrawAll);
		this.renderPackedGraphics(buffer.graphics, metadata);
		this.fontDirty = false;
		this.cursorAppearanceDirty = false;
		this.searchDirty = false;
		this.surfaceDirty = false;
		this.completeFrameTelemetry(backend, timestamp, reasons, redrawAll, 0);
	}

	private renderPackedGraphics(
		graphics: PackedRendererGraphics | undefined,
		metadata: PackedRendererMetadata,
	): void {
		if (!graphics || graphics.placements.length === 0) {
			this.releaseGraphicsOverlay();
			return;
		}
		const context = this.ensureGraphicsOverlay();
		if (!context) return;
		this.syncGraphicsOverlayGeometry();
		const cssWidth = this.renderedColumns * this.metrics.width;
		const cssHeight = this.renderedRows * this.metrics.height;
		context.clearRect(0, 0, cssWidth, cssHeight);
		const imageIds = new Set(graphics.images.keys());
		for (const imageId of this.graphicsResourceCanvases.keys()) {
			if (!imageIds.has(imageId)) this.graphicsResourceCanvases.delete(imageId);
		}
		const hostCellWidth = metadata.cellPixelWidth ?? this.metrics.width;
		const hostCellHeight = metadata.cellPixelHeight ?? this.metrics.height;
		const horizontalScale = this.metrics.width / hostCellWidth;
		const verticalScale = this.metrics.height / hostCellHeight;
		for (const placement of graphics.placements) {
			const image = graphics.images.get(placement.imageId);
			if (!image) continue;
			const source = this.graphicsResourceCanvas(image);
			if (!source) continue;
			context.drawImage(
				source,
				placement.sourceX,
				placement.sourceY,
				placement.sourceWidth,
				placement.sourceHeight,
				placement.column * this.metrics.width + placement.offsetX * horizontalScale,
				placement.row * this.metrics.height + placement.offsetY * verticalScale,
				placement.destinationWidth * horizontalScale,
				placement.destinationHeight * verticalScale,
			);
			this.frameOverlayInstances++;
		}
	}

	private ensureGraphicsOverlay(): CanvasRenderingContext2D | null {
		if (this.graphicsContext) return this.graphicsContext;
		const parent = this.canvas.parentElement;
		if (!parent) return null;
		const ownerDocument = this.canvas.ownerDocument ?? document;
		const canvas = ownerDocument.createElement('canvas');
		canvas.setAttribute('aria-hidden', 'true');
		canvas.setAttribute('data-terminal-graphics-overlay', 'true');
		canvas.style.display = 'block';
		canvas.style.insetBlockStart = '0';
		canvas.style.insetInlineStart = '0';
		canvas.style.pointerEvents = 'none';
		canvas.style.position = 'absolute';
		canvas.style.zIndex = '1';
		const context = canvas.getContext('2d', { alpha: true });
		if (!context) return null;
		parent.appendChild(canvas);
		this.graphicsCanvas = canvas;
		this.graphicsContext = context;
		return context;
	}

	private graphicsResourceCanvas(image: TerminalGraphicsImage): HTMLCanvasElement | null {
		const cached = this.graphicsResourceCanvases.get(image.id);
		if (cached?.generation === image.generation) return cached.canvas;
		const ownerDocument = this.canvas.ownerDocument ?? document;
		const canvas = ownerDocument.createElement('canvas');
		canvas.width = image.width;
		canvas.height = image.height;
		const context = canvas.getContext('2d', { alpha: true });
		if (!context) return null;
		const pixels = context.createImageData(image.width, image.height);
		writeGraphicsPixels(pixels.data, image);
		context.putImageData(pixels, 0, 0);
		this.graphicsResourceCanvases.set(image.id, { canvas, generation: image.generation });
		return canvas;
	}

	private releaseGraphicsOverlay(): void {
		this.graphicsCanvas?.remove();
		this.graphicsCanvas = null;
		this.graphicsContext = null;
		this.graphicsResourceCanvases.clear();
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
		rows: number,
	): PreeditLayout | null {
		return projectPreeditLayout(
			this.preeditCells,
			this.preeditSelection,
			{ column: cursor.x, row: cursor.y },
			columns,
			rows,
		);
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
		const cursorPresentation = resolvePackedCursorPresentation(cursor, row.widths);
		const preeditRow = preeditLayoutRow(preeditLayout, rowIndex);
		let shapingEnd = 0;
		for (let column = 0; column < columns; column++) {
			const text = texts[column] ?? '';
			const width = packedCellWidth(row.widths[column] ?? TerminalCellWidthCode.spacerTail);
			const meaningful =
				text.length > 0 ||
				(row.styleFlags[column] ?? 0) !== 0 ||
				(row.backgroundColors[column] ?? TerminalColorEncoding.default) !==
					TerminalColorEncoding.default ||
				(row.underlineKinds[column] ?? TerminalUnderlineKindCode.none) !==
					TerminalUnderlineKindCode.none ||
				(row.underlineColors[column] ?? TerminalColorEncoding.none) !== TerminalColorEncoding.none;
			if (meaningful) shapingEnd = Math.max(shapingEnd, column + Math.max(1, width));
		}
		shapingEnd = Math.min(columns, shapingEnd);
		const eligible = (column: number): boolean => {
			const text = texts[column] ?? '';
			const codepoint = text.codePointAt(0) ?? 0;
			const flags = row.styleFlags[column] ?? 0;
			const cellWidth = packedCellWidth(row.widths[column] ?? TerminalCellWidthCode.spacerTail);
			const hidden =
				(flags & TerminalStyleFlag.invisible) !== 0 ||
				((flags & TerminalStyleFlag.blink) !== 0 && !this.textBlinkVisible);
			const cursorBoundary =
				cursorEnabled &&
				cursor.row === rowIndex &&
				column >= cursorPresentation.column &&
				column < cursorPresentation.column + cursorPresentation.width;
			const preeditBoundary =
				preeditRow !== undefined && column >= preeditRow.start && column < preeditRow.end;
			return (
				column < shapingEnd &&
				// Browser shapers expose only natural advances inside a run. Keep wide Host
				// graphemes on their allocated cell heads so fallback faces cannot drift.
				cellWidth === 1 &&
				!hidden &&
				!cursorBoundary &&
				!preeditBoundary &&
				!isGhosttySprite(codepoint) &&
				!(codepoint > 0x7f && isSymbolCodepoint(codepoint))
			);
		};
		// WebGPU must not classify a mixed monochrome/emoji atlas entry as wholly colored.
		const colorGlyphKey = (column: number): boolean =>
			COLOR_GLYPH_CANDIDATE.test(texts[column] ?? '');
		const sameStyle = (left: number, right: number): boolean =>
			row.styleFlags[left] === row.styleFlags[right] &&
			row.underlineKinds[left] === row.underlineKinds[right] &&
			row.underlineColors[left] === row.underlineColors[right] &&
			colorGlyphKey(left) === colorGlyphKey(right);
		return {
			codepoints,
			texts,
			plan: buildTextShapingPlan(
				{
					length: columns,
					eligible,
					sameStyle,
					text: (column) => texts[column] ?? '',
					width: (column) =>
						packedCellWidth(row.widths[column] ?? TerminalCellWidthCode.spacerTail),
				},
				{ allowOptionalLigatures: this.presentationConfiguration.fontLigatures },
			),
		};
	}

	private packedTextPaintSegments(
		startColumn: number,
		endColumn: number,
		row: PackedRendererRow,
		metadata: PackedRendererMetadata,
		viewportRow: number,
		presentation: PackedPresentationColors,
	): readonly PackedTextPaintSegment[] {
		const segments: PackedTextPaintSegment[] = [];
		for (let column = startColumn; column < endColumn; column++) {
			const foregroundRgb = this.packedTextForegroundRgb(
				column,
				viewportRow,
				this.packedCellForegroundRgb(row, column, metadata, presentation),
				presentation,
			);
			const previous = segments.at(-1);
			if (previous?.foregroundRgb === foregroundRgb) {
				previous.endColumn = column + 1;
				continue;
			}
			segments.push({
				endColumn: column + 1,
				foreground: rgbChannels(foregroundRgb),
				foregroundRgb,
				startColumn: column,
			});
		}
		return segments;
	}

	private packedTextForegroundRgb(
		column: number,
		row: number,
		baseForegroundRgb: number,
		presentation: PackedPresentationColors,
	): number {
		if (this.isCoordinateInSelection(column, row))
			return rgbValue(presentation.selectionForeground);
		const search = this.getSearchHighlight(column, row);
		if (search === 'active') return rgbValue(presentation.searchActiveForeground);
		if (search === 'match') return rgbValue(presentation.searchForeground);
		return baseForegroundRgb;
	}

	private packedCellForegroundRgb(
		row: PackedRendererRow,
		column: number,
		metadata: PackedRendererMetadata,
		presentation: PackedPresentationColors,
	): number {
		const flags = row.styleFlags[column] ?? 0;
		if ((flags & TerminalStyleFlag.inverse) !== 0) {
			return packedColorRgb(
				row.backgroundColors[column] ?? TerminalColorEncoding.default,
				metadata.defaultColors.backgroundRgb,
				rgbValue(presentation.background),
			);
		}
		return packedColorRgb(
			row.foregroundColors[column] ?? TerminalColorEncoding.default,
			metadata.defaultColors.foregroundRgb,
			rgbValue(presentation.foreground),
		);
	}

	private packedCellPresentation(
		row: PackedRendererRow,
		column: number,
		metadata: PackedRendererMetadata,
		presentation: PackedPresentationColors,
	): PackedCellPresentation {
		const flags = row.styleFlags[column] ?? 0;
		const foregroundToken = row.foregroundColors[column] ?? TerminalColorEncoding.default;
		const backgroundToken = row.backgroundColors[column] ?? TerminalColorEncoding.default;
		let foregroundRgb = packedColorRgb(
			foregroundToken,
			metadata.defaultColors.foregroundRgb,
			rgbValue(presentation.foreground),
		);
		let backgroundRgb = packedColorRgb(
			backgroundToken,
			metadata.defaultColors.backgroundRgb,
			rgbValue(presentation.background),
		);
		const inverse = (flags & TerminalStyleFlag.inverse) !== 0;
		const displayedBackgroundToken = inverse ? foregroundToken : backgroundToken;
		if (inverse) [foregroundRgb, backgroundRgb] = [backgroundRgb, foregroundRgb];
		writeRgbChannels(backgroundRgb, this.cellPresentationScratch.background);
		writeRgbChannels(foregroundRgb, this.cellPresentationScratch.foreground);
		this.cellPresentationScratch.requiresBackgroundFill =
			inverse ||
			(displayedBackgroundToken !== TerminalColorEncoding.default &&
				displayedBackgroundToken !== TerminalColorEncoding.none);
		return this.cellPresentationScratch;
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

	private shouldResetShapingCache(
		buffer: PackedRendererSource,
		shapings: readonly ((PackedRowTextShaping & { readonly codepoints: Uint32Array }) | null)[],
		columns: number,
	): boolean {
		if (this.shapingRasterPixels < MAX_CACHED_SHAPING_RASTER_PIXELS) return false;
		for (const [rowIndex, row] of buffer.rows.entries()) {
			const shaping = shapings[rowIndex];
			if (shaping === null || shaping === undefined) continue;
			for (let column = 0; column < columns; column++) {
				const span = shaping.plan[column];
				if (span === null || span === undefined || span.start !== column) continue;
				const key = this.getGlyphKey(
					span.text,
					row.styleFlags[column] ?? 0,
					span.columns,
					0,
					this.packedConstraintWidth(row, shaping, column, columns),
				);
				if (!this.shapingGlyphKeys.has(key)) return true;
			}
		}
		return false;
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
		this.syncTextBlink(this.hasPackedBlinkingCells(buffer));
		const physicalWidth = Math.round(columns * this.metrics.width * this.devicePixelRatio);
		const physicalHeight = Math.round(rows * this.metrics.height * this.devicePixelRatio);
		const needsResize =
			this.canvas.width !== physicalWidth || this.canvas.height !== physicalHeight;
		if (needsResize) this.resize(columns, rows);

		const preeditLayout = this.getPreeditLayout({ x: cursor.column, y: cursor.row }, columns, rows);
		const renderedCursorStyle: RenderedCursorStyle = this.focused ? cursor.shape : 'block_hollow';
		const cursorEnabled =
			cursor.visible && (!this.focused || this.cursorVisible) && preeditLayout === null;
		const cursorPresentation = resolvePackedCursorPresentation(
			cursor,
			buffer.rows[cursor.row]?.widths,
		);
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
		}
		if (this.shouldResetShapingCache(buffer, shapings, columns)) this.resetAtlas();

		for (let y = 0; y < rows; y++) {
			const row = buffer.rows[y];
			const shaping = shapings[y];
			if (!row || !shaping) continue;
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
					span ? 0 : (shaping.codepoints[x] ?? 0),
					this.packedConstraintWidth(row, shaping, x, columns),
				);
			}
		}
		if (preeditLayout) {
			for (const preeditRow of preeditLayout.rows) {
				for (const cell of preeditRow.cells) this.ensureGlyph(cell.text, 0, cell.width);
			}
		}

		const bgData = this.backgroundVertexData;
		const textData = this.textVertexData;
		const overlayData = this.overlayVertexData;
		bgData.length = 0;
		textData.length = 0;
		overlayData.length = 0;
		const presentation = resolvePackedPresentationColors(
			metadata.defaultColors,
			this.theme,
			this.cursorAccentOverride,
			this.presentationConfiguration.defaultColorOverride,
		);
		const defaultBackground = presentation.background;
		const defaultForeground = presentation.foreground;
		const cursorColor = presentation.cursor;
		const cursorAccent = presentation.cursorAccent;
		const atlasWidth = this.atlasCanvas.width;
		const atlasHeight = this.atlasCanvas.height;

		for (let y = 0; y < rows; y++) {
			const row = buffer.rows[y];
			const shaping = shapings[y];
			if (!row || !shaping) continue;
			const preeditRow = preeditLayoutRow(preeditLayout, y);
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
					cursorPresentation.column === x &&
					cursor.row === y;
				const isFullBlockCursor =
					isBlockCursor && this.detailedMetrics.cursorHeight === this.detailedMetrics.cellHeight;
				const cellPresentation = this.packedCellPresentation(row, x, metadata, presentation);
				let foreground: readonly [number, number, number] = cellPresentation.foreground;
				let background: readonly [number, number, number] = cellPresentation.background;
				const selected = this.isCoordinateInSelection(x, y);
				const search = this.getSearchHighlight(x, y);
				if (selected) {
					background = presentation.selectionBackground;
					foreground = presentation.selectionForeground;
				} else if (search !== null) {
					background =
						search === 'active'
							? presentation.searchActiveBackground
							: presentation.searchBackground;
					foreground =
						search === 'active'
							? presentation.searchActiveForeground
							: presentation.searchForeground;
				}
				if (isFullBlockCursor) background = cursorColor;
				if (
					cellPresentation.requiresBackgroundFill ||
					isFullBlockCursor ||
					selected ||
					search !== null
				) {
					pushRect(bgData, cellX, cellY, cellWidth, this.metrics.height, background);
				}
				if (isBlockCursor && !isFullBlockCursor) {
					const geometry = cursorBlockGeometry(
						x,
						y,
						this.detailedMetrics,
						cursorPresentation.width,
					);
					pushRect(bgData, geometry.x, geometry.y, geometry.width, geometry.height, cursorColor);
				}
				if (preeditRow && x >= preeditRow.start && x < preeditRow.end) continue;
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
						span ? 0 : (shaping.codepoints[x] ?? 0),
						this.packedConstraintWidth(row, shaping, x, columns),
					);
					if (glyph) {
						const glyphX = cellX + glyph.offsetX;
						const glyphY = cellY + glyph.offsetY;
						if (span) {
							for (const segment of this.packedTextPaintSegments(
								span.start,
								span.end,
								row,
								metadata,
								y,
								presentation,
							)) {
								pushGlyph(
									textData,
									glyph,
									glyphX,
									glyphY,
									atlasWidth,
									atlasHeight,
									segment.foreground,
									alpha,
									segment.startColumn === span.start
										? glyphX
										: segment.startColumn * this.metrics.width,
									segment.endColumn === span.end
										? glyphX + glyph.renderWidth
										: segment.endColumn * this.metrics.width,
								);
							}
						} else {
							pushGlyph(
								textData,
								glyph,
								glyphX,
								glyphY,
								atlasWidth,
								atlasHeight,
								foreground,
								alpha,
							);
						}
					}
				}

				const underline = row.underlineKinds[x] ?? TerminalUnderlineKindCode.none;
				const underlineRgb = packedOptionalColorRgb(
					row.underlineColors[x] ?? TerminalColorEncoding.none,
					metadata.defaultColors.foregroundRgb,
					rgbValue(presentation.foreground),
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
				if (this.hoveredLinkRange && isCellInRange(x, y, this.hoveredLinkRange)) {
					pushRect(
						overlayData,
						cellX,
						cellY + this.detailedMetrics.underlinePosition,
						cellWidth,
						this.detailedMetrics.underlineThickness,
						[74, 144, 226],
						alpha,
					);
				}
			}
		}

		if (preeditLayout) {
			for (const preeditRow of preeditLayout.rows) {
				const cellY = preeditRow.row * this.metrics.height;
				const startX = preeditRow.start * this.metrics.width;
				pushRect(
					bgData,
					startX,
					cellY,
					preeditRow.width * this.metrics.width,
					this.metrics.height,
					defaultBackground,
				);
				let column = preeditRow.start;
				for (const cell of preeditRow.cells) {
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
					preeditRow.width * this.metrics.width,
					this.detailedMetrics.underlineThickness,
					defaultForeground,
				);
			}
		}
		if (cursorEnabled && renderedCursorStyle !== 'block') {
			const geometries =
				renderedCursorStyle === 'block_hollow'
					? cursorOutlineGeometry(
							cursorPresentation.column,
							cursor.row,
							this.detailedMetrics,
							cursorPresentation.width,
						)
					: [
							cursorStrokeGeometry(
								renderedCursorStyle,
								cursorPresentation.column,
								cursor.row,
								this.detailedMetrics,
								cursorPresentation.width,
							),
						];
			for (const geometry of geometries) {
				pushRect(overlayData, geometry.x, geometry.y, geometry.width, geometry.height, cursorColor);
			}
		}
		this.submitWebGPUFrame(bgData, textData, overlayData, columns, rows, defaultBackground);
		this.lastCursorPosition = { x: cursor.column, y: cursor.row };
		this.lastCursorEnabled = cursorEnabled;
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
			this.uniformUpload[0] = columns * this.metrics.width * this.devicePixelRatio;
			this.uniformUpload[1] = rows * this.metrics.height * this.devicePixelRatio;
			this.uniformUpload[2] = this.devicePixelRatio;
			this.uniformUpload[3] = 0;
			this.device.queue.writeBuffer(this.uniformBuffer, 0, this.uniformUpload);
			this.frameVertexUploadBytes += 4 * Float32Array.BYTES_PER_ELEMENT;
		}
		const commandEncoder = this.device.createCommandEncoder();
		const renderPass = commandEncoder.beginRenderPass({
			colorAttachments: [
				{
					view: this.context.getCurrentTexture().createView(),
					clearValue: {
						r: this.presentationConfiguration.allowTransparency ? 0 : clearColor[0] / 255,
						g: this.presentationConfiguration.allowTransparency ? 0 : clearColor[1] / 255,
						b: this.presentationConfiguration.allowTransparency ? 0 : clearColor[2] / 255,
						a: this.presentationConfiguration.allowTransparency ? 0 : 1,
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
		this.syncTextBlink(this.hasPackedBlinkingCells(buffer));
		if (
			buffer.needsFullRedraw ||
			this.preeditDirty ||
			this.fontDirty ||
			this.textBlinkDirty ||
			this.searchDirty
		) {
			forceAll = true;
		}
		const physicalWidth = Math.round(columns * this.metrics.width * this.devicePixelRatio);
		const physicalHeight = Math.round(rows * this.metrics.height * this.devicePixelRatio);
		const needsResize =
			this.canvas.width !== physicalWidth || this.canvas.height !== physicalHeight;
		if (needsResize) {
			this.resize(columns, rows);
			forceAll = true;
		}
		const preeditLayout = this.getPreeditLayout({ x: cursor.column, y: cursor.row }, columns, rows);
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
		if (cursorEnabled || this.lastCursorEnabled) {
			rowsToRender.add(cursor.row);
			rowsToRender.add(this.lastCursorPosition.y);
		}
		const presentation = resolvePackedPresentationColors(
			metadata.defaultColors,
			this.theme,
			this.cursorAccentOverride,
			this.presentationConfiguration.defaultColorOverride,
		);
		const defaultBackground = presentation.background;
		const defaultForeground = presentation.foreground;
		for (const y of rowsToRender) {
			const row = buffer.rows[y];
			if (!row || y < 0 || y >= rows) continue;
			const preeditRow = preeditLayoutRow(preeditLayout, y);
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
			if (!this.presentationConfiguration.allowTransparency) {
				ctx.fillStyle = `rgb(${defaultBackground[0]}, ${defaultBackground[1]}, ${defaultBackground[2]})`;
				ctx.fillRect(0, lineY, lineWidth, this.metrics.height);
			}

			for (let x = 0; x < columns; x++) {
				const width = packedCellWidth(row.widths[x] ?? TerminalCellWidthCode.spacerTail);
				if (width === 0) continue;
				this.frameCellsVisited++;
				const cellPresentation = this.packedCellPresentation(row, x, metadata, presentation);
				let background: readonly [number, number, number] = cellPresentation.background;
				const selected = this.isCoordinateInSelection(x, y);
				const search = this.getSearchHighlight(x, y);
				if (selected) {
					background = presentation.selectionBackground;
				} else if (search !== null) {
					background =
						search === 'active'
							? presentation.searchActiveBackground
							: presentation.searchBackground;
				}
				if (cellPresentation.requiresBackgroundFill || selected || search !== null) {
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
				if (preeditRow && x >= preeditRow.start && x < preeditRow.end) continue;
				const span = shaping.plan[x];
				let text = shaping.texts[x] ?? '';
				if (span) text = span.start === x ? span.text : '';
				const foregroundRgb = this.packedTextForegroundRgb(
					x,
					y,
					this.packedCellForegroundRgb(row, x, metadata, presentation),
					presentation,
				);
				writeRgbChannels(foregroundRgb, this.cellTextForegroundScratch);
				const foreground = this.cellTextForegroundScratch;
				if (span && span.start === x && text.length > 0) {
					let paintedSegments = 0;
					for (const segment of this.packedTextPaintSegments(
						span.start,
						span.end,
						row,
						metadata,
						y,
						presentation,
					)) {
						const clipStart =
							segment.startColumn === span.start ? 0 : segment.startColumn * this.metrics.width;
						const clipEnd =
							segment.endColumn === span.end ? lineWidth : segment.endColumn * this.metrics.width;
						ctx.save();
						ctx.beginPath();
						ctx.rect(clipStart, lineY, clipEnd - clipStart, this.metrics.height);
						ctx.clip();
						const painted = this.drawPackedCellText2D(
							ctx,
							row,
							shaping,
							x,
							y,
							span.columns,
							text,
							metadata,
							presentation,
							segment.foreground,
							false,
						);
						ctx.restore();
						if (painted) paintedSegments++;
					}
					this.frameGlyphInstances += paintedSegments;
					if (paintedSegments > 0) this.frameShapingRuns++;
					this.drawPackedCellText2D(
						ctx,
						row,
						shaping,
						x,
						y,
						width,
						'',
						metadata,
						presentation,
						foreground,
					);
					continue;
				}
				const paintedText = this.drawPackedCellText2D(
					ctx,
					row,
					shaping,
					x,
					y,
					width,
					text,
					metadata,
					presentation,
					foreground,
				);
				if (paintedText) {
					this.frameGlyphInstances++;
				}
			}
		}

		if (preeditLayout) {
			this.renderPackedPreedit2D(ctx, preeditLayout, defaultForeground, defaultBackground);
		} else if (cursorEnabled) {
			this.renderPackedCursor2D(ctx, buffer, metadata, renderedCursorStyle, presentation);
		}
		this.lastCursorPosition = { x: cursor.column, y: cursor.row };
		this.lastCursorEnabled = cursorEnabled;
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
		presentation: PackedPresentationColors,
		colorOverride?: readonly [number, number, number],
		drawDecorations = true,
	): boolean {
		const flags = row.styleFlags[column] ?? 0;
		if ((flags & TerminalStyleFlag.invisible) !== 0) return false;
		if ((flags & TerminalStyleFlag.blink) !== 0 && !this.textBlinkVisible) return false;
		const cellX = column * this.metrics.width;
		const cellY = rowIndex * this.metrics.height;
		const cellWidth = width * this.metrics.width;
		const fontStyle = (flags & TerminalStyleFlag.italic) !== 0 ? 'italic' : 'normal';
		const fontWeight =
			(flags & TerminalStyleFlag.bold) !== 0
				? this.presentationConfiguration.fontWeightBold
				: this.presentationConfiguration.fontWeight;
		ctx.font = canvasFont(
			fontStyle,
			fontWeight,
			this.presentationConfiguration.fontSize,
			this.presentationConfiguration.fontFamily,
		);
		let foreground = colorOverride;
		if (foreground === undefined) {
			writeRgbChannels(
				this.packedCellForegroundRgb(row, column, metadata, presentation),
				this.drawnTextForegroundScratch,
			);
			foreground = this.drawnTextForegroundScratch;
		}
		ctx.fillStyle = `rgb(${foreground[0]}, ${foreground[1]}, ${foreground[2]})`;
		if ((flags & TerminalStyleFlag.faint) !== 0) ctx.globalAlpha = 0.5;
		if (text.length > 0) {
			const span = shaping.plan[column];
			this.drawConstrainedText2D(
				ctx,
				text,
				cellX,
				cellY,
				span ? 0 : (shaping.codepoints[column] ?? 0),
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
					rgbValue(presentation.foreground),
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
			if (this.hoveredLinkRange && isCellInRange(column, rowIndex, this.hoveredLinkRange)) {
				ctx.strokeStyle = '#4A90E2';
				ctx.lineWidth = this.detailedMetrics.underlineThickness;
				ctx.beginPath();
				ctx.moveTo(cellX, cellY + this.detailedMetrics.underlinePosition);
				ctx.lineTo(cellX + cellWidth, cellY + this.detailedMetrics.underlinePosition);
				ctx.stroke();
			}
		}
		if ((flags & TerminalStyleFlag.faint) !== 0) ctx.globalAlpha = 1;
		return text.length > 0;
	}

	private renderPackedCursor2D(
		ctx: CanvasRenderingContext2D,
		buffer: PackedRendererSource,
		metadata: PackedRendererMetadata,
		style: RenderedCursorStyle,
		presentation: PackedPresentationColors,
	): void {
		const { row } = metadata.cursor;
		const packedRow = buffer.rows[row];
		const { column, width } = resolvePackedCursorPresentation(metadata.cursor, packedRow?.widths);
		const cursorColor = presentation.cursor;
		ctx.fillStyle = `rgb(${cursorColor[0]}, ${cursorColor[1]}, ${cursorColor[2]})`;
		if (style === 'block') {
			const geometry = cursorBlockGeometry(column, row, this.detailedMetrics, width);
			ctx.fillRect(geometry.x, geometry.y, geometry.width, geometry.height);
			if (packedRow) {
				const shaping = this.buildPackedRowTextShaping(
					packedRow,
					metadata.columns,
					row,
					metadata.cursor,
					true,
					null,
				);
				if (packedCellWidth(packedRow.widths[column] ?? TerminalCellWidthCode.spacerTail) > 0) {
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
						presentation,
						presentation.cursorAccent,
						false,
					);
					ctx.restore();
				}
			}
			return;
		}
		const geometries =
			style === 'block_hollow'
				? cursorOutlineGeometry(column, row, this.detailedMetrics, width)
				: [cursorStrokeGeometry(style, column, row, this.detailedMetrics, width)];
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
		ctx.font = canvasFont(
			'normal',
			this.presentationConfiguration.fontWeight,
			this.presentationConfiguration.fontSize,
			this.presentationConfiguration.fontFamily,
		);
		for (const preeditRow of layout.rows) {
			const cellY = preeditRow.row * this.metrics.height;
			const startX = preeditRow.start * this.metrics.width;
			const width = preeditRow.width * this.metrics.width;
			ctx.fillStyle = `rgb(${background[0]}, ${background[1]}, ${background[2]})`;
			ctx.fillRect(startX, cellY, width, this.metrics.height);
			ctx.fillStyle = `rgb(${foreground[0]}, ${foreground[1]}, ${foreground[2]})`;
			let column = preeditRow.start;
			for (const cell of preeditRow.cells) {
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
		this.configureTextRunAdvance(ctx, codepoint === 0, 1);
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

	// ==========================================================================
	// Shared Helpers
	// ==========================================================================

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
		const rows = this.currentVisibleRows;
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
		}, CURSOR_BLINK_INTERVAL_MS);
	}

	private restartCursorBlink(): void {
		if (!this.cursorBlink) return;
		this.stopCursorBlink();
		this.startCursorBlink();
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

	/** Prepare every fallible measurement without mutating the rendered presentation. */
	preparePresentationConfiguration(
		configuration: TerminalPresentationConfiguration,
	): PreparedTerminalPresentationConfiguration {
		if (this.frozen || this.disposed) throw new Error('Renderer is retired');
		const normalized = normalizeTerminalPresentationConfiguration(configuration);
		if (normalized.kind === 'invalid') {
			throw new TypeError(`Invalid terminal presentation configuration field: ${normalized.field}`);
		}
		const nextConfiguration = normalized.configuration;
		const nextDetailedMetrics = this.measureFont(nextConfiguration);
		const nextMetrics = publicFontMetrics(nextDetailedMetrics);
		return {
			configuration: nextConfiguration,
			detailedMetrics: nextDetailedMetrics,
			metrics: nextMetrics,
		};
	}

	/** Publish one already prepared complete configuration in one synchronous renderer commit. */
	commitPresentationConfiguration(prepared: PreparedTerminalPresentationConfiguration): void {
		if (this.frozen || this.disposed) return;
		this.presentationConfiguration = prepared.configuration;
		this.detailedMetrics = prepared.detailedMetrics;
		this.metrics = prepared.metrics;
		this.resetAtlas();
		if (this.renderedColumns > 0 && this.renderedRows > 0) {
			this.resize(this.renderedColumns, this.renderedRows);
		} else {
			this.syncSurfacePadding();
		}
		this.fontDirty = true;
		this.surfaceDirty = true;
		this.requestRenderCallback?.();
	}

	/** Validate one complete overlay set without changing the currently rendered colors. */
	prepareThemePresentation(
		presentation: TerminalThemeOverlayPresentation,
	): PreparedTerminalThemePresentation {
		if (this.frozen || this.disposed) throw new Error('Renderer is retired');
		const cursorAccent = rgbHex(presentation.cursorAccentRgb);
		const theme: Required<ITheme> = {
			...this.theme,
			cursorAccent,
			searchActiveMatchBackground: rgbHex(presentation.searchActiveMatchBackgroundRgb),
			searchActiveMatchForeground: rgbHex(presentation.searchActiveMatchForegroundRgb),
			searchMatchBackground: rgbHex(presentation.searchMatchBackgroundRgb),
			searchMatchForeground: rgbHex(presentation.searchMatchForegroundRgb),
			selectionBackground: rgbHex(presentation.selectionBackgroundRgb),
			selectionForeground: rgbHex(presentation.selectionForegroundRgb),
		};
		return Object.freeze({ cursorAccent, theme: Object.freeze(theme) });
	}

	/** Publish one already validated overlay set in the same synchronous renderer commit. */
	commitThemePresentation(prepared: PreparedTerminalThemePresentation): void {
		if (this.frozen || this.disposed) return;
		this.theme = { ...prepared.theme };
		this.cursorAccentOverride = prepared.cursorAccent;
		this.cursorAppearanceDirty = true;
		this.searchDirty = true;
		this.surfaceDirty = true;
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

	getMetrics(): FontMetrics {
		return { ...this.metrics };
	}

	/** Measure regular preedit text using the same CSS font as the hidden input. */
	measureTextWidth(text: string): number {
		this.atlasCtx.font = canvasFont(
			'normal',
			this.presentationConfiguration.fontWeight,
			this.presentationConfiguration.fontSize,
			this.presentationConfiguration.fontFamily,
		);
		this.configureTextRunAdvance(this.atlasCtx, false, 1);
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

	setProjectedViewportState(absoluteTop: number, selection: SelectionCoordinates | null): void {
		const nextTop = Math.max(0, Math.floor(absoluteTop));
		const nextSelection = normalizeSelectionCoordinates(selection);
		if (
			this.currentAbsoluteTop === nextTop &&
			JSON.stringify(this.projectedSelectionCoords) === JSON.stringify(nextSelection)
		) {
			return;
		}
		this.currentAbsoluteTop = nextTop;
		this.projectedSelectionCoords = nextSelection;
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

	setHoveredLinkRange(
		range: { startX: number; startY: number; endX: number; endY: number } | null,
	): void {
		if (JSON.stringify(this.hoveredLinkRange) === JSON.stringify(range)) return;
		this.hoveredLinkRange = range;
		this.searchDirty = true;
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
			const defaultBackgroundRgb = this.effectiveDefaultBackgroundRgb();
			if (!this.presentationConfiguration.allowTransparency && defaultBackgroundRgb !== null) {
				const [red, green, blue] = rgbChannels(defaultBackgroundRgb);
				this.ctx2d.fillStyle = `rgb(${red}, ${green}, ${blue})`;
				this.ctx2d.fillRect(0, 0, this.canvas.width, this.canvas.height);
			}
		}
		if (this.graphicsCanvas && this.graphicsContext) {
			this.graphicsContext.clearRect(0, 0, this.graphicsCanvas.width, this.graphicsCanvas.height);
		}
	}

	private emitHealth(event: RendererHealthEvent): void {
		this.healthChangeCallback?.(event);
	}

	/** Stop live rendering behavior without clearing or releasing the presented canvas. */
	freeze(): void {
		if (this.disposed || this.frozen) return;
		this.frozen = true;
		this.suspended = true;
		this.stopCursorBlink();
		this.stopTextBlink();
		this.requestRenderCallback = undefined;
		this.healthChangeCallback = undefined;
		this.telemetryCallback = undefined;
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
		this.releaseGraphicsOverlay();
		this.backgroundVertexData = [];
		this.textVertexData = [];
		this.overlayVertexData = [];
		this.backgroundVertexUpload = new Float32Array(0);
		this.textVertexUpload = new Float32Array(0);
		this.overlayVertexUpload = new Float32Array(0);
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
	width = 1,
): RectangleGeometry {
	const thickness = metrics.cursorThickness;
	const box = cursorBlockGeometry(column, row, metrics, width);
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
	width = 1,
): RectangleGeometry {
	return {
		x: column * metrics.cellWidth,
		y: row * metrics.cellHeight + (metrics.cellHeight - metrics.cursorHeight) / 2,
		width: metrics.cellWidth * width,
		height: metrics.cursorHeight,
	};
}

function cursorOutlineGeometry(
	column: number,
	row: number,
	metrics: DetailedFontMetrics,
	width = 1,
): readonly RectangleGeometry[] {
	const thickness = metrics.cursorThickness;
	const box = cursorBlockGeometry(column, row, metrics, width);
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

function preeditLayoutRow(layout: PreeditLayout | null, row: number): PreeditLayoutRow | undefined {
	return layout?.rows.find((candidate) => candidate.row === row);
}
