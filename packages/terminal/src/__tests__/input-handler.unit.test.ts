import { beforeEach, describe, expect, it, vi } from "vitest";
import { InputHandler } from "../input-handler";
import type { InputHandlerDependencies } from "../input-handler";
import type { Terminal } from "../terminal";
import { TerminalData } from "../types";
import type { WasmBridge } from "../wasm-bridge";
import { createTerminal, initializeWasm } from "./test-helpers";

const listeners = new Map<string, EventListener>();

function createElement(): HTMLElement {
  return {
    style: {},
    hasAttribute: () => false,
    setAttribute: () => {},
    addEventListener: (name: string, listener: EventListenerOrEventListenerObject) => {
      if (typeof listener === "function") listeners.set(name, listener);
    },
    removeEventListener: (name: string) => listeners.delete(name),
  } as unknown as HTMLElement;
}

function createTextarea(): HTMLTextAreaElement {
  return {
    ...createElement(),
    value: "",
    selectionStart: 0,
    selectionEnd: 0,
  } as HTMLTextAreaElement;
}

beforeEach(() => listeners.clear());

function mouseDependencies(
  bridge: WasmBridge,
  terminal: Terminal,
  getMode: (mode: number) => boolean,
): InputHandlerDependencies {
  return {
    bridge,
    getTerminalHandle: () => Reflect.get(terminal, "handle") as number,
    hasMouseTracking: () => true,
    getCellDimensions: () => ({ width: 8, height: 17 }),
    getScreenDimensions: () => ({ width: 2_400, height: 408 }),
    getCanvasOffset: () => ({ left: 0, top: 0 }),
    getMode,
  };
}

function mouseEvent(values: Partial<MouseEvent & WheelEvent>): Event {
  return {
    button: 0,
    clientX: 0,
    clientY: 0,
    deltaX: 0,
    deltaY: 0,
    shiftKey: false,
    metaKey: false,
    ctrlKey: false,
    preventDefault: vi.fn<() => void>(),
    ...values,
  } as unknown as Event;
}

describe("InputHandler", () => {
  it("attaches input listeners, rings the bell, and disposes", async () => {
    const bridge = await initializeWasm();
    const bell = vi.fn<() => void>();
    const dependencies: InputHandlerDependencies = {
      bridge,
      getTerminalHandle: () => 0,
      hasMouseTracking: () => false,
      getCellDimensions: () => ({ width: 8, height: 17 }),
      getScreenDimensions: () => ({ width: 640, height: 408 }),
      getCanvasOffset: () => ({ left: 0, top: 0 }),
      getMode: () => false,
    };
    const handler = new InputHandler(
      dependencies,
      createElement(),
      vi.fn<(data: string) => void>(),
      bell,
    );
    expect(listeners.has("keydown")).toBe(true);
    handler.bell();
    expect(bell).toHaveBeenCalledOnce();
    handler.dispose();
    expect(listeners.size).toBe(0);
  });

  it("reports modifier changes used by the surface mouse state", async () => {
    const bridge = await initializeWasm();
    const onSurfaceModifierChange = vi.fn<(event: KeyboardEvent) => void>();
    const handler = new InputHandler(
      {
        bridge,
        getTerminalHandle: () => 0,
        hasMouseTracking: () => false,
        getCellDimensions: () => ({ width: 8, height: 17 }),
        getScreenDimensions: () => ({ width: 640, height: 408 }),
        getCanvasOffset: () => ({ left: 0, top: 0 }),
        getMode: () => false,
        onSurfaceModifierChange,
      },
      createElement(),
      vi.fn<(data: string) => void>(),
      () => {},
    );
    const shift = {
      code: "ShiftLeft",
      key: "Shift",
      isComposing: true,
    } as unknown as KeyboardEvent;
    const ordinary = {
      code: "KeyA",
      key: "a",
      isComposing: true,
    } as unknown as KeyboardEvent;

    listeners.get("keydown")?.(shift);
    listeners.get("keyup")?.(shift);
    listeners.get("keydown")?.(ordinary);

    expect(onSurfaceModifierChange.mock.calls).toEqual([[shift], [shift]]);
    handler.dispose();
  });

  it("commits composed CJK text and clears the hidden textarea", async () => {
    const bridge = await initializeWasm();
    const output = vi.fn<(data: string) => void>();
    const composition =
      vi.fn<
        (data: string | null, selection?: { readonly start: number; readonly end: number }) => void
      >();
    const textarea = createTextarea();
    const dependencies: InputHandlerDependencies = {
      bridge,
      getTerminalHandle: () => 0,
      hasMouseTracking: () => false,
      getCellDimensions: () => ({ width: 8, height: 17 }),
      getScreenDimensions: () => ({ width: 640, height: 408 }),
      getCanvasOffset: () => ({ left: 0, top: 0 }),
      getMode: () => false,
    };
    const handler = new InputHandler(
      dependencies,
      createElement(),
      output,
      () => {},
      undefined,
      undefined,
      undefined,
      textarea,
      composition,
    );
    textarea.value = "中文";
    listeners.get("compositionstart")?.({ data: "" } as unknown as Event);
    textarea.selectionStart = 5;
    textarea.selectionEnd = 5;
    listeners.get("compositionupdate")?.({ data: "zhong wen" } as unknown as Event);
    listeners.get("compositionend")?.({ data: "中文" } as unknown as Event);
    expect(output).toHaveBeenCalledWith("中文");
    expect(composition.mock.calls).toEqual([
      ["", { start: 0, end: 0 }],
      ["zhong wen", { start: 5, end: 5 }],
      [null],
    ]);
    expect(textarea.value).toBe("");
    handler.dispose();
  });

  it("suppresses a bare C0 control committed by an active IME", async () => {
    const bridge = await initializeWasm();
    const output = vi.fn<(data: string) => void>();
    const textarea = createTextarea();
    const handler = new InputHandler(
      {
        bridge,
        getTerminalHandle: () => 0,
        hasMouseTracking: () => false,
        getCellDimensions: () => ({ width: 8, height: 17 }),
        getScreenDimensions: () => ({ width: 640, height: 408 }),
        getCanvasOffset: () => ({ left: 0, top: 0 }),
        getMode: () => false,
      },
      createElement(),
      output,
      () => {},
      undefined,
      undefined,
      undefined,
      textarea,
    );

    listeners.get("compositionstart")?.({ data: "" } as unknown as Event);
    listeners.get("compositionend")?.({ data: "\u0008" } as unknown as Event);

    expect(output).not.toHaveBeenCalled();
    expect(textarea.value).toBe("");
    handler.dispose();
  });

  it("cancels preedit state without committing composition text", async () => {
    const bridge = await initializeWasm();
    const output = vi.fn<(data: string) => void>();
    const composition =
      vi.fn<
        (data: string | null, selection?: { readonly start: number; readonly end: number }) => void
      >();
    const textarea = createTextarea();
    const handler = new InputHandler(
      {
        bridge,
        getTerminalHandle: () => 0,
        hasMouseTracking: () => false,
        getCellDimensions: () => ({ width: 8, height: 17 }),
        getScreenDimensions: () => ({ width: 640, height: 408 }),
        getCanvasOffset: () => ({ left: 0, top: 0 }),
        getMode: () => false,
      },
      createElement(),
      output,
      () => {},
      undefined,
      undefined,
      undefined,
      textarea,
      composition,
    );
    textarea.value = "zhong wen";
    listeners.get("compositionstart")?.({ data: "" } as unknown as Event);
    listeners.get("compositionupdate")?.({ data: "zhong wen" } as unknown as Event);
    handler.cancelComposition();
    expect(composition).toHaveBeenLastCalledWith(null);
    expect(output).not.toHaveBeenCalled();
    expect(textarea.value).toBe("");
    handler.dispose();
  });

  it("delegates clipboard and beforeinput paste to the host callback", async () => {
    const bridge = await initializeWasm();
    const output = vi.fn<(data: string) => void>();
    const paste = vi.fn<(data: string, source: "clipboard" | "beforeinput") => void>();
    const dependencies: InputHandlerDependencies = {
      bridge,
      getTerminalHandle: () => 0,
      hasMouseTracking: () => false,
      getCellDimensions: () => ({ width: 8, height: 17 }),
      getScreenDimensions: () => ({ width: 640, height: 408 }),
      getCanvasOffset: () => ({ left: 0, top: 0 }),
      getMode: () => false,
    };
    const handler = new InputHandler(
      dependencies,
      createElement(),
      output,
      () => {},
      undefined,
      undefined,
      undefined,
      createTextarea(),
      undefined,
      paste,
    );
    const preventDefault = vi.fn<() => void>();
    const stopPropagation = vi.fn<() => void>();

    listeners.get("paste")?.({
      clipboardData: { getData: () => "clipboard paste" },
      preventDefault,
      stopPropagation,
    } as unknown as ClipboardEvent);
    listeners.get("beforeinput")?.({
      inputType: "insertFromPaste",
      data: "beforeinput paste",
      isComposing: false,
      preventDefault,
      stopPropagation,
    } as unknown as InputEvent);

    expect(paste.mock.calls).toEqual([
      ["clipboard paste", "clipboard"],
      ["beforeinput paste", "beforeinput"],
    ]);
    expect(output).not.toHaveBeenCalled();
    handler.dispose();
  });

  it("encodes SGR pixel mouse coordinates and horizontal wheel buttons", async () => {
    const bridge = await initializeWasm();
    const terminal = createTerminal();
    terminal.write("\x1b[?1000h\x1b[?1016h");
    const output = vi.fn<(data: string) => void>();
    const handler = new InputHandler(
      {
        bridge,
        getTerminalHandle: () => Reflect.get(terminal, "handle") as number,
        hasMouseTracking: () => true,
        getCellDimensions: () => ({ width: 8, height: 17 }),
        getScreenDimensions: () => ({ width: 640, height: 408 }),
        getCanvasOffset: () => ({ left: 10, top: 20 }),
        getMode: (mode) => mode === 1000 || mode === 1016,
      },
      createElement(),
      output,
      () => {},
    );
    const event = mouseEvent({ clientX: 15, clientY: 25, button: 0 });
    listeners.get("mousedown")?.(event);
    listeners.get("wheel")?.(mouseEvent({ clientX: 15, clientY: 25, deltaX: -4, deltaY: 0 }));
    expect(output).toHaveBeenCalledTimes(1);
    listeners.get("wheel")?.(mouseEvent({ clientX: 15, clientY: 25, deltaX: -4, deltaY: 0 }));
    expect(output.mock.calls).toEqual([["\x1b[<0;5;5M"], ["\x1b[<67;5;5M"]]);
    handler.dispose();
    terminal.dispose();
  });

  it("keeps Shift-drag selection out of terminal mouse reporting", async () => {
    const bridge = await initializeWasm();
    const terminal = createTerminal();
    terminal.write("\x1b[?1002h\x1b[?1006h");
    const output = vi.fn<(data: string) => void>();
    const handler = new InputHandler(
      mouseDependencies(bridge, terminal, (mode) => mode === 1002 || mode === 1006),
      createElement(),
      output,
      () => {},
    );
    const shifted = mouseEvent({ clientX: 9, clientY: 18, button: 0, shiftKey: true });

    listeners.get("mousedown")?.(shifted);
    listeners.get("mousemove")?.(shifted);
    listeners.get("mouseup")?.(shifted);
    expect(output).not.toHaveBeenCalled();

    listeners.get("mousedown")?.(mouseEvent({ clientX: 9, clientY: 18, button: 0 }));
    expect(output).toHaveBeenCalledWith("\x1b[<0;2;2M");
    handler.dispose();
    terminal.dispose();
  });

  it("encodes URXVT and UTF-8 extended mouse formats", async () => {
    const bridge = await initializeWasm();
    const urxvtTerminal = createTerminal();
    urxvtTerminal.write("\x1b[?1000h\x1b[?1015h");
    const urxvtOutput = vi.fn<(data: string) => void>();
    const urxvt = new InputHandler(
      mouseDependencies(bridge, urxvtTerminal, (mode) => mode === 1000 || mode === 1015),
      createElement(),
      urxvtOutput,
      () => {},
    );
    listeners.get("mousedown")?.(mouseEvent({ clientX: 9, clientY: 18, button: 0 }));
    expect(urxvtOutput).toHaveBeenCalledWith("\x1b[32;2;2M");
    urxvt.dispose();
    urxvtTerminal.dispose();

    const utf8Terminal = createTerminal();
    utf8Terminal.write("\x1b[?1000h\x1b[?1005h");
    const utf8Output = vi.fn<(data: string) => void>();
    const utf8 = new InputHandler(
      mouseDependencies(bridge, utf8Terminal, (mode) => mode === 1000 || mode === 1005),
      createElement(),
      utf8Output,
      () => {},
    );
    listeners.get("mousedown")?.(mouseEvent({ clientX: 2_399, clientY: 0, button: 0 }));
    expect(utf8Output).toHaveBeenCalledWith(
      `\x1b[M ${String.fromCodePoint(332)}${String.fromCodePoint(33)}`,
    );
    utf8.dispose();
    utf8Terminal.dispose();

    const x10Terminal = createTerminal();
    x10Terminal.write("\x1b[?9h");
    const x10Output = vi.fn<(data: string) => void>();
    const x10Binary = vi.fn<(data: string) => void>();
    const x10 = new InputHandler(
      {
        ...mouseDependencies(bridge, x10Terminal, (mode) => mode === 9),
        emitBinary: x10Binary,
      },
      createElement(),
      x10Output,
      () => {},
    );
    const x10Event = mouseEvent({ clientX: 0, clientY: 0, button: 0 });
    listeners.get("mousedown")?.(x10Event);
    listeners.get("mouseup")?.(x10Event);
    listeners.get("mousedown")?.(mouseEvent({ clientX: 799, clientY: 0, button: 0 }));
    expect(x10Output).toHaveBeenCalledTimes(1);
    expect(x10Binary).toHaveBeenCalledOnce();
    expect(
      [...String(x10Binary.mock.calls[0]?.[0])].map((character) => character.charCodeAt(0)),
    ).toEqual([27, 91, 77, 32, 132, 33]);
    x10.dispose();
    x10Terminal.dispose();
  });

  it("routes terminal shortcuts through named actions", async () => {
    const bridge = await initializeWasm();
    const action = vi.fn<(id: string) => boolean>(() => true);
    const handler = new InputHandler(
      {
        bridge,
        getTerminalHandle: () => 0,
        hasMouseTracking: () => false,
        getCellDimensions: () => ({ width: 8, height: 17 }),
        getScreenDimensions: () => ({ width: 640, height: 408 }),
        getCanvasOffset: () => ({ left: 0, top: 0 }),
        getMode: () => false,
      },
      createElement(),
      () => {},
      () => {},
      undefined,
      undefined,
      undefined,
      createTextarea(),
      undefined,
      undefined,
      action,
    );
    const preventCopy = vi.fn<() => void>();
    const preventSelectAll = vi.fn<() => void>();

    listeners.get("keydown")?.({
      code: "KeyC",
      key: "c",
      metaKey: true,
      ctrlKey: false,
      shiftKey: false,
      isComposing: false,
      keyCode: 67,
      preventDefault: preventCopy,
    } as unknown as KeyboardEvent);
    listeners.get("keydown")?.({
      code: "KeyA",
      key: "a",
      metaKey: true,
      ctrlKey: false,
      shiftKey: false,
      isComposing: false,
      keyCode: 65,
      preventDefault: preventSelectAll,
    } as unknown as KeyboardEvent);

    expect(action.mock.calls).toEqual([["terminal.copy"], ["terminal.select-all"]]);
    expect(preventCopy).toHaveBeenCalledOnce();
    expect(preventSelectAll).toHaveBeenCalledOnce();
    handler.dispose();
  });

  it("repairs missing keyup events when Kitty reports releases", async () => {
    const bridge = await initializeWasm();
    const terminal = createTerminal();
    terminal.write("\x1b[>3u");
    const terminalHandle = Reflect.get(terminal, "handle") as number;
    const flagsPointer = bridge.allocU8Array(1);
    expect(
      bridge
        .getExports()
        .ghostty_terminal_get(terminalHandle, TerminalData.KITTY_KEYBOARD_FLAGS, flagsPointer),
    ).toBe(0);
    expect(bridge.readBytes(flagsPointer, 1)[0]).toBe(3);
    bridge.freeU8Array(flagsPointer, 1);
    const exports = bridge.getExports();
    const keyEvent = bridge.allocAndReadOpaque((allocator, outputPointer) =>
      exports.ghostty_key_event_new(allocator, outputPointer),
    );
    exports.ghostty_key_event_set_action(keyEvent, 0);
    expect(exports.ghostty_key_event_get_action(keyEvent)).toBe(0);
    exports.ghostty_key_event_free(keyEvent);
    const output = vi.fn<(data: string) => void>();
    const handler = new InputHandler(
      {
        bridge,
        getTerminalHandle: () => terminalHandle,
        hasMouseTracking: () => false,
        getCellDimensions: () => ({ width: 8, height: 17 }),
        getScreenDimensions: () => ({ width: 640, height: 408 }),
        getCanvasOffset: () => ({ left: 0, top: 0 }),
        getMode: () => false,
      },
      createElement(),
      output,
      () => {},
    );
    const preventDefault = vi.fn<() => void>();
    const stopPropagation = vi.fn<() => void>();

    listeners.get("keydown")?.({
      code: "KeyA",
      key: "a",
      location: 0,
      repeat: false,
      shiftKey: false,
      ctrlKey: false,
      altKey: false,
      metaKey: false,
      isComposing: false,
      keyCode: 65,
      preventDefault,
      stopPropagation,
    } as unknown as KeyboardEvent);
    handler.releasePressedKeys();
    handler.releasePressedKeys();

    expect(output.mock.calls).toEqual([["a"], ["\x1b[97;1:3u"]]);
    expect(preventDefault).toHaveBeenCalledOnce();
    expect(stopPropagation).toHaveBeenCalledOnce();
    handler.dispose();
    terminal.dispose();
  });

  it("does not duplicate text for legacy keyup events", async () => {
    const bridge = await initializeWasm();
    const terminal = createTerminal();
    const output = vi.fn<(data: string) => void>();
    const handler = new InputHandler(
      {
        bridge,
        getTerminalHandle: () => Reflect.get(terminal, "handle") as number,
        hasMouseTracking: () => false,
        getCellDimensions: () => ({ width: 8, height: 17 }),
        getScreenDimensions: () => ({ width: 640, height: 408 }),
        getCanvasOffset: () => ({ left: 0, top: 0 }),
        getMode: () => false,
      },
      createElement(),
      output,
      () => {},
    );
    const event = {
      code: "KeyA",
      key: "a",
      location: 0,
      repeat: false,
      shiftKey: false,
      ctrlKey: false,
      altKey: false,
      metaKey: false,
      isComposing: false,
      keyCode: 65,
      preventDefault: vi.fn<() => void>(),
      stopPropagation: vi.fn<() => void>(),
    } as unknown as KeyboardEvent;

    listeners.get("keydown")?.(event);
    listeners.get("keyup")?.(event);
    handler.releasePressedKeys();

    expect(output.mock.calls).toEqual([["a"]]);
    handler.dispose();
    terminal.dispose();
  });

  it("preserves numpad identity for Ghostty application-keypad encoding", async () => {
    const bridge = await initializeWasm();
    const terminal = createTerminal();
    terminal.write("\x1b[?1035l\x1b=");
    const output = vi.fn<(data: string) => void>();
    const handler = new InputHandler(
      {
        bridge,
        getTerminalHandle: () => Reflect.get(terminal, "handle") as number,
        hasMouseTracking: () => false,
        getCellDimensions: () => ({ width: 8, height: 17 }),
        getScreenDimensions: () => ({ width: 640, height: 408 }),
        getCanvasOffset: () => ({ left: 0, top: 0 }),
        getMode: () => false,
      },
      createElement(),
      output,
      () => {},
    );

    listeners.get("keydown")?.({
      code: "Numpad1",
      key: "1",
      location: 3,
      repeat: false,
      shiftKey: false,
      ctrlKey: false,
      altKey: false,
      metaKey: false,
      isComposing: false,
      keyCode: 97,
      getModifierState: () => false,
      preventDefault: () => {},
      stopPropagation: () => {},
    } as unknown as KeyboardEvent);

    expect(output).toHaveBeenCalledWith("\x1bOq");
    handler.dispose();
    terminal.dispose();
  });

  it("routes mobile backspace through Ghostty's backarrow mode", async () => {
    const bridge = await initializeWasm();
    const terminal = createTerminal();
    terminal.write("\x1b[?67h");
    const output = vi.fn<(data: string) => void>();
    const handler = new InputHandler(
      {
        bridge,
        getTerminalHandle: () => Reflect.get(terminal, "handle") as number,
        hasMouseTracking: () => false,
        getCellDimensions: () => ({ width: 8, height: 17 }),
        getScreenDimensions: () => ({ width: 640, height: 408 }),
        getCanvasOffset: () => ({ left: 0, top: 0 }),
        getMode: () => false,
      },
      createElement(),
      output,
      () => {},
      undefined,
      undefined,
      undefined,
      createTextarea(),
    );

    listeners.get("beforeinput")?.({
      inputType: "deleteContentBackward",
      data: null,
      isComposing: false,
      preventDefault: () => {},
      stopPropagation: () => {},
    } as unknown as InputEvent);

    expect(output).toHaveBeenCalledWith("\x08");
    handler.dispose();
    terminal.dispose();
  });
});
