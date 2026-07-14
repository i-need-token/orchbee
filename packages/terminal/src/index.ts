/**
 * @orchbee/terminal — Ghostty-powered terminal for the browser.
 *
 * High-performance terminal emulation using Ghostty's battle-tested
 * VT100 parser compiled to WebAssembly. Uses the upstream Ghostty
 * C API with iterator-based cell access for rendering.
 *
 * @example
 * ```ts
 * import { init, Terminal, DirtyState } from "@orchbee/terminal";
 *
 * // Initialize (load WASM module)
 * await init({ source: "/ghostty-vt.wasm" });
 *
 * // Create terminal
 * const term = new Terminal(80, 24);
 *
 * // Write data
 * term.write("Hello, world!\r\n");
 *
 * // Per-frame render loop
 * const dirty = term.grid.update();
 * if (dirty !== DirtyState.NONE) {
 *   term.grid.forEachRow((row, reader) => {
 *     while (reader.next()) {
 *       const text = reader.readText();
 *       // ... render text ...
 *     }
 *   });
 *   term.grid.markClean();
 * }
 *
 * term.dispose();
 * ```
 */

// Bridge & initialization
export { init, getBridge, type InitOptions, WasmBridge } from './wasm-bridge';

// Terminal class
export {
	Terminal,
	type TerminalCapabilities,
	type TerminalConfig,
	type TerminalOptions,
} from './terminal';

// Host capabilities
export { createBrowserTerminalHost } from './host/browser-terminal-host';
export { resolveHostCapabilities } from './host/terminal-capabilities';
export type {
	ClipboardPayload,
	ClipboardReadRequest,
	ClipboardReadSource,
	ClipboardTarget,
	ClipboardWriteRequest,
	ClipboardWriteSource,
	ContextMenuActionState,
	ContextMenuRequest,
	OpenTargetRequest,
	PasteConfirmationRequest,
	PointerRequest,
	TerminalClipboardHost,
	TerminalDropRequest,
	TerminalDropResolution,
	TerminalHost,
	TerminalHostCapabilities,
	TerminalHostErrorEvent,
	TerminalHostOperation,
	TerminalInteractionContext,
	TerminalModifierState,
	TerminalNotificationRequest,
	TerminalOpenTarget,
	TerminalSelectionSnapshot,
} from './host/terminal-host';

export {
	ContextMenuRegistry,
	type ContextMenuItem,
	type ContextMenuProvider,
} from './context-menu/context-menu-registry';

// GhosttyTerminal compatibility layer
export { GhosttyTerminal } from './ghostty-terminal';

// Grid data reader
export { Grid, CellReader, type CellData } from './grid';

// Event system
export { EventEmitter, type IDisposable, type IEvent } from './event-emitter';

// Actions
export {
	TerminalActionRegistry,
	type TerminalActionContext,
	type TerminalActionDefinition,
	type TerminalActionResult,
	type TerminalActionSource,
	type TerminalActionState,
} from './actions/action-registry';

// Renderer
export {
	WebGPURenderer,
	DEFAULT_THEME,
	type RendererFrameReason,
	type RendererFrameTelemetry,
	type RendererHealthEvent,
} from './renderer';
export {
	applyFontMetricModifier,
	applyFontMetricModifiers,
	fontSizeAdjustmentScale,
	parseFontMetricModifier,
	type DetailedFontMetrics,
	type FontFaceMetrics,
	type ParsedFontMetricModifier,
} from './font-metrics';
export {
	additionalSurfaceMargins,
	balanceGridPadding,
	balanceSurfacePadding,
	calculateSurfaceGrid,
	type SurfaceGridSize,
	type SurfaceSize,
} from './surface-size';

// Input handling
export {
	InputHandler,
	type CompositionSelection,
	type InputHandlerDependencies,
} from './input-handler';
export {
	calculateInputAnchor,
	type InputAnchorLayout,
	type InputAnchorRequest,
} from './input/input-anchor';
export { OrderedInputDispatcher } from './input/ordered-input-dispatcher';
export {
	KeyEventRegistry,
	type TerminalKeyEventContext,
	type TerminalKeyEventHandler,
} from './input/key-event-registry';
export { buildPreeditState, type PreeditCellData, type PreeditState } from './input/preedit';
export {
	AccessibleTextModel,
	buildAccessibleRow,
	normalizeAccessibleRows,
	TerminalAccessibility,
	type AccessibleCellInput,
	type AccessibleCursor,
	type AccessibleOffsetRange,
	type AccessibleRow,
	type AccessibleTextPosition,
	type AccessibleTextRange,
	type TerminalAccessibilityOptions,
} from './accessibility/terminal-accessibility';
export {
	findTerminalMatches,
	searchMatchContainsCell,
	type TerminalSearchMatch,
	type TerminalSearchOptions,
} from './search/terminal-search';
export {
	formatTerminalIdentity,
	type TerminalProtocolIdentity,
} from './protocol/terminal-identity';
export {
	readGhosttyBuildCapabilities,
	type GhosttyBuildCapabilities,
} from './protocol/build-capabilities';
export {
	encodePaste,
	inspectPaste,
	type PasteCodec,
	type PasteInspection,
	type PastePolicyOptions,
	type PasteResult,
	type PasteSource,
} from './input/paste-policy';

// Selection
export {
	SelectionManager,
	type ISelectionTerminal,
	type SelectionCopyResult,
	type SelectionCell,
	type SelectionManagerOptions,
	type SelectionModel,
} from './selection-manager';
export {
	GhosttySelectionController,
	type GhosttySelectionAdjustment,
	type GhosttySelectionAutoscroll,
	type GhosttySelectionGestureAutoscrollTick,
	type GhosttySelectionGestureDrag,
	type GhosttySelectionGesturePoint,
	type GhosttySelectionGesturePress,
	type GhosttySelectionRange,
} from './selection/ghostty-selection';

// Link detection
export { LinkDetector, type ITerminalForLinkDetector } from './link-detector';
export { OSC8LinkProvider, type ITerminalForOSC8Provider } from './providers/osc8-link-provider';
export { UrlRegexProvider, type ITerminalForUrlProvider } from './providers/url-regex-provider';
export { PathLinkProvider, type ITerminalForPathProvider } from './providers/path-link-provider';

// Buffer API
export { BufferNamespace, Buffer, BufferLine, BufferCell, type BufferTerminal } from './buffer';

// Addons
export { FitAddon } from './addons/fit';

// Types & enums
export {
	DirtyState,
	type RenderStateHandle,
	RenderStateData,
	RenderStateOption,
	RowCellsData,
	RowData,
	CellDataField,
	CellSemanticContent,
	UnderlineStyle,
	CellWide,
	TerminalData,
	TerminalOption,
	Result,
	BuildInfo,
	DecMode,
	AnsiMode,
	buildModeTag,
	type RGB,
	type CursorState,
	type TerminalColors,
	type TerminalHandle,
	type WasmExports,
	type KeyEvent,
	KeyAction,
	KeyEncoderOption,
	Mods,
	Key,
	PointTag,
	CellFlags,
	type GhosttyCell,
	TERMINAL_OPTIONS_SIZE,
	RENDER_STATE_COLORS_SIZE,
	COLOR_RGB_SIZE,
	CELL_VALUE_SIZE,
	STYLE_SIZE,
	BUFFER_SIZE,
	POINT_SIZE,
	GRID_REF_SIZE,
	// xterm.js-compatible interfaces
	type ITerminalOptions,
	type ITheme,
	type IKeyEvent,
	type IBufferRange,
	type ITerminalAddon,
	type ITerminalCore,
	type IUnicodeVersionProvider,
	// Renderer types
	type FontMetrics,
	type FontMetricModifier,
	type FontMetricModifiers,
	type FontSizeAdjustment,
	type PaddingBalance,
	type SurfacePadding,
	type RendererOptions,
	type IRenderable,
	type IScrollbackProvider,
	type RenderStateCursor,
	// Selection types
	type SelectionCoordinates,
	// Link detection types
	type IBufferCellPosition,
	type ILinkRange,
	type ILink,
	type ILinkProvider,
	type ILinkBufferLine,
	type ILinkBufferCell,
	type ILinkBuffer,
	type ITerminalDimensions,
} from './types';
