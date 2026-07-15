/**
 * Terminal — high-level terminal emulator class.
 *
 * Wraps the Ghostty WASM VT parser with a clean, object-oriented API.
 * Uses the upstream Ghostty C API (not the ghostty-web patched API).
 *
 * Key differences from the patched API:
 * - Terminal creation uses double-pointer pattern + options struct
 * - Writing uses ghostty_terminal_vt_write (not ghostty_terminal_write)
 * - Resize requires cellWidthPx/cellHeightPx parameters
 * - Properties are read via ghostty_terminal_get/render_state_get with field IDs
 * - Cell access is iterator-based, not flat struct array
 * - DSR responses come via write_pty callback, not read_response
 *
 * @example
 * ```ts
 * import { init, Terminal } from "@orchbee/terminal";
 *
 * await init({ source: "/ghostty-vt.wasm" });
 * const term = new Terminal(80, 24, { scrollbackLimit: 10_000_000 });
 *
 * term.write("Hello, world!\r\n");
 *
 * // Per-frame render loop
 * const dirty = term.grid.update();
 * if (dirty !== 0) {
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

import { Grid } from '../grid';
import { getBridge, WasmBridge } from '../wasm-bridge';
import {
	buildModeTag,
	COLOR_RGB_SIZE,
	type CursorState,
	type CellData,
	CellSemanticContent,
	DecMode,
	DirtyState,
	POINT_SIZE,
	PointTag,
	Result,
	type RGB,
	RENDER_STATE_COLORS_SIZE,
	RenderStateData,
	type TerminalColors,
	type TerminalConfig,
	TerminalData,
	TerminalOption,
	TERMINAL_OPTIONS_SIZE,
	type TerminalHandle,
	type ITerminalOptions,
	type IKeyEvent,
	type IBufferRange,
	type ITheme,
	type ITerminalAddon,
	type ILink,
	type ILinkProvider,
	type IUnicodeVersionProvider,
	type RendererOptions,
} from '../types';
import { EventEmitter, type IDisposable, type IEvent } from '../event-emitter';
import {
	DEFAULT_THEME,
	type RendererFrameTelemetry,
	type RendererHealthEvent,
	WebGPURenderer,
} from '../rendering';
import {
	buildPreeditState,
	calculateInputAnchor,
	classifySurfaceMousePlatform,
	encodePaste,
	InputHandler,
	inspectPaste,
	KeyEventRegistry,
	OrderedInputDispatcher,
	resolveSurfaceMouseCursor,
	type CompositionSelection,
	type InputHandlerDependencies,
	type PasteResult,
	type PasteSource,
	type TerminalKeyEventHandler,
} from '../input';
import {
	GhosttySelectionController,
	SelectionManager,
	type GhosttySelectionAdjustment,
	type ISelectionTerminal,
	type SelectionCopyResult,
} from '../selection';
import { LinkDetector, type ITerminalForLinkDetector } from '../link-detector';
import { OSC8LinkProvider, PathLinkProvider, UrlRegexProvider } from '../providers';
import { GhosttyTerminal } from '../ghostty-terminal';
import { BufferNamespace } from '../buffer';
import {
	createBrowserTerminalHost,
	resolveHostCapabilities,
	type ClipboardPayload,
	type TerminalHost,
	type TerminalHostErrorEvent,
	type TerminalInteractionContext,
	type TerminalModifierState,
} from '../host';
import {
	formatTerminalIdentity,
	readGhosttyBuildCapabilities,
	type GhosttyBuildCapabilities,
	type TerminalProtocolIdentity,
} from '../protocol';
import {
	buildAccessibleRow,
	TerminalAccessibility,
	type AccessibleOffsetRange,
	type AccessibleRow,
	type AccessibleTextRange,
} from '../accessibility';
import {
	findDirectionalTerminalMatch,
	findTerminalMatches,
	resolveTerminalSearchResultLimit,
	TERMINAL_SEARCH_DEBOUNCE_MS,
	type TerminalSearchMatch,
	type TerminalSearchOptions,
	type TerminalSearchResultChangeEvent,
	validateTerminalSearchQuery,
} from '../search';
import {
	TerminalActionRegistry,
	type TerminalActionResult,
	type TerminalActionSource,
} from '../actions';
import {
	ContextMenuRegistry,
	type ContextMenuItem,
	type ContextMenuProvider,
} from '../context-menu';

// Re-export types for the public API
export type { TerminalConfig } from '../types';

const DEFAULT_SCROLLBACK_BYTES = 10_000_000;
const POINT_COORDINATE_SIZE = 8;

interface ViewportResizeAnchor {
	resolveScreenRow(): number | undefined;
	dispose(): void;
}

interface TerminalSearchSnapshot {
	readonly lines: readonly (readonly CellData[])[];
	readonly wrappedRows: readonly boolean[];
}

function createViewportResizeAnchor(
	bridge: WasmBridge,
	handle: TerminalHandle,
	viewportOffset: number,
	scrollbackLength: number,
): ViewportResizeAnchor | undefined {
	if (viewportOffset <= 0) return undefined;
	const exports = bridge.getExports();
	const pointBuffer = bridge.allocU8Array(POINT_SIZE);
	const trackerOutput = bridge.allocOpaque();
	let coordinateBuffer = 0;
	let tracker = 0;
	try {
		bridge.writeBytes(pointBuffer, new Uint8Array(POINT_SIZE));
		const point = bridge.createView(pointBuffer, POINT_SIZE);
		point.setUint32(0, PointTag.SCREEN, true);
		point.setUint16(8, 0, true);
		point.setUint32(12, Math.max(0, scrollbackLength - Math.floor(viewportOffset)), true);
		if (
			exports.ghostty_terminal_grid_ref_track(handle, pointBuffer, trackerOutput) !== Result.SUCCESS
		) {
			return undefined;
		}
		tracker = bridge.createView(trackerOutput, 4).getUint32(0, true);
		if (tracker === 0) return undefined;
		coordinateBuffer = bridge.allocU8Array(POINT_COORDINATE_SIZE);
		let active = true;
		return {
			resolveScreenRow: (): number | undefined => {
				if (!active) return undefined;
				if (
					exports.ghostty_tracked_grid_ref_point(tracker, PointTag.SCREEN, coordinateBuffer) !==
					Result.SUCCESS
				) {
					return undefined;
				}
				return bridge.createView(coordinateBuffer, POINT_COORDINATE_SIZE).getUint32(4, true);
			},
			dispose: (): void => {
				if (!active) return;
				exports.ghostty_tracked_grid_ref_free(tracker);
				bridge.freeU8Array(coordinateBuffer, POINT_COORDINATE_SIZE);
				active = false;
			},
		};
	} finally {
		if (tracker === 0 && coordinateBuffer !== 0) {
			bridge.freeU8Array(coordinateBuffer, POINT_COORDINATE_SIZE);
		}
		bridge.freeU8Array(pointBuffer, POINT_SIZE);
		bridge.freeOpaque(trackerOutput);
	}
}

export interface TerminalOptions extends ITerminalOptions {
	readonly host?: TerminalHost;
	readonly identity?: Partial<TerminalProtocolIdentity>;
	readonly enquiryResponse?: string;
}

export interface TerminalCapabilities {
	readonly renderer: 'webgpu' | 'canvas2d' | 'headless' | 'uninitialized';
	readonly mouseFormats: readonly ('x10' | 'utf8' | 'sgr' | 'urxvt' | 'sgr-pixel')[];
	readonly graphics: readonly ('kitty' | 'glyph')[];
	readonly clipboardRead: boolean;
	readonly clipboardWrite: boolean;
	readonly contextMenu: boolean;
	readonly notifications: boolean;
	readonly openTarget: boolean;
	readonly pasteConfirmation: boolean;
	readonly fileDrop: boolean;
	readonly accessibility: boolean;
	readonly semanticPrompts: boolean;
}

/**
 * Terminal emulator backed by Ghostty's WASM VT parser.
 */
export class Terminal {
	private readonly bridge: WasmBridge;
	private handle: TerminalHandle;
	private rsHandle: number;
	private columnCount: number;
	private rowCount: number;
	private writePtyCallback: { readonly index: number; dispose(): void } | undefined;
	private bellCallback: { readonly index: number; dispose(): void } | undefined;
	private titleCallback: { readonly index: number; dispose(): void } | undefined;
	private pwdCallback: { readonly index: number; dispose(): void } | undefined;
	private sizeCallback: { readonly index: number; dispose(): void } | undefined;
	private colorSchemeCallback: { readonly index: number; dispose(): void } | undefined;
	private xtversionCallback: { readonly index: number; dispose(): void } | undefined;
	private enquiryCallback: { readonly index: number; dispose(): void } | undefined;
	private readonly identityBuffer: number;
	private readonly identityLength: number;
	private readonly enquiryBuffer: number;
	private readonly enquiryLength: number;
	private lastAlternateScreen = false;
	private lastCursorPosition: { x: number; y: number } | undefined;
	private readonly host: TerminalHost;
	private readonly selectionController: GhosttySelectionController;
	private readonly inputDispatcher: OrderedInputDispatcher;
	private cachedColorScheme: 0 | 1 = 1;

	/** Grid data reader (viewport, scrollback, graphemes). */
	readonly grid: Grid;
	/** xterm-compatible read-only buffer namespace. */
	readonly buffer: BufferNamespace;
	/** xterm-compatible terminal options. */
	readonly options: Required<ITerminalOptions>;
	private readonly optionState: Required<ITerminalOptions>;
	/** Unicode version implemented by Ghostty. */
	readonly unicode: IUnicodeVersionProvider = { activeVersion: '15.1' };
	/** Compile-time features of the packaged Ghostty WASM binary. */
	readonly ghosttyBuildCapabilities: GhosttyBuildCapabilities;
	/** Named terminal actions shared by menus, keybindings, and automation. */
	readonly actions = new TerminalActionRegistry();
	/** Serializable application/addon context-menu contributions. */
	readonly contextMenu = new ContextMenuRegistry();
	/** Prioritized application and surface key handlers. */
	readonly keyEvents = new KeyEventRegistry();
	private readonly addons = new Set<ITerminalAddon>();
	private customKeyEventHandler: ((event: KeyboardEvent) => boolean) | undefined;
	private customWheelEventHandler: ((event: WheelEvent) => boolean) | undefined;
	private targetViewportY = 0;
	private scrollAnimationFrame = 0;
	private scrollAnimationLastTime = 0;
	private contextMenuRequestId = 0;
	private visibilityListenerAttached = false;
	private windowResizeListenerAttached = false;
	private devicePixelRatioQuery: MediaQueryList | undefined;
	private accessibility: TerminalAccessibility | undefined;
	private renderedDefaultColorsKey = '';

	constructor(options?: TerminalOptions);
	constructor(cols: number, rows: number, config?: TerminalConfig);
	constructor(bridge: WasmBridge, cols: number, rows: number, config?: TerminalConfig);
	constructor(
		bridgeOrCols: WasmBridge | number | TerminalOptions = {},
		rowsOrConfig?: number | TerminalConfig,
		colsOrConfig?: number | TerminalConfig,
		configArg?: TerminalConfig,
	) {
		// Overload resolution
		let resolvedConfig: TerminalConfig | undefined;
		let optionInput: TerminalOptions = {};
		if (bridgeOrCols instanceof WasmBridge) {
			if (typeof rowsOrConfig !== 'number' || typeof colsOrConfig !== 'number') {
				throw new TypeError('Terminal dimensions must be numbers');
			}
			this.bridge = bridgeOrCols;
			this.columnCount = rowsOrConfig;
			this.rowCount = colsOrConfig;
			resolvedConfig = configArg;
		} else if (typeof bridgeOrCols === 'number') {
			if (typeof rowsOrConfig !== 'number') throw new TypeError('Terminal rows must be a number');
			this.bridge = getBridge();
			this.columnCount = bridgeOrCols;
			this.rowCount = rowsOrConfig;
			resolvedConfig = typeof colsOrConfig === 'object' ? colsOrConfig : undefined;
		} else {
			this.bridge = getBridge();
			optionInput = bridgeOrCols;
			this.columnCount = optionInput.cols ?? 80;
			this.rowCount = optionInput.rows ?? 24;
			const theme = optionInput.theme;
			resolvedConfig = {
				scrollbackLimit: optionInput.scrollback ?? DEFAULT_SCROLLBACK_BYTES,
				...(theme?.foreground ? { fgColor: packRgb(parseCssColor(theme.foreground)) } : {}),
				...(theme?.background ? { bgColor: packRgb(parseCssColor(theme.background)) } : {}),
				...(theme?.cursor ? { cursorColor: packRgb(parseCssColor(theme.cursor)) } : {}),
			};
		}
		this.host = optionInput.host ?? createBrowserTerminalHost();
		this.ghosttyBuildCapabilities = readGhosttyBuildCapabilities(this.bridge);
		this.inputDispatcher = new OrderedInputDispatcher((error) =>
			this.onHostErrorEmitter.fire({ operation: 'input-dispatch', error }),
		);

		const baseOptions: Required<ITerminalOptions> = {
			cols: this.columnCount,
			rows: this.rowCount,
			cursorBlink: optionInput.cursorBlink ?? false,
			cursorStyle: optionInput.cursorStyle ?? 'block',
			theme: optionInput.theme ?? {},
			scrollback: resolvedConfig?.scrollbackLimit ?? DEFAULT_SCROLLBACK_BYTES,
			fontSize: optionInput.fontSize ?? 15,
			fontFamily: optionInput.fontFamily ?? 'monospace',
			fontWeight: optionInput.fontWeight ?? 'normal',
			fontWeightBold: optionInput.fontWeightBold ?? 'bold',
			fontLigatures: optionInput.fontLigatures ?? true,
			fontMetricModifiers: optionInput.fontMetricModifiers ?? {},
			paddingBalance: optionInput.paddingBalance ?? false,
			allowTransparency: optionInput.allowTransparency ?? false,
			convertEol: optionInput.convertEol ?? false,
			disableStdin: optionInput.disableStdin ?? false,
			smoothScrollDuration: optionInput.smoothScrollDuration ?? 100,
			copyOnSelect: optionInput.copyOnSelect ?? true,
			pasteProtection: optionInput.pasteProtection ?? true,
			pasteBracketedSafe: optionInput.pasteBracketedSafe ?? true,
		};
		this.optionState = baseOptions;
		this.options = new Proxy(this.optionState, {
			set: (target, property, value): boolean => {
				const previous = Reflect.get(target, property);
				if (property === 'disableStdin' && previous === false && value === true) {
					this.inputHandler?.releasePressedKeys();
					this.inputHandler?.releasePressedMouseButtons();
					this.inputHandler?.cancelComposition();
				}
				const updated = Reflect.set(target, property, value);
				if (updated && previous !== value) this.handleOptionChange(String(property));
				return updated;
			},
		});
		this.cachedColorScheme = colorSchemeFor(
			parseCssColor({ ...DEFAULT_THEME, ...baseOptions.theme }.background),
		);
		const identityBytes = new TextEncoder().encode(formatTerminalIdentity(optionInput.identity));
		this.identityLength = identityBytes.length;
		this.identityBuffer = this.bridge.allocU8Array(this.identityLength);
		this.bridge.writeBytes(this.identityBuffer, identityBytes);
		const enquiryBytes = new TextEncoder().encode(optionInput.enquiryResponse ?? '').slice(0, 1024);
		this.enquiryLength = enquiryBytes.length;
		this.enquiryBuffer =
			this.enquiryLength === 0 ? 0 : this.bridge.allocU8Array(this.enquiryLength);
		if (this.enquiryLength > 0) this.bridge.writeBytes(this.enquiryBuffer, enquiryBytes);

		this.handle = this.createHandle(this.columnCount, this.rowCount, resolvedConfig);
		this.setPaletteOnHandle(this.handle, optionInput.theme ?? {});
		this.writePtyCallback = this.bridge.registerCallback4((_terminal, _userdata, data, len) => {
			const response = new TextDecoder().decode(this.bridge.readBytes(data, len));
			if (response.length > 0) this.onDataEmitter.fire(response);
		});
		const callbackResult = this.bridge
			.getExports()
			.ghostty_terminal_set(this.handle, TerminalOption.WRITE_PTY, this.writePtyCallback.index);
		if (callbackResult !== 0) {
			this.writePtyCallback.dispose();
			this.writePtyCallback = undefined;
			this.bridge.getExports().ghostty_terminal_free(this.handle);
			this.handle = 0;
			throw new Error(`Failed to register Ghostty write_pty callback: ${callbackResult}`);
		}
		this.bellCallback = this.bridge.registerCallback2(() => this.onBellEmitter.fire());
		this.titleCallback = this.bridge.registerCallback2(() =>
			this.onTitleChangeEmitter.fire(this.getTitle()),
		);
		this.pwdCallback = this.bridge.registerCallback2(() =>
			this.onPwdChangeEmitter.fire(this.getPwd()),
		);
		this.sizeCallback = this.bridge.registerCallback3ReturningI32(
			(_terminal, _userdata, output) => {
				const metrics = this.renderer?.getMetrics() ?? { width: 8, height: 16 };
				const size = this.bridge.createView(output, 12);
				size.setUint16(0, this.rowCount, true);
				size.setUint16(2, this.columnCount, true);
				size.setUint32(4, Math.max(1, Math.round(metrics.width)), true);
				size.setUint32(8, Math.max(1, Math.round(metrics.height)), true);
				return 1;
			},
		);
		this.colorSchemeCallback = this.bridge.registerCallback3ReturningI32(
			(_terminal, _userdata, output) => {
				this.bridge.createView(output, 4).setUint32(0, this.cachedColorScheme, true);
				return 1;
			},
		);
		this.xtversionCallback = this.bridge.registerCallback3((output) => {
			const result = this.bridge.createView(output, 8);
			result.setUint32(0, this.identityBuffer, true);
			result.setUint32(4, this.identityLength, true);
		});
		this.enquiryCallback = this.bridge.registerCallback3((output) => {
			const result = this.bridge.createView(output, 8);
			result.setUint32(0, this.enquiryBuffer, true);
			result.setUint32(4, this.enquiryLength, true);
		});
		this.bridge
			.getExports()
			.ghostty_terminal_set(this.handle, TerminalOption.BELL, this.bellCallback.index);
		this.bridge
			.getExports()
			.ghostty_terminal_set(this.handle, TerminalOption.TITLE_CHANGED, this.titleCallback.index);
		this.bridge
			.getExports()
			.ghostty_terminal_set(this.handle, TerminalOption.PWD_CHANGED, this.pwdCallback.index);
		this.bridge
			.getExports()
			.ghostty_terminal_set(this.handle, TerminalOption.SIZE, this.sizeCallback.index);
		this.bridge
			.getExports()
			.ghostty_terminal_set(
				this.handle,
				TerminalOption.COLOR_SCHEME,
				this.colorSchemeCallback.index,
			);
		this.bridge
			.getExports()
			.ghostty_terminal_set(this.handle, TerminalOption.XTVERSION, this.xtversionCallback.index);
		this.bridge
			.getExports()
			.ghostty_terminal_set(this.handle, TerminalOption.ENQUIRY, this.enquiryCallback.index);
		this.setDefaultCursorStyle(this.options.cursorStyle);
		this.setDefaultCursorBlink(this.options.cursorBlink);
		this.selectionController = new GhosttySelectionController(this.bridge, this.handle);
		this.rsHandle = this.createRenderState();
		this.grid = new Grid(this.bridge, this.handle, this.rsHandle, this.columnCount, this.rowCount);
		this.ghosttyTerm = new GhosttyTerminal(this);
		this.buffer = new BufferNamespace(this);
		this.lastAlternateScreen = this.isAlternateScreen();
		this.registerBuiltInActions();
	}

	get cols(): number {
		return this.columnCount;
	}

	get rows(): number {
		return this.rowCount;
	}

	get capabilities(): TerminalCapabilities {
		const host = resolveHostCapabilities(this.host);
		return {
			renderer: this.renderer?.getBackend() ?? 'headless',
			mouseFormats: ['x10', 'utf8', 'sgr', 'urxvt', 'sgr-pixel'],
			graphics: [],
			clipboardRead: host.clipboardRead,
			clipboardWrite: host.clipboardWrite,
			contextMenu: host.contextMenu,
			notifications: host.notifications,
			openTarget: host.openTarget,
			pasteConfirmation: host.pasteConfirmation,
			fileDrop: host.fileDrop,
			accessibility: true,
			semanticPrompts: true,
		};
	}

	// ==========================================================================
	// Lifecycle
	// ==========================================================================

	/** Write data to the terminal (parses VT sequences). */
	write(data: string | Uint8Array, callback?: () => void): void {
		if (this.options.convertEol && typeof data === 'string') {
			data = data.replace(/\r?\n/g, '\r\n');
		}
		const bytes = typeof data === 'string' ? new TextEncoder().encode(data) : data;
		if (bytes.length === 0) {
			callback?.();
			return;
		}

		const ptr = this.bridge.allocU8Array(bytes.length);
		const oldScrollbackLength = this.grid.getScrollbackLength();
		try {
			this.bridge.writeBytes(ptr, bytes);
			this.bridge.getExports().ghostty_terminal_vt_write(this.handle, ptr, bytes.length);
			this.linkDetector?.invalidateCache();
			this.clearHoveredLink();
			this.syncSurfaceMouseOwnership();
			this.invalidateSearchAfterBufferChange();
			const newScrollbackLength = this.grid.getScrollbackLength();
			if (newScrollbackLength < oldScrollbackLength) {
				this.setViewportY(Math.min(this.viewportOffset, newScrollbackLength));
			} else if (this.viewportOffset > 0 && newScrollbackLength > oldScrollbackLength) {
				this.viewportOffset = Math.min(
					newScrollbackLength,
					this.viewportOffset + newScrollbackLength - oldScrollbackLength,
				);
				this.targetViewportY = this.viewportOffset;
			}
			const alternateScreen = this.isAlternateScreen();
			if (alternateScreen !== this.lastAlternateScreen) {
				this.lastAlternateScreen = alternateScreen;
				this.inputHandler?.cancelComposition();
				this.selectionManager?.clearSelection();
				this.viewportOffset = 0;
				this.targetViewportY = 0;
				this.cancelSmoothScroll();
				this.buffer.emitBufferChange(this.buffer.active);
			}
			if (this.selectionController.clearSelectionIfInvalid()) {
				this.searchOwnsSelection = false;
				this.notifySelectionChange();
			}
		} finally {
			this.bridge.freeU8Array(ptr, bytes.length);
		}
		if (callback) {
			this.requestRender();
			if (typeof requestAnimationFrame === 'function') requestAnimationFrame(() => callback());
			else queueMicrotask(callback);
		} else {
			this.requestRender();
		}
	}

	/** Write data followed by CRLF. */
	writeln(data: string | Uint8Array, callback?: () => void): void {
		if (typeof data === 'string') {
			this.write(`${data}\r\n`, callback);
			return;
		}
		const line = new Uint8Array(data.length + 2);
		line.set(data);
		line[data.length] = 0x0d;
		line[data.length + 1] = 0x0a;
		this.write(line, callback);
	}

	/** Send paste data to the PTY-facing onData event. */
	paste(data: string): void {
		void this.pasteAsync(data);
	}

	async pasteAsync(data: string, source: PasteSource = 'api'): Promise<PasteResult> {
		return await this.inputDispatcher.dispatch(() => this.performPaste(data, source));
	}

	async dropAsync(text: string, files: readonly File[] = []): Promise<PasteResult> {
		return await this.inputDispatcher.dispatch(async () => {
			let resolvedText = text;
			if (files.length > 0) {
				if (!this.host.resolveDrop) return this.blockDrop('file-drop-unsupported');
				try {
					const resolution = await this.host.resolveDrop({
						text,
						files,
						cwd: this.getPwd() || null,
					});
					if (!resolution) return this.blockDrop('file-drop-rejected');
					resolvedText = resolution.text;
				} catch (error) {
					this.onHostErrorEmitter.fire({ operation: 'resolve-drop', error });
					return this.blockDrop('file-drop-failed');
				}
			}
			return await this.performPaste(resolvedText, 'drop');
		});
	}

	private blockDrop(reason: string): PasteResult {
		const inspection = inspectPaste(
			'',
			{
				bracketed: this.hasBracketedPaste(),
				protection: this.options.pasteProtection,
				bracketedSafe: this.options.pasteBracketedSafe,
			},
			this.bridge,
		);
		return { kind: 'blocked', reason, source: 'drop', inspection };
	}

	private async performPaste(data: string, source: PasteSource): Promise<PasteResult> {
		const inspection = inspectPaste(
			data,
			{
				bracketed: this.hasBracketedPaste(),
				protection: this.options.pasteProtection,
				bracketedSafe: this.options.pasteBracketedSafe,
			},
			this.bridge,
		);
		if (data.length === 0) return { kind: 'empty', source, inspection };
		if (this.options.disableStdin) {
			return { kind: 'blocked', reason: 'stdin-disabled', source, inspection };
		}
		if (inspection.classification === 'confirm') {
			if (!this.host.confirmPaste) {
				return {
					kind: 'blocked',
					reason: 'paste-confirmation-unsupported',
					source,
					inspection,
				};
			}
			let confirmed: boolean;
			try {
				confirmed = await this.host.confirmPaste({
					text: inspection.original,
					sanitized: inspection.sanitized,
					bracketed: inspection.bracketed,
					multiline: inspection.multiline,
					embeddedBracketTerminator: inspection.embeddedBracketTerminator,
					replacedControlBytes: inspection.replacedControlBytes,
				});
			} catch (error) {
				this.onHostErrorEmitter.fire({ operation: 'confirm-paste', error });
				return { kind: 'blocked', reason: 'paste-confirmation-failed', source, inspection };
			}
			if (!confirmed) return { kind: 'cancelled', source, inspection };
		}

		if (this.options.disableStdin) {
			return { kind: 'blocked', reason: 'stdin-disabled', source, inspection };
		}
		if (this.handle === 0) {
			return { kind: 'blocked', reason: 'terminal-disposed', source, inspection };
		}
		const encoded = encodePaste(inspection, this.bridge);
		this.emitUserInputNow(encoded);
		return {
			kind: 'emitted',
			bytes: new TextEncoder().encode(encoded).length,
			source,
			inspection,
		};
	}

	/** Input data as user input or terminal output. */
	input(data: string, wasUserInput = false): void {
		if (wasUserInput) this.emitUserInput(data);
		else this.write(data);
	}

	/** Clear the visible screen and move the cursor home. */
	clear(): void {
		this.write('\x1b[2J\x1b[H');
	}

	/** Reset Ghostty's terminal state. */
	reset(): void {
		this.inputHandler?.cancelComposition();
		this.inputHandler?.releasePressedKeys();
		this.inputHandler?.releasePressedMouseButtons();
		this.selectionManager?.clearSelection();
		this.clearHoveredLink();
		this.clearSearch();
		this.bridge.getExports().ghostty_terminal_reset(this.handle);
		this.syncSurfaceMouseOwnership();
		const alternateScreen = this.isAlternateScreen();
		if (alternateScreen !== this.lastAlternateScreen) {
			this.lastAlternateScreen = alternateScreen;
			this.buffer.emitBufferChange(this.buffer.active);
		}
		this.viewportOffset = 0;
		this.targetViewportY = 0;
		this.cancelSmoothScroll();
		this.linkDetector?.invalidateCache();
		this.requestRender();
	}

	focus(): void {
		(this.textAreaElement ?? this.hostElement)?.focus();
	}

	blur(): void {
		(this.textAreaElement ?? this.hostElement)?.blur();
	}

	/**
	 * Resize the terminal grid.
	 *
	 * @param cols New column count.
	 * @param rows New row count.
	 * @param cellWidthPx Width of a single cell in pixels (for image protocols/size reports).
	 * @param cellHeightPx Height of a single cell in pixels.
	 */
	resize(cols: number, rows: number, cellWidthPx = 8, cellHeightPx = 16): void {
		if (cols === this.columnCount && rows === this.rowCount) return;
		const previousViewportOffset = this.viewportOffset;
		const viewportAnchor = createViewportResizeAnchor(
			this.bridge,
			this.handle,
			this.viewportOffset,
			this.grid.getScrollbackLength(),
		);
		try {
			this.columnCount = cols;
			this.rowCount = rows;
			this.optionState.cols = cols;
			this.optionState.rows = rows;

			this.bridge
				.getExports()
				.ghostty_terminal_resize(this.handle, cols, rows, cellWidthPx, cellHeightPx);
			this.grid.resize(cols, rows);
			this.linkDetector?.invalidateCache();
			this.clearHoveredLink();
			this.invalidateSearchAfterBufferChange();
			const scrollbackLength = this.grid.getScrollbackLength();
			const anchoredRow = viewportAnchor?.resolveScreenRow();
			this.viewportOffset = Math.max(
				0,
				Math.min(
					scrollbackLength,
					anchoredRow === undefined ? this.viewportOffset : scrollbackLength - anchoredRow,
				),
			);
			this.targetViewportY = this.viewportOffset;
			this.cancelSmoothScroll();
			if (this.viewportOffset !== previousViewportOffset) {
				this.onScrollEmitter.fire(this.viewportOffset);
			}
			this.renderer?.resize(cols, rows);
			this.requestRender();
			this.onResizeEmitter.fire({ cols, rows });
		} finally {
			viewportAnchor?.dispose();
		}
	}

	/** Free all WASM resources. Terminal is unusable after dispose(). */
	dispose(): void {
		if (this.handle === 0) return;

		this.stopRenderLoop();
		this.cancelScheduledSearch();
		if (this.scrollbarFadeTimer) clearTimeout(this.scrollbarFadeTimer);
		this.inputHandler?.dispose();
		this.selectionManager?.dispose();
		this.linkDetector?.dispose();
		this.renderer?.dispose();
		this.inputHandler = undefined;
		this.selectionManager = undefined;
		this.linkDetector = undefined;
		this.renderer = undefined;
		for (const addon of this.addons) addon.dispose();
		this.addons.clear();
		this.actions.dispose();
		this.contextMenu.dispose();
		this.keyEvents.dispose();
		this.contextMenuRequestId++;

		if (this.canvas) {
			this.canvas.removeEventListener('wheel', this.handleCanvasWheel);
			this.canvas.removeEventListener('mousemove', this.handleCanvasMouseMove);
			this.canvas.removeEventListener('mouseleave', this.handleCanvasMouseLeave);
			this.canvas.removeEventListener('click', this.handleCanvasClick);
			this.canvas.removeEventListener('contextmenu', this.handleCanvasContextMenu);
			this.canvas.removeEventListener('mousedown', this.handleInputFocusMouseDown);
			this.canvas.removeEventListener('touchend', this.handleInputFocusTouchEnd);
			this.canvas.removeEventListener('mousedown', this.handleScrollbarMouseDown, true);
			document.removeEventListener('mousemove', this.handleScrollbarMouseMove);
			document.removeEventListener('mouseup', this.handleScrollbarMouseUp);
			this.canvas.remove();
		}
		this.accessibility?.dispose();
		this.accessibility = undefined;
		this.hostElement?.removeEventListener('focusin', this.handleFocusIn);
		this.hostElement?.removeEventListener('focusout', this.handleFocusOut);
		this.hostElement?.removeEventListener('dragover', this.handleDragOver);
		this.hostElement?.removeEventListener('drop', this.handleDrop);
		if (this.visibilityListenerAttached) {
			document.removeEventListener('visibilitychange', this.handleVisibilityChange);
			this.visibilityListenerAttached = false;
		}
		this.detachDevicePixelRatioListeners();
		this.textAreaElement?.remove();
		if (this.hostElement && this.originalElementPosition !== undefined) {
			this.hostElement.style.position = this.originalElementPosition;
		}
		this.canvas = undefined;
		this.textAreaElement = undefined;
		this.hostElement = undefined;
		this.originalElementPosition = undefined;

		this.grid.dispose();
		this.selectionController.dispose();
		this.bridge.getExports().ghostty_terminal_set(this.handle, TerminalOption.WRITE_PTY, 0);
		this.bridge.getExports().ghostty_terminal_set(this.handle, TerminalOption.BELL, 0);
		this.bridge.getExports().ghostty_terminal_set(this.handle, TerminalOption.TITLE_CHANGED, 0);
		this.bridge.getExports().ghostty_terminal_set(this.handle, TerminalOption.PWD_CHANGED, 0);
		this.bridge.getExports().ghostty_terminal_set(this.handle, TerminalOption.SIZE, 0);
		this.bridge.getExports().ghostty_terminal_set(this.handle, TerminalOption.COLOR_SCHEME, 0);
		this.bridge.getExports().ghostty_terminal_set(this.handle, TerminalOption.XTVERSION, 0);
		this.bridge.getExports().ghostty_terminal_set(this.handle, TerminalOption.ENQUIRY, 0);
		this.writePtyCallback?.dispose();
		this.bellCallback?.dispose();
		this.titleCallback?.dispose();
		this.pwdCallback?.dispose();
		this.sizeCallback?.dispose();
		this.colorSchemeCallback?.dispose();
		this.xtversionCallback?.dispose();
		this.enquiryCallback?.dispose();
		this.writePtyCallback = undefined;
		this.bellCallback = undefined;
		this.titleCallback = undefined;
		this.pwdCallback = undefined;
		this.sizeCallback = undefined;
		this.colorSchemeCallback = undefined;
		this.xtversionCallback = undefined;
		this.enquiryCallback = undefined;
		this.bridge.freeU8Array(this.identityBuffer, this.identityLength);
		if (this.enquiryLength > 0) {
			this.bridge.freeU8Array(this.enquiryBuffer, this.enquiryLength);
		}
		this.bridge.getExports().ghostty_render_state_free(this.rsHandle);
		this.bridge.getExports().ghostty_terminal_free(this.handle);
		this.handle = 0;
		this.rsHandle = 0;
		this.disposeEvents();
	}

	// ==========================================================================
	// DOM Lifecycle (open / close)
	// ==========================================================================

	renderer: WebGPURenderer | undefined;
	private inputHandler: InputHandler | undefined;
	private selectionManager: SelectionManager | undefined;
	private linkDetector: LinkDetector | undefined;
	private readonly ghosttyTerm: GhosttyTerminal;
	private canvas: HTMLCanvasElement | undefined;
	private textAreaElement: HTMLTextAreaElement | undefined;
	private renderLoopId = 0;
	private viewportOffset = 0;
	private hostElement: HTMLElement | undefined;
	private opening = false;
	private originalElementPosition: string | undefined;
	private preeditCellWidth = 0;
	private preeditCaretPixelOffset = 0;
	private hoveredLink: ILink | undefined;
	private hoveredLinkEnabled = false;
	private searchQuery = '';
	private searchOptions: TerminalSearchOptions = {};
	private searchTimer: ReturnType<typeof setTimeout> | undefined;
	private searchPendingIncremental = false;
	private searchOwnsSelection = false;
	private mutatingSearchSelection = false;
	private linkLookupId = 0;
	private surfaceModifiers: TerminalModifierState = EMPTY_MODIFIERS;
	private surfaceMouseOwnedByApplication = false;
	private readonly surfaceMousePlatform = classifySurfaceMousePlatform(
		globalThis.navigator?.platform ?? '',
	);
	private draggingScrollbar = false;
	private scrollbarDragStartY = 0;
	private scrollbarDragStartViewportY = 0;

	// Scrollbar state
	private scrollbarOpacity = 0;
	private scrollbarFadeTimer: ReturnType<typeof setTimeout> | undefined;

	// Events
	private readonly onDataEmitter = new EventEmitter<string>();
	private readonly onBinaryEmitter = new EventEmitter<string>();
	private readonly onResizeEmitter = new EventEmitter<{ cols: number; rows: number }>();
	private readonly onBellEmitter = new EventEmitter<void>();
	private readonly onSelectionChangeEmitter = new EventEmitter<void>();
	private readonly onKeyEmitter = new EventEmitter<IKeyEvent>();
	private readonly onTitleChangeEmitter = new EventEmitter<string>();
	private readonly onPwdChangeEmitter = new EventEmitter<string>();
	private readonly onScrollEmitter = new EventEmitter<number>();
	private readonly onRenderEmitter = new EventEmitter<void>();
	private readonly onCursorMoveEmitter = new EventEmitter<{ x: number; y: number }>();
	private readonly onHostErrorEmitter = new EventEmitter<TerminalHostErrorEvent>();
	private readonly onRendererHealthEmitter = new EventEmitter<RendererHealthEvent>();
	private readonly onRendererTelemetryEmitter = new EventEmitter<RendererFrameTelemetry>();
	private readonly onSearchResultsEmitter = new EventEmitter<TerminalSearchResultChangeEvent>();

	/** Open the terminal in a parent DOM element. */
	async open(parent: HTMLElement, options?: ITerminalOptions): Promise<void> {
		if (this.handle === 0) throw new Error('Cannot open a disposed terminal');
		if (this.hostElement || this.opening) throw new Error('Terminal is already open');
		this.opening = true;
		try {
			await this.openInternal(parent, options);
		} catch (error) {
			this.rollbackOpen();
			throw error;
		} finally {
			this.opening = false;
		}
	}

	private async openInternal(parent: HTMLElement, options?: ITerminalOptions): Promise<void> {
		if (options) {
			const cols = options.cols ?? this.columnCount;
			const rows = options.rows ?? this.rowCount;
			if (cols !== this.columnCount || rows !== this.rowCount) this.resize(cols, rows);
			if (options.cursorBlink !== undefined) this.options.cursorBlink = options.cursorBlink;
			if (options.cursorStyle !== undefined) this.options.cursorStyle = options.cursorStyle;
			if (options.fontSize !== undefined) this.options.fontSize = options.fontSize;
			if (options.fontFamily !== undefined) this.options.fontFamily = options.fontFamily;
			if (options.fontWeight !== undefined) this.options.fontWeight = options.fontWeight;
			if (options.fontWeightBold !== undefined)
				this.options.fontWeightBold = options.fontWeightBold;
			if (options.fontLigatures !== undefined) this.options.fontLigatures = options.fontLigatures;
			if (options.fontMetricModifiers !== undefined)
				this.options.fontMetricModifiers = options.fontMetricModifiers;
			if (options.paddingBalance !== undefined)
				this.options.paddingBalance = options.paddingBalance;
			if (options.allowTransparency !== undefined)
				this.options.allowTransparency = options.allowTransparency;
			if (options.convertEol !== undefined) this.options.convertEol = options.convertEol;
			if (options.disableStdin !== undefined) this.options.disableStdin = options.disableStdin;
			if (options.smoothScrollDuration !== undefined)
				this.options.smoothScrollDuration = options.smoothScrollDuration;
			if (options.copyOnSelect !== undefined) this.options.copyOnSelect = options.copyOnSelect;
			if (options.pasteProtection !== undefined)
				this.options.pasteProtection = options.pasteProtection;
			if (options.pasteBracketedSafe !== undefined)
				this.options.pasteBracketedSafe = options.pasteBracketedSafe;
			if (options.theme) this.options.theme = { ...this.options.theme, ...options.theme };
		}
		await this.loadConfiguredFont();
		if (this.handle === 0) throw new Error('Cannot open a disposed terminal');
		this.hostElement = parent;
		if (window.getComputedStyle(parent).position === 'static') {
			this.originalElementPosition = parent.style.position;
			parent.style.position = 'relative';
		}

		// Create canvas
		const canvas = document.createElement('canvas');
		canvas.style.display = 'block';
		parent.appendChild(canvas);
		this.canvas = canvas;

		// Create hidden textarea for IME/mobile keyboard
		const textarea = document.createElement('textarea');
		textarea.setAttribute('autocorrect', 'off');
		textarea.setAttribute('autocapitalize', 'off');
		textarea.setAttribute('spellcheck', 'false');
		textarea.setAttribute('tabindex', '0');
		textarea.setAttribute('aria-label', 'Terminal input');
		textarea.style.position = 'absolute';
		textarea.style.left = '0';
		textarea.style.top = '0';
		textarea.style.width = '1px';
		textarea.style.height = '1px';
		textarea.style.padding = '0';
		textarea.style.border = 'none';
		textarea.style.margin = '0';
		textarea.style.opacity = '0';
		textarea.style.clipPath = 'inset(50%)';
		textarea.style.overflow = 'hidden';
		textarea.style.whiteSpace = 'nowrap';
		textarea.style.resize = 'none';
		textarea.style.color = 'transparent';
		textarea.style.background = 'transparent';
		textarea.style.caretColor = 'transparent';
		textarea.style.fontFamily = this.options.fontFamily;
		textarea.style.fontSize = `${this.options.fontSize}px`;
		textarea.style.fontWeight = String(this.options.fontWeight);
		parent.appendChild(textarea);
		this.textAreaElement = textarea;
		canvas.setAttribute('aria-hidden', 'true');
		this.accessibility = new TerminalAccessibility(parent, this.rowCount, {
			onSelectionChange: (range) => this.handleAccessibleSelection(range),
		});

		// Create renderer
		const resolvedOptions = this.options;
		const theme = this.options.theme;
		const rendererOpts: RendererOptions = {};
		rendererOpts.fontSize = resolvedOptions.fontSize;
		rendererOpts.fontFamily = resolvedOptions.fontFamily;
		rendererOpts.fontWeight = resolvedOptions.fontWeight;
		rendererOpts.fontWeightBold = resolvedOptions.fontWeightBold;
		rendererOpts.fontLigatures = resolvedOptions.fontLigatures;
		rendererOpts.fontMetricModifiers = resolvedOptions.fontMetricModifiers;
		rendererOpts.paddingBalance = resolvedOptions.paddingBalance;
		rendererOpts.cursorStyle = resolvedOptions.cursorStyle;
		rendererOpts.cursorBlink = resolvedOptions.cursorBlink;
		rendererOpts.theme = theme;
		rendererOpts.devicePixelRatio = window.devicePixelRatio ?? 1;
		rendererOpts.allowTransparency = resolvedOptions.allowTransparency;
		this.renderer = new WebGPURenderer(canvas, rendererOpts);
		this.renderer.setRenderRequestCallback(() => this.requestRender());
		this.renderer.setHealthChangeCallback((event) => this.onRendererHealthEmitter.fire(event));
		this.renderer.setFrameTelemetryCallback((telemetry) =>
			this.onRendererTelemetryEmitter.fire(telemetry),
		);
		await this.renderer.initGPU();
		if (this.handle === 0) throw new Error('Cannot open a disposed terminal');

		const resolvedTheme = { ...DEFAULT_THEME, ...theme };
		this.setForegroundColor(parseCssColor(resolvedTheme.foreground));
		this.setBackgroundColor(parseCssColor(resolvedTheme.background));
		this.setCursorColor(parseCssColor(resolvedTheme.cursor));
		this.setDefaultCursorStyle(resolvedOptions.cursorStyle);
		this.setDefaultCursorBlink(resolvedOptions.cursorBlink);

		// Resize canvas to terminal dimensions
		this.renderer.resize(this.columnCount, this.rowCount);
		this.updateInputPosition();

		// Create selection manager
		this.selectionManager = new SelectionManager(this.createSelectionTerminal(), canvas, {
			...(this.host.clipboard && this.capabilities.clipboardWrite
				? { clipboard: this.host.clipboard }
				: {}),
			copyOnSelect: this.options.copyOnSelect,
			onCopyError: (error) => this.onHostErrorEmitter.fire({ operation: 'clipboard-write', error }),
			selectionModel: this.selectionController,
		});
		this.selectionManager.onSelectionChange(() => {
			if (!this.mutatingSearchSelection) this.searchOwnsSelection = false;
			this.notifySelectionChange();
		});
		this.renderer.setSelectionManager(this.selectionManager);

		// Create input handler
		this.inputHandler = new InputHandler(
			this.createInputDeps(),
			parent,
			(data: string) => this.emitUserInput(data),
			() => this.onBellEmitter.fire(),
			(keyEvent: IKeyEvent) => this.onKeyEmitter.fire(keyEvent),
			(event) => this.dispatchCustomKeyEvent(event),
			() => {
				return this.selectionManager?.copySelection() ?? false;
			},
			textarea,
			(data, selection) => this.handleComposition(data, selection),
			(data, source) => {
				void this.pasteAsync(data, source);
			},
			(id) => this.dispatchKeyboardAction(id),
			(error) => this.onHostErrorEmitter.fire({ operation: 'input-encode', error }),
			(data) => this.emitStateRepairInput(data),
		);

		// Create link detector + providers
		this.linkDetector = new LinkDetector(this.createLinkDetectorTerminal());
		this.linkDetector.registerProvider(new OSC8LinkProvider(this.createOSC8Terminal()));
		this.linkDetector.registerProvider(new UrlRegexProvider(this.createUrlRegexTerminal()));
		this.linkDetector.registerProvider(new PathLinkProvider(this.createPathProviderTerminal()));

		// Wire mouse events for scrollbar + link hover
		canvas.addEventListener('wheel', this.handleCanvasWheel, { passive: false });
		canvas.addEventListener('mousemove', this.handleCanvasMouseMove);
		canvas.addEventListener('mouseleave', this.handleCanvasMouseLeave);
		canvas.addEventListener('click', this.handleCanvasClick);
		canvas.addEventListener('contextmenu', this.handleCanvasContextMenu);
		canvas.addEventListener('mousedown', this.handleInputFocusMouseDown);
		canvas.addEventListener('touchend', this.handleInputFocusTouchEnd, { passive: false });
		canvas.addEventListener('mousedown', this.handleScrollbarMouseDown, true);
		document.addEventListener('mousemove', this.handleScrollbarMouseMove);
		document.addEventListener('mouseup', this.handleScrollbarMouseUp);
		parent.addEventListener('focusin', this.handleFocusIn);
		parent.addEventListener('focusout', this.handleFocusOut);
		this.renderer.setFocused(parent.contains(document.activeElement));
		parent.addEventListener('dragover', this.handleDragOver);
		parent.addEventListener('drop', this.handleDrop);
		document.addEventListener('visibilitychange', this.handleVisibilityChange);
		this.visibilityListenerAttached = true;
		window.addEventListener('resize', this.handleWindowResize);
		this.windowResizeListenerAttached = true;
		this.watchDevicePixelRatio();
		this.updateSurfaceMouseCursor();

		// Start render loop
		this.startRenderLoop();
	}

	private rollbackOpen(): void {
		this.stopRenderLoop();
		if (this.scrollbarFadeTimer) clearTimeout(this.scrollbarFadeTimer);
		this.scrollbarFadeTimer = undefined;
		this.inputHandler?.dispose();
		this.selectionManager?.dispose();
		this.linkDetector?.dispose();
		this.renderer?.dispose();
		this.inputHandler = undefined;
		this.selectionManager = undefined;
		this.linkDetector = undefined;
		this.renderer = undefined;
		this.accessibility?.dispose();
		this.accessibility = undefined;

		const canvas = this.canvas;
		if (canvas) {
			canvas.removeEventListener('wheel', this.handleCanvasWheel);
			canvas.removeEventListener('mousemove', this.handleCanvasMouseMove);
			canvas.removeEventListener('mouseleave', this.handleCanvasMouseLeave);
			canvas.removeEventListener('click', this.handleCanvasClick);
			canvas.removeEventListener('contextmenu', this.handleCanvasContextMenu);
			canvas.removeEventListener('mousedown', this.handleInputFocusMouseDown);
			canvas.removeEventListener('touchend', this.handleInputFocusTouchEnd);
			canvas.removeEventListener('mousedown', this.handleScrollbarMouseDown, true);
			canvas.remove();
		}
		document.removeEventListener('mousemove', this.handleScrollbarMouseMove);
		document.removeEventListener('mouseup', this.handleScrollbarMouseUp);
		if (this.visibilityListenerAttached) {
			document.removeEventListener('visibilitychange', this.handleVisibilityChange);
			this.visibilityListenerAttached = false;
		}
		this.detachDevicePixelRatioListeners();
		this.hostElement?.removeEventListener('focusin', this.handleFocusIn);
		this.hostElement?.removeEventListener('focusout', this.handleFocusOut);
		this.hostElement?.removeEventListener('dragover', this.handleDragOver);
		this.hostElement?.removeEventListener('drop', this.handleDrop);
		this.textAreaElement?.remove();
		if (this.hostElement && this.originalElementPosition !== undefined) {
			this.hostElement.style.position = this.originalElementPosition;
		}
		this.canvas = undefined;
		this.textAreaElement = undefined;
		this.hostElement = undefined;
		this.originalElementPosition = undefined;
		this.renderedDefaultColorsKey = '';
	}

	/** Get the terminal's DOM element. */
	get element(): HTMLElement | undefined {
		return this.hostElement;
	}

	/** Get the hidden textarea element (for IME/mobile keyboard). */
	get textarea(): HTMLTextAreaElement | undefined {
		return this.textAreaElement;
	}

	getAccessibleText(): string {
		return this.accessibility?.getText() ?? '';
	}

	getAccessibleVisibleRange(): AccessibleOffsetRange {
		return this.accessibility?.getVisibleRange() ?? { start: 0, end: 0 };
	}

	getAccessibleSelection(): AccessibleOffsetRange | null {
		return this.accessibility?.getSelectionOffsets() ?? null;
	}

	selectAccessibleRange(range: AccessibleOffsetRange): boolean {
		return this.accessibility?.selectOffsets(range) ?? false;
	}

	get wasmTerm(): GhosttyTerminal {
		return this.ghosttyTerm;
	}

	// ==========================================================================
	// Events
	// ==========================================================================

	get onData(): IEvent<string> {
		return this.onDataEmitter.event;
	}
	get onBinary(): IEvent<string> {
		return this.onBinaryEmitter.event;
	}
	get onResize(): IEvent<{ cols: number; rows: number }> {
		return this.onResizeEmitter.event;
	}
	get onBell(): IEvent<void> {
		return this.onBellEmitter.event;
	}
	get onSelectionChange(): IEvent<void> {
		return this.onSelectionChangeEmitter.event;
	}
	get onKey(): IEvent<IKeyEvent> {
		return this.onKeyEmitter.event;
	}
	get onTitleChange(): IEvent<string> {
		return this.onTitleChangeEmitter.event;
	}
	get onPwdChange(): IEvent<string> {
		return this.onPwdChangeEmitter.event;
	}
	get onScroll(): IEvent<number> {
		return this.onScrollEmitter.event;
	}
	get onRender(): IEvent<void> {
		return this.onRenderEmitter.event;
	}
	get onCursorMove(): IEvent<{ x: number; y: number }> {
		return this.onCursorMoveEmitter.event;
	}
	get onHostError(): IEvent<TerminalHostErrorEvent> {
		return this.onHostErrorEmitter.event;
	}
	get onRendererHealth(): IEvent<RendererHealthEvent> {
		return this.onRendererHealthEmitter.event;
	}
	get onRendererTelemetry(): IEvent<RendererFrameTelemetry> {
		return this.onRendererTelemetryEmitter.event;
	}
	get onSearchResults(): IEvent<TerminalSearchResultChangeEvent> {
		return this.onSearchResultsEmitter.event;
	}

	getRendererTelemetry(): RendererFrameTelemetry | null {
		return this.renderer?.getLastFrameTelemetry() ?? null;
	}

	executeAction(
		id: string,
		source: TerminalActionSource = 'api',
		signal: AbortSignal = new AbortController().signal,
	): Promise<TerminalActionResult> {
		return this.actions.execute(id, { source, signal });
	}

	// ==========================================================================
	// Selection API (delegates to SelectionManager)
	// ==========================================================================

	getSelection(): string {
		return this.selectionManager?.getSelection() ?? '';
	}
	getSelectionPayload(): ClipboardPayload {
		return this.selectionController.getClipboardPayload();
	}
	hasSelection(): boolean {
		return this.selectionManager?.hasSelection() ?? false;
	}
	clearSelection(): void {
		this.searchOwnsSelection = false;
		this.selectionManager?.clearSelection();
	}
	copySelection(): boolean {
		return this.selectionManager?.copySelection() ?? false;
	}
	async copySelectionAsync(): Promise<SelectionCopyResult> {
		return (await this.selectionManager?.copySelectionAsync()) ?? { kind: 'empty' };
	}
	selectAll(): void {
		this.selectionManager?.selectAll();
	}
	selectOutputAt(column: number, row: number): boolean {
		return this.selectionManager?.selectOutputAt(column, row) ?? false;
	}
	adjustSelection(adjustment: GhosttySelectionAdjustment): boolean {
		return this.selectionManager?.adjustSelection(adjustment) ?? false;
	}
	select(column: number, row: number, length: number): void {
		this.selectionManager?.select(column, row, length);
	}
	selectLines(start: number, end: number): void {
		this.selectionManager?.selectLines(start, end);
	}
	getSelectionPosition(): IBufferRange | null {
		return this.selectionManager?.getSelectionPosition() ?? null;
	}

	findAll(query: string, options: TerminalSearchOptions = {}): readonly TerminalSearchMatch[] {
		const snapshot = this.captureSearchSnapshot();
		return findTerminalMatches(snapshot.lines, query, options, snapshot.wrappedRows);
	}

	findNext(query: string, options: TerminalSearchOptions = {}): boolean {
		this.cancelScheduledSearch();
		const snapshot = this.captureSearchSnapshot();
		const matches = findTerminalMatches(snapshot.lines, query, options, snapshot.wrappedRows);
		const match = this.findDirectionalSearchMatch(snapshot, query, options, false, false);
		return this.selectSearchMatch(matches, match, query, options, true);
	}

	/**
	 * Schedule a trailing-edge incremental search. Repeated calls within 200 ms collapse into the
	 * latest query; navigation methods always execute immediately.
	 */
	updateSearch(query: string, options: TerminalSearchOptions = {}): void {
		if (!query) {
			this.clearOwnedSearchSelection();
			this.clearSearch();
			return;
		}
		this.scheduleSearch(query, options, true);
	}

	highlightSearch(
		query: string,
		options: TerminalSearchOptions = {},
	): readonly TerminalSearchMatch[] {
		this.cancelScheduledSearch();
		const matches = this.findAll(query, options);
		this.applySearchResults(query, options, matches, null);
		return matches;
	}

	clearSearch(): void {
		this.cancelScheduledSearch();
		this.searchQuery = '';
		this.searchOptions = {};
		this.searchPendingIncremental = false;
		this.renderer?.setSearchMatches([]);
		this.onSearchResultsEmitter.fire({
			query: '',
			regexError: null,
			resultCount: 0,
			resultIndex: -1,
			resultLimitReached: false,
		});
	}

	findPrevious(query: string, options: TerminalSearchOptions = {}): boolean {
		this.cancelScheduledSearch();
		const snapshot = this.captureSearchSnapshot();
		const matches = findTerminalMatches(snapshot.lines, query, options, snapshot.wrappedRows);
		const match = this.findDirectionalSearchMatch(snapshot, query, options, true, false);
		return this.selectSearchMatch(matches, match, query, options, true);
	}

	// ==========================================================================
	// Scroll API
	// ==========================================================================

	get viewportY(): number {
		return this.viewportOffset;
	}

	getViewportY(): number {
		return this.viewportOffset;
	}

	scrollLines(amount: number): void {
		this.setViewportY(this.viewportOffset - amount);
	}

	scrollPages(amount: number): void {
		this.scrollLines(amount * this.rowCount);
	}

	scrollToTop(): void {
		this.setViewportY(this.grid.getScrollbackLength());
	}
	scrollToBottom(): void {
		this.setViewportY(0);
	}
	scrollToLine(line: number): void {
		const scrollbackLength = this.grid.getScrollbackLength();
		this.setViewportY(scrollbackLength - line);
	}

	private setViewportY(value: number): void {
		this.cancelSmoothScroll();
		const next = Math.max(0, Math.min(value, this.grid.getScrollbackLength()));
		if (next === this.viewportOffset) {
			this.targetViewportY = next;
			return;
		}
		this.clearHoveredLink();
		this.viewportOffset = next;
		this.targetViewportY = this.viewportOffset;
		this.onScrollEmitter.fire(this.viewportOffset);
		this.requestRender();
	}

	// ==========================================================================
	// Buffer Data
	// ==========================================================================

	getScrollbackLength(): number {
		return this.grid.getScrollbackLength();
	}

	getLine(y: number): CellData[] | null {
		this.ensureSnapshot();
		return this.ghosttyTerm.getCellDataLine(y);
	}

	getScrollbackLine(offset: number): CellData[] | null {
		this.ensureSnapshot();
		return this.ghosttyTerm.getScrollbackCellData(offset);
	}

	isRowWrapped(row: number): boolean {
		this.ensureSnapshot();
		return this.ghosttyTerm.isRowWrapped(row);
	}

	isScrollbackRowWrapped(offset: number): boolean {
		this.ensureSnapshot();
		return this.ghosttyTerm.isScrollbackRowWrapped(offset);
	}

	// ==========================================================================
	// Addon System
	// ==========================================================================

	loadAddon(addon: ITerminalAddon): void {
		addon.activate(this);
		this.addons.add(addon);
	}

	attachCustomKeyEventHandler(handler: (event: KeyboardEvent) => boolean): void {
		this.customKeyEventHandler = handler;
		this.inputHandler?.setCustomKeyEventHandler((event) => this.dispatchCustomKeyEvent(event));
	}

	registerKeyEventHandler(handler: TerminalKeyEventHandler): IDisposable {
		return this.keyEvents.register(handler);
	}

	attachCustomWheelEventHandler(handler?: (event: WheelEvent) => boolean): void {
		this.customWheelEventHandler = handler;
	}

	registerLinkProvider(provider: ILinkProvider): void {
		if (!this.linkDetector) throw new Error('Terminal must be opened before registering links');
		this.linkDetector.registerProvider(provider);
	}

	registerContextMenuProvider(provider: ContextMenuProvider): IDisposable {
		return this.contextMenu.register(provider);
	}

	async requestContextMenuAt(
		x: number,
		y: number,
		modifiers: TerminalModifierState = EMPTY_MODIFIERS,
	): Promise<TerminalActionResult | null> {
		const showContextMenu = this.host.showContextMenu;
		if (!showContextMenu || !this.capabilities.contextMenu || !this.canvas) return null;
		const requestId = ++this.contextMenuRequestId;
		const interaction = this.createInteractionContext(x, y, modifiers);
		const items = this.contextMenu.provide(
			interaction,
			this.createBaseContextMenuItems(interaction),
		);
		const actionIds = collectContextMenuActionIds(items);
		const actionContext = {
			source: 'context-menu' as const,
			signal: new AbortController().signal,
			interaction,
		};
		const actions = actionIds.flatMap((id) => {
			const definition = this.actions.get(id);
			const state = this.actions.getState(id, actionContext);
			return definition && state
				? [{ id, title: definition.title, enabled: state.enabled, checked: state.checked }]
				: [];
		});

		let selectedAction: string | null;
		try {
			selectedAction = await showContextMenu({
				x,
				y,
				actionIds,
				items,
				actions,
				context: interaction,
			});
		} catch (error) {
			this.onHostErrorEmitter.fire({ operation: 'context-menu', error });
			return { kind: 'blocked', reason: 'context-menu-failed' };
		}
		if (requestId !== this.contextMenuRequestId || !selectedAction) return null;
		return await this.actions.execute(selectedAction, actionContext);
	}

	// ==========================================================================
	// Render Loop
	// ==========================================================================

	private startRenderLoop(): void {
		this.requestRender();
	}

	private notifySelectionChange(): void {
		this.onSelectionChangeEmitter.fire();
		this.actions.notifyStateChange('terminal.copy');
		this.actions.notifyStateChange('terminal.clear-selection');
	}

	private requestRender(): void {
		if (!this.renderer || this.handle === 0 || this.renderLoopId !== 0) return;
		this.renderLoopId = requestAnimationFrame(() => {
			this.renderLoopId = 0;
			if (this.handle !== 0) this.renderFrame();
		});
	}

	private stopRenderLoop(): void {
		if (this.renderLoopId !== 0) {
			cancelAnimationFrame(this.renderLoopId);
			this.renderLoopId = 0;
		}
		this.cancelSmoothScroll();
	}

	private renderFrame(): void {
		if (!this.renderer) return;

		this.renderer.setDevicePixelRatio(window.devicePixelRatio ?? 1);

		const hadFrame = this.ghosttyTerm.hasFrame;
		const dirty = this.grid.update();
		if (dirty !== 0 || !this.ghosttyTerm.hasFrame) {
			this.ghosttyTerm.refresh(dirty);
			this.emitSnapshotEvents();
		}
		this.syncEffectiveRendererColors();
		this.renderer.render(
			this.ghosttyTerm,
			!hadFrame || dirty === DirtyState.FULL,
			this.viewportOffset,
			this.ghosttyTerm,
			this.scrollbarOpacity,
		);
		this.updateAccessibility();
		this.updateInputPosition();
		this.onRenderEmitter.fire();
	}

	private ensureSnapshot(): void {
		const dirty = this.grid.update();
		if (dirty !== 0 || !this.ghosttyTerm.hasFrame) {
			this.ghosttyTerm.refresh(dirty);
			this.emitSnapshotEvents();
		}
	}

	private updateAccessibility(): void {
		if (!this.accessibility) return;
		const offset = Math.floor(this.viewportOffset);
		const scrollbackLength = this.grid.getScrollbackLength();
		const rows: AccessibleRow[] = [];
		for (let row = 0; row < this.rowCount; row++) {
			const sourceRow = row - offset;
			const cells =
				sourceRow < 0
					? this.ghosttyTerm.getScrollbackCellData(scrollbackLength + sourceRow)
					: this.ghosttyTerm.getCellDataLine(sourceRow);
			rows.push(buildAccessibleRow(cells ?? [], this.columnCount));
		}
		const cursor = this.ghosttyTerm.getCursor();
		const selection = this.selectionManager?.getSelectionCoords();
		const accessibleSelection =
			selection &&
			selection.startRow >= 0 &&
			selection.startRow < this.rowCount &&
			selection.endRow >= 0 &&
			selection.endRow < this.rowCount
				? {
						start: { row: selection.startRow, column: selection.startCol },
						end: { row: selection.endRow, column: selection.endCol },
					}
				: undefined;
		this.accessibility.update(
			rows,
			{ column: cursor.viewportX, row: cursor.viewportY + offset },
			this.getSelection(),
			accessibleSelection,
		);
	}

	private handleAccessibleSelection(range: AccessibleTextRange | null): void {
		if (!this.selectionManager) return;
		if (!range || (range.start.row === range.end.row && range.start.column === range.end.column)) {
			this.selectionManager.clearSelection();
			return;
		}

		const top = this.grid.getScrollbackLength() - Math.floor(this.viewportOffset);
		const start = {
			x: Math.min(this.columnCount - 1, range.start.column),
			y: top + range.start.row,
		};
		const end =
			range.end.column === 0 && range.end.row > range.start.row
				? { x: this.columnCount - 1, y: top + range.end.row - 1 }
				: {
						x: Math.max(0, Math.min(this.columnCount - 1, range.end.column - 1)),
						y: top + range.end.row,
					};
		this.selectionManager.selectAbsoluteRange(start, end);
	}

	private scheduleSearch(
		query: string,
		options: TerminalSearchOptions,
		incremental: boolean,
	): void {
		this.cancelScheduledSearch();
		this.searchQuery = query;
		this.searchOptions = { ...options };
		this.searchPendingIncremental = incremental;
		this.searchTimer = setTimeout(() => {
			this.searchTimer = undefined;
			this.searchPendingIncremental = false;
			if (this.handle === 0 || this.searchQuery !== query) return;
			const snapshot = this.captureSearchSnapshot();
			const matches = findTerminalMatches(
				snapshot.lines,
				query,
				this.searchOptions,
				snapshot.wrappedRows,
			);
			if (incremental || this.searchOwnsSelection) {
				const match = this.findDirectionalSearchMatch(
					snapshot,
					query,
					this.searchOptions,
					true,
					true,
				);
				this.selectSearchMatch(matches, match, query, this.searchOptions, incremental);
				return;
			}
			this.applySearchResults(query, this.searchOptions, matches, null);
		}, TERMINAL_SEARCH_DEBOUNCE_MS);
	}

	private captureSearchSnapshot(): TerminalSearchSnapshot {
		this.ensureSnapshot();
		const lines: CellData[][] = [];
		const wrappedRows: boolean[] = [];
		const scrollbackLength = this.grid.getScrollbackLength();
		for (let row = 0; row < scrollbackLength; row++) {
			lines.push(this.ghosttyTerm.getScrollbackCellData(row) ?? []);
			wrappedRows.push(this.ghosttyTerm.isScrollbackRowWrapped(row));
		}
		for (let row = 0; row < this.rowCount; row++) {
			lines.push(this.ghosttyTerm.getCellDataLine(row) ?? []);
			wrappedRows.push(this.ghosttyTerm.isRowWrapped(row));
		}
		return { lines, wrappedRows };
	}

	private findDirectionalSearchMatch(
		snapshot: TerminalSearchSnapshot,
		query: string,
		options: TerminalSearchOptions,
		reverse: boolean,
		preserveStart: boolean,
	): TerminalSearchMatch | null {
		const current = this.getSelectionPosition()?.start;
		return findDirectionalTerminalMatch(
			snapshot.lines,
			query,
			options,
			snapshot.wrappedRows,
			current
				? { preserveStart, reverse, start: { column: current.x, row: current.y } }
				: { preserveStart, reverse },
		);
	}

	private cancelScheduledSearch(): void {
		if (this.searchTimer !== undefined) clearTimeout(this.searchTimer);
		this.searchTimer = undefined;
		this.searchPendingIncremental = false;
	}

	private invalidateSearchAfterBufferChange(): void {
		if (!this.searchQuery) return;
		const incremental = this.searchTimer !== undefined && this.searchPendingIncremental;
		this.renderer?.setSearchMatches([]);
		this.scheduleSearch(this.searchQuery, this.searchOptions, incremental);
	}

	private applySearchResults(
		query: string,
		options: TerminalSearchOptions,
		matches: readonly TerminalSearchMatch[],
		active: TerminalSearchMatch | null,
	): void {
		this.searchQuery = query;
		this.searchOptions = { ...options };
		this.renderer?.setSearchMatches(matches, active);
		const resultIndex = active
			? matches.findIndex((match) => terminalSearchMatchesEqual(match, active))
			: -1;
		this.onSearchResultsEmitter.fire({
			query,
			regexError: validateTerminalSearchQuery(query, options),
			resultCount: matches.length,
			resultIndex,
			resultLimitReached:
				matches.length > 0 &&
				matches.length >= resolveTerminalSearchResultLimit(options.maxResults),
		});
	}

	private selectSearchMatch(
		matches: readonly TerminalSearchMatch[],
		match: TerminalSearchMatch | null,
		query: string,
		options: TerminalSearchOptions,
		reveal: boolean,
	): boolean {
		if (!match) {
			this.clearOwnedSearchSelection();
			this.applySearchResults(query, options, matches, null);
			return false;
		}
		if (!this.selectionManager) {
			this.searchOwnsSelection = false;
			this.applySearchResults(query, options, matches, null);
			return false;
		}
		if (reveal) this.revealAbsoluteRow(match.start.row);
		this.mutatingSearchSelection = true;
		let selected = false;
		try {
			selected = this.selectionManager.selectAbsoluteRange(
				{ x: match.start.column, y: match.start.row },
				{ x: match.end.column, y: match.end.row },
			);
		} finally {
			this.mutatingSearchSelection = false;
		}
		this.searchOwnsSelection = selected;
		this.applySearchResults(query, options, matches, selected ? match : null);
		return selected;
	}

	private clearOwnedSearchSelection(): void {
		if (!this.searchOwnsSelection) return;
		this.mutatingSearchSelection = true;
		try {
			this.selectionManager?.clearSelection();
		} finally {
			this.mutatingSearchSelection = false;
			this.searchOwnsSelection = false;
		}
	}

	private revealAbsoluteRow(row: number): void {
		const scrollbackLength = this.grid.getScrollbackLength();
		const top = scrollbackLength - Math.floor(this.viewportOffset);
		const bottom = top + this.rowCount - 1;
		let viewportY = this.viewportOffset;
		if (row < top) viewportY = scrollbackLength - row;
		else if (row > bottom) viewportY = scrollbackLength - (row - this.rowCount + 1);
		viewportY = Math.max(0, Math.min(scrollbackLength, viewportY));
		if (viewportY === this.viewportOffset) return;
		this.clearHoveredLink();
		this.viewportOffset = viewportY;
		this.targetViewportY = viewportY;
		this.cancelSmoothScroll();
		this.onScrollEmitter.fire(viewportY);
		this.requestRender();
	}

	private emitSnapshotEvents(): void {
		const cursor = this.ghosttyTerm.getCursor();
		if (
			!this.lastCursorPosition ||
			cursor.x !== this.lastCursorPosition.x ||
			cursor.y !== this.lastCursorPosition.y
		) {
			this.lastCursorPosition = { x: cursor.x, y: cursor.y };
			this.onCursorMoveEmitter.fire(this.lastCursorPosition);
		}
	}

	private syncEffectiveRendererColors(): void {
		if (!this.renderer) return;
		const colors = this.getColors();
		const cursor = colors.cursor ?? colors.foreground;
		const key = `${packRgb(colors.foreground)}:${packRgb(colors.background)}:${packRgb(cursor)}`;
		if (key === this.renderedDefaultColorsKey) return;
		this.renderedDefaultColorsKey = key;
		this.renderer.setTheme({
			...this.options.theme,
			foreground: rgbToCss(colors.foreground),
			background: rgbToCss(colors.background),
			cursor: rgbToCss(cursor),
		});
	}

	// ==========================================================================
	// Wheel Handler
	// ==========================================================================

	private handleWheel(e: WheelEvent): void {
		if (this.customWheelEventHandler?.(e)) return;
		if (this.hasApplicationMouseOwnership() && !e.shiftKey) return;
		e.preventDefault();

		if (this.isAlternateScreen()) {
			// In alternate screen, wheel sends arrow keys
			const sequence = e.deltaY > 0 ? '\x1b[B' : '\x1b[A';
			this.emitUserInput(sequence);
		} else {
			// Normal screen: scroll viewport
			const lineHeight = this.renderer?.getMetrics().height ?? 20;
			let deltaLines = e.deltaY / lineHeight;
			if (e.deltaMode === 1) deltaLines = e.deltaY;
			if (e.deltaMode === 2) deltaLines = e.deltaY * this.rowCount;
			this.smoothScrollTo(this.targetViewportY - deltaLines);

			// Show scrollbar
			this.scrollbarOpacity = 1;
			this.scheduleScrollbarFade();
		}
	}

	private readonly handleCanvasWheel = (event: WheelEvent): void => this.handleWheel(event);

	private smoothScrollTo(target: number): void {
		this.targetViewportY = Math.max(0, Math.min(this.grid.getScrollbackLength(), target));
		if (this.targetViewportY !== this.viewportOffset) this.clearHoveredLink();
		if (this.options.smoothScrollDuration <= 0 || typeof requestAnimationFrame !== 'function') {
			this.setViewportY(this.targetViewportY);
			return;
		}
		if (this.scrollAnimationFrame !== 0) return;
		this.scrollAnimationLastTime = performance.now();
		this.scrollAnimationFrame = requestAnimationFrame(this.animateScroll);
	}

	private readonly animateScroll = (timestamp: number): void => {
		this.scrollAnimationFrame = 0;
		const elapsed = Math.max(1, timestamp - this.scrollAnimationLastTime);
		this.scrollAnimationLastTime = timestamp;
		const distance = this.targetViewportY - this.viewportOffset;
		if (Math.abs(distance) < 0.01) {
			this.viewportOffset = this.targetViewportY;
			this.onScrollEmitter.fire(this.viewportOffset);
			this.requestRender();
			return;
		}
		const duration = Math.max(1, this.options.smoothScrollDuration);
		const progress = 1 - Math.exp((-5 * elapsed) / duration);
		this.viewportOffset += distance * progress;
		this.onScrollEmitter.fire(this.viewportOffset);
		this.requestRender();
		this.scrollAnimationFrame = requestAnimationFrame(this.animateScroll);
	};

	private cancelSmoothScroll(): void {
		if (this.scrollAnimationFrame !== 0) cancelAnimationFrame(this.scrollAnimationFrame);
		this.scrollAnimationFrame = 0;
	}

	private readonly handleCanvasMouseMove = (event: MouseEvent): void => {
		if (!this.canvas || !this.renderer || !this.linkDetector) return;
		this.setSurfaceModifiers(event);
		if (this.hasApplicationMouseOwnership() && !event.shiftKey) {
			this.clearHoveredLink();
			return;
		}
		const rect = this.canvas.getBoundingClientRect();
		const metrics = this.renderer.getMetrics();
		const col = Math.floor((event.clientX - rect.left) / metrics.width);
		const viewportRow = Math.floor((event.clientY - rect.top) / metrics.height);
		if (col < 0 || col >= this.columnCount || viewportRow < 0 || viewportRow >= this.rowCount) {
			this.clearHoveredLink();
			return;
		}

		const visibleTop = this.grid.getScrollbackLength() - Math.floor(this.viewportOffset);
		const lookupId = ++this.linkLookupId;
		void this.linkDetector.getLinkAt(col, visibleTop + viewportRow).then((link) => {
			if (lookupId !== this.linkLookupId) return;
			this.hoveredLink?.hover?.(false);
			this.hoveredLinkEnabled = false;
			this.hoveredLink = link;
			this.updateHoveredLinkPresentation();
		});
	};

	private readonly handleCanvasMouseLeave = (): void => this.clearHoveredLink();

	private readonly handleCanvasClick = (event: MouseEvent): void => {
		this.setSurfaceModifiers(event);
		if (this.selectionManager?.consumeSelectionDragClick(event)) return;
		if (this.hasApplicationMouseOwnership() && !event.shiftKey) {
			this.clearHoveredLink();
			return;
		}
		const link = this.hoveredLink;
		const cell = this.getBufferCellAt(event.clientX, event.clientY);
		if (
			!link ||
			!cell ||
			!isBufferCellInRange(cell.column, cell.bufferRow, link.range) ||
			!this.isLinkActivationEnabled(link, this.surfaceModifiers)
		) {
			return;
		}
		if (link.target) {
			if (!this.host.openTarget || !this.capabilities.openTarget) {
				this.onHostErrorEmitter.fire({
					operation: 'open-target',
					error: new Error('The terminal host does not support opening targets'),
				});
				return;
			}
			void this.host.openTarget({ source: 'link', target: link.target }).catch((error: unknown) => {
				this.onHostErrorEmitter.fire({ operation: 'open-target', error });
			});
			return;
		}
		link.activate?.(event);
	};

	private readonly handleCanvasContextMenu = (event: MouseEvent): void => {
		if (this.hasApplicationMouseOwnership() && !event.shiftKey) {
			event.preventDefault();
			event.stopPropagation();
			return;
		}
		if (!this.host.showContextMenu || !this.capabilities.contextMenu) return;
		event.preventDefault();
		event.stopPropagation();
		void this.requestContextMenuAt(event.clientX, event.clientY, {
			alt: event.altKey,
			control: event.ctrlKey,
			meta: event.metaKey,
			shift: event.shiftKey,
		});
	};

	private createInteractionContext(
		clientX: number,
		clientY: number,
		modifiers: TerminalModifierState,
	): TerminalInteractionContext {
		const cell = this.getBufferCellAt(clientX, clientY);
		const selectionRange = this.selectionController.getRange();
		const selection = selectionRange
			? {
					shape: selectionRange.rectangle ? ('rectangle' as const) : ('linear' as const),
					start: { column: selectionRange.start.x, row: selectionRange.start.y },
					end: { column: selectionRange.end.x, row: selectionRange.end.y },
					text: this.selectionController.getSelection(),
				}
			: null;
		const link =
			this.hoveredLink &&
			cell &&
			isBufferCellInRange(cell.column, cell.bufferRow, this.hoveredLink.range)
				? { text: this.hoveredLink.text, target: this.hoveredLink.target ?? null }
				: null;
		const semanticZone = cell
			? this.getSemanticZoneAtBufferCell(cell.bufferRow, cell.column)
			: null;
		return {
			cell,
			selection,
			link,
			semanticZone,
			cwd: this.getPwd() || null,
			modifiers,
			mouseReporting: this.hasApplicationMouseOwnership(),
		};
	}

	private getBufferCellAt(clientX: number, clientY: number): TerminalInteractionContext['cell'] {
		const rect = this.canvas?.getBoundingClientRect();
		const dimensions = this.renderer?.getMetrics() ?? { width: 8, height: 16 };
		if (!rect) return null;
		const column = Math.floor((clientX - rect.left) / dimensions.width);
		const row = Math.floor((clientY - rect.top) / dimensions.height);
		if (column < 0 || column >= this.columnCount || row < 0 || row >= this.rowCount) return null;
		return {
			column,
			row,
			bufferRow: this.grid.getScrollbackLength() + row - Math.floor(this.viewportOffset),
		};
	}

	private createBaseContextMenuItems(
		interaction: TerminalInteractionContext,
	): readonly ContextMenuItem[] {
		return [
			{ kind: 'action', actionId: 'terminal.copy', group: '10-clipboard', order: 10 },
			{ kind: 'action', actionId: 'terminal.paste', group: '10-clipboard', order: 20 },
			...(interaction.link?.target
				? ([
						{
							kind: 'action',
							actionId: 'terminal.open-link',
							group: '20-link',
							order: 10,
						},
					] as const)
				: []),
			{ kind: 'action', actionId: 'terminal.select-all', group: '30-selection', order: 10 },
			...(interaction.semanticZone === 'output' && interaction.cell
				? ([
						{
							kind: 'action',
							actionId: 'terminal.select-output',
							group: '30-selection',
							order: 15,
						},
					] as const)
				: []),
			{ kind: 'action', actionId: 'terminal.clear-selection', group: '30-selection', order: 20 },
			{ kind: 'action', actionId: 'terminal.reset', group: '90-terminal', order: 10 },
		];
	}

	private isLinkActivationEnabled(link: ILink, modifiers: TerminalModifierState): boolean {
		return link.activation !== 'modifier' || modifiers.control || modifiers.meta;
	}

	private setSurfaceModifiers(
		event: Pick<MouseEvent, 'altKey' | 'ctrlKey' | 'metaKey' | 'shiftKey'>,
	): void {
		this.surfaceModifiers = {
			alt: event.altKey,
			control: event.ctrlKey,
			meta: event.metaKey,
			shift: event.shiftKey,
		};
	}

	private readonly handleSurfaceModifierChange = (event: KeyboardEvent): void => {
		this.setSurfaceModifiers(event);
		this.updateHoveredLinkPresentation();
	};

	private updateSurfaceMouseCursor(): void {
		const cursor = resolveSurfaceMouseCursor({
			hidden: false,
			modifiers: this.surfaceModifiers,
			mouseTracking: this.hasApplicationMouseOwnership(),
			overLink: false,
			platform: this.surfaceMousePlatform,
		});
		if (cursor && this.canvas) this.canvas.style.cursor = cursor;
	}

	private updateHoveredLinkPresentation(): void {
		const link = this.hoveredLink;
		const enabled = link ? this.isLinkActivationEnabled(link, this.surfaceModifiers) : false;
		if (link && enabled !== this.hoveredLinkEnabled) link.hover?.(enabled);
		this.hoveredLinkEnabled = enabled;
		if (enabled && this.canvas) this.canvas.style.cursor = 'pointer';
		else this.updateSurfaceMouseCursor();

		const renderer = this.renderer;
		if (!renderer) return;
		renderer.setHoveredHyperlinkId(0);
		const visibleTop = this.grid.getScrollbackLength() - Math.floor(this.viewportOffset);
		renderer.setHoveredLinkRange(
			link && enabled
				? {
						startX: link.range.start.x,
						startY: link.range.start.y - visibleTop,
						endX: link.range.end.x,
						endY: link.range.end.y - visibleTop,
					}
				: null,
		);
	}

	private hasApplicationMouseOwnership(): boolean {
		return !this.options.disableStdin && this.hasMouseTracking();
	}

	private syncSurfaceMouseOwnership(): void {
		const owned = this.hasApplicationMouseOwnership();
		if (owned === this.surfaceMouseOwnedByApplication) return;
		this.surfaceMouseOwnedByApplication = owned;
		this.updateSurfaceMouseCursor();
	}

	private readonly handleInputFocusMouseDown = (event: MouseEvent): void => {
		if (event.button !== 0) return;
		event.preventDefault();
		this.updateInputPosition();
		this.textAreaElement?.focus();
	};

	private readonly handleInputFocusTouchEnd = (event: TouchEvent): void => {
		event.preventDefault();
		this.textAreaElement?.focus();
	};

	private emitUserInput(data: string, preserveSurfaceState = false): void {
		this.inputDispatcher.dispatchSync(() =>
			this.emitUserInputNow(data, false, preserveSurfaceState),
		);
	}

	private emitStateRepairInput(data: string): void {
		this.inputDispatcher.dispatchSync(() => this.emitUserInputNow(data, true, true));
	}

	private emitUserInputNow(
		data: string,
		allowWhenDisabled = false,
		preserveSurfaceState = false,
	): void {
		if (this.handle === 0) return;
		if (this.options.disableStdin && !allowWhenDisabled) return;
		if (!preserveSurfaceState) {
			this.revealActiveViewportOnUserInput(data);
			this.selectionManager?.clearSelection();
		}
		this.onDataEmitter.fire(data);
	}

	private emitUserBinary(
		data: string,
		allowWhenDisabled = false,
		preserveSurfaceState = false,
	): void {
		this.inputDispatcher.dispatchSync(() => {
			if (this.handle === 0 || (this.options.disableStdin && !allowWhenDisabled)) return;
			if (!preserveSurfaceState) {
				this.revealActiveViewportOnUserInput(data);
				this.selectionManager?.clearSelection();
			}
			this.onBinaryEmitter.fire(data);
		});
	}

	private revealActiveViewportOnUserInput(data: string): void {
		if (data.length > 0 && this.viewportOffset > 0) this.scrollToBottom();
	}

	private dispatchKeyboardAction(id: string): boolean {
		const context = { source: 'keyboard' as const, signal: new AbortController().signal };
		const state = this.actions.getState(id, context);
		if (!state?.enabled) return false;
		void this.actions.execute(id, context);
		return true;
	}

	private dispatchCustomKeyEvent(event: KeyboardEvent): boolean {
		const context = {
			alternateScreen: this.isAlternateScreen(),
			hasSelection: this.hasSelection(),
			readOnly: this.options.disableStdin,
		};
		if (this.keyEvents.dispatch(event, context)) return true;
		if (this.customKeyEventHandler?.(event)) return true;
		return this.handleViewportKeyEvent(event);
	}

	private handleViewportKeyEvent(event: KeyboardEvent): boolean {
		if (!event.shiftKey || event.ctrlKey || event.altKey || event.metaKey) return false;
		if (event.code === 'PageUp' || event.code === 'NumpadPageUp') {
			this.scrollPages(-1);
			return true;
		}
		if (event.code === 'PageDown' || event.code === 'NumpadPageDown') {
			this.scrollPages(1);
			return true;
		}
		return false;
	}

	private handleComposition(data: string | null, selection?: CompositionSelection): void {
		if (data !== null && !this.options.disableStdin) {
			if (this.viewportOffset > 0) this.scrollToBottom();
			this.selectionManager?.clearSelection();
		}
		const state = buildPreeditState(data ?? '', selection, (codepoint) =>
			this.bridge.getExports().ghostty_unicode_codepoint_width(codepoint),
		);
		this.preeditCellWidth = state.width;
		const caretOffset = Math.max(
			0,
			Math.min(data?.length ?? 0, selection?.end ?? data?.length ?? 0),
		);
		this.preeditCaretPixelOffset = data
			? (this.renderer?.measureTextWidth(data.slice(0, caretOffset)) ??
				state.selection.end * (this.renderer?.getMetrics().width ?? 0))
			: 0;
		this.renderer?.setPreedit(state.cells, state.selection);
		this.updateInputPosition();
	}

	private updateInputPosition(): void {
		if (!this.textAreaElement || !this.hostElement || !this.canvas || !this.renderer) return;
		const elementRect = this.hostElement.getBoundingClientRect();
		const canvasRect = this.canvas.getBoundingClientRect();
		const metrics = this.renderer.getMetrics();
		const anchor = calculateInputAnchor({
			canvasLeft: canvasRect.left,
			canvasTop: canvasRect.top,
			elementLeft: elementRect.left,
			elementTop: elementRect.top,
			cursorColumn: this.cursorX,
			cursorRow: this.cursorY,
			columns: this.columnCount,
			rows: this.rowCount,
			cellWidth: metrics.width,
			cellHeight: metrics.height,
			preeditCellWidth: this.preeditCellWidth,
			preeditCaretPixelOffset: this.preeditCaretPixelOffset,
		});
		this.textAreaElement.style.left = `${anchor.left}px`;
		this.textAreaElement.style.top = `${anchor.top}px`;
		this.textAreaElement.style.width = `${anchor.width}px`;
		this.textAreaElement.style.height = `${anchor.height}px`;
		this.textAreaElement.style.lineHeight = `${anchor.height}px`;
		this.textAreaElement.style.fontFamily = this.options.fontFamily;
		this.textAreaElement.style.fontSize = `${this.options.fontSize}px`;
		this.textAreaElement.style.fontWeight = String(this.options.fontWeight);
	}

	private async loadConfiguredFont(): Promise<void> {
		if (typeof document === 'undefined') return;
		const fonts = document.fonts;
		if (!fonts || typeof fonts.load !== 'function') return;
		try {
			await fonts.load(
				`${this.options.fontWeight} ${this.options.fontSize}px ${this.options.fontFamily}`,
				'MW你🚀',
			);
		} catch (error) {
			this.onHostErrorEmitter.fire({ operation: 'font-load', error });
		}
	}

	private clearHoveredLink(): void {
		this.linkLookupId++;
		if (!this.hoveredLink && !this.hoveredLinkEnabled) return;
		this.hoveredLink?.hover?.(false);
		this.hoveredLink = undefined;
		this.hoveredLinkEnabled = false;
		this.updateHoveredLinkPresentation();
	}

	private readonly handleScrollbarMouseDown = (event: MouseEvent): void => {
		if (!this.canvas || event.button !== 0) return;
		const scrollbackLength = this.grid.getScrollbackLength();
		if (scrollbackLength === 0) return;
		const rect = this.canvas.getBoundingClientRect();
		if (event.clientX - rect.left < rect.width - 16) return;

		event.preventDefault();
		event.stopImmediatePropagation();
		this.clearHoveredLink();
		this.scrollbarOpacity = 1;
		const metrics = this.getScrollbarMetrics(rect.height, scrollbackLength);
		const pointerY = event.clientY - rect.top;
		const currentThumbY =
			metrics.padding + metrics.travel * (1 - this.viewportOffset / scrollbackLength);
		if (pointerY < currentThumbY || pointerY > currentThumbY + metrics.thumbHeight) {
			const ratio = Math.max(
				0,
				Math.min(1, (pointerY - metrics.padding - metrics.thumbHeight / 2) / metrics.travel),
			);
			this.viewportOffset = Math.round((1 - ratio) * scrollbackLength);
			this.targetViewportY = this.viewportOffset;
			this.cancelSmoothScroll();
			this.onScrollEmitter.fire(this.viewportOffset);
		}
		this.draggingScrollbar = true;
		this.scrollbarDragStartY = event.clientY;
		this.scrollbarDragStartViewportY = this.viewportOffset;
		this.requestRender();
	};

	private readonly handleScrollbarMouseMove = (event: MouseEvent): void => {
		if (!this.draggingScrollbar || !this.canvas) return;
		if ((event.buttons & 1) === 0) {
			this.handleScrollbarMouseUp();
			return;
		}
		const scrollbackLength = this.grid.getScrollbackLength();
		const metrics = this.getScrollbarMetrics(
			this.canvas.getBoundingClientRect().height,
			scrollbackLength,
		);
		const deltaRows =
			((event.clientY - this.scrollbarDragStartY) / Math.max(1, metrics.travel)) * scrollbackLength;
		this.viewportOffset = Math.max(
			0,
			Math.min(scrollbackLength, Math.round(this.scrollbarDragStartViewportY - deltaRows)),
		);
		this.targetViewportY = this.viewportOffset;
		this.onScrollEmitter.fire(this.viewportOffset);
		this.requestRender();
	};

	private readonly handleScrollbarMouseUp = (): void => {
		if (!this.draggingScrollbar) return;
		this.draggingScrollbar = false;
		this.scheduleScrollbarFade();
	};

	private getScrollbarMetrics(
		canvasHeight: number,
		scrollbackLength: number,
	): {
		padding: number;
		thumbHeight: number;
		travel: number;
	} {
		const padding = 4;
		const trackHeight = canvasHeight - padding * 2;
		const thumbHeight = Math.max(
			20,
			(this.rowCount / (scrollbackLength + this.rowCount)) * trackHeight,
		);
		return { padding, thumbHeight, travel: Math.max(1, trackHeight - thumbHeight) };
	}

	private scheduleScrollbarFade(): void {
		if (this.scrollbarFadeTimer) clearTimeout(this.scrollbarFadeTimer);
		this.scrollbarFadeTimer = setTimeout(() => {
			this.scrollbarOpacity = 0;
			this.requestRender();
		}, 1500);
	}

	private readonly handleFocusIn = (event: FocusEvent): void => {
		const relatedTarget = event.relatedTarget;
		if (relatedTarget instanceof Node && this.hostElement?.contains(relatedTarget)) return;
		this.renderer?.setFocused(true);
		if (this.hasFocusEvents()) {
			this.emitUserInput(new TextDecoder().decode(this.bridge.encodeFocus(true)), true);
		}
	};

	private readonly handleFocusOut = (event: FocusEvent): void => {
		const relatedTarget = event.relatedTarget;
		if (relatedTarget instanceof Node && this.hostElement?.contains(relatedTarget)) return;
		this.renderer?.setFocused(false);
		this.selectionManager?.cancelActiveGesture();
		this.handleScrollbarMouseUp();
		this.inputHandler?.releasePressedKeys();
		this.inputHandler?.releasePressedMouseButtons();
		this.inputHandler?.cancelComposition();
		if (this.hasFocusEvents()) {
			this.emitUserInput(new TextDecoder().decode(this.bridge.encodeFocus(false)), true);
		}
	};

	private readonly handleDragOver = (event: DragEvent): void => {
		if (this.options.disableStdin || !event.dataTransfer) return;
		const supported = [...event.dataTransfer.types].some(
			(type) => type === 'Files' || type === 'text/plain',
		);
		if (!supported) return;
		event.preventDefault();
		event.dataTransfer.dropEffect = 'copy';
	};

	private readonly handleDrop = (event: DragEvent): void => {
		const transfer = event.dataTransfer;
		if (!transfer) return;
		event.preventDefault();
		event.stopPropagation();
		const text = transfer.getData('text/plain');
		void this.dropAsync(text, [...transfer.files]);
	};

	private readonly handleVisibilityChange = (): void => {
		const hidden = document.visibilityState === 'hidden';
		this.renderer?.setSuspended(hidden);
		if (hidden) {
			this.selectionManager?.cancelActiveGesture();
			this.handleScrollbarMouseUp();
			this.inputHandler?.releasePressedKeys();
			this.inputHandler?.releasePressedMouseButtons();
			this.stopRenderLoop();
			this.targetViewportY = this.viewportOffset;
		} else this.requestRender();
	};

	private readonly handleWindowResize = (): void => {
		this.syncDevicePixelRatio();
		this.renderer?.syncSurfacePadding();
	};

	private readonly handleDevicePixelRatioChange = (): void => {
		this.syncDevicePixelRatio();
	};

	private syncDevicePixelRatio(): void {
		if (!this.renderer?.setDevicePixelRatio(window.devicePixelRatio ?? 1)) return;
		this.watchDevicePixelRatio();
		this.requestRender();
	}

	private watchDevicePixelRatio(): void {
		this.devicePixelRatioQuery?.removeEventListener('change', this.handleDevicePixelRatioChange);
		this.devicePixelRatioQuery = undefined;
		if (typeof window.matchMedia !== 'function') return;
		const ratio = window.devicePixelRatio ?? 1;
		this.devicePixelRatioQuery = window.matchMedia(`(resolution: ${ratio}dppx)`);
		this.devicePixelRatioQuery.addEventListener('change', this.handleDevicePixelRatioChange);
	}

	private detachDevicePixelRatioListeners(): void {
		if (this.windowResizeListenerAttached) {
			window.removeEventListener('resize', this.handleWindowResize);
			this.windowResizeListenerAttached = false;
		}
		this.devicePixelRatioQuery?.removeEventListener('change', this.handleDevicePixelRatioChange);
		this.devicePixelRatioQuery = undefined;
	}

	private disposeEvents(): void {
		this.onDataEmitter.dispose();
		this.onBinaryEmitter.dispose();
		this.onResizeEmitter.dispose();
		this.onBellEmitter.dispose();
		this.onSelectionChangeEmitter.dispose();
		this.onKeyEmitter.dispose();
		this.onTitleChangeEmitter.dispose();
		this.onPwdChangeEmitter.dispose();
		this.onScrollEmitter.dispose();
		this.onRenderEmitter.dispose();
		this.onCursorMoveEmitter.dispose();
		this.onHostErrorEmitter.dispose();
		this.onRendererHealthEmitter.dispose();
		this.onRendererTelemetryEmitter.dispose();
		this.onSearchResultsEmitter.dispose();
	}

	// ==========================================================================
	// Dependency Factories
	// ==========================================================================

	private createInputDeps(): InputHandlerDependencies {
		return {
			bridge: this.bridge,
			getTerminalHandle: () => this.handle,
			hasMouseTracking: () => this.hasMouseTracking(),
			getCellDimensions: () => {
				const metrics = this.renderer?.getMetrics() ?? { width: 8, height: 16 };
				return metrics;
			},
			getScreenDimensions: () => {
				const metrics = this.renderer?.getMetrics() ?? { width: 8, height: 16 };
				return { width: metrics.width * this.columnCount, height: metrics.height * this.rowCount };
			},
			getCanvasOffset: () => {
				const rect = this.canvas?.getBoundingClientRect() ?? { left: 0, top: 0 };
				return { left: rect.left, top: rect.top };
			},
			getMode: (mode: number, isAnsi?: boolean) => this.getMode(mode, isAnsi),
			isReadOnly: () => this.options.disableStdin,
			emitBinary: (data) => this.emitUserBinary(data),
			emitStateRepairBinary: (data) => this.emitUserBinary(data, true, true),
			onSurfaceModifierChange: this.handleSurfaceModifierChange,
		};
	}

	private createSelectionTerminal(): ISelectionTerminal {
		return {
			getCellDimensions: () => this.renderer?.getMetrics() ?? { width: 8, height: 16 },
			getDimensions: () => ({ cols: this.columnCount, rows: this.rowCount }),
			getCanvasOffset: () => {
				const rect = this.canvas?.getBoundingClientRect() ?? { left: 0, top: 0 };
				return { left: rect.left, top: rect.top };
			},
			getScrollbackLength: () => this.grid.getScrollbackLength(),
			getViewportY: () => this.viewportOffset,
			getLine: (row) => {
				this.ensureSnapshot();
				return this.ghosttyTerm.getLine(row);
			},
			getScrollbackLine: (offset) => {
				this.ensureSnapshot();
				return this.ghosttyTerm.getScrollbackLine(offset);
			},
			getGraphemeString: (row, col) => {
				this.ensureSnapshot();
				return this.ghosttyTerm.getGraphemeString(row, col);
			},
			getScrollbackGraphemeString: (offset, col) => {
				this.ensureSnapshot();
				return this.ghosttyTerm.getScrollbackGraphemeString(offset, col);
			},
			isRowWrapped: (row) => this.ghosttyTerm.isRowWrapped(row),
			isScrollbackRowWrapped: (offset) => this.ghosttyTerm.isScrollbackRowWrapped(offset),
			scrollLines: (amount) => this.scrollLines(amount),
			shouldSelect: (event) => !this.hasApplicationMouseOwnership() || event.shiftKey,
			requestRender: () => {
				this.requestRender();
			},
		};
	}

	private createLinkDetectorTerminal(): ITerminalForLinkDetector {
		return {
			buffer: this.buffer,
		};
	}

	private createOSC8Terminal() {
		return {
			buffer: this.buffer,
			wasmTerm: {
				getHyperlinkUri: (row: number, col: number) => this.ghosttyTerm.getHyperlinkUri(row, col),
				getScrollbackHyperlinkUri: (offset: number, col: number) =>
					this.ghosttyTerm.getScrollbackHyperlinkUri(offset, col),
				getScrollbackLength: () => this.grid.getScrollbackLength(),
			},
		};
	}

	private createUrlRegexTerminal() {
		return {
			buffer: this.buffer,
		};
	}

	private createPathProviderTerminal() {
		return {
			buffer: this.buffer,
			getPwd: () => this.getPwd(),
			getSemanticZone: (row: number, column: number) =>
				this.getSemanticZoneAtBufferCell(row, column),
		};
	}

	private getSemanticZoneAtBufferCell(
		row: number,
		column: number,
	): TerminalInteractionContext['semanticZone'] {
		const scrollbackLength = this.grid.getScrollbackLength();
		const cell =
			row < scrollbackLength
				? this.ghosttyTerm.getScrollbackCellData(row)?.[column]
				: (this.ghosttyTerm.getCellDataLine(row - scrollbackLength)?.[column] ??
					this.grid.getCell(row - scrollbackLength, column));
		return cell ? resolveSemanticZone(cell.semanticContent) : null;
	}

	// ==========================================================================
	// Cursor
	// ==========================================================================

	/** Get cursor X position (0-based column, viewport-relative). */
	get cursorX(): number {
		return this.bridge.getU16(
			this.bridge.getExports().ghostty_render_state_get,
			this.rsHandle,
			RenderStateData.CURSOR_VIEWPORT_X,
		);
	}

	/** Get cursor Y position (0-based row, viewport-relative). */
	get cursorY(): number {
		return this.bridge.getU16(
			this.bridge.getExports().ghostty_render_state_get,
			this.rsHandle,
			RenderStateData.CURSOR_VIEWPORT_Y,
		);
	}

	/** Is the cursor visible? */
	get cursorVisible(): boolean {
		return this.bridge.getBool(
			this.bridge.getExports().ghostty_render_state_get,
			this.rsHandle,
			RenderStateData.CURSOR_VISIBLE,
		);
	}

	get cursorBlinking(): boolean {
		return this.bridge.getBool(
			this.bridge.getExports().ghostty_render_state_get,
			this.rsHandle,
			RenderStateData.CURSOR_BLINKING,
		);
	}

	get cursorStyle(): 'block' | 'underline' | 'bar' {
		const style = this.bridge.getU32(
			this.bridge.getExports().ghostty_render_state_get,
			this.rsHandle,
			RenderStateData.CURSOR_VISUAL_STYLE,
		);
		if (style === 0) return 'bar';
		if (style === 2) return 'underline';
		return 'block';
	}

	/** Get full cursor state. */
	getCursor(): CursorState {
		return {
			x: this.cursorX,
			y: this.cursorY,
			visible: this.cursorVisible,
		};
	}

	// ==========================================================================
	// Colors
	// ==========================================================================

	/** Default foreground color. */
	get foregroundColor(): RGB {
		return this.readRenderStateColor(RenderStateData.COLOR_FOREGROUND);
	}

	/** Default background color. */
	get backgroundColor(): RGB {
		return this.readRenderStateColor(RenderStateData.COLOR_BACKGROUND);
	}

	/** Get all default terminal colors. */
	getColors(): TerminalColors {
		const ptr = this.bridge.allocU8Array(RENDER_STATE_COLORS_SIZE);
		try {
			// Set size field for sized-struct pattern
			this.bridge.createView(ptr, 4).setUint32(0, RENDER_STATE_COLORS_SIZE, true);
			const result = this.bridge.getExports().ghostty_render_state_colors_get(this.rsHandle, ptr);
			if (result !== 0) {
				return {
					foreground: { r: 255, g: 255, b: 255 },
					background: { r: 0, g: 0, b: 0 },
					cursor: null,
				};
			}
			const view = this.bridge.createView(ptr + 4, 10); // after size field
			return {
				foreground: { r: view.getUint8(3), g: view.getUint8(4), b: view.getUint8(5) },
				background: { r: view.getUint8(0), g: view.getUint8(1), b: view.getUint8(2) },
				cursor:
					view.getUint8(9) !== 0
						? { r: view.getUint8(6), g: view.getUint8(7), b: view.getUint8(8) }
						: null,
			};
		} finally {
			this.bridge.freeU8Array(ptr, RENDER_STATE_COLORS_SIZE);
		}
	}

	// ==========================================================================
	// Modes
	// ==========================================================================

	/** Is the alternate screen active? */
	isAlternateScreen(): boolean {
		const outPtr = this.bridge.allocU8Array(4);
		try {
			const result = this.bridge
				.getExports()
				.ghostty_terminal_get(this.handle, TerminalData.ACTIVE_SCREEN, outPtr);
			if (result !== 0) return false;
			return this.bridge.createView(outPtr, 4).getUint32(0, true) === 1;
		} finally {
			this.bridge.freeU8Array(outPtr, 4);
		}
	}

	/** Is any mouse tracking mode enabled? */
	hasMouseTracking(): boolean {
		return this.bridge.getBool(
			this.bridge.getExports().ghostty_terminal_get,
			this.handle,
			TerminalData.MOUSE_TRACKING,
		);
	}

	/** Is bracketed paste mode enabled (DEC mode 2004)? */
	hasBracketedPaste(): boolean {
		return this.getMode(DecMode.BRACKETED_PASTE);
	}

	/** Is focus event reporting enabled (DEC mode 1004)? */
	hasFocusEvents(): boolean {
		return this.getMode(DecMode.FOCUS_EVENTS);
	}

	/** Is cursor visible mode enabled (DEC mode 25)? */
	isCursorVisibleMode(): boolean {
		return this.getMode(DecMode.CURSOR_VISIBLE);
	}

	/**
	 * Query arbitrary terminal mode.
	 *
	 * @param mode Mode number (e.g., 25 for cursor visibility).
	 * @param isAnsi True for ANSI modes, false for DEC modes (default: false).
	 */
	getMode(mode: number, isAnsi = false): boolean {
		const outPtr = this.bridge.allocU8();
		try {
			const tag = buildModeTag(mode, isAnsi);
			const result = this.bridge.getExports().ghostty_terminal_mode_get(this.handle, tag, outPtr);
			if (result !== 0) return false;
			return this.bridge.createView(outPtr, 1).getUint8(0) !== 0;
		} finally {
			this.bridge.freeU8(outPtr);
		}
	}

	// ==========================================================================
	// Terminal Properties
	// ==========================================================================

	/** Get the terminal title (set via OSC 0/2). */
	getTitle(): string {
		return this.readString(TerminalData.TITLE);
	}

	/** Get the terminal working directory (set via OSC 7). */
	getPwd(): string {
		return this.readString(TerminalData.PWD);
	}

	/** Is the viewport currently following the active area? */
	isViewportActive(): boolean {
		return this.bridge.getBool(
			this.bridge.getExports().ghostty_terminal_get,
			this.handle,
			TerminalData.VIEWPORT_ACTIVE,
		);
	}

	// ==========================================================================
	// Configuration
	// ==========================================================================

	private handleOptionChange(key: string): void {
		switch (key) {
			case 'cursorBlink':
				this.renderer?.setCursorBlink(this.options.cursorBlink);
				this.setDefaultCursorBlink(this.options.cursorBlink);
				break;
			case 'cursorStyle':
				this.renderer?.setCursorStyle(this.options.cursorStyle);
				this.setDefaultCursorStyle(this.options.cursorStyle);
				break;
			case 'theme':
				this.renderer?.setTheme(this.options.theme);
				const resolvedTheme = { ...DEFAULT_THEME, ...this.options.theme };
				this.cachedColorScheme = colorSchemeFor(parseCssColor(resolvedTheme.background));
				this.setPaletteOnHandle(this.handle, this.options.theme);
				this.setForegroundColor(parseCssColor(resolvedTheme.foreground));
				this.setBackgroundColor(parseCssColor(resolvedTheme.background));
				this.setCursorColor(parseCssColor(resolvedTheme.cursor));
				break;
			case 'fontSize':
				this.renderer?.setFontSize(this.options.fontSize);
				this.renderer?.resize(this.columnCount, this.rowCount);
				break;
			case 'fontFamily':
				this.renderer?.setFontFamily(this.options.fontFamily);
				this.renderer?.resize(this.columnCount, this.rowCount);
				break;
			case 'fontWeight':
				this.renderer?.setFontWeight(this.options.fontWeight);
				this.renderer?.resize(this.columnCount, this.rowCount);
				break;
			case 'fontWeightBold':
				this.renderer?.setFontWeightBold(this.options.fontWeightBold);
				break;
			case 'fontLigatures':
				this.renderer?.setFontLigatures(this.options.fontLigatures);
				break;
			case 'fontMetricModifiers':
				this.renderer?.setFontMetricModifiers(this.options.fontMetricModifiers);
				break;
			case 'paddingBalance':
				this.renderer?.setPaddingBalance(this.options.paddingBalance);
				break;
			case 'allowTransparency':
				this.renderer?.setAllowTransparency(this.options.allowTransparency);
				break;
			case 'copyOnSelect':
				this.selectionManager?.setCopyOnSelect(this.options.copyOnSelect);
				break;
			case 'pasteProtection':
			case 'pasteBracketedSafe':
				break;
			case 'disableStdin':
				this.actions.notifyStateChange('terminal.paste');
				this.clearHoveredLink();
				this.syncSurfaceMouseOwnership();
				break;
			case 'cols':
			case 'rows':
				this.resize(this.options.cols, this.options.rows);
				break;
		}
	}

	private registerBuiltInActions(): void {
		this.actions.register({
			id: 'terminal.open-link',
			title: 'Open Link',
			scope: 'surface',
			isEnabled: (context) =>
				context.interaction?.link?.target !== null &&
				context.interaction?.link?.target !== undefined &&
				this.host.openTarget !== undefined &&
				this.capabilities.openTarget,
			run: async (context) => {
				const target = context.interaction?.link?.target;
				const openTarget = this.host.openTarget;
				if (!target || !openTarget || !this.capabilities.openTarget) {
					return { kind: 'unsupported' };
				}
				try {
					await openTarget({ source: 'context-menu', target });
					return { kind: 'consumed' };
				} catch (error) {
					this.onHostErrorEmitter.fire({ operation: 'open-target', error });
					return { kind: 'blocked', reason: 'open-target-failed' };
				}
			},
		});
		this.actions.register({
			id: 'terminal.copy',
			title: 'Copy',
			scope: 'surface',
			isEnabled: () => this.hasSelection() && this.capabilities.clipboardWrite,
			run: async () => {
				const result = await this.copySelectionAsync();
				return result.kind === 'written'
					? { kind: 'consumed' }
					: { kind: 'blocked', reason: `clipboard-${result.kind}` };
			},
		});
		this.actions.register({
			id: 'terminal.paste',
			title: 'Paste',
			scope: 'surface',
			isEnabled: () => this.capabilities.clipboardRead && !this.options.disableStdin,
			run: async () => {
				const clipboard = this.host.clipboard;
				if (!clipboard || !this.capabilities.clipboardRead) return { kind: 'unsupported' };
				let payload: ClipboardPayload | null;
				try {
					payload = await clipboard.read({ target: 'standard', source: 'paste' });
				} catch (error) {
					this.onHostErrorEmitter.fire({ operation: 'clipboard-read', error });
					return { kind: 'blocked', reason: 'clipboard-read-failed' };
				}
				if (!payload?.text) return { kind: 'blocked', reason: 'clipboard-empty' };
				const result = await this.pasteAsync(payload.text, 'clipboard');
				return result.kind === 'emitted'
					? { kind: 'consumed' }
					: { kind: 'blocked', reason: `paste-${result.kind}` };
			},
		});
		this.actions.register({
			id: 'terminal.select-all',
			title: 'Select All',
			scope: 'surface',
			run: () => {
				this.selectAll();
				return { kind: 'consumed' };
			},
		});
		this.actions.register({
			id: 'terminal.select-output',
			title: 'Select Command Output',
			scope: 'surface',
			isEnabled: (context) =>
				context.interaction?.semanticZone === 'output' &&
				context.interaction.cell !== null &&
				this.selectionManager !== undefined,
			run: (context) => {
				const cell = context.interaction?.cell;
				if (!cell || !this.selectionManager?.selectOutputAtAbsolute(cell.column, cell.bufferRow)) {
					return { kind: 'blocked', reason: 'semantic-output-unavailable' };
				}
				return { kind: 'consumed' };
			},
		});
		this.actions.register({
			id: 'terminal.clear-selection',
			title: 'Clear Selection',
			scope: 'surface',
			isEnabled: () => this.hasSelection(),
			run: () => {
				this.clearSelection();
				return { kind: 'consumed' };
			},
		});
		this.actions.register({
			id: 'terminal.reset',
			title: 'Reset Terminal',
			scope: 'surface',
			run: () => {
				this.reset();
				return { kind: 'consumed' };
			},
		});
	}

	/**
	 * Set the default foreground color.
	 * @param color RGB color values.
	 */
	setForegroundColor(color: RGB): void {
		this.setColor(TerminalOption.COLOR_FOREGROUND, color);
	}

	/**
	 * Set the default background color.
	 * @param color RGB color values.
	 */
	setBackgroundColor(color: RGB): void {
		this.setColor(TerminalOption.COLOR_BACKGROUND, color);
	}

	/**
	 * Set the cursor color.
	 * @param color RGB color values.
	 */
	setCursorColor(color: RGB): void {
		this.setColor(TerminalOption.COLOR_CURSOR, color);
	}

	private setDefaultCursorStyle(style: 'block' | 'underline' | 'bar'): void {
		const ptr = this.bridge.allocU8Array(4);
		try {
			let value = 2;
			if (style === 'bar') value = 0;
			if (style === 'block') value = 1;
			this.bridge.createView(ptr, 4).setUint32(0, value, true);
			this.bridge
				.getExports()
				.ghostty_terminal_set(this.handle, TerminalOption.DEFAULT_CURSOR_STYLE, ptr);
		} finally {
			this.bridge.freeU8Array(ptr, 4);
		}
	}

	private setDefaultCursorBlink(blink: boolean): void {
		const ptr = this.bridge.allocU8();
		try {
			this.bridge.createView(ptr, 1).setUint8(0, blink ? 1 : 0);
			this.bridge
				.getExports()
				.ghostty_terminal_set(this.handle, TerminalOption.DEFAULT_CURSOR_BLINK, ptr);
		} finally {
			this.bridge.freeU8(ptr);
		}
	}

	// ==========================================================================
	// Private Helpers
	// ==========================================================================

	/** Create the WASM terminal handle with optional config. */
	private createHandle(cols: number, rows: number, config?: TerminalConfig): TerminalHandle {
		const exp = this.bridge.getExports();

		// Allocate options struct (8 bytes: cols u16 + rows u16 + max_scrollback u32)
		const optsPtr = this.bridge.allocU8Array(TERMINAL_OPTIONS_SIZE);
		try {
			const view = this.bridge.createView(optsPtr, TERMINAL_OPTIONS_SIZE);
			view.setUint16(0, cols, true);
			view.setUint16(2, rows, true);
			view.setUint32(4, config?.scrollbackLimit ?? DEFAULT_SCROLLBACK_BYTES, true);

			const handle = this.bridge.allocAndReadOpaque((alloc, ptrPtr) =>
				exp.ghostty_terminal_new(alloc, ptrPtr, optsPtr),
			);

			// Ghostty render state resolves foreground/background as a pair, so both
			// defaults must always be configured even when only one was customized.
			this.setColorOnHandle(handle, TerminalOption.COLOR_FOREGROUND, config?.fgColor ?? 0xffffff);
			this.setColorOnHandle(handle, TerminalOption.COLOR_BACKGROUND, config?.bgColor ?? 0x000000);
			if (config?.cursorColor !== undefined) {
				this.setColorOnHandle(handle, TerminalOption.COLOR_CURSOR, config.cursorColor);
			}

			return handle;
		} finally {
			this.bridge.freeU8Array(optsPtr, TERMINAL_OPTIONS_SIZE);
		}
	}

	/** Create the WASM render state handle. */
	private createRenderState(): number {
		return this.bridge.allocAndReadOpaque(this.bridge.getExports().ghostty_render_state_new);
	}

	/** Read a color from render state. */
	private readRenderStateColor(fieldId: number): RGB {
		const ptr = this.bridge.allocU8Array(COLOR_RGB_SIZE);
		try {
			const result = this.bridge.getExports().ghostty_render_state_get(this.rsHandle, fieldId, ptr);
			if (result !== 0) return { r: 0, g: 0, b: 0 };
			const view = this.bridge.createView(ptr, COLOR_RGB_SIZE);
			return { r: view.getUint8(0), g: view.getUint8(1), b: view.getUint8(2) };
		} finally {
			this.bridge.freeU8Array(ptr, COLOR_RGB_SIZE);
		}
	}

	/** Set a color option on a terminal handle. */
	private setColorOnHandle(handle: number, option: number, packedColor: number): void {
		const ptr = this.bridge.allocU8Array(COLOR_RGB_SIZE);
		try {
			const view = this.bridge.createView(ptr, COLOR_RGB_SIZE);
			view.setUint8(0, (packedColor >> 16) & 0xff);
			view.setUint8(1, (packedColor >> 8) & 0xff);
			view.setUint8(2, packedColor & 0xff);
			this.bridge.getExports().ghostty_terminal_set(handle, option, ptr);
		} finally {
			this.bridge.freeU8Array(ptr, COLOR_RGB_SIZE);
		}
	}

	private setPaletteOnHandle(handle: number, theme: ITheme): void {
		const palette = buildColorPalette(theme);
		const ptr = this.bridge.allocU8Array(palette.length);
		try {
			this.bridge.writeBytes(ptr, palette);
			const result = this.bridge
				.getExports()
				.ghostty_terminal_set(handle, TerminalOption.COLOR_PALETTE, ptr);
			if (result !== 0) throw new Error(`Failed to set terminal color palette: ${result}`);
		} finally {
			this.bridge.freeU8Array(ptr, palette.length);
		}
	}

	/** Set a color option. */
	private setColor(option: number, color: RGB): void {
		const ptr = this.bridge.allocU8Array(COLOR_RGB_SIZE);
		try {
			const view = this.bridge.createView(ptr, COLOR_RGB_SIZE);
			view.setUint8(0, color.r);
			view.setUint8(1, color.g);
			view.setUint8(2, color.b);
			this.bridge.getExports().ghostty_terminal_set(this.handle, option, ptr);
			this.requestRender();
		} finally {
			this.bridge.freeU8Array(ptr, COLOR_RGB_SIZE);
		}
	}

	/** Read a GhosttyString field from terminal_get. */
	private readString(fieldId: number): string {
		const exp = this.bridge.getExports();
		// GhosttyString = { ptr: u32, len: u32 } = 8 bytes
		const ptr = this.bridge.allocU8Array(8);
		try {
			const result = exp.ghostty_terminal_get(this.handle, fieldId, ptr);
			if (result !== 0) return '';
			const view = this.bridge.createView(ptr, 8);
			const strPtr = view.getUint32(0, true);
			const strLen = view.getUint32(4, true);
			if (strLen === 0) return '';
			return new TextDecoder().decode(new Uint8Array(exp.memory.buffer, strPtr, strLen));
		} finally {
			this.bridge.freeU8Array(ptr, 8);
		}
	}
}

function parseCssColor(color: string): RGB {
	const hex = color.startsWith('#') ? color.slice(1) : color;
	if (/^[\da-fA-F]{3}$/.test(hex)) {
		return {
			r: Number.parseInt(hex.charAt(0).repeat(2), 16),
			g: Number.parseInt(hex.charAt(1).repeat(2), 16),
			b: Number.parseInt(hex.charAt(2).repeat(2), 16),
		};
	}
	if (/^[\da-fA-F]{6}(?:[\da-fA-F]{2})?$/.test(hex)) {
		return {
			r: Number.parseInt(hex.slice(0, 2), 16),
			g: Number.parseInt(hex.slice(2, 4), 16),
			b: Number.parseInt(hex.slice(4, 6), 16),
		};
	}
	const rgb = color.match(/^rgba?\(\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)/);
	if (rgb) {
		return {
			r: Math.min(255, Number(rgb[1])),
			g: Math.min(255, Number(rgb[2])),
			b: Math.min(255, Number(rgb[3])),
		};
	}
	throw new Error(`Unsupported terminal color: ${color}`);
}

function packRgb(color: RGB): number {
	return (color.r << 16) | (color.g << 8) | color.b;
}

function rgbToCss(color: RGB): string {
	return `rgb(${color.r}, ${color.g}, ${color.b})`;
}

function colorSchemeFor(background: RGB): 0 | 1 {
	const luminance = (0.2126 * background.r + 0.7152 * background.g + 0.0722 * background.b) / 255;
	return luminance < 0.5 ? 1 : 0;
}

function buildColorPalette(theme: ITheme): Uint8Array {
	const resolved = { ...DEFAULT_THEME, ...theme };
	const colors = [
		resolved.black,
		resolved.red,
		resolved.green,
		resolved.yellow,
		resolved.blue,
		resolved.magenta,
		resolved.cyan,
		resolved.white,
		resolved.brightBlack,
		resolved.brightRed,
		resolved.brightGreen,
		resolved.brightYellow,
		resolved.brightBlue,
		resolved.brightMagenta,
		resolved.brightCyan,
		resolved.brightWhite,
	];
	const palette = new Uint8Array(256 * 3);
	colors.forEach((value, index) => {
		const color = parseCssColor(value);
		palette.set([color.r, color.g, color.b], index * 3);
	});

	const levels = [0, 95, 135, 175, 215, 255];
	let index = 16;
	for (const red of levels) {
		for (const green of levels) {
			for (const blue of levels) {
				palette.set([red, green, blue], index * 3);
				index++;
			}
		}
	}
	for (let grayIndex = 0; grayIndex < 24; grayIndex++) {
		const value = 8 + grayIndex * 10;
		palette.set([value, value, value], (232 + grayIndex) * 3);
	}
	return palette;
}

const EMPTY_MODIFIERS: TerminalModifierState = {
	alt: false,
	control: false,
	meta: false,
	shift: false,
};

function collectContextMenuActionIds(items: readonly ContextMenuItem[]): readonly string[] {
	const ids = new Set<string>();
	const visit = (item: ContextMenuItem): void => {
		if (item.kind === 'action') ids.add(item.actionId);
		else if (item.kind === 'submenu') item.children.forEach(visit);
	};
	items.forEach(visit);
	return [...ids];
}

function resolveSemanticZone(
	semanticContent: CellSemanticContent,
): TerminalInteractionContext['semanticZone'] {
	if (semanticContent === CellSemanticContent.PROMPT) return 'prompt';
	if (semanticContent === CellSemanticContent.INPUT) return 'input';
	return 'output';
}

function isBufferCellInRange(
	column: number,
	row: number,
	range: { start: { x: number; y: number }; end: { x: number; y: number } },
): boolean {
	const afterStart = row > range.start.y || (row === range.start.y && column >= range.start.x);
	const beforeEnd = row < range.end.y || (row === range.end.y && column <= range.end.x);
	return afterStart && beforeEnd;
}

function terminalSearchMatchesEqual(
	left: TerminalSearchMatch,
	right: TerminalSearchMatch,
): boolean {
	return (
		left.start.column === right.start.column &&
		left.start.row === right.start.row &&
		left.end.column === right.end.column &&
		left.end.row === right.end.row
	);
}
