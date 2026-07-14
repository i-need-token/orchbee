/**
 * Type definitions for the Ghostty WASM VT parser API.
 *
 * Based on the upstream Ghostty C API (libghostty-vt), not the
 * ghostty-web patched API. Uses the generic get/set pattern with
 * field identifiers and iterator-based cell access.
 *
 * NOTE: Uses `const` objects instead of `enum` because the project
 * uses `erasableSyntaxOnly: true` (TypeScript 5.9+) which forbids
 * enums that emit runtime code.
 */

// ============================================================================
// Primitives
// ============================================================================

import type { TerminalOpenTarget } from './terminal-contracts';

/** RGB color (0-255 per channel, matches GhosttyColorRgb C struct). */
export interface RGB {
	readonly r: number;
	readonly g: number;
	readonly b: number;
}

/** Opaque terminal pointer (WASM memory address). */
export type TerminalHandle = number;

/** Opaque render state pointer. */
export type RenderStateHandle = number;

/** Opaque row iterator pointer. */
export type RowIteratorHandle = number;

/** Opaque row cells pointer. */
export type RowCellsHandle = number;

// ============================================================================
// Result Codes
// ============================================================================

/** GhosttyResult return codes from C API. */
export const Result = {
	SUCCESS: 0,
	OUT_OF_MEMORY: -1,
	INVALID_VALUE: -2,
	OUT_OF_SPACE: -3,
	NO_VALUE: -4,
} as const;
export type Result = (typeof Result)[keyof typeof Result];

export const BuildInfo = {
	SIMD: 1,
	KITTY_GRAPHICS: 2,
	TMUX_CONTROL_MODE: 3,
	OPTIMIZE: 4,
	VERSION_STRING: 5,
} as const;
export type BuildInfo = (typeof BuildInfo)[keyof typeof BuildInfo];

// ============================================================================
// Dirty State
// ============================================================================

/** Render dirty state from ghostty_render_state_get(DATA_DIRTY). */
export const DirtyState = {
	NONE: 0,
	PARTIAL: 1,
	FULL: 2,
} as const;
export type DirtyState = (typeof DirtyState)[keyof typeof DirtyState];

// ============================================================================
// Cell Data
// ============================================================================

/**
 * Parsed cell data from the viewport.
 *
 * Unlike the patched API which used a flat 16-byte struct,
 * the upstream API reads cell fields individually via the
 * row_cells iterator. This interface collects those fields.
 */
export interface CellData {
	/** Unicode codepoint (first of grapheme cluster, 0 = empty). */
	readonly codepoint: number;
	/** UTF-8 string for the full grapheme cluster. */
	readonly text: string;
	/** Foreground color (null = use default). */
	readonly fg: RGB | null;
	/** Background color (null = use default). */
	readonly bg: RGB | null;
	/** Whether the cell is within the current selection. */
	readonly selected: boolean;
	/** Whether the cell has any explicit styling. */
	readonly hasStyling: boolean;
	/** Resolved style flags for rendering and buffer inspection. */
	readonly flags: number;
	/** Display width: 0 for a spacer, 1 for narrow, 2 for wide. */
	readonly width: number;
	/** Non-zero when the cell contains an OSC 8 hyperlink. */
	readonly hyperlinkId: number;
	/** Number of codepoints in the grapheme cluster (0 = empty cell). */
	readonly graphemeLen: number;
	/** OSC 133 semantic content class reported by Ghostty. */
	readonly semanticContent: CellSemanticContent;
	/** SGR underline style, including double/curly/dotted/dashed variants. */
	readonly underlineStyle: UnderlineStyle;
	/** Explicit SGR underline color, or null to use the foreground. */
	readonly underlineColor: RGB | null;
	/** Whether SGR overline is active. */
	readonly overline: boolean;
}

/** Field identifiers for ghostty_cell_get(). */
export const CellDataField = {
	CODEPOINT: 1,
	CONTENT_TAG: 2,
	WIDE: 3,
	HAS_TEXT: 4,
	HAS_STYLING: 5,
	STYLE_ID: 6,
	HAS_HYPERLINK: 7,
	PROTECTED: 8,
	SEMANTIC_CONTENT: 9,
	COLOR_PALETTE: 10,
	COLOR_RGB: 11,
} as const;
export type CellDataField = (typeof CellDataField)[keyof typeof CellDataField];

/** Values returned by GHOSTTY_CELL_DATA_SEMANTIC_CONTENT. */
export const CellSemanticContent = {
	OUTPUT: 0,
	INPUT: 1,
	PROMPT: 2,
} as const;
export type CellSemanticContent = (typeof CellSemanticContent)[keyof typeof CellSemanticContent];

export const UnderlineStyle = {
	NONE: 0,
	SINGLE: 1,
	DOUBLE: 2,
	CURLY: 3,
	DOTTED: 4,
	DASHED: 5,
} as const;
export type UnderlineStyle = (typeof UnderlineStyle)[keyof typeof UnderlineStyle];

/** Values returned by GHOSTTY_CELL_DATA_WIDE. */
export const CellWide = {
	NARROW: 0,
	WIDE: 1,
	SPACER_TAIL: 2,
	SPACER_HEAD: 3,
} as const;
export type CellWide = (typeof CellWide)[keyof typeof CellWide];

// ============================================================================
// Terminal Data Field IDs (GhosttyTerminalData enum)
// ============================================================================

/** Field identifiers for ghostty_terminal_get(). */
export const TerminalData = {
	COLS: 1,
	ROWS: 2,
	CURSOR_X: 3,
	CURSOR_Y: 4,
	CURSOR_PENDING_WRAP: 5,
	ACTIVE_SCREEN: 6,
	CURSOR_VISIBLE: 7,
	KITTY_KEYBOARD_FLAGS: 8,
	SCROLLBAR: 9,
	CURSOR_STYLE: 10,
	MOUSE_TRACKING: 11,
	TITLE: 12,
	PWD: 13,
	TOTAL_ROWS: 14,
	SCROLLBACK_ROWS: 15,
	WIDTH_PX: 16,
	HEIGHT_PX: 17,
	COLOR_FOREGROUND: 18,
	COLOR_BACKGROUND: 19,
	COLOR_CURSOR: 20,
	COLOR_PALETTE: 21,
	COLOR_FOREGROUND_DEFAULT: 22,
	COLOR_BACKGROUND_DEFAULT: 23,
	COLOR_CURSOR_DEFAULT: 24,
	COLOR_PALETTE_DEFAULT: 25,
	SELECTION: 31,
	VIEWPORT_ACTIVE: 32,
} as const;
export type TerminalData = (typeof TerminalData)[keyof typeof TerminalData];

// ============================================================================
// Terminal Option IDs (GhosttyTerminalOption enum)
// ============================================================================

/** Option identifiers for ghostty_terminal_set(). */
export const TerminalOption = {
	USERDATA: 0,
	WRITE_PTY: 1,
	BELL: 2,
	ENQUIRY: 3,
	XTVERSION: 4,
	TITLE_CHANGED: 5,
	SIZE: 6,
	COLOR_SCHEME: 7,
	DEVICE_ATTRIBUTES: 8,
	TITLE: 9,
	PWD: 10,
	COLOR_FOREGROUND: 11,
	COLOR_BACKGROUND: 12,
	COLOR_CURSOR: 13,
	COLOR_PALETTE: 14,
	SELECTION: 21,
	DEFAULT_CURSOR_STYLE: 22,
	DEFAULT_CURSOR_BLINK: 23,
	PWD_CHANGED: 25,
} as const;
export type TerminalOption = (typeof TerminalOption)[keyof typeof TerminalOption];

// ============================================================================
// Render State Data Field IDs (GhosttyRenderStateData enum)
// ============================================================================

/** Field identifiers for ghostty_render_state_get(). */
export const RenderStateData = {
	COLS: 1,
	ROWS: 2,
	DIRTY: 3,
	ROW_ITERATOR: 4,
	COLOR_BACKGROUND: 5,
	COLOR_FOREGROUND: 6,
	COLOR_CURSOR: 7,
	COLOR_CURSOR_HAS_VALUE: 8,
	COLOR_PALETTE: 9,
	CURSOR_VISUAL_STYLE: 10,
	CURSOR_VISIBLE: 11,
	CURSOR_BLINKING: 12,
	CURSOR_PASSWORD_INPUT: 13,
	CURSOR_VIEWPORT_HAS_VALUE: 14,
	CURSOR_VIEWPORT_X: 15,
	CURSOR_VIEWPORT_Y: 16,
	CURSOR_VIEWPORT_WIDE_TAIL: 17,
} as const;
export type RenderStateData = (typeof RenderStateData)[keyof typeof RenderStateData];

// ============================================================================
// Render State Option IDs (GhosttyRenderStateOption enum)
// ============================================================================

/** Option identifiers for ghostty_render_state_set(). */
export const RenderStateOption = {
	DIRTY: 0,
} as const;
export type RenderStateOption = (typeof RenderStateOption)[keyof typeof RenderStateOption];

// ============================================================================
// Row Data Field IDs (GhosttyRenderStateRowData enum)
// ============================================================================

/** Field identifiers for ghostty_render_state_row_get(). */
export const RowData = {
	DIRTY: 1,
	RAW: 2,
	CELLS: 3,
	SELECTION: 4,
} as const;
export type RowData = (typeof RowData)[keyof typeof RowData];

/** Field identifiers for ghostty_row_get(). */
export const RawRowData = {
	WRAP: 1,
	WRAP_CONTINUATION: 2,
} as const;
export type RawRowData = (typeof RawRowData)[keyof typeof RawRowData];

// ============================================================================
// Row Cells Data Field IDs (GhosttyRenderStateRowCellsData enum)
// ============================================================================

/** Field identifiers for ghostty_render_state_row_cells_get(). */
export const RowCellsData = {
	RAW: 1,
	STYLE: 2,
	GRAPHEMES_LEN: 3,
	GRAPHEMES_BUF: 4,
	BG_COLOR: 5,
	FG_COLOR: 6,
	SELECTED: 7,
	HAS_STYLING: 8,
	GRAPHEMES_UTF8: 9,
} as const;
export type RowCellsData = (typeof RowCellsData)[keyof typeof RowCellsData];

// ============================================================================
// Cursor & Colors
// ============================================================================

/** Cursor position and visibility. */
export interface CursorState {
	readonly x: number;
	readonly y: number;
	readonly visible: boolean;
}

/** Default terminal colors from render state. */
export interface TerminalColors {
	readonly foreground: RGB;
	readonly background: RGB;
	readonly cursor: RGB | null;
}

// ============================================================================
// Terminal Modes
// ============================================================================

/** DEC mode numbers (use with getMode). */
export const DecMode = {
	CURSOR_VISIBLE: 25,
	MOUSE_TRACKING_X10: 9,
	MOUSE_TRACKING_NORMAL: 1000,
	MOUSE_TRACKING_BUTTON: 1002,
	MOUSE_TRACKING_ANY: 1003,
	FOCUS_EVENTS: 1004,
	ALT_SCREEN: 1047,
	ALT_SCREEN_WITH_CURSOR: 1049,
	BRACKETED_PASTE: 2004,
} as const;
export type DecMode = (typeof DecMode)[keyof typeof DecMode];

/** ANSI mode numbers (use with getMode). */
export const AnsiMode = {
	INSERT: 4,
} as const;
export type AnsiMode = (typeof AnsiMode)[keyof typeof AnsiMode];

/**
 * Build a mode tag for ghostty_terminal_mode_get/set.
 * Lower 15 bits = mode number, bit 15 = isAnsi flag.
 */
export function buildModeTag(mode: number, isAnsi: boolean): number {
	return isAnsi ? mode | (1 << 15) : mode;
}

// ============================================================================
// Terminal Configuration
// ============================================================================

/**
 * Terminal configuration (passed to Terminal constructor).
 *
 * Maps to GhosttyTerminalOptions C struct (8 bytes):
 *   cols(u16) + rows(u16) + max_scrollback(u32)
 */
export interface TerminalConfig {
	/** Maximum scrollback history (default: 10000, 0 disables scrollback). */
	readonly scrollbackLimit?: number;
	/** Default foreground color (0xRRGGBB, 0 = default). */
	readonly fgColor?: number;
	/** Default background color (0xRRGGBB, 0 = default). */
	readonly bgColor?: number;
	/** Cursor color (0xRRGGBB, 0 = default). */
	readonly cursorColor?: number;
}

/** Size of GhosttyTerminalOptions struct in WASM memory (bytes). */
export const TERMINAL_OPTIONS_SIZE = 8;

/** Size of GhosttyRenderStateColors struct in WASM memory (bytes). */
export const RENDER_STATE_COLORS_SIZE = 784;

/** Size of GhosttyColorRgb struct in WASM memory (3 bytes). */
export const COLOR_RGB_SIZE = 3;

/** Size of the opaque GhosttyCell value returned by render state. */
export const CELL_VALUE_SIZE = 8;

/** Size of GhosttyStyle for the current wasm32 Ghostty ABI. */
export const STYLE_SIZE = 72;

/** Size of GhosttyBuffer struct in WASM memory (12 bytes). */
export const BUFFER_SIZE = 12;

/** Size of GhosttyPoint struct in WASM memory (24 bytes). */
export const POINT_SIZE = 24;

/** Size of GhosttyGridRef struct in WASM memory (12 bytes). */
export const GRID_REF_SIZE = 12;

// ============================================================================
// Key Encoding
// ============================================================================

/** Key action for key event encoding. */
export const KeyAction = {
	RELEASE: 0,
	PRESS: 1,
	REPEAT: 2,
} as const;
export type KeyAction = (typeof KeyAction)[keyof typeof KeyAction];

/** Key encoder option indices. */
export const KeyEncoderOption = {
	CURSOR_KEY_APPLICATION: 0,
	KEYPAD_KEY_APPLICATION: 1,
	IGNORE_KEYPAD_WITH_NUMLOCK: 2,
	ALT_ESC_PREFIX: 3,
	MODIFY_OTHER_KEYS_STATE_2: 4,
	KITTY_KEYBOARD_FLAGS: 5,
	MACOS_OPTION_AS_ALT: 6,
	BACKARROW_KEY_MODE: 7,
} as const;
export type KeyEncoderOption = (typeof KeyEncoderOption)[keyof typeof KeyEncoderOption];

/** Modifier key flags (OR'd together). */
export const Mods = {
	NONE: 0,
	SHIFT: 1 << 0,
	CTRL: 1 << 1,
	ALT: 1 << 2,
	SUPER: 1 << 3,
	CAPSLOCK: 1 << 4,
	NUMLOCK: 1 << 5,
	SHIFT_SIDE: 1 << 6,
	CTRL_SIDE: 1 << 7,
	ALT_SIDE: 1 << 8,
	SUPER_SIDE: 1 << 9,
} as const;
export type Mods = (typeof Mods)[keyof typeof Mods];

/** Physical key codes matching Ghostty's internal Key enum. */
export const Key = {
	UNIDENTIFIED: 0,
	GRAVE: 1,
	BACKSLASH: 2,
	BRACKET_LEFT: 3,
	BRACKET_RIGHT: 4,
	COMMA: 5,
	ZERO: 6,
	ONE: 7,
	TWO: 8,
	THREE: 9,
	FOUR: 10,
	FIVE: 11,
	SIX: 12,
	SEVEN: 13,
	EIGHT: 14,
	NINE: 15,
	EQUAL: 16,
	INTL_BACKSLASH: 17,
	INTL_RO: 18,
	INTL_YEN: 19,
	A: 20,
	B: 21,
	C: 22,
	D: 23,
	E: 24,
	F: 25,
	G: 26,
	H: 27,
	I: 28,
	J: 29,
	K: 30,
	L: 31,
	M: 32,
	N: 33,
	O: 34,
	P: 35,
	Q: 36,
	R: 37,
	S: 38,
	T: 39,
	U: 40,
	V: 41,
	W: 42,
	X: 43,
	Y: 44,
	Z: 45,
	MINUS: 46,
	PERIOD: 47,
	QUOTE: 48,
	SEMICOLON: 49,
	SLASH: 50,
	ALT_LEFT: 51,
	ALT_RIGHT: 52,
	BACKSPACE: 53,
	CAPS_LOCK: 54,
	CONTEXT_MENU: 55,
	CONTROL_LEFT: 56,
	CONTROL_RIGHT: 57,
	ENTER: 58,
	META_LEFT: 59,
	META_RIGHT: 60,
	SHIFT_LEFT: 61,
	SHIFT_RIGHT: 62,
	SPACE: 63,
	TAB: 64,
	CONVERT: 65,
	KANA_MODE: 66,
	NON_CONVERT: 67,
	DELETE: 68,
	END: 69,
	HELP: 70,
	HOME: 71,
	INSERT: 72,
	PAGE_DOWN: 73,
	PAGE_UP: 74,
	DOWN: 75,
	LEFT: 76,
	RIGHT: 77,
	UP: 78,
	NUM_LOCK: 79,
	NUMPAD_0: 80,
	NUMPAD_1: 81,
	NUMPAD_2: 82,
	NUMPAD_3: 83,
	NUMPAD_4: 84,
	NUMPAD_5: 85,
	NUMPAD_6: 86,
	NUMPAD_7: 87,
	NUMPAD_8: 88,
	NUMPAD_9: 89,
	NUMPAD_ADD: 90,
	NUMPAD_BACKSPACE: 91,
	NUMPAD_CLEAR: 92,
	NUMPAD_CLEAR_ENTRY: 93,
	NUMPAD_COMMA: 94,
	NUMPAD_DECIMAL: 95,
	NUMPAD_DIVIDE: 96,
	NUMPAD_ENTER: 97,
	NUMPAD_EQUAL: 98,
	NUMPAD_MEMORY_ADD: 99,
	NUMPAD_MEMORY_CLEAR: 100,
	NUMPAD_MEMORY_RECALL: 101,
	NUMPAD_MEMORY_STORE: 102,
	NUMPAD_MEMORY_SUBTRACT: 103,
	NUMPAD_MULTIPLY: 104,
	NUMPAD_PAREN_LEFT: 105,
	NUMPAD_PAREN_RIGHT: 106,
	NUMPAD_SUBTRACT: 107,
	NUMPAD_SEPARATOR: 108,
	NUMPAD_UP: 109,
	NUMPAD_DOWN: 110,
	NUMPAD_RIGHT: 111,
	NUMPAD_LEFT: 112,
	NUMPAD_BEGIN: 113,
	NUMPAD_HOME: 114,
	NUMPAD_END: 115,
	NUMPAD_INSERT: 116,
	NUMPAD_DELETE: 117,
	NUMPAD_PAGE_UP: 118,
	NUMPAD_PAGE_DOWN: 119,
	ESCAPE: 120,
	F1: 121,
	F2: 122,
	F3: 123,
	F4: 124,
	F5: 125,
	F6: 126,
	F7: 127,
	F8: 128,
	F9: 129,
	F10: 130,
	F11: 131,
	F12: 132,
	F13: 133,
	F14: 134,
	F15: 135,
	F16: 136,
	F17: 137,
	F18: 138,
	F19: 139,
	F20: 140,
	F21: 141,
	F22: 142,
	F23: 143,
	F24: 144,
	F25: 145,
	FN: 146,
	FN_LOCK: 147,
	PRINT_SCREEN: 148,
	SCROLL_LOCK: 149,
	PAUSE: 150,
	BROWSER_BACK: 151,
	BROWSER_FAVORITES: 152,
	BROWSER_FORWARD: 153,
	BROWSER_HOME: 154,
	BROWSER_REFRESH: 155,
	BROWSER_SEARCH: 156,
	BROWSER_STOP: 157,
	EJECT: 158,
	LAUNCH_APP_1: 159,
	LAUNCH_APP_2: 160,
	LAUNCH_MAIL: 161,
	MEDIA_PLAY_PAUSE: 162,
	MEDIA_SELECT: 163,
	MEDIA_STOP: 164,
	MEDIA_TRACK_NEXT: 165,
	MEDIA_TRACK_PREVIOUS: 166,
	POWER: 167,
	SLEEP: 168,
	AUDIO_VOLUME_DOWN: 169,
	AUDIO_VOLUME_MUTE: 170,
	AUDIO_VOLUME_UP: 171,
	WAKE_UP: 172,
	COPY: 173,
	CUT: 174,
	PASTE: 175,
} as const;
export type Key = (typeof Key)[keyof typeof Key];

/** Key event for encoding keyboard input to terminal sequences. */
export interface KeyEvent {
	readonly action: KeyAction;
	readonly key: Key;
	readonly mods: Mods;
	readonly consumedMods?: Mods;
	readonly composing?: boolean;
	readonly utf8?: string;
	readonly unshiftedCodepoint?: number;
}

// ============================================================================
// Cell Style Flags
// ============================================================================

/** Cell style flags (bitfield, matches GhosttyCell.flags). */
export const CellFlags = {
	BOLD: 1 << 0,
	ITALIC: 1 << 1,
	UNDERLINE: 1 << 2,
	STRIKETHROUGH: 1 << 3,
	INVERSE: 1 << 4,
	INVISIBLE: 1 << 5,
	BLINK: 1 << 6,
	FAINT: 1 << 7,
} as const;
export type CellFlags = (typeof CellFlags)[keyof typeof CellFlags];

// ============================================================================
// GhosttyCell (flat 16-byte struct for batch viewport reads)
// ============================================================================

/**
 * Cell structure matching ghostty_cell_t (16 bytes).
 * Used by the GhosttyTerminal compatibility layer for batch reads.
 */
export interface GhosttyCell {
	/** Unicode codepoint (first of grapheme cluster, 0 = empty). */
	codepoint: number;
	/** Foreground red. */
	fg_r: number;
	/** Foreground green. */
	fg_g: number;
	/** Foreground blue. */
	fg_b: number;
	/** Background red. */
	bg_r: number;
	/** Background green. */
	bg_g: number;
	/** Background blue. */
	bg_b: number;
	/** Style flags (CellFlags bitfield). */
	flags: number;
	/** Character width (1 = normal, 2 = wide). */
	width: number;
	/** Hyperlink ID (0 = no link). */
	hyperlink_id: number;
	/** Extra codepoints beyond first in grapheme cluster. */
	grapheme_len: number;
	/** Whether Ghostty reports this cell as selected. */
	selected: boolean;
	underline_style: UnderlineStyle;
	underline_r: number;
	underline_g: number;
	underline_b: number;
	overline: boolean;
}

// ============================================================================
// Point Types (for grid_ref)
// ============================================================================

/** Point coordinate tag (which coordinate system). */
export const PointTag = {
	ACTIVE: 0,
	VIEWPORT: 1,
	SCREEN: 2,
	HISTORY: 3,
} as const;
export type PointTag = (typeof PointTag)[keyof typeof PointTag];

// ============================================================================
// WASM Exports Interface
// ============================================================================

/**
 * Interface for libghostty-vt WASM exports (upstream API).
 *
 * This matches the 185 exports from the upstream Ghostty build,
 * including terminal lifecycle, render state, cell iteration,
 * grid references, key encoding, and memory helpers.
 */
export interface WasmExports extends WebAssembly.Exports {
	readonly ghostty_build_info: (data: number, outputPtr: number) => number;
	readonly memory: WebAssembly.Memory;
	readonly ['__indirect_function_table']: WebAssembly.Table;

	// Memory helpers
	readonly ghostty_wasm_alloc_opaque: () => number;
	readonly ghostty_wasm_free_opaque: (ptr: number) => void;
	readonly ghostty_wasm_alloc_u8_array: (len: number) => number;
	readonly ghostty_wasm_free_u8_array: (ptr: number, len: number) => void;
	readonly ghostty_wasm_alloc_u16_array: (len: number) => number;
	readonly ghostty_wasm_free_u16_array: (ptr: number, len: number) => void;
	readonly ghostty_wasm_alloc_u8: () => number;
	readonly ghostty_wasm_free_u8: (ptr: number) => void;
	readonly ghostty_wasm_alloc_usize: () => number;
	readonly ghostty_wasm_free_usize: (ptr: number) => void;
	readonly ghostty_wasm_alloc_sgr_attribute: () => number;
	readonly ghostty_wasm_free_sgr_attribute: (ptr: number) => void;

	// General allocation
	readonly ghostty_alloc: (allocator: number, size: number) => number;
	readonly ghostty_free: (allocator: number, ptr: number, size: number) => void;

	// Unicode width helpers
	readonly ghostty_unicode_codepoint_width: (codepoint: number) => number;

	// Terminal lifecycle (upstream API)
	readonly ghostty_terminal_new: (
		allocator: number,
		terminalPtrPtr: number,
		optionsPtr: number,
	) => number;
	readonly ghostty_terminal_free: (terminal: TerminalHandle) => void;
	readonly ghostty_terminal_reset: (terminal: TerminalHandle) => void;
	readonly ghostty_terminal_vt_write: (
		terminal: TerminalHandle,
		dataPtr: number,
		dataLen: number,
	) => void;
	readonly ghostty_terminal_resize: (
		terminal: TerminalHandle,
		cols: number,
		rows: number,
		cellWidthPx: number,
		cellHeightPx: number,
	) => number;
	readonly ghostty_terminal_set: (
		terminal: TerminalHandle,
		option: number,
		valuePtr: number,
	) => number;
	readonly ghostty_terminal_get: (terminal: TerminalHandle, data: number, outPtr: number) => number;
	readonly ghostty_terminal_get_multi: (
		terminal: TerminalHandle,
		count: number,
		keysPtr: number,
		valuesPtr: number,
		outWrittenPtr: number,
	) => number;
	readonly ghostty_terminal_mode_get: (
		terminal: TerminalHandle,
		mode: number,
		outPtr: number,
	) => number;
	readonly ghostty_terminal_mode_set: (
		terminal: TerminalHandle,
		mode: number,
		value: number,
	) => number;
	readonly ghostty_terminal_scroll_viewport: (
		terminal: TerminalHandle,
		behaviorPtr: number,
	) => void;

	// Terminal grid references
	readonly ghostty_terminal_grid_ref: (
		terminal: TerminalHandle,
		pointPtr: number,
		outRefPtr: number,
	) => number;
	readonly ghostty_terminal_grid_ref_track: (
		terminal: TerminalHandle,
		pointPtr: number,
		outRefPtr: number,
	) => number;
	readonly ghostty_terminal_point_from_grid_ref: (
		terminal: TerminalHandle,
		gridRefPtr: number,
		tag: number,
		outPointPtr: number,
	) => number;

	// Selection snapshots and terminal-owned tracked selection
	readonly ghostty_terminal_select_word: (
		terminal: TerminalHandle,
		optionsPtr: number,
		outSelectionPtr: number,
	) => number;
	readonly ghostty_terminal_select_word_between: (
		terminal: TerminalHandle,
		optionsPtr: number,
		outSelectionPtr: number,
	) => number;
	readonly ghostty_terminal_select_line: (
		terminal: TerminalHandle,
		optionsPtr: number,
		outSelectionPtr: number,
	) => number;
	readonly ghostty_terminal_select_all: (
		terminal: TerminalHandle,
		outSelectionPtr: number,
	) => number;
	readonly ghostty_terminal_select_output: (
		terminal: TerminalHandle,
		gridRefPtr: number,
		outSelectionPtr: number,
	) => number;
	readonly ghostty_terminal_selection_format_buf: (
		terminal: TerminalHandle,
		optionsPtr: number,
		outputPtr: number,
		outputLength: number,
		outWrittenPtr: number,
	) => number;
	readonly ghostty_terminal_selection_adjust: (
		terminal: TerminalHandle,
		selectionPtr: number,
		adjustment: number,
	) => number;
	readonly ghostty_terminal_selection_order: (
		terminal: TerminalHandle,
		selectionPtr: number,
		outOrderPtr: number,
	) => number;
	readonly ghostty_terminal_selection_ordered: (
		terminal: TerminalHandle,
		selectionPtr: number,
		desiredOrder: number,
		outSelectionPtr: number,
	) => number;
	readonly ghostty_selection_gesture_new: (allocator: number, outGesturePtr: number) => number;
	readonly ghostty_selection_gesture_free: (gesture: number, terminal: TerminalHandle) => void;
	readonly ghostty_selection_gesture_reset: (gesture: number, terminal: TerminalHandle) => void;
	readonly ghostty_selection_gesture_event: (
		gesture: number,
		terminal: TerminalHandle,
		event: number,
		outSelectionPtr: number,
	) => number;
	readonly ghostty_selection_gesture_get: (
		gesture: number,
		terminal: TerminalHandle,
		data: number,
		outValuePtr: number,
	) => number;
	readonly ghostty_selection_gesture_event_new: (
		allocator: number,
		outEventPtr: number,
		type: number,
	) => number;
	readonly ghostty_selection_gesture_event_free: (event: number) => void;
	readonly ghostty_selection_gesture_event_set: (
		event: number,
		option: number,
		valuePtr: number,
	) => number;

	// Grid ref data access
	readonly ghostty_grid_ref_cell: (gridRefPtr: number, outCellPtr: number) => number;
	readonly ghostty_grid_ref_row: (gridRefPtr: number, outRowPtr: number) => number;
	readonly ghostty_grid_ref_graphemes: (
		gridRefPtr: number,
		outBufPtr: number,
		outBufLen: number,
		outLenPtr: number,
	) => number;
	readonly ghostty_grid_ref_hyperlink_uri: (
		gridRefPtr: number,
		outBufPtr: number,
		outBufLen: number,
		outLenPtr: number,
	) => number;
	readonly ghostty_grid_ref_style: (gridRefPtr: number, outStylePtr: number) => number;

	// Tracked grid ref
	readonly ghostty_tracked_grid_ref_free: (ref: number) => void;
	readonly ghostty_tracked_grid_ref_has_value: (ref: number) => number;

	// Cell/Row standalone data
	readonly ghostty_cell_get: (cell: bigint, data: number, outPtr: number) => number;
	readonly ghostty_cell_get_multi: (
		cell: number,
		count: number,
		keysPtr: number,
		valuesPtr: number,
		outWrittenPtr: number,
	) => number;
	readonly ghostty_row_get: (row: bigint, data: number, outPtr: number) => number;
	readonly ghostty_row_get_multi: (
		row: bigint,
		count: number,
		keysPtr: number,
		valuesPtr: number,
		outWrittenPtr: number,
	) => number;

	// Render state
	readonly ghostty_render_state_new: (allocator: number, statePtrPtr: number) => number;
	readonly ghostty_render_state_free: (state: RenderStateHandle) => void;
	readonly ghostty_render_state_update: (
		state: RenderStateHandle,
		terminal: TerminalHandle,
	) => number;
	readonly ghostty_render_state_begin_update: (
		state: RenderStateHandle,
		terminal: TerminalHandle,
	) => number;
	readonly ghostty_render_state_end_update: (state: RenderStateHandle) => number;
	readonly ghostty_render_state_get: (
		state: RenderStateHandle,
		data: number,
		outPtr: number,
	) => number;
	readonly ghostty_render_state_get_multi: (
		state: RenderStateHandle,
		count: number,
		keysPtr: number,
		valuesPtr: number,
		outWrittenPtr: number,
	) => number;
	readonly ghostty_render_state_set: (
		state: RenderStateHandle,
		option: number,
		valuePtr: number,
	) => number;
	readonly ghostty_render_state_colors_get: (
		state: RenderStateHandle,
		outColorsPtr: number,
	) => number;

	// Row iterator
	readonly ghostty_render_state_row_iterator_new: (
		allocator: number,
		iteratorPtrPtr: number,
	) => number;
	readonly ghostty_render_state_row_iterator_next: (iterator: RowIteratorHandle) => number;
	readonly ghostty_render_state_row_iterator_free: (iterator: RowIteratorHandle) => void;
	readonly ghostty_render_state_row_get: (
		iterator: RowIteratorHandle,
		data: number,
		outPtr: number,
	) => number;
	readonly ghostty_render_state_row_get_multi: (
		iterator: RowIteratorHandle,
		count: number,
		keysPtr: number,
		valuesPtr: number,
		outWrittenPtr: number,
	) => number;
	readonly ghostty_render_state_row_set: (
		iterator: RowIteratorHandle,
		option: number,
		valuePtr: number,
	) => number;

	// Row cells
	readonly ghostty_render_state_row_cells_new: (allocator: number, cellsPtrPtr: number) => number;
	readonly ghostty_render_state_row_cells_next: (cells: RowCellsHandle) => number;
	readonly ghostty_render_state_row_cells_select: (cells: RowCellsHandle, x: number) => number;
	readonly ghostty_render_state_row_cells_get: (
		cells: RowCellsHandle,
		data: number,
		outPtr: number,
	) => number;
	readonly ghostty_render_state_row_cells_get_multi: (
		cells: RowCellsHandle,
		count: number,
		keysPtr: number,
		valuesPtr: number,
		outWrittenPtr: number,
	) => number;
	readonly ghostty_render_state_row_cells_free: (cells: RowCellsHandle) => void;

	// Key encoder
	readonly ghostty_key_encoder_new: (allocator: number, encoderPtrPtr: number) => number;
	readonly ghostty_key_encoder_free: (encoder: number) => void;
	readonly ghostty_key_encoder_setopt: (encoder: number, option: number, valuePtr: number) => void;
	readonly ghostty_key_encoder_setopt_from_terminal: (
		encoder: number,
		terminal: TerminalHandle,
	) => void;
	readonly ghostty_key_encoder_encode: (
		encoder: number,
		eventPtr: number,
		bufPtr: number,
		bufLen: number,
		writtenPtr: number,
	) => number;

	// Key event
	readonly ghostty_key_event_new: (allocator: number, eventPtrPtr: number) => number;
	readonly ghostty_key_event_free: (event: number) => void;
	readonly ghostty_key_event_set_action: (event: number, action: number) => void;
	readonly ghostty_key_event_get_action: (event: number) => number;
	readonly ghostty_key_event_set_key: (event: number, key: number) => void;
	readonly ghostty_key_event_get_key: (event: number) => number;
	readonly ghostty_key_event_set_mods: (event: number, mods: number) => void;
	readonly ghostty_key_event_get_mods: (event: number) => number;
	readonly ghostty_key_event_set_consumed_mods: (event: number, mods: number) => void;
	readonly ghostty_key_event_get_consumed_mods: (event: number) => number;
	readonly ghostty_key_event_set_composing: (event: number, composing: number) => void;
	readonly ghostty_key_event_get_composing: (event: number) => number;
	readonly ghostty_key_event_set_utf8: (event: number, ptr: number, len: number) => void;
	readonly ghostty_key_event_get_utf8: (event: number, lenPtr: number) => number;
	readonly ghostty_key_event_set_unshifted_codepoint: (event: number, cp: number) => void;
	readonly ghostty_key_event_get_unshifted_codepoint: (event: number) => number;

	// Mouse encoder
	readonly ghostty_mouse_encoder_new: (allocator: number, encoderPtrPtr: number) => number;
	readonly ghostty_mouse_encoder_free: (encoder: number) => void;
	readonly ghostty_mouse_encoder_setopt: (
		encoder: number,
		option: number,
		valuePtr: number,
	) => void;
	readonly ghostty_mouse_encoder_setopt_from_terminal: (
		encoder: number,
		terminal: TerminalHandle,
	) => void;
	readonly ghostty_mouse_encoder_reset: (encoder: number) => void;
	readonly ghostty_mouse_encoder_encode: (
		encoder: number,
		event: number,
		bufPtr: number,
		bufLen: number,
		writtenPtr: number,
	) => number;

	// Mouse event
	readonly ghostty_mouse_event_new: (allocator: number, eventPtrPtr: number) => number;
	readonly ghostty_mouse_event_free: (event: number) => void;
	readonly ghostty_mouse_event_set_action: (event: number, action: number) => void;
	readonly ghostty_mouse_event_set_button: (event: number, button: number) => void;
	readonly ghostty_mouse_event_clear_button: (event: number) => void;
	readonly ghostty_mouse_event_set_mods: (event: number, mods: number) => void;
	readonly ghostty_mouse_event_set_position: (event: number, positionPtr: number) => void;

	// Focus and paste encoding
	readonly ghostty_focus_encode: (
		event: number,
		bufPtr: number,
		bufLen: number,
		writtenPtr: number,
	) => number;
	readonly ghostty_paste_is_safe: (dataPtr: number, dataLen: number) => number;
	readonly ghostty_paste_encode: (
		dataPtr: number,
		dataLen: number,
		bracketed: number,
		bufPtr: number,
		bufLen: number,
		writtenPtr: number,
	) => number;

	// Type info
	readonly ghostty_type_json: () => number;
}

// ============================================================================
// xterm.js-Compatible Interfaces
// ============================================================================

/** Terminal options (xterm.js-compatible). */
export interface ITerminalOptions {
	cols?: number;
	rows?: number;
	cursorBlink?: boolean;
	cursorStyle?: 'block' | 'underline' | 'bar';
	theme?: ITheme;
	scrollback?: number;
	fontSize?: number;
	fontFamily?: string;
	fontLigatures?: boolean;
	/** Ghostty-compatible deltas for grid, decoration, cursor, and icon metrics. */
	fontMetricModifiers?: FontMetricModifiers;
	/** Balance unused surface pixels around the terminal grid. */
	paddingBalance?: PaddingBalance;
	allowTransparency?: boolean;
	convertEol?: boolean;
	disableStdin?: boolean;
	smoothScrollDuration?: number;
	copyOnSelect?: boolean;
	pasteProtection?: boolean;
	pasteBracketedSafe?: boolean;
}

/** Terminal color theme (xterm.js-compatible). */
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

/** Keyboard event with key string and DOM event. */
export interface IKeyEvent {
	key: string;
	domEvent: KeyboardEvent;
}

/** Buffer range for selection coordinates. */
export interface IBufferRange {
	start: { x: number; y: number };
	end: { x: number; y: number };
}

/** Addon interface (xterm.js-compatible). */
export interface ITerminalAddon {
	activate(terminal: ITerminalCore): void;
	dispose(): void;
}

/** Core terminal interface exposed to addons. */
export interface ITerminalCore {
	cols: number;
	rows: number;
	element?: HTMLElement | undefined;
	textarea?: HTMLTextAreaElement | undefined;
}

/** Unicode version provider (xterm.js compatibility). */
export interface IUnicodeVersionProvider {
	readonly activeVersion: string;
}

// ============================================================================
// Buffer API Interfaces (xterm.js compatibility)
// ============================================================================

/** Top-level buffer namespace. */
export interface IBufferNamespace {
	readonly active: IBuffer;
	readonly normal: IBuffer;
	readonly alternate: IBuffer;
	readonly onBufferChange: IEvent<IBuffer>;
}

/** A terminal buffer (normal or alternate screen). */
export interface IBuffer {
	readonly type: 'normal' | 'alternate';
	readonly cursorX: number;
	readonly cursorY: number;
	readonly viewportY: number;
	readonly baseY: number;
	readonly length: number;
	getLine(y: number): IBufferLine | undefined;
	getNullCell(): IBufferCell;
}

/** A single line in the buffer. */
export interface IBufferLine {
	readonly length: number;
	readonly isWrapped: boolean;
	getCell(x: number): IBufferCell | undefined;
	translateToString(trimRight?: boolean, startColumn?: number, endColumn?: number): string;
}

/** A single cell in the buffer. */
export interface IBufferCell {
	getChars(): string;
	getCode(): number;
	getWidth(): number;
	getFgColorMode(): number;
	getBgColorMode(): number;
	getFgColor(): number;
	getBgColor(): number;
	isBold(): number;
	isItalic(): number;
	isUnderline(): number;
	isStrikethrough(): number;
	isBlink(): number;
	isInverse(): number;
	isInvisible(): number;
	isFaint(): number;
	getHyperlinkId(): number;
	getCodepoint(): number;
	isDim(): boolean;
}

// ============================================================================
// Event System
// ============================================================================

export type IEvent<T> = (listener: (arg: T) => void) => IDisposable;

export interface IDisposable {
	dispose(): void;
}

// ============================================================================
// Renderer Types
// ============================================================================

/** Font metrics (CSS pixels). */
export interface FontMetrics {
	width: number;
	height: number;
	baseline: number;
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

/** Ghostty padding modes; `true` caps top padding and `equal` centers the grid. */
export type PaddingBalance = false | true | 'equal';

export interface SurfacePadding {
	readonly top: number;
	readonly bottom: number;
	readonly left: number;
	readonly right: number;
}

/** Renderer configuration options. */
export interface RendererOptions {
	fontSize?: number;
	fontFamily?: string;
	fontLigatures?: boolean;
	fontMetricModifiers?: FontMetricModifiers;
	paddingBalance?: PaddingBalance;
	cursorStyle?: 'block' | 'underline' | 'bar';
	cursorBlink?: boolean;
	theme?: Partial<ITheme>;
	devicePixelRatio?: number;
	allowTransparency?: boolean;
}

/** Abstraction the renderer draws from. */
export interface IRenderable {
	getLine(y: number): GhosttyCell[] | null;
	getCursor(): RenderStateCursor;
	getDimensions(): { cols: number; rows: number };
	isRowDirty(y: number): boolean;
	needsFullRedraw?(): boolean;
	clearDirty(): void;
	getGraphemeString?(row: number, col: number): string;
}

/** Scrollback data provider for the renderer. */
export interface IScrollbackProvider {
	getScrollbackLine(offset: number): GhosttyCell[] | null;
	getScrollbackLength(): number;
	getScrollbackGraphemeString?(offset: number, col: number): string;
}

/** Cursor state from render state. */
export interface RenderStateCursor {
	x: number;
	y: number;
	viewportX: number;
	viewportY: number;
	visible: boolean;
	blinking: boolean;
	style: 'block' | 'underline' | 'bar';
}

// ============================================================================
// Selection Types
// ============================================================================

/** Selection coordinates in viewport space. */
export interface SelectionCoordinates {
	startCol: number;
	startRow: number;
	endCol: number;
	endRow: number;
	rectangle: boolean;
}

// ============================================================================
// Link Detection Types
// ============================================================================

/** Coordinate in the terminal buffer. */
export interface IBufferCellPosition {
	x: number;
	y: number;
}

/** Range in the terminal buffer (may span multiple lines). */
export interface ILinkRange {
	start: IBufferCellPosition;
	end: IBufferCellPosition;
}

/** A detected link in the terminal. */
export interface ILink {
	text: string;
	range: ILinkRange;
	target?: TerminalOpenTarget;
	activation?: 'direct' | 'modifier';
	activate?(event: MouseEvent): void;
	hover?(isHovered: boolean): void;
	dispose?(): void;
}

/** Provides link detection for a specific type of link. */
export interface ILinkProvider {
	provideLinks(y: number, callback: (links: ILink[] | undefined) => void): void;
	dispose?(): void;
}

/** Simplified buffer line interface for link providers. */
export interface ILinkBufferLine {
	length: number;
	readonly isWrapped: boolean;
	getCell(x: number): ILinkBufferCell | undefined;
	translateToString(trimRight?: boolean, startColumn?: number, endColumn?: number): string;
}

/** Simplified buffer cell interface for link providers. */
export interface ILinkBufferCell {
	getCodepoint(): number;
	getHyperlinkId(): number;
	getWidth(): number;
	isBold(): number;
	isItalic(): number;
	isDim(): boolean;
}

/** Simplified terminal buffer interface for link providers. */
export interface ILinkBuffer {
	length: number;
	getLine(y: number): ILinkBufferLine | undefined;
}

/** Terminal dimensions for FitAddon. */
export interface ITerminalDimensions {
	cols: number;
	rows: number;
}
