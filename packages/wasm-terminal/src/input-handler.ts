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

import type { WasmBridge } from "./wasm-bridge.js";
import { type IKeyEvent, Key, KeyAction, Mods, TerminalData, type WasmExports } from "./types.js";

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
    if (event.unshiftedCodepoint !== undefined) {
      this.exports.ghostty_key_event_set_unshifted_codepoint(eventPtr, event.unshiftedCodepoint);
    }

    const utf8Bytes = event.utf8 ? new TextEncoder().encode(event.utf8) : undefined;
    const utf8Ptr = utf8Bytes ? this.bridge.allocU8Array(utf8Bytes.length) : 0;
    let bufferSize = 32;
    let bufPtr = this.bridge.allocU8Array(bufferSize);
    const writtenPtr = this.bridge.allocUsize();

    try {
      if (utf8Bytes) {
        this.bridge.writeBytes(utf8Ptr, utf8Bytes);
        this.exports.ghostty_key_event_set_utf8(eventPtr, utf8Ptr, utf8Bytes.length);
      }

      let encodeResult = this.exports.ghostty_key_encoder_encode(
        this.encoder,
        eventPtr,
        bufPtr,
        bufferSize,
        writtenPtr,
      );
      if (encodeResult === -3) {
        const required = this.bridge.createView(writtenPtr, 4).getUint32(0, true);
        this.bridge.freeU8Array(bufPtr, bufferSize);
        bufferSize = required;
        bufPtr = this.bridge.allocU8Array(bufferSize);
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
      if (utf8Bytes) this.bridge.freeU8Array(utf8Ptr, utf8Bytes.length);
      this.bridge.freeU8Array(bufPtr, bufferSize);
      this.bridge.freeUsize(writtenPtr);
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
  Enter: Key.ENTER,
  Escape: Key.ESCAPE,
  Backspace: Key.BACKSPACE,
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

  // Special function keys
  PrintScreen: Key.INSERT,
  ScrollLock: Key.INSERT,
  Pause: Key.INSERT,
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
  NumLock: Key.CAPS_LOCK,
  NumpadDivide: Key.SLASH,
  NumpadMultiply: Key.EIGHT,
  NumpadSubtract: Key.MINUS,
  NumpadAdd: Key.EQUAL,
  NumpadEnter: Key.ENTER,
  Numpad1: Key.ONE,
  Numpad2: Key.TWO,
  Numpad3: Key.THREE,
  Numpad4: Key.FOUR,
  Numpad5: Key.FIVE,
  Numpad6: Key.SIX,
  Numpad7: Key.SEVEN,
  Numpad8: Key.EIGHT,
  Numpad9: Key.NINE,
  Numpad0: Key.ZERO,
  NumpadDecimal: Key.PERIOD,

  // International
  IntlBackslash: Key.BACKSLASH,
  ContextMenu: Key.INSERT,
};

const UNSHIFTED_CHARACTERS: Readonly<Record<string, string>> = {
  Digit0: "0",
  Digit1: "1",
  Digit2: "2",
  Digit3: "3",
  Digit4: "4",
  Digit5: "5",
  Digit6: "6",
  Digit7: "7",
  Digit8: "8",
  Digit9: "9",
  Space: " ",
  Minus: "-",
  Equal: "=",
  BracketLeft: "[",
  BracketRight: "]",
  Backslash: "\\",
  Semicolon: ";",
  Quote: "'",
  Backquote: "`",
  Comma: ",",
  Period: ".",
  Slash: "/",
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
  /** Check if SGR extended mouse mode is enabled (mode 1006). */
  hasSgrMouseMode(): boolean;
  /** Get cell dimensions for pixel-to-cell conversion. */
  getCellDimensions(): { width: number; height: number };
  /** Get canvas/container offset for accurate position calculation. */
  getCanvasOffset(): { left: number; top: number };
  /** Query a terminal mode (DEC or ANSI). */
  getMode(mode: number, isAnsi?: boolean): boolean;
}

export interface CompositionSelection {
  readonly start: number;
  readonly end: number;
}

// ============================================================================
// InputHandler
// ============================================================================

export class InputHandler {
  private readonly encoder: KeyEncoder;
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
    | ((data: string, source: "clipboard" | "beforeinput") => void)
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
  private readonly pressedKeys = new Map<
    string,
    {
      readonly key: Key;
      readonly mods: Mods;
      readonly utf8?: string;
      readonly unshiftedCodepoint?: number;
    }
  >();

  // De-duplication state
  private lastKeyDownData: string | null = null;
  private lastKeyDownTime = 0;
  private lastPasteData: string | null = null;
  private lastPasteTime = 0;
  private lastPasteSource: "paste" | "beforeinput" | null = null;
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
    onPaste?: (data: string, source: "clipboard" | "beforeinput") => void,
    onAction?: (id: string) => boolean,
    onError?: (error: unknown) => void,
  ) {
    this.deps = deps;
    this.encoder = new KeyEncoder(deps.bridge);
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

    this.attach();
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
      typeof this.container.hasAttribute === "function" &&
      typeof this.container.setAttribute === "function"
    ) {
      if (!this.container.hasAttribute("tabindex")) {
        this.container.setAttribute("tabindex", "0");
      }
      if (this.container.style) {
        this.container.style.outline = "none";
      }
    }

    this.keydownListener = this.handleKeyDown.bind(this);
    this.container.addEventListener("keydown", this.keydownListener);
    this.keyupListener = this.handleKeyUp.bind(this);
    this.container.addEventListener("keyup", this.keyupListener);

    this.pasteListener = this.handlePaste.bind(this);
    this.container.addEventListener("paste", this.pasteListener);
    if (this.inputElement && this.inputElement !== this.container) {
      this.inputElement.addEventListener("paste", this.pasteListener);
    }

    if (this.inputElement) {
      this.beforeInputListener = this.handleBeforeInput.bind(this);
      this.inputElement.addEventListener("beforeinput", this.beforeInputListener);
    }

    this.compositionStartListener = this.handleCompositionStart.bind(this);
    this.container.addEventListener("compositionstart", this.compositionStartListener);

    this.compositionUpdateListener = this.handleCompositionUpdate.bind(this);
    this.container.addEventListener("compositionupdate", this.compositionUpdateListener);

    this.compositionEndListener = this.handleCompositionEnd.bind(this);
    this.container.addEventListener("compositionend", this.compositionEndListener);

    this.mousedownListener = this.handleMouseDown.bind(this);
    this.container.addEventListener("mousedown", this.mousedownListener);

    this.mouseupListener = this.handleMouseUp.bind(this);
    this.container.addEventListener("mouseup", this.mouseupListener);

    this.mousemoveListener = this.handleMouseMove.bind(this);
    this.container.addEventListener("mousemove", this.mousemoveListener);

    this.wheelListener = this.handleWheel.bind(this);
    this.container.addEventListener("wheel", this.wheelListener, { passive: false });
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
    if (event.getModifierState?.("CapsLock")) mods |= Mods.CAPSLOCK;
    if (event.getModifierState?.("NumLock")) mods |= Mods.NUMLOCK;
    if (event.location === 2) {
      if (event.shiftKey) mods |= 1 << 6;
      if (event.ctrlKey) mods |= 1 << 7;
      if (event.altKey) mods |= 1 << 8;
      if (event.metaKey) mods |= 1 << 9;
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
    if ((event.ctrlKey || event.metaKey) && event.code === "KeyV") {
      return;
    }

    // Platform copy shortcuts are actions; Ctrl+C without Shift remains SIGINT.
    if ((event.metaKey || (event.ctrlKey && event.shiftKey)) && event.code === "KeyC") {
      const consumed = this.onActionCallback?.("terminal.copy") ?? this.onCopyCallback?.() ?? false;
      if (consumed) {
        event.preventDefault();
      }
      return;
    }

    if (event.metaKey && event.code === "KeyA") {
      if (this.onActionCallback?.("terminal.select-all")) event.preventDefault();
      return;
    }

    // Map physical key code
    const key = this.mapKeyCode(event.code);
    if (key === null) {
      if (this.isPrintableCharacter(event)) {
        event.preventDefault();
        this.onDataCallback(event.key);
        this.recordKeyDownData(event.key);
      }
      return;
    }

    const mods = this.extractModifiers(event);
    const action = event.repeat ? KeyAction.REPEAT : KeyAction.PRESS;

    try {
      this.encoder.syncFromTerminal(this.deps.getTerminalHandle());
      const utf8 = event.key.length === 1 ? event.key : undefined;
      const unshiftedCodepoint = this.getUnshiftedCodepoint(event);

      const encoded = this.encoder.encode({
        action,
        key,
        mods,
        ...(utf8 !== undefined ? { utf8 } : {}),
        ...(unshiftedCodepoint === undefined ? {} : { unshiftedCodepoint }),
      });
      const data = new TextDecoder().decode(encoded);

      if (!event.repeat) {
        this.pressedKeys.set(event.code, {
          key,
          mods,
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
    const key = this.mapKeyCode(event.code);
    if (key === null) return;
    this.pressedKeys.delete(event.code);
    if (this.isComposing || event.isComposing) return;
    if (!this.reportsKeyReleases()) return;

    try {
      this.encoder.syncFromTerminal(this.deps.getTerminalHandle());
      const utf8 = event.key.length === 1 ? event.key : undefined;
      const unshiftedCodepoint = this.getUnshiftedCodepoint(event);
      const encoded = this.encoder.encode({
        action: KeyAction.RELEASE,
        key,
        mods: this.extractModifiers(event),
        ...(utf8 === undefined ? {} : { utf8 }),
        ...(unshiftedCodepoint === undefined ? {} : { unshiftedCodepoint }),
      });
      if (encoded.length === 0) return;
      event.preventDefault();
      event.stopPropagation();
      this.onDataCallback(new TextDecoder().decode(encoded));
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

    const text = clipboardData.getData("text/plain");
    if (!text) return;

    if (this.shouldIgnorePasteEvent(text, "paste")) return;

    this.emitPasteData(text, "clipboard");
    this.recordPasteData(text, "paste");
  }

  private emitPasteData(text: string, source: "clipboard" | "beforeinput"): void {
    if (this.onPasteCallback) {
      this.onPasteCallback(text, source);
      return;
    }
    const hasBracketedPaste = this.deps.getMode(2004);
    if (hasBracketedPaste) {
      this.onDataCallback("\x1b[200~" + text + "\x1b[201~");
    } else {
      this.onDataCallback(text);
    }
  }

  // ==========================================================================
  // BeforeInput Handling (mobile/IME)
  // ==========================================================================

  private handleBeforeInput(event: InputEvent): void {
    if (this.isDisposed) return;
    if (this.isComposing || event.isComposing) return;

    const inputType = event.inputType;
    const data = event.data ?? "";
    let output: string | null = null;

    switch (inputType) {
      case "insertText":
      case "insertReplacementText":
        output = data.length > 0 ? data.replace(/\n/g, "\r") : null;
        break;
      case "insertLineBreak":
      case "insertParagraph":
        output = "\r";
        break;
      case "deleteContentBackward":
        output = "\x7F";
        break;
      case "deleteContentForward":
        output = "\x1B[3~";
        break;
      case "insertFromPaste":
        if (!data) return;
        if (this.shouldIgnorePasteEvent(data, "beforeinput")) {
          event.preventDefault();
          event.stopPropagation();
          return;
        }
        event.preventDefault();
        event.stopPropagation();
        this.emitPasteData(data, "beforeinput");
        this.recordPasteData(data, "beforeinput");
        return;
      default:
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
    const data = event.data ?? "";
    this.onCompositionCallback?.(data, this.readCompositionSelection(data));
  }

  private handleCompositionUpdate(event: CompositionEvent): void {
    if (this.isDisposed) return;
    const data = event.data ?? "";
    this.onCompositionCallback?.(data, this.readCompositionSelection(data));
  }

  private handleCompositionEnd(event: CompositionEvent): void {
    if (this.isDisposed) return;
    this.isComposing = false;
    this.onCompositionCallback?.(null);

    const data = event.data;
    if (data && data.length > 0) {
      if (this.shouldIgnoreCompositionEnd(data)) {
        this.clearInputElement();
        this.cleanupCompositionTextNodes();
        return;
      }
      this.onDataCallback(data);
      this.recordCompositionData(data);
    }

    this.clearInputElement();
    this.cleanupCompositionTextNodes();
  }

  private clearInputElement(): void {
    if (this.inputElement && "value" in this.inputElement) {
      this.inputElement.value = "";
    }
  }

  private readCompositionSelection(data: string): CompositionSelection {
    const input = this.inputElement;
    if (!input || !("selectionStart" in input) || !("selectionEnd" in input)) {
      return { start: data.length, end: data.length };
    }
    const start = typeof input.selectionStart === "number" ? input.selectionStart : data.length;
    const end = typeof input.selectionEnd === "number" ? input.selectionEnd : start;
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

  private pixelToCell(
    event: MouseEvent,
  ): { col: number; row: number; pixelX: number; pixelY: number } | null {
    const dims = this.deps.getCellDimensions();
    const offset = this.deps.getCanvasOffset();

    if (dims.width <= 0 || dims.height <= 0) return null;

    const x = event.clientX - offset.left;
    const y = event.clientY - offset.top;

    const col = Math.max(1, Math.floor(x / dims.width) + 1);
    const row = Math.max(1, Math.floor(y / dims.height) + 1);

    return {
      col,
      row,
      pixelX: Math.max(1, Math.floor(x) + 1),
      pixelY: Math.max(1, Math.floor(y) + 1),
    };
  }

  private getMouseModifiers(event: MouseEvent): number {
    let mods = 0;
    if (event.shiftKey) mods |= 4;
    if (event.altKey) mods |= 8;
    if (event.ctrlKey) mods |= 16;
    return mods;
  }

  private encodeMouseSGR(
    button: number,
    col: number,
    row: number,
    isRelease: boolean,
    modifiers: number,
  ): string {
    const btn = button + modifiers;
    const suffix = isRelease ? "m" : "M";
    return `\x1b[<${btn};${col};${row}${suffix}`;
  }

  private encodeMouseX10(button: number, col: number, row: number, modifiers: number): string {
    const btn = button + modifiers + 32;
    const colChar = String.fromCharCode(Math.min(col + 32, 255));
    const rowChar = String.fromCharCode(Math.min(row + 32, 255));
    return `\x1b[M${String.fromCharCode(btn)}${colChar}${rowChar}`;
  }

  private encodeMouseUtf8(button: number, col: number, row: number, modifiers: number): string {
    return `\x1b[M${String.fromCodePoint(button + modifiers + 32)}${String.fromCodePoint(
      Math.min(col, 2015) + 32,
    )}${String.fromCodePoint(Math.min(row, 2015) + 32)}`;
  }

  private encodeMouseUrxvt(button: number, col: number, row: number, modifiers: number): string {
    return `\x1b[${button + modifiers + 32};${col};${row}M`;
  }

  private sendMouseEvent(
    button: number,
    col: number,
    row: number,
    pixelX: number,
    pixelY: number,
    isRelease: boolean,
    event: MouseEvent,
  ): void {
    const modifiers = this.getMouseModifiers(event);
    let sequence: string;
    if (this.deps.getMode(1016)) {
      sequence = this.encodeMouseSGR(button, pixelX, pixelY, isRelease, modifiers);
    } else if (this.deps.hasSgrMouseMode()) {
      sequence = this.encodeMouseSGR(button, col, row, isRelease, modifiers);
    } else if (this.deps.getMode(1015)) {
      sequence = this.encodeMouseUrxvt(isRelease ? 3 : button, col, row, modifiers);
    } else if (this.deps.getMode(1005)) {
      sequence = this.encodeMouseUtf8(isRelease ? 3 : button, col, row, modifiers);
    } else {
      const x10Button = isRelease ? 3 : button;
      sequence = this.encodeMouseX10(x10Button, col, row, modifiers);
    }

    this.onDataCallback(sequence);
  }

  private handleMouseDown(event: MouseEvent): void {
    if (this.isDisposed) return;
    if (!this.deps.hasMouseTracking()) return;

    const cell = this.pixelToCell(event);
    if (!cell) return;

    const button = event.button;
    this.mouseButtonsPressed |= 1 << button;
    this.sendMouseEvent(button, cell.col, cell.row, cell.pixelX, cell.pixelY, false, event);
  }

  private handleMouseUp(event: MouseEvent): void {
    if (this.isDisposed) return;
    if (!this.deps.hasMouseTracking()) return;
    if (this.deps.getMode(9)) return;

    const cell = this.pixelToCell(event);
    if (!cell) return;

    const button = event.button;
    this.mouseButtonsPressed &= ~(1 << button);
    this.sendMouseEvent(button, cell.col, cell.row, cell.pixelX, cell.pixelY, true, event);
  }

  private handleMouseMove(event: MouseEvent): void {
    if (this.isDisposed) return;
    if (!this.deps.hasMouseTracking()) return;

    const hasButtonMotion = this.deps.getMode(1002);
    const hasAnyMotion = this.deps.getMode(1003);

    if (!hasButtonMotion && !hasAnyMotion) return;
    if (hasButtonMotion && !hasAnyMotion && this.mouseButtonsPressed === 0) return;

    const cell = this.pixelToCell(event);
    if (!cell) return;

    let button = 32;
    if (this.mouseButtonsPressed & 1) button += 0;
    else if (this.mouseButtonsPressed & 2) button += 1;
    else if (this.mouseButtonsPressed & 4) button += 2;

    this.sendMouseEvent(button, cell.col, cell.row, cell.pixelX, cell.pixelY, false, event);
  }

  private handleWheel(event: WheelEvent): void {
    if (this.isDisposed) return;
    if (!this.deps.hasMouseTracking()) return;

    const cell = this.pixelToCell(event);
    if (!cell) return;
    if (event.deltaX === 0 && event.deltaY === 0) return;

    const horizontal = Math.abs(event.deltaX) > Math.abs(event.deltaY);
    let button: number;
    if (horizontal) button = event.deltaX < 0 ? 66 : 67;
    else button = event.deltaY < 0 ? 64 : 65;
    this.sendMouseEvent(button, cell.col, cell.row, cell.pixelX, cell.pixelY, false, event);
    event.preventDefault();
  }

  // ==========================================================================
  // De-duplication
  // ==========================================================================

  private recordKeyDownData(data: string): void {
    this.lastKeyDownData = data;
    this.lastKeyDownTime = this.getNow();
  }

  private recordPasteData(data: string, source: "paste" | "beforeinput"): void {
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

  private shouldIgnorePasteEvent(data: string, source: "paste" | "beforeinput"): boolean {
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
    return typeof performance !== "undefined" && typeof performance.now === "function"
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
      this.container.removeEventListener("keydown", this.keydownListener);
      this.keydownListener = null;
    }

    if (this.keyupListener) {
      this.container.removeEventListener("keyup", this.keyupListener);
      this.keyupListener = null;
    }

    if (this.pasteListener) {
      this.container.removeEventListener("paste", this.pasteListener);
      if (this.inputElement && this.inputElement !== this.container) {
        this.inputElement.removeEventListener("paste", this.pasteListener);
      }
      this.pasteListener = null;
    }

    if (this.beforeInputListener && this.inputElement) {
      this.inputElement.removeEventListener("beforeinput", this.beforeInputListener);
      this.beforeInputListener = null;
    }

    if (this.compositionStartListener) {
      this.container.removeEventListener("compositionstart", this.compositionStartListener);
      this.compositionStartListener = null;
    }

    if (this.compositionUpdateListener) {
      this.container.removeEventListener("compositionupdate", this.compositionUpdateListener);
      this.compositionUpdateListener = null;
    }

    if (this.compositionEndListener) {
      this.container.removeEventListener("compositionend", this.compositionEndListener);
      this.compositionEndListener = null;
    }

    if (this.mousedownListener) {
      this.container.removeEventListener("mousedown", this.mousedownListener);
      this.mousedownListener = null;
    }

    if (this.mouseupListener) {
      this.container.removeEventListener("mouseup", this.mouseupListener);
      this.mouseupListener = null;
    }

    if (this.mousemoveListener) {
      this.container.removeEventListener("mousemove", this.mousemoveListener);
      this.mousemoveListener = null;
    }

    if (this.wheelListener) {
      this.container.removeEventListener("wheel", this.wheelListener);
      this.wheelListener = null;
    }

    this.encoder.dispose();
    this.isDisposed = true;
  }

  isActive(): boolean {
    return !this.isDisposed;
  }
}
