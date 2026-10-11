import type { TerminalOpenTarget } from './terminal-contracts';

/**
 * Product presentation colors for a projected terminal surface.
 *
 * Host projection metadata owns terminal foreground, background, cursor, and palette colors. The
 * packed renderer consumes the cursor-accent and selection/search fields as application overlays;
 * the remaining terminal-color fields stay in this compatibility shape but never replace Host
 * state. `TerminalPresentationConfiguration.defaultColorOverride` is a separate client-only
 * projection for default-coded cells and never writes Host metadata.
 */
export interface ITheme {
	foreground?: string;
	background?: string;
	cursor?: string;
	cursorAccent?: string;
	selectionBackground?: string;
	selectionForeground?: string;
	searchMatchBackground?: string;
	searchMatchForeground?: string;
	searchActiveMatchBackground?: string;
	searchActiveMatchForeground?: string;
	black?: string;
	red?: string;
	green?: string;
	yellow?: string;
	blue?: string;
	magenta?: string;
	cyan?: string;
	white?: string;
	brightBlack?: string;
	brightRed?: string;
	brightGreen?: string;
	brightYellow?: string;
	brightBlue?: string;
	brightMagenta?: string;
	brightCyan?: string;
	brightWhite?: string;
}

/** Public font metrics in CSS pixels. */
export interface FontMetrics {
	readonly width: number;
	readonly height: number;
	readonly baseline: number;
}

/** A pixel delta or Ghostty-compatible percentage delta such as `"20%"`. */
export type FontMetricModifier = number | string;

/** Renderer-owned equivalents of Ghostty's `adjust-*` font metric options. */
export interface FontMetricModifiers {
	readonly cellWidth?: FontMetricModifier;
	readonly cellHeight?: FontMetricModifier;
	readonly fontBaseline?: FontMetricModifier;
	readonly underlinePosition?: FontMetricModifier;
	readonly underlineThickness?: FontMetricModifier;
	readonly strikethroughPosition?: FontMetricModifier;
	readonly strikethroughThickness?: FontMetricModifier;
	readonly overlinePosition?: FontMetricModifier;
	readonly overlineThickness?: FontMetricModifier;
	readonly cursorThickness?: FontMetricModifier;
	readonly cursorHeight?: FontMetricModifier;
	readonly boxThickness?: FontMetricModifier;
	readonly iconHeight?: FontMetricModifier;
}

/** Metric used to normalize a fallback face to a primary face. */
export type FontSizeAdjustment = 'none' | 'ic-width' | 'ex-height' | 'cap-height' | 'line-height';

/** CSS font weight used for regular or bold terminal cells. */
export type FontWeight = number | 'normal' | 'bold';

/** Ghostty-compatible padding modes. */
export type PaddingBalance = false | true | 'equal';

/** Client-only display replacement for default-coded terminal foreground and background cells. */
export interface TerminalDefaultColorOverride {
	readonly background: `#${string}`;
	readonly foreground: `#${string}`;
}

export interface SurfacePadding {
	readonly top: number;
	readonly bottom: number;
	readonly left: number;
	readonly right: number;
}

/** Renderer configuration accepted by one projected terminal surface. */
export interface RendererOptions {
	readonly allowTransparency?: boolean;
	readonly backgroundOpacity?: number;
	readonly cursorBlink?: boolean;
	readonly defaultColorOverride?: TerminalDefaultColorOverride | null;
	readonly devicePixelRatio?: number;
	readonly fontFamily?: string;
	readonly fontLigatures?: boolean;
	readonly fontMetricModifiers?: FontMetricModifiers;
	readonly fontSize?: number;
	readonly fontWeight?: FontWeight;
	readonly fontWeightBold?: FontWeight;
	readonly paddingBalance?: PaddingBalance;
	/** Uniform client-side inset in CSS pixels around the projected grid. */
	readonly terminalPadding?: number;
	readonly theme?: Partial<ITheme>;
}

/** Selection coordinates in the currently projected viewport. */
export interface SelectionCoordinates {
	readonly startCol: number;
	readonly startRow: number;
	readonly endCol: number;
	readonly endRow: number;
	readonly rectangle: boolean;
}

interface IBufferCellPosition {
	readonly x: number;
	readonly y: number;
}

export interface ILinkRange {
	readonly start: IBufferCellPosition;
	readonly end: IBufferCellPosition;
}

/** A bounded link projection discovered from Host-provided terminal rows. */
export interface ILink {
	readonly text: string;
	readonly range: ILinkRange;
	readonly target?: TerminalOpenTarget;
	readonly activation?: 'direct' | 'modifier';
}
