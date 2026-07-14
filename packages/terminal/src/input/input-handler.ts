/**
 * InputHandler — converts browser keyboard/mouse events to terminal input.
 *
 * Handles:
 * - Keyboard event listening on a container element
 * - Mapping KeyboardEvent.code to USB HID Key codes
 * - Extracting modifier keys (Ctrl, Alt, Shift, Meta)
 * - Encoding keys using Ghostty's KeyEncoder (via WasmBridge)
 * - Mouse tracking (X10, SGR, button-motion, any-motion)
 * - Paste (bracketed paste mode)
 * - IME/composition events
 * - De-duplication between keydown/beforeinput/composition/paste
 */

import type { WasmBridge, WasmTypeLayout } from '../wasm-bridge';
import {
	type IKeyEvent,
	Key,
	KeyAction,
	Mods,
	Result,
	TerminalData,
	type WasmExports,
} from '../types';
import { shouldSuppressComposingControlInput, type CompositionSelection } from './preedit';
import { isSurfaceMouseModifierKey } from './surface-mouse';

// ============================================================================
// KeyEncoder (WASM-backed)
// ============================================================================

/**
 * KeyEncoder — wraps the Ghostty WASM key encoder.
 *
 * Uses WasmBridge for all memory operations instead of touching
 * WASM memory directly.
 */
class KeyEncoder {
	private readonly bridge: WasmBridge;
	private readonly exports: WasmExports;
	private encoder = 0;

	constructor(bridge: WasmBridge) {
		this.bridge = bridge;
		this.exports = bridge.getExports();
		this.encoder = bridge.allocAndReadOpaque((alloc, ptrPtr) =>
			this.exports.ghostty_key_encoder_new(alloc, ptrPtr),
		);
	}

	syncFromTerminal(terminal: number): void {
		this.exports.ghostty_key_encoder_setopt_from_terminal(this.encoder, terminal);
	}

	encode(event: {
		action: KeyAction;
		key: Key;
		mods: Mods;
		consumedMods?: Mods;
		composing?: boolean;
		utf8?: string;
		unshiftedCodepoint?: number;
	}): Uint8Array {
		// Create key event
		const eventPtr = this.bridge.allocAndReadOpaque((alloc, ptrPtr) =>
			this.exports.ghostty_key_event_new(alloc, ptrPtr),
		);

		this.exports.ghostty_key_event_set_action(eventPtr, event.action);
		this.exports.ghostty_key_event_set_key(eventPtr, event.key);
		this.exports.ghostty_key_event_set_mods(eventPtr, event.mods);
		this.exports.ghostty_key_event_set_consumed_mods(eventPtr, event.consumedMods ?? Mods.NONE);
		this.exports.ghostty_key_event_set_composing(eventPtr, event.composing ? 1 : 0);
		if (event.unshiftedCodepoint !== undefined) {
			this.exports.ghostty_key_event_set_unshifted_codepoint(eventPtr, event.unshiftedCodepoint);
		}

		const utf8Bytes = event.utf8 ? new TextEncoder().encode(event.utf8) : undefined;
		let utf8Ptr = 0;
		let bufferSize = 32;
		let bufPtr = 0;
		let writtenPtr = 0;

		try {
			if (utf8Bytes) {
				utf8Ptr = requireWasmPointer(
					this.bridge.allocU8Array(utf8Bytes.length),
					'key UTF-8 buffer',
				);
				this.bridge.writeBytes(utf8Ptr, utf8Bytes);
				this.exports.ghostty_key_event_set_utf8(eventPtr, utf8Ptr, utf8Bytes.length);
			}
			bufPtr = requireWasmPointer(this.bridge.allocU8Array(bufferSize), 'key encoding buffer');
			writtenPtr = requireWasmPointer(this.bridge.allocUsize(), 'key encoding length slot');

			let encodeResult = this.exports.ghostty_key_encoder_encode(
				this.encoder,
				eventPtr,
				bufPtr,
				bufferSize,
				writtenPtr,
			);
			if (encodeResult === -3) {
				const required = this.bridge.createView(writtenPtr, 4).getUint32(0, true);
				const replacement = requireWasmPointer(
					this.bridge.allocU8Array(required),
					'expanded key encoding buffer',
				);
				this.bridge.freeU8Array(bufPtr, bufferSize);
				bufferSize = required;
				bufPtr = replacement;
				encodeResult = this.exports.ghostty_key_encoder_encode(
					this.encoder,
					eventPtr,
					bufPtr,
					bufferSize,
					writtenPtr,
				);
			}
			if (encodeResult !== 0) {
				throw new Error(`Failed to encode key: ${encodeResult}`);
			}

			const bytesWritten = this.bridge.createView(writtenPtr, 4).getUint32(0, true);
			return this.bridge.readBytes(bufPtr, bytesWritten);
		} finally {
			if (utf8Bytes && utf8Ptr !== 0) this.bridge.freeU8Array(utf8Ptr, utf8Bytes.length);
			if (bufPtr !== 0) this.bridge.freeU8Array(bufPtr, bufferSize);
			if (writtenPtr !== 0) this.bridge.freeUsize(writtenPtr);
			this.exports.ghostty_key_event_free(eventPtr);
		}
	}

	dispose(): void {
		if (this.encoder) {
			this.exports.ghostty_key_encoder_free(this.encoder);
			this.encoder = 0;
		}
	}
}

// ============================================================================
// MouseEncoder (WASM-backed)
// ============================================================================

const MouseAction = {
	PRESS: 0,
	RELEASE: 1,
	MOTION: 2,
} as const;

const MouseButton = {
	LEFT: 1,
	RIGHT: 2,
	MIDDLE: 3,
	FOUR: 4,
	FIVE: 5,
	SIX: 6,
	SEVEN: 7,
} as const;

const MouseEncoderOption = {
	SIZE: 2,
	ANY_BUTTON_PRESSED: 3,
	TRACK_LAST_CELL: 4,
} as const;

interface MouseEncodingEvent {
	readonly action: (typeof MouseAction)[keyof typeof MouseAction];
	readonly button: number | null;
	readonly mods: Mods;
	readonly x: number;
	readonly y: number;
	readonly anyButtonPressed: boolean;
	readonly screenWidth: number;
	readonly screenHeight: number;
	readonly cellWidth: number;
	readonly cellHeight: number;
}

class MouseEncoder {
	private readonly bridge: WasmBridge;
	private readonly exports: WasmExports;
	private readonly positionPointer: number;
	private readonly positionSize: number;
	private readonly positionXOffset: number;
	private readonly positionYOffset: number;
	private readonly sizePointer: number;
	private readonly sizeSize: number;
	private readonly sizeOffsets: readonly number[];
	private readonly optionPointer: number;
	private encoder: number;
	private event: number;
	private configurationSignature = '';
	private disposed = false;

	constructor(bridge: WasmBridge) {
		this.bridge = bridge;
		this.exports = bridge.getExports();
		const positionLayout = bridge.getTypeLayout('GhosttyMousePosition');
		this.positionSize = positionLayout.size;
		this.positionXOffset = getTypeFieldOffset(positionLayout, 'x');
		this.positionYOffset = getTypeFieldOffset(positionLayout, 'y');
		const sizeLayout = bridge.getTypeLayout('GhosttyMouseEncoderSize');
		this.sizeSize = sizeLayout.size;
		this.sizeOffsets = [
			'size',
			'screen_width',
			'screen_height',
			'cell_width',
			'cell_height',
			'padding_top',
			'padding_bottom',
			'padding_right',
			'padding_left',
		].map((field) => getTypeFieldOffset(sizeLayout, field));
		let positionPointer = 0;
		let sizePointer = 0;
		let optionPointer = 0;
		let encoder = 0;
		let event = 0;
		try {
			positionPointer = requireWasmPointer(
				this.exports.ghostty_alloc(0, positionLayout.size),
				'mouse position',
			);
			sizePointer = requireWasmPointer(
				this.exports.ghostty_alloc(0, sizeLayout.size),
				'mouse encoder size',
			);
			optionPointer = requireWasmPointer(bridge.allocU8(), 'mouse encoder option');
			encoder = bridge.allocAndReadOpaque((allocator, output) =>
				this.exports.ghostty_mouse_encoder_new(allocator, output),
			);
			event = bridge.allocAndReadOpaque((allocator, output) =>
				this.exports.ghostty_mouse_event_new(allocator, output),
			);
			bridge.createView(optionPointer, 1).setUint8(0, 1);
			this.exports.ghostty_mouse_encoder_setopt(
				encoder,
				MouseEncoderOption.TRACK_LAST_CELL,
				optionPointer,
			);
		} catch (error) {
			if (event !== 0) this.exports.ghostty_mouse_event_free(event);
			if (encoder !== 0) this.exports.ghostty_mouse_encoder_free(encoder);
			if (optionPointer !== 0) bridge.freeU8(optionPointer);
			if (sizePointer !== 0) this.exports.ghostty_free(0, sizePointer, sizeLayout.size);
			if (positionPointer !== 0) {
				this.exports.ghostty_free(0, positionPointer, positionLayout.size);
			}
			throw error;
		}
		this.positionPointer = positionPointer;
		this.sizePointer = sizePointer;
		this.optionPointer = optionPointer;
		this.encoder = encoder;
		this.event = event;
	}

	syncFromTerminal(terminal: number, signature: string): void {
		if (signature === this.configurationSignature) return;
		this.exports.ghostty_mouse_encoder_setopt_from_terminal(this.encoder, terminal);
		this.configurationSignature = signature;
	}

	encode(input: MouseEncodingEvent): Uint8Array {
		const position = this.bridge.createView(this.positionPointer, this.positionSize);
		position.setFloat32(this.positionXOffset, input.x, true);
		position.setFloat32(this.positionYOffset, input.y, true);

		const size = this.bridge.createView(this.sizePointer, this.sizeSize);
		const values = [
			this.sizeSize,
			input.screenWidth,
			input.screenHeight,
			input.cellWidth,
			input.cellHeight,
			0,
			0,
			0,
			0,
		];
		for (const [index, value] of values.entries()) {
			const offset = this.sizeOffsets[index];
			if (offset === undefined) throw new Error('Ghostty mouse size layout is incomplete');
			size.setUint32(offset, value, true);
		}

		this.exports.ghostty_mouse_encoder_setopt(
			this.encoder,
			MouseEncoderOption.SIZE,
			this.sizePointer,
		);
		this.bridge.createView(this.optionPointer, 1).setUint8(0, input.anyButtonPressed ? 1 : 0);
		this.exports.ghostty_mouse_encoder_setopt(
			this.encoder,
			MouseEncoderOption.ANY_BUTTON_PRESSED,
			this.optionPointer,
		);

		this.exports.ghostty_mouse_event_set_action(this.event, input.action);
		if (input.button === null) this.exports.ghostty_mouse_event_clear_button(this.event);
		else this.exports.ghostty_mouse_event_set_button(this.event, input.button);
		this.exports.ghostty_mouse_event_set_mods(this.event, input.mods);
		this.exports.ghostty_mouse_event_set_position(this.event, this.positionPointer);

		let bufferSize = 32;
		let buffer = 0;
		let written = 0;
		try {
			buffer = requireWasmPointer(this.bridge.allocU8Array(bufferSize), 'mouse encoding buffer');
			written = requireWasmPointer(this.bridge.allocUsize(), 'mouse encoding length slot');
			let result = this.exports.ghostty_mouse_encoder_encode(
				this.encoder,
				this.event,
				buffer,
				bufferSize,
				written,
			);
			if (result === Result.OUT_OF_SPACE) {
				const required = this.bridge.createView(written, 4).getUint32(0, true);
				const replacement = requireWasmPointer(
					this.bridge.allocU8Array(required),
					'expanded mouse encoding buffer',
				);
				this.bridge.freeU8Array(buffer, bufferSize);
				bufferSize = required;
				buffer = replacement;
				result = this.exports.ghostty_mouse_encoder_encode(
					this.encoder,
					this.event,
					buffer,
					bufferSize,
					written,
				);
			}
			if (result !== Result.SUCCESS) {
				throw new Error(`Failed to encode mouse event: ${result}`);
			}
			const length = this.bridge.createView(written, 4).getUint32(0, true);
			return this.bridge.readBytes(buffer, length);
		} finally {
			if (buffer !== 0) this.bridge.freeU8Array(buffer, bufferSize);
			if (written !== 0) this.bridge.freeUsize(written);
		}
	}

	dispose(): void {
		if (this.disposed) return;
		if (this.event) {
			this.exports.ghostty_mouse_event_free(this.event);
			this.event = 0;
		}
		if (this.encoder) {
			this.exports.ghostty_mouse_encoder_free(this.encoder);
			this.encoder = 0;
		}
		this.exports.ghostty_free(0, this.positionPointer, this.positionSize);
		this.exports.ghostty_free(0, this.sizePointer, this.sizeSize);
		this.bridge.freeU8(this.optionPointer);
		this.disposed = true;
	}
}

function getTypeFieldOffset(layout: WasmTypeLayout, field: string): number {
	const value = layout.fields[field];
	if (!value) throw new Error(`Ghostty type field is missing: ${field}`);
	return value.offset;
}

function requireWasmPointer(pointer: number, purpose: string): number {
	if (pointer === 0) throw new Error(`WASM allocation failed: ${purpose}`);
	return pointer;
}

// ============================================================================
// Key Map
// ============================================================================

/**
 * Map KeyboardEvent.code values to USB HID Key values.
 * Based on: https://developer.mozilla.org/en-US/docs/Web/API/KeyboardEvent/code
 */
const KEY_MAP: Record<string, Key> = {
	// Letters
	KeyA: Key.A,
	KeyB: Key.B,
	KeyC: Key.C,
	KeyD: Key.D,
	KeyE: Key.E,
	KeyF: Key.F,
	KeyG: Key.G,
	KeyH: Key.H,
	KeyI: Key.I,
	KeyJ: Key.J,
	KeyK: Key.K,
	KeyL: Key.L,
	KeyM: Key.M,
	KeyN: Key.N,
	KeyO: Key.O,
	KeyP: Key.P,
	KeyQ: Key.Q,
	KeyR: Key.R,
	KeyS: Key.S,
	KeyT: Key.T,
	KeyU: Key.U,
	KeyV: Key.V,
	KeyW: Key.W,
	KeyX: Key.X,
	KeyY: Key.Y,
	KeyZ: Key.Z,

	// Numbers
	Digit1: Key.ONE,
	Digit2: Key.TWO,
	Digit3: Key.THREE,
	Digit4: Key.FOUR,
	Digit5: Key.FIVE,
	Digit6: Key.SIX,
	Digit7: Key.SEVEN,
	Digit8: Key.EIGHT,
	Digit9: Key.NINE,
	Digit0: Key.ZERO,

	// Special keys
	AltLeft: Key.ALT_LEFT,
	AltRight: Key.ALT_RIGHT,
	Enter: Key.ENTER,
	Escape: Key.ESCAPE,
	Backspace: Key.BACKSPACE,
	ControlLeft: Key.CONTROL_LEFT,
	ControlRight: Key.CONTROL_RIGHT,
	MetaLeft: Key.META_LEFT,
	MetaRight: Key.META_RIGHT,
	OSLeft: Key.META_LEFT,
	OSRight: Key.META_RIGHT,
	ShiftLeft: Key.SHIFT_LEFT,
	ShiftRight: Key.SHIFT_RIGHT,
	Tab: Key.TAB,
	Space: Key.SPACE,

	// Punctuation
	Minus: Key.MINUS,
	Equal: Key.EQUAL,
	BracketLeft: Key.BRACKET_LEFT,
	BracketRight: Key.BRACKET_RIGHT,
	Backslash: Key.BACKSLASH,
	Semicolon: Key.SEMICOLON,
	Quote: Key.QUOTE,
	Backquote: Key.GRAVE,
	Comma: Key.COMMA,
	Period: Key.PERIOD,
	Slash: Key.SLASH,

	// Function keys
	CapsLock: Key.CAPS_LOCK,
	F1: Key.F1,
	F2: Key.F2,
	F3: Key.F3,
	F4: Key.F4,
	F5: Key.F5,
	F6: Key.F6,
	F7: Key.F7,
	F8: Key.F8,
	F9: Key.F9,
	F10: Key.F10,
	F11: Key.F11,
	F12: Key.F12,
	F13: Key.F13,
	F14: Key.F14,
	F15: Key.F15,
	F16: Key.F16,
	F17: Key.F17,
	F18: Key.F18,
	F19: Key.F19,
	F20: Key.F20,
	F21: Key.F21,
	F22: Key.F22,
	F23: Key.F23,
	F24: Key.F24,
	F25: Key.F25,

	// Special function keys
	Fn: Key.FN,
	FnLock: Key.FN_LOCK,
	PrintScreen: Key.PRINT_SCREEN,
	ScrollLock: Key.SCROLL_LOCK,
	Pause: Key.PAUSE,
	Insert: Key.INSERT,
	Home: Key.HOME,
	PageUp: Key.PAGE_UP,
	Delete: Key.DELETE,
	End: Key.END,
	PageDown: Key.PAGE_DOWN,

	// Arrow keys
	ArrowRight: Key.RIGHT,
	ArrowLeft: Key.LEFT,
	ArrowDown: Key.DOWN,
	ArrowUp: Key.UP,

	// Keypad
	NumLock: Key.NUM_LOCK,
	Numpad0: Key.NUMPAD_0,
	Numpad1: Key.NUMPAD_1,
	Numpad2: Key.NUMPAD_2,
	Numpad3: Key.NUMPAD_3,
	Numpad4: Key.NUMPAD_4,
	Numpad5: Key.NUMPAD_5,
	Numpad6: Key.NUMPAD_6,
	Numpad7: Key.NUMPAD_7,
	Numpad8: Key.NUMPAD_8,
	Numpad9: Key.NUMPAD_9,
	NumpadAdd: Key.NUMPAD_ADD,
	NumpadBackspace: Key.NUMPAD_BACKSPACE,
	NumpadClear: Key.NUMPAD_CLEAR,
	NumpadClearEntry: Key.NUMPAD_CLEAR_ENTRY,
	NumpadComma: Key.NUMPAD_COMMA,
	NumpadDecimal: Key.NUMPAD_DECIMAL,
	NumpadDivide: Key.NUMPAD_DIVIDE,
	NumpadEnter: Key.NUMPAD_ENTER,
	NumpadEqual: Key.NUMPAD_EQUAL,
	NumpadMemoryAdd: Key.NUMPAD_MEMORY_ADD,
	NumpadMemoryClear: Key.NUMPAD_MEMORY_CLEAR,
	NumpadMemoryRecall: Key.NUMPAD_MEMORY_RECALL,
	NumpadMemoryStore: Key.NUMPAD_MEMORY_STORE,
	NumpadMemorySubtract: Key.NUMPAD_MEMORY_SUBTRACT,
	NumpadMultiply: Key.NUMPAD_MULTIPLY,
	NumpadParenLeft: Key.NUMPAD_PAREN_LEFT,
	NumpadParenRight: Key.NUMPAD_PAREN_RIGHT,
	NumpadSubtract: Key.NUMPAD_SUBTRACT,
	NumpadSeparator: Key.NUMPAD_SEPARATOR,
	NumpadUp: Key.NUMPAD_UP,
	NumpadDown: Key.NUMPAD_DOWN,
	NumpadRight: Key.NUMPAD_RIGHT,
	NumpadLeft: Key.NUMPAD_LEFT,
	NumpadBegin: Key.NUMPAD_BEGIN,
	NumpadHome: Key.NUMPAD_HOME,
	NumpadEnd: Key.NUMPAD_END,
	NumpadInsert: Key.NUMPAD_INSERT,
	NumpadDelete: Key.NUMPAD_DELETE,
	NumpadPageUp: Key.NUMPAD_PAGE_UP,
	NumpadPageDown: Key.NUMPAD_PAGE_DOWN,

	// International
	IntlBackslash: Key.INTL_BACKSLASH,
	IntlRo: Key.INTL_RO,
	IntlYen: Key.INTL_YEN,
	Convert: Key.CONVERT,
	KanaMode: Key.KANA_MODE,
	NonConvert: Key.NON_CONVERT,
	ContextMenu: Key.CONTEXT_MENU,
	Help: Key.HELP,

	// Media and browser keys
	BrowserBack: Key.BROWSER_BACK,
	BrowserFavorites: Key.BROWSER_FAVORITES,
	BrowserForward: Key.BROWSER_FORWARD,
	BrowserHome: Key.BROWSER_HOME,
	BrowserRefresh: Key.BROWSER_REFRESH,
	BrowserSearch: Key.BROWSER_SEARCH,
	BrowserStop: Key.BROWSER_STOP,
	Eject: Key.EJECT,
	LaunchApp1: Key.LAUNCH_APP_1,
	LaunchApp2: Key.LAUNCH_APP_2,
	LaunchMail: Key.LAUNCH_MAIL,
	MediaPlayPause: Key.MEDIA_PLAY_PAUSE,
	MediaSelect: Key.MEDIA_SELECT,
	MediaStop: Key.MEDIA_STOP,
	MediaTrackNext: Key.MEDIA_TRACK_NEXT,
	MediaTrackPrevious: Key.MEDIA_TRACK_PREVIOUS,
	Power: Key.POWER,
	Sleep: Key.SLEEP,
	AudioVolumeDown: Key.AUDIO_VOLUME_DOWN,
	AudioVolumeMute: Key.AUDIO_VOLUME_MUTE,
	AudioVolumeUp: Key.AUDIO_VOLUME_UP,
	WakeUp: Key.WAKE_UP,
	Copy: Key.COPY,
	Cut: Key.CUT,
	Paste: Key.PASTE,
};

const UNSHIFTED_CHARACTERS: Readonly<Record<string, string>> = {
	Digit0: '0',
	Digit1: '1',
	Digit2: '2',
	Digit3: '3',
	Digit4: '4',
	Digit5: '5',
	Digit6: '6',
	Digit7: '7',
	Digit8: '8',
	Digit9: '9',
	Space: ' ',
	Minus: '-',
	Equal: '=',
	BracketLeft: '[',
	BracketRight: ']',
	Backslash: '\\',
	Semicolon: ';',
	Quote: "'",
	Backquote: '`',
	Comma: ',',
	Period: '.',
	Slash: '/',
};

// ============================================================================
// Dependencies Interface
// ============================================================================

/** Dependencies the InputHandler needs from its host. */
export interface InputHandlerDependencies {
	/** WASM bridge for creating the KeyEncoder. */
	bridge: WasmBridge;
	/** Current Ghostty terminal handle used to synchronize encoder modes. */
	getTerminalHandle(): number;
	/** Check if any mouse tracking mode is enabled. */
	hasMouseTracking(): boolean;
	/** Get cell dimensions for pixel-to-cell conversion. */
	getCellDimensions(): { width: number; height: number };
	/** Get the rendered surface dimensions in CSS pixels. */
	getScreenDimensions(): { width: number; height: number };
	/** Get canvas/container offset for accurate position calculation. */
	getCanvasOffset(): { left: number; top: number };
	/** Query a terminal mode (DEC or ANSI). */
	getMode(mode: number, isAnsi?: boolean): boolean;
	/** Emit non-UTF-8 mouse bytes through the host's binary input channel. */
	emitBinary?(data: string): void;
	/** Refresh pointer presentation when a mouse-shape modifier changes. */
	onSurfaceModifierChange?(event: KeyboardEvent): void;
}

// ============================================================================
// InputHandler
// ============================================================================

export class InputHandler {
	private readonly encoder: KeyEncoder;
	private readonly mouseEncoder: MouseEncoder;
	private readonly deps: InputHandlerDependencies;
	private container: HTMLElement;
	private inputElement?: HTMLElement | undefined;
	private onDataCallback: (data: string) => void;
	private onBellCallback: (() => void) | undefined;
	private onKeyCallback?: ((keyEvent: IKeyEvent) => void) | undefined;
	private customKeyEventHandler?: ((event: KeyboardEvent) => boolean) | undefined;
	private onCopyCallback?: (() => boolean) | undefined;
	private onCompositionCallback?:
		| ((data: string | null, selection?: CompositionSelection) => void)
		| undefined;
	private onPasteCallback?:
		| ((data: string, source: 'clipboard' | 'beforeinput') => void)
		| undefined;
	private onActionCallback?: ((id: string) => boolean) | undefined;
	private onErrorCallback?: ((error: unknown) => void) | undefined;

	// Event listener references (for cleanup)
	private keydownListener: ((e: KeyboardEvent) => void) | null = null;
	private keyupListener: ((e: KeyboardEvent) => void) | null = null;
	private pasteListener: ((e: ClipboardEvent) => void) | null = null;
	private beforeInputListener: ((e: InputEvent) => void) | null = null;
	private compositionStartListener: ((e: CompositionEvent) => void) | null = null;
	private compositionUpdateListener: ((e: CompositionEvent) => void) | null = null;
	private compositionEndListener: ((e: CompositionEvent) => void) | null = null;
	private mousedownListener: ((e: MouseEvent) => void) | null = null;
	private mouseupListener: ((e: MouseEvent) => void) | null = null;
	private mousemoveListener: ((e: MouseEvent) => void) | null = null;
	private wheelListener: ((e: WheelEvent) => void) | null = null;

	// State
	private isComposing = false;
	private isDisposed = false;
	private mouseButtonsPressed = 0;
	private pendingWheelX = 0;
	private pendingWheelY = 0;
	private readonly pressedKeys = new Map<
		string,
		{
			readonly key: Key;
			readonly mods: Mods;
			readonly consumedMods?: Mods;
			readonly utf8?: string;
			readonly unshiftedCodepoint?: number;
		}
	>();

	// De-duplication state
	private lastKeyDownData: string | null = null;
	private lastKeyDownTime = 0;
	private lastPasteData: string | null = null;
	private lastPasteTime = 0;
	private lastPasteSource: 'paste' | 'beforeinput' | null = null;
	private lastCompositionData: string | null = null;
	private lastCompositionTime = 0;
	private lastBeforeInputData: string | null = null;
	private lastBeforeInputTime = 0;

	private static readonly BEFORE_INPUT_IGNORE_MS = 100;

	constructor(
		deps: InputHandlerDependencies,
		container: HTMLElement,
		onData: (data: string) => void,
		onBell: () => void,
		onKey?: (keyEvent: IKeyEvent) => void,
		customKeyEventHandler?: (event: KeyboardEvent) => boolean,
		onCopy?: () => boolean,
		inputElement?: HTMLElement,
		onComposition?: (data: string | null, selection?: CompositionSelection) => void,
		onPaste?: (data: string, source: 'clipboard' | 'beforeinput') => void,
		onAction?: (id: string) => boolean,
		onError?: (error: unknown) => void,
	) {
		this.deps = deps;
		this.encoder = new KeyEncoder(deps.bridge);
		try {
			this.mouseEncoder = new MouseEncoder(deps.bridge);
		} catch (error) {
			this.encoder.dispose();
			throw error;
		}
		this.container = container;
		this.inputElement = inputElement;
		this.onDataCallback = onData;
		this.onBellCallback = onBell;
		this.onKeyCallback = onKey;
		this.customKeyEventHandler = customKeyEventHandler;
		this.onCopyCallback = onCopy;
		this.onCompositionCallback = onComposition;
		this.onPasteCallback = onPaste;
		this.onActionCallback = onAction;
		this.onErrorCallback = onError;

		try {
			this.attach();
		} catch (error) {
			this.dispose();
			throw error;
		}
	}

	/** Trigger the bell callback. */
	bell(): void {
		this.onBellCallback?.();
	}

	/** Set custom key event handler (for runtime updates). */
	setCustomKeyEventHandler(handler: (event: KeyboardEvent) => boolean): void {
		this.customKeyEventHandler = handler;
	}

	// ==========================================================================
	// Event Listener Attachment
	// ==========================================================================

	private attach(): void {
		if (
			typeof this.container.hasAttribute === 'function' &&
			typeof this.container.setAttribute === 'function'
		) {
			if (!this.container.hasAttribute('tabindex')) {
				this.container.setAttribute('tabindex', '0');
			}
			if (this.container.style) {
				this.container.style.outline = 'none';
			}
		}

		this.keydownListener = this.handleKeyDown.bind(this);
		this.container.addEventListener('keydown', this.keydownListener);
		this.keyupListener = this.handleKeyUp.bind(this);
		this.container.addEventListener('keyup', this.keyupListener);

		this.pasteListener = this.handlePaste.bind(this);
		this.container.addEventListener('paste', this.pasteListener);
		if (this.inputElement && this.inputElement !== this.container) {
			this.inputElement.addEventListener('paste', this.pasteListener);
		}

		if (this.inputElement) {
			this.beforeInputListener = this.handleBeforeInput.bind(this);
			this.inputElement.addEventListener('beforeinput', this.beforeInputListener);
		}

		this.compositionStartListener = this.handleCompositionStart.bind(this);
		this.container.addEventListener('compositionstart', this.compositionStartListener);

		this.compositionUpdateListener = this.handleCompositionUpdate.bind(this);
		this.container.addEventListener('compositionupdate', this.compositionUpdateListener);

		this.compositionEndListener = this.handleCompositionEnd.bind(this);
		this.container.addEventListener('compositionend', this.compositionEndListener);

		this.mousedownListener = this.handleMouseDown.bind(this);
		this.container.addEventListener('mousedown', this.mousedownListener);

		this.mouseupListener = this.handleMouseUp.bind(this);
		this.container.addEventListener('mouseup', this.mouseupListener);

		this.mousemoveListener = this.handleMouseMove.bind(this);
		this.container.addEventListener('mousemove', this.mousemoveListener);

		this.wheelListener = this.handleWheel.bind(this);
		this.container.addEventListener('wheel', this.wheelListener, { passive: false });
	}

	// ==========================================================================
	// Key Mapping
	// ==========================================================================

	private mapKeyCode(code: string): Key | null {
		return KEY_MAP[code] ?? null;
	}

	private extractModifiers(event: KeyboardEvent): Mods {
		let mods = Mods.NONE;

		if (event.shiftKey) mods |= Mods.SHIFT;
		if (event.ctrlKey) mods |= Mods.CTRL;
		if (event.altKey) mods |= Mods.ALT;
		if (event.metaKey) mods |= Mods.SUPER;
		if (event.getModifierState?.('CapsLock')) mods |= Mods.CAPSLOCK;
		if (event.getModifierState?.('NumLock')) mods |= Mods.NUMLOCK;
		if (event.location === 2) {
			if (event.shiftKey) mods |= Mods.SHIFT_SIDE;
			if (event.ctrlKey) mods |= Mods.CTRL_SIDE;
			if (event.altKey) mods |= Mods.ALT_SIDE;
			if (event.metaKey) mods |= Mods.SUPER_SIDE;
		}

		return mods;
	}

	private isPrintableCharacter(event: KeyboardEvent): boolean {
		if (event.ctrlKey && !event.altKey) return false;
		if (event.altKey && !event.ctrlKey) return false;
		if (event.metaKey) return false;
		return event.key.length === 1;
	}

	private getUnshiftedCodepoint(event: KeyboardEvent): number | undefined {
		if (/^Key[A-Z]$/.test(event.code)) return event.code.charCodeAt(3) + 32;
		return UNSHIFTED_CHARACTERS[event.code]?.codePointAt(0);
	}

	private getConsumedModifiers(event: KeyboardEvent, utf8: string | undefined): Mods {
		if (!utf8) return Mods.NONE;
		let consumed = Mods.NONE;
		const unshifted = this.getUnshiftedCodepoint(event);
		if (event.shiftKey && unshifted !== undefined && unshifted !== utf8.codePointAt(0)) {
			consumed |= Mods.SHIFT;
		}
		if (event.getModifierState?.('AltGraph')) consumed |= Mods.CTRL | Mods.ALT;
		return consumed;
	}

	private encodeKeyEvent(event: {
		readonly action: KeyAction;
		readonly key: Key;
		readonly mods: Mods;
		readonly consumedMods?: Mods;
		readonly composing?: boolean;
		readonly utf8?: string;
		readonly unshiftedCodepoint?: number;
	}): string {
		this.encoder.syncFromTerminal(this.deps.getTerminalHandle());
		return new TextDecoder().decode(this.encoder.encode(event));
	}

	private reportsKeyReleases(): boolean {
		const outputPointer = this.deps.bridge.allocU8();
		try {
			const result = this.deps.bridge
				.getExports()
				.ghostty_terminal_get(
					this.deps.getTerminalHandle(),
					TerminalData.KITTY_KEYBOARD_FLAGS,
					outputPointer,
				);
			const flags = this.deps.bridge.readBytes(outputPointer, 1)[0] ?? 0;
			return result === 0 && (flags & (1 << 1)) !== 0;
		} finally {
			this.deps.bridge.freeU8(outputPointer);
		}
	}

	// ==========================================================================
	// Keyboard Event Handling
	// ==========================================================================

	private handleKeyDown(event: KeyboardEvent): void {
		if (this.isDisposed) return;
		if (isSurfaceMouseModifierKey(event.code)) this.deps.onSurfaceModifierChange?.(event);

		// Ignore keydown during composition
		if (this.isComposing || event.isComposing || event.keyCode === 229) {
			return;
		}

		// Emit onKey event first
		if (this.onKeyCallback) {
			this.onKeyCallback({ key: event.key, domEvent: event });
		}

		// Check custom key event handler
		if (this.customKeyEventHandler) {
			if (this.customKeyEventHandler(event)) {
				event.preventDefault();
				return;
			}
		}

		// Allow Ctrl+V / Cmd+V to trigger native paste
		if ((event.ctrlKey || event.metaKey) && event.code === 'KeyV') {
			return;
		}

		// Platform copy shortcuts are actions; Ctrl+C without Shift remains SIGINT.
		if ((event.metaKey || (event.ctrlKey && event.shiftKey)) && event.code === 'KeyC') {
			const consumed = this.onActionCallback?.('terminal.copy') ?? this.onCopyCallback?.() ?? false;
			if (consumed) {
				event.preventDefault();
			}
			return;
		}

		if (event.metaKey && event.code === 'KeyA') {
			if (this.onActionCallback?.('terminal.select-all')) event.preventDefault();
			return;
		}

		// Map physical key code
		const key = this.mapKeyCode(event.code);
		if (key === null) {
			if (this.isPrintableCharacter(event)) {
				event.preventDefault();
				try {
					const data = this.encodeKeyEvent({
						action: event.repeat ? KeyAction.REPEAT : KeyAction.PRESS,
						key: Key.UNIDENTIFIED,
						mods: this.extractModifiers(event),
						consumedMods: this.getConsumedModifiers(event, event.key),
						utf8: event.key,
					});
					if (data.length > 0) {
						this.onDataCallback(data);
						this.recordKeyDownData(data);
					}
				} catch (error) {
					this.onErrorCallback?.(error);
				}
			}
			return;
		}

		const mods = this.extractModifiers(event);
		const action = event.repeat ? KeyAction.REPEAT : KeyAction.PRESS;

		try {
			const utf8 = event.key.length === 1 ? event.key : undefined;
			const unshiftedCodepoint = this.getUnshiftedCodepoint(event);
			const consumedMods = this.getConsumedModifiers(event, utf8);

			const data = this.encodeKeyEvent({
				action,
				key,
				mods,
				consumedMods,
				...(utf8 !== undefined ? { utf8 } : {}),
				...(unshiftedCodepoint === undefined ? {} : { unshiftedCodepoint }),
			});

			if (!event.repeat) {
				this.pressedKeys.set(event.code, {
					key,
					mods,
					consumedMods,
					...(utf8 === undefined ? {} : { utf8 }),
					...(unshiftedCodepoint === undefined ? {} : { unshiftedCodepoint }),
				});
			}

			event.preventDefault();
			event.stopPropagation();

			if (data.length > 0) {
				this.onDataCallback(data);
				this.recordKeyDownData(data);
			}
		} catch (error) {
			this.onErrorCallback?.(error);
		}
	}

	private handleKeyUp(event: KeyboardEvent): void {
		if (this.isDisposed) return;
		if (isSurfaceMouseModifierKey(event.code)) this.deps.onSurfaceModifierChange?.(event);
		const key = this.mapKeyCode(event.code);
		if (key === null) return;
		this.pressedKeys.delete(event.code);
		if (this.isComposing || event.isComposing) return;
		if (!this.reportsKeyReleases()) return;

		try {
			const utf8 = event.key.length === 1 ? event.key : undefined;
			const unshiftedCodepoint = this.getUnshiftedCodepoint(event);
			const data = this.encodeKeyEvent({
				action: KeyAction.RELEASE,
				key,
				mods: this.extractModifiers(event),
				consumedMods: this.getConsumedModifiers(event, utf8),
				...(utf8 === undefined ? {} : { utf8 }),
				...(unshiftedCodepoint === undefined ? {} : { unshiftedCodepoint }),
			});
			if (data.length === 0) return;
			event.preventDefault();
			event.stopPropagation();
			this.onDataCallback(data);
		} catch (error) {
			this.onErrorCallback?.(error);
		}
	}

	// ==========================================================================
	// Paste Handling
	// ==========================================================================

	private handlePaste(event: ClipboardEvent): void {
		if (this.isDisposed) return;

		event.preventDefault();
		event.stopPropagation();

		const clipboardData = event.clipboardData;
		if (!clipboardData) return;

		const text = clipboardData.getData('text/plain');
		if (!text) return;

		if (this.shouldIgnorePasteEvent(text, 'paste')) return;

		this.emitPasteData(text, 'clipboard');
		this.recordPasteData(text, 'paste');
	}

	private emitPasteData(text: string, source: 'clipboard' | 'beforeinput'): void {
		if (this.onPasteCallback) {
			this.onPasteCallback(text, source);
			return;
		}
		const encoded = this.deps.bridge.encodePaste(text, this.deps.getMode(2004));
		this.onDataCallback(new TextDecoder().decode(encoded));
	}

	// ==========================================================================
	// BeforeInput Handling (mobile/IME)
	// ==========================================================================

	private handleBeforeInput(event: InputEvent): void {
		if (this.isDisposed) return;
		if (this.isComposing || event.isComposing) return;

		const inputType = event.inputType;
		const data = event.data ?? '';
		let key: Key = Key.UNIDENTIFIED;
		let utf8: string | undefined;

		switch (inputType) {
			case 'insertText':
			case 'insertReplacementText':
				if (data.length === 0) return;
				utf8 = data;
				break;
			case 'insertLineBreak':
			case 'insertParagraph':
				key = Key.ENTER;
				break;
			case 'deleteContentBackward':
				key = Key.BACKSPACE;
				break;
			case 'deleteContentForward':
				key = Key.DELETE;
				break;
			case 'insertFromPaste':
				if (!data) return;
				if (this.shouldIgnorePasteEvent(data, 'beforeinput')) {
					event.preventDefault();
					event.stopPropagation();
					return;
				}
				event.preventDefault();
				event.stopPropagation();
				this.emitPasteData(data, 'beforeinput');
				this.recordPasteData(data, 'beforeinput');
				return;
			default:
				return;
		}

		let output: string;
		try {
			output = this.encodeKeyEvent({
				action: KeyAction.PRESS,
				key,
				mods: Mods.NONE,
				...(utf8 === undefined ? {} : { utf8 }),
			});
		} catch (error) {
			this.onErrorCallback?.(error);
			return;
		}
		if (!output) return;

		if (this.shouldIgnoreBeforeInput(output)) {
			event.preventDefault();
			event.stopPropagation();
			return;
		}

		if (data && this.shouldIgnoreBeforeInputFromComposition(data)) {
			event.preventDefault();
			event.stopPropagation();
			return;
		}

		event.preventDefault();
		event.stopPropagation();
		this.onDataCallback(output);
		if (data) this.recordBeforeInputData(data);
	}

	// ==========================================================================
	// Composition Handling (IME/CJK)
	// ==========================================================================

	private handleCompositionStart(event: CompositionEvent): void {
		if (this.isDisposed) return;
		this.isComposing = true;
		const data = event.data ?? '';
		this.onCompositionCallback?.(data, this.readCompositionSelection(data));
	}

	private handleCompositionUpdate(event: CompositionEvent): void {
		if (this.isDisposed) return;
		const data = event.data ?? '';
		this.onCompositionCallback?.(data, this.readCompositionSelection(data));
	}

	private handleCompositionEnd(event: CompositionEvent): void {
		if (this.isDisposed) return;
		const wasComposing = this.isComposing;
		this.isComposing = false;
		this.onCompositionCallback?.(null);

		const data = event.data;
		if (data && data.length > 0) {
			if (shouldSuppressComposingControlInput(data, wasComposing)) {
				this.clearInputElement();
				this.cleanupCompositionTextNodes();
				return;
			}
			if (this.shouldIgnoreCompositionEnd(data)) {
				this.clearInputElement();
				this.cleanupCompositionTextNodes();
				return;
			}
			try {
				const encoded = this.encodeKeyEvent({
					action: KeyAction.PRESS,
					key: Key.UNIDENTIFIED,
					mods: Mods.NONE,
					consumedMods: Mods.NONE,
					composing: false,
					utf8: data,
				});
				if (encoded.length > 0) {
					this.onDataCallback(encoded);
					this.recordCompositionData(data);
				}
			} catch (error) {
				this.onErrorCallback?.(error);
			}
		}

		this.clearInputElement();
		this.cleanupCompositionTextNodes();
	}

	private clearInputElement(): void {
		if (this.inputElement && 'value' in this.inputElement) {
			this.inputElement.value = '';
		}
	}

	private readCompositionSelection(data: string): CompositionSelection {
		const input = this.inputElement;
		if (!input || !('selectionStart' in input) || !('selectionEnd' in input)) {
			return { start: data.length, end: data.length };
		}
		const start = typeof input.selectionStart === 'number' ? input.selectionStart : data.length;
		const end = typeof input.selectionEnd === 'number' ? input.selectionEnd : start;
		const clampedStart = Math.max(0, Math.min(data.length, start));
		return {
			start: clampedStart,
			end: Math.max(clampedStart, Math.min(data.length, end)),
		};
	}

	private cleanupCompositionTextNodes(): void {
		if (this.container && this.container.childNodes) {
			for (let i = this.container.childNodes.length - 1; i >= 0; i--) {
				const node = this.container.childNodes[i];
				if (!node) continue;
				if (node.nodeType === 3) {
					this.container.removeChild(node);
				}
			}
		}
	}

	// ==========================================================================
	// Mouse Event Handling
	// ==========================================================================

	private getMousePosition(event: MouseEvent): { x: number; y: number } | null {
		const dims = this.deps.getCellDimensions();
		const offset = this.deps.getCanvasOffset();

		if (dims.width <= 0 || dims.height <= 0) return null;

		const x = event.clientX - offset.left;
		const y = event.clientY - offset.top;
		return { x, y };
	}

	private getMouseModifiers(event: MouseEvent): Mods {
		let mods = Mods.NONE;
		if (event.shiftKey) mods |= Mods.SHIFT;
		if (event.ctrlKey) mods |= Mods.CTRL;
		if (event.altKey) mods |= Mods.ALT;
		if (event.metaKey) mods |= Mods.SUPER;
		return mods;
	}

	private mapMouseButton(button: number): number | null {
		if (button === 0) return MouseButton.LEFT;
		if (button === 1) return MouseButton.MIDDLE;
		if (button === 2) return MouseButton.RIGHT;
		if (button === 3) return MouseButton.FOUR;
		if (button === 4) return MouseButton.FIVE;
		return null;
	}

	private getPressedMouseButton(): number | null {
		for (const button of [0, 1, 2, 3, 4]) {
			if ((this.mouseButtonsPressed & (1 << button)) !== 0) return this.mapMouseButton(button);
		}
		return null;
	}

	private getMouseConfigurationSignature(): string {
		return [9, 1000, 1002, 1003, 1005, 1006, 1015, 1016]
			.map((mode) => (this.deps.getMode(mode) ? '1' : '0'))
			.join('');
	}

	private sendMouseEvent(
		action: (typeof MouseAction)[keyof typeof MouseAction],
		button: number | null,
		event: MouseEvent,
	): void {
		const position = this.getMousePosition(event);
		if (!position) return;
		const cell = this.deps.getCellDimensions();
		const screen = this.deps.getScreenDimensions();
		this.mouseEncoder.syncFromTerminal(
			this.deps.getTerminalHandle(),
			this.getMouseConfigurationSignature(),
		);
		const encoded = this.mouseEncoder.encode({
			action,
			button,
			mods: this.getMouseModifiers(event),
			x: position.x,
			y: position.y,
			anyButtonPressed: this.mouseButtonsPressed !== 0,
			screenWidth: Math.max(1, Math.round(screen.width)),
			screenHeight: Math.max(1, Math.round(screen.height)),
			cellWidth: Math.max(1, Math.round(cell.width)),
			cellHeight: Math.max(1, Math.round(cell.height)),
		});
		if (encoded.length === 0) return;
		if (!this.deps.getMode(1005) && encoded.some((byte) => byte >= 0x80)) {
			const binary = String.fromCharCode(...encoded);
			if (this.deps.emitBinary) this.deps.emitBinary(binary);
			else this.onDataCallback(binary);
			return;
		}
		this.onDataCallback(new TextDecoder().decode(encoded));
	}

	private handleMouseDown(event: MouseEvent): void {
		if (this.isDisposed) return;
		const button = event.button;
		const mappedButton = this.mapMouseButton(button);
		if (mappedButton === null) return;
		this.mouseButtonsPressed |= 1 << button;
		if (!this.deps.hasMouseTracking() || event.shiftKey) return;
		this.sendMouseEvent(MouseAction.PRESS, mappedButton, event);
	}

	private handleMouseUp(event: MouseEvent): void {
		if (this.isDisposed) return;
		const button = event.button;
		const mappedButton = this.mapMouseButton(button);
		if (mappedButton === null) return;
		this.mouseButtonsPressed &= ~(1 << button);
		if (!this.deps.hasMouseTracking() || this.deps.getMode(9) || event.shiftKey) return;
		this.sendMouseEvent(MouseAction.RELEASE, mappedButton, event);
	}

	private handleMouseMove(event: MouseEvent): void {
		if (this.isDisposed) return;
		if (!this.deps.hasMouseTracking()) return;
		if (event.shiftKey && this.mouseButtonsPressed !== 0) return;

		this.sendMouseEvent(MouseAction.MOTION, this.getPressedMouseButton(), event);
	}

	private handleWheel(event: WheelEvent): void {
		if (this.isDisposed) return;
		if (!this.deps.hasMouseTracking()) return;

		if (event.deltaX === 0 && event.deltaY === 0) return;
		event.preventDefault();
		const cell = this.deps.getCellDimensions();
		const screen = this.deps.getScreenDimensions();
		const xUnit = Math.max(1, cell.width);
		const yUnit = Math.max(1, cell.height);
		let xScale = 1;
		let yScale = 1;
		if (event.deltaMode === 1) {
			xScale = xUnit;
			yScale = yUnit;
		} else if (event.deltaMode === 2) {
			xScale = screen.width;
			yScale = screen.height;
		}
		const nextX = this.pendingWheelX + event.deltaX * xScale;
		const nextY = this.pendingWheelY + event.deltaY * yScale;
		const xSteps = Math.trunc(nextX / xUnit);
		const ySteps = Math.trunc(nextY / yUnit);
		this.pendingWheelX = nextX - xSteps * xUnit;
		this.pendingWheelY = nextY - ySteps * yUnit;

		const verticalButton = ySteps < 0 ? MouseButton.FOUR : MouseButton.FIVE;
		for (let index = 0; index < Math.abs(ySteps); index++) {
			this.sendMouseEvent(MouseAction.PRESS, verticalButton, event);
		}
		const horizontalButton = xSteps < 0 ? MouseButton.SEVEN : MouseButton.SIX;
		for (let index = 0; index < Math.abs(xSteps); index++) {
			this.sendMouseEvent(MouseAction.PRESS, horizontalButton, event);
		}
	}

	// ==========================================================================
	// De-duplication
	// ==========================================================================

	private recordKeyDownData(data: string): void {
		this.lastKeyDownData = data;
		this.lastKeyDownTime = this.getNow();
	}

	private recordPasteData(data: string, source: 'paste' | 'beforeinput'): void {
		this.lastPasteData = data;
		this.lastPasteTime = this.getNow();
		this.lastPasteSource = source;
	}

	private recordBeforeInputData(data: string): void {
		this.lastBeforeInputData = data;
		this.lastBeforeInputTime = this.getNow();
	}

	private recordCompositionData(data: string): void {
		this.lastCompositionData = data;
		this.lastCompositionTime = this.getNow();
	}

	private shouldIgnoreBeforeInput(data: string): boolean {
		if (!this.lastKeyDownData) return false;
		const now = this.getNow();
		const isDuplicate =
			now - this.lastKeyDownTime < InputHandler.BEFORE_INPUT_IGNORE_MS &&
			this.lastKeyDownData === data;
		this.lastKeyDownData = null;
		return isDuplicate;
	}

	private shouldIgnoreBeforeInputFromComposition(data: string): boolean {
		if (!this.lastCompositionData) return false;
		const now = this.getNow();
		const isDuplicate =
			now - this.lastCompositionTime < InputHandler.BEFORE_INPUT_IGNORE_MS &&
			this.lastCompositionData === data;
		if (isDuplicate) this.lastCompositionData = null;
		return isDuplicate;
	}

	private shouldIgnoreCompositionEnd(data: string): boolean {
		if (!this.lastBeforeInputData) return false;
		const now = this.getNow();
		const isDuplicate =
			now - this.lastBeforeInputTime < InputHandler.BEFORE_INPUT_IGNORE_MS &&
			this.lastBeforeInputData === data;
		if (isDuplicate) this.lastBeforeInputData = null;
		return isDuplicate;
	}

	private shouldIgnorePasteEvent(data: string, source: 'paste' | 'beforeinput'): boolean {
		if (!this.lastPasteData) return false;
		if (this.lastPasteSource === source) return false;
		const now = this.getNow();
		const isDuplicate =
			now - this.lastPasteTime < InputHandler.BEFORE_INPUT_IGNORE_MS && this.lastPasteData === data;
		if (isDuplicate) {
			this.lastPasteData = null;
			this.lastPasteSource = null;
		}
		return isDuplicate;
	}

	private getNow(): number {
		return typeof performance !== 'undefined' && typeof performance.now === 'function'
			? performance.now()
			: Date.now();
	}

	// ==========================================================================
	// Lifecycle
	// ==========================================================================

	cancelComposition(): void {
		if (this.isDisposed) return;
		this.isComposing = false;
		this.onCompositionCallback?.(null);
		this.clearInputElement();
		this.cleanupCompositionTextNodes();
	}

	/** Synthesize release events for keys whose browser keyup was lost on focus change. */
	releasePressedKeys(): void {
		if (this.isDisposed || this.pressedKeys.size === 0) return;
		const pressed = [...this.pressedKeys.values()];
		this.pressedKeys.clear();
		if (!this.reportsKeyReleases()) return;

		try {
			this.encoder.syncFromTerminal(this.deps.getTerminalHandle());
		} catch (error) {
			this.onErrorCallback?.(error);
			return;
		}

		for (const event of pressed) {
			try {
				const encoded = this.encoder.encode({ action: KeyAction.RELEASE, ...event });
				if (encoded.length > 0) this.onDataCallback(new TextDecoder().decode(encoded));
			} catch (error) {
				this.onErrorCallback?.(error);
			}
		}
	}

	dispose(): void {
		if (this.isDisposed) return;

		this.pressedKeys.clear();

		if (this.keydownListener) {
			this.container.removeEventListener('keydown', this.keydownListener);
			this.keydownListener = null;
		}

		if (this.keyupListener) {
			this.container.removeEventListener('keyup', this.keyupListener);
			this.keyupListener = null;
		}

		if (this.pasteListener) {
			this.container.removeEventListener('paste', this.pasteListener);
			if (this.inputElement && this.inputElement !== this.container) {
				this.inputElement.removeEventListener('paste', this.pasteListener);
			}
			this.pasteListener = null;
		}

		if (this.beforeInputListener && this.inputElement) {
			this.inputElement.removeEventListener('beforeinput', this.beforeInputListener);
			this.beforeInputListener = null;
		}

		if (this.compositionStartListener) {
			this.container.removeEventListener('compositionstart', this.compositionStartListener);
			this.compositionStartListener = null;
		}

		if (this.compositionUpdateListener) {
			this.container.removeEventListener('compositionupdate', this.compositionUpdateListener);
			this.compositionUpdateListener = null;
		}

		if (this.compositionEndListener) {
			this.container.removeEventListener('compositionend', this.compositionEndListener);
			this.compositionEndListener = null;
		}

		if (this.mousedownListener) {
			this.container.removeEventListener('mousedown', this.mousedownListener);
			this.mousedownListener = null;
		}

		if (this.mouseupListener) {
			this.container.removeEventListener('mouseup', this.mouseupListener);
			this.mouseupListener = null;
		}

		if (this.mousemoveListener) {
			this.container.removeEventListener('mousemove', this.mousemoveListener);
			this.mousemoveListener = null;
		}

		if (this.wheelListener) {
			this.container.removeEventListener('wheel', this.wheelListener);
			this.wheelListener = null;
		}

		this.encoder.dispose();
		this.mouseEncoder.dispose();
		this.isDisposed = true;
	}

	isActive(): boolean {
		return !this.isDisposed;
	}
}
