import { beforeEach, describe, expect, it, vi } from "vitest";
import { InputHandler } from "../input-handler.js";
import type { InputHandlerDependencies } from "../input-handler.js";
import { TerminalData } from "../types.js";
import type { WasmBridge } from "../wasm-bridge.js";
import { createTerminal, initializeWasm } from "./test-helpers.js";

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
  getMode: (mode: number) => boolean,
): InputHandlerDependencies {
  return {
    bridge,
    getTerminalHandle: () => 0,
    hasMouseTracking: () => true,
    hasSgrMouseMode: () => false,
    getCellDimensions: () => ({ width: 8, height: 17 }),
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
      hasSgrMouseMode: () => false,
      getCellDimensions: () => ({ width: 8, height: 17 }),
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
      hasSgrMouseMode: () => false,
      getCellDimensions: () => ({ width: 8, height: 17 }),
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
        hasSgrMouseMode: () => false,
        getCellDimensions: () => ({ width: 8, height: 17 }),
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
      hasSgrMouseMode: () => false,
      getCellDimensions: () => ({ width: 8, height: 17 }),
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
    const output = vi.fn<(data: string) => void>();
    const handler = new InputHandler(
      {
        bridge,
        getTerminalHandle: () => 0,
        hasMouseTracking: () => true,
        hasSgrMouseMode: () => false,
        getCellDimensions: () => ({ width: 8, height: 17 }),
        getCanvasOffset: () => ({ left: 10, top: 20 }),
        getMode: (mode) => mode === 1016,
      },
      createElement(),
      output,
      () => {},
    );
    const event = mouseEvent({ clientX: 15, clientY: 25, button: 0 });
    listeners.get("mousedown")?.(event);
    listeners.get("wheel")?.(mouseEvent({ clientX: 15, clientY: 25, deltaX: -10, deltaY: 0 }));
    expect(output.mock.calls).toEqual([["\x1b[<0;6;6M"], ["\x1b[<66;6;6M"]]);
    handler.dispose();
  });

  it("encodes URXVT and UTF-8 extended mouse formats", async () => {
    const bridge = await initializeWasm();
    const urxvtOutput = vi.fn<(data: string) => void>();
    const urxvt = new InputHandler(
      mouseDependencies(bridge, (mode) => mode === 1015),
      createElement(),
      urxvtOutput,
      () => {},
    );
    listeners.get("mousedown")?.(mouseEvent({ clientX: 9, clientY: 18, button: 0 }));
    expect(urxvtOutput).toHaveBeenCalledWith("\x1b[32;2;2M");
    urxvt.dispose();

    const utf8Output = vi.fn<(data: string) => void>();
    const utf8 = new InputHandler(
      mouseDependencies(bridge, (mode) => mode === 1005),
      createElement(),
      utf8Output,
      () => {},
    );
    listeners.get("mousedown")?.(mouseEvent({ clientX: 2_399, clientY: 0, button: 0 }));
    expect(utf8Output).toHaveBeenCalledWith(
      `\x1b[M ${String.fromCodePoint(332)}${String.fromCodePoint(33)}`,
    );
    utf8.dispose();

    const x10Output = vi.fn<(data: string) => void>();
    const x10 = new InputHandler(
      mouseDependencies(bridge, (mode) => mode === 9),
      createElement(),
      x10Output,
      () => {},
    );
    const x10Event = mouseEvent({ clientX: 0, clientY: 0, button: 0 });
    listeners.get("mousedown")?.(x10Event);
    listeners.get("mouseup")?.(x10Event);
    expect(x10Output).toHaveBeenCalledTimes(1);
    x10.dispose();
  });

  it("routes terminal shortcuts through named actions", async () => {
    const bridge = await initializeWasm();
    const action = vi.fn<(id: string) => boolean>(() => true);
    const handler = new InputHandler(
      {
        bridge,
        getTerminalHandle: () => 0,
        hasMouseTracking: () => false,
        hasSgrMouseMode: () => false,
        getCellDimensions: () => ({ width: 8, height: 17 }),
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
        hasSgrMouseMode: () => false,
        getCellDimensions: () => ({ width: 8, height: 17 }),
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
        hasSgrMouseMode: () => false,
        getCellDimensions: () => ({ width: 8, height: 17 }),
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
});
