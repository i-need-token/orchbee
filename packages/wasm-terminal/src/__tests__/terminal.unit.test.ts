import { beforeAll, describe, expect, it, vi } from "vitest";
import { AnsiMode, DecMode } from "../types.js";
import { Terminal } from "../terminal.js";
import type {
  ContextMenuRequest,
  TerminalClipboardHost,
  TerminalHost,
} from "../host/terminal-host.js";
import { createTerminal, initializeWasm } from "./test-helpers.js";

beforeAll(initializeWasm);

describe("Terminal lifecycle", () => {
  it("creates default and custom dimensions", () => {
    const defaultTerminal = createTerminal();
    const customTerminal = createTerminal(120, 40);
    expect([defaultTerminal.cols, defaultTerminal.rows]).toEqual([80, 24]);
    expect([customTerminal.cols, customTerminal.rows]).toEqual([120, 40]);
    defaultTerminal.dispose();
    customTerminal.dispose();
  });

  it("returns empty accessibility ranges before the DOM surface opens", () => {
    const terminal = createTerminal();
    expect(terminal.getAccessibleText()).toBe("");
    expect(terminal.getAccessibleVisibleRange()).toEqual({ start: 0, end: 0 });
    expect(terminal.getAccessibleSelection()).toBeNull();
    expect(terminal.selectAccessibleRange({ start: 0, end: 1 })).toBe(false);
    terminal.dispose();
  });

  it("supports xterm-style options", () => {
    const terminal = new Terminal({
      cols: 12,
      rows: 3,
      scrollback: 7,
      cursorBlink: true,
      fontLigatures: false,
      theme: { foreground: "#123456", background: "#010203" },
    });
    terminal.grid.update();
    expect(terminal.options.scrollback).toBe(7);
    expect(terminal.options.fontLigatures).toBe(false);
    expect(terminal.foregroundColor).toEqual({ r: 0x12, g: 0x34, b: 0x56 });
    expect(terminal.backgroundColor).toEqual({ r: 1, g: 2, b: 3 });
    terminal.dispose();
  });

  it("loads the configured font before measuring the terminal surface", async () => {
    const terminal = new Terminal({ fontFamily: "Test Mono, monospace", fontSize: 17 });
    const load = vi.fn<(font: string, text?: string) => Promise<FontFace[]>>(async () => []);
    vi.stubGlobal("document", { fonts: { load } });
    try {
      const loadConfiguredFont = Reflect.get(terminal, "loadConfiguredFont") as () => Promise<void>;
      await loadConfiguredFont.call(terminal);

      expect(load).toHaveBeenCalledWith("17px Test Mono, monospace", "MW你🚀");
    } finally {
      vi.unstubAllGlobals();
      terminal.dispose();
    }
  });

  it("reports font loading failures without rejecting terminal initialization", async () => {
    const terminal = new Terminal({ fontFamily: "Missing Font", fontSize: 14 });
    const failure = new Error("Font loading failed");
    const errors: unknown[] = [];
    const disposable = terminal.onHostError((event) => errors.push(event));
    const load = vi.fn<(font: string, text?: string) => Promise<FontFace[]>>(async () => {
      throw failure;
    });
    vi.stubGlobal("document", { fonts: { load } });
    try {
      const loadConfiguredFont = Reflect.get(terminal, "loadConfiguredFont") as () => Promise<void>;
      await expect(loadConfiguredFont.call(terminal)).resolves.toBeUndefined();

      expect(errors).toEqual([{ operation: "font-load", error: failure }]);
    } finally {
      vi.unstubAllGlobals();
      disposable.dispose();
      terminal.dispose();
    }
  });

  it("exposes a capability snapshot without claiming unimplemented graphics", () => {
    const terminal = new Terminal({
      host: {
        capabilities: { clipboardRead: false },
        clipboard: { read: async () => null, write: async () => {} },
        showContextMenu: async () => null,
      },
    });
    expect(terminal.capabilities).toEqual({
      renderer: "headless",
      mouseFormats: ["x10", "utf8", "sgr", "urxvt", "sgr-pixel"],
      graphics: [],
      clipboardRead: false,
      clipboardWrite: true,
      contextMenu: true,
      notifications: false,
      openTarget: false,
      pasteConfirmation: false,
      fileDrop: false,
      accessibility: true,
      semanticPrompts: true,
    });
    expect(terminal.ghosttyBuildCapabilities).toEqual({
      simd: false,
      kittyGraphics: false,
      tmuxControlMode: false,
    });
    terminal.dispose();
  });

  it("rejects opening a terminal after disposal", async () => {
    const terminal = createTerminal();
    terminal.dispose();
    await expect(terminal.open({} as HTMLElement)).rejects.toThrow("disposed terminal");
  });

  it("rolls back parent state when opening fails before renderer initialization", async () => {
    const terminal = createTerminal();
    const parent = {
      style: { position: "" },
      addEventListener: () => {},
      removeEventListener: () => {},
    } as unknown as HTMLElement;
    vi.stubGlobal("window", { getComputedStyle: () => ({ position: "static" }) });
    vi.stubGlobal("document", {
      createElement: () => {
        throw new Error("DOM allocation failed");
      },
      removeEventListener: () => {},
    });
    try {
      await expect(terminal.open(parent)).rejects.toThrow("DOM allocation failed");
      expect(terminal.element).toBeUndefined();
      expect(parent.style.position).toBe("");
      await expect(terminal.open(parent)).rejects.toThrow("DOM allocation failed");
    } finally {
      vi.unstubAllGlobals();
      terminal.dispose();
    }
  });

  it("rebuilds the renderer and rearms its media query when DPR changes", () => {
    const terminal = createTerminal();
    const setDevicePixelRatio = vi.fn<(ratio: number) => boolean>(() => true);
    const addEventListener = vi.fn<MediaQueryList["addEventListener"]>();
    const removeEventListener = vi.fn<MediaQueryList["removeEventListener"]>();
    const mediaQuery = { addEventListener, removeEventListener } as unknown as MediaQueryList;
    const matchMedia = vi.fn<(query: string) => MediaQueryList>(() => mediaQuery);
    const requestAnimationFrame = vi.fn<(callback: FrameRequestCallback) => number>(() => 1);
    Reflect.set(terminal, "renderer", { setDevicePixelRatio });
    vi.stubGlobal("window", { devicePixelRatio: 2, matchMedia });
    vi.stubGlobal("requestAnimationFrame", requestAnimationFrame);
    try {
      const syncDevicePixelRatio = Reflect.get(terminal, "syncDevicePixelRatio") as () => void;
      syncDevicePixelRatio.call(terminal);

      expect(setDevicePixelRatio).toHaveBeenCalledWith(2);
      expect(matchMedia).toHaveBeenCalledWith("(resolution: 2dppx)");
      expect(addEventListener).toHaveBeenCalledWith("change", expect.any(Function));
      expect(requestAnimationFrame).toHaveBeenCalledOnce();
    } finally {
      Reflect.set(terminal, "renderLoopId", 0);
      Reflect.set(terminal, "renderer", undefined);
      vi.unstubAllGlobals();
      terminal.dispose();
    }
  });

  it("synchronizes renderer focus for Ghostty-style cursor presentation", () => {
    const terminal = createTerminal();
    const setFocused = vi.fn<(focused: boolean) => void>();
    vi.stubGlobal("Node", Object);
    Reflect.set(terminal, "renderer", { setFocused });
    const handleFocusIn = Reflect.get(terminal, "handleFocusIn") as () => void;
    const handleFocusOut = Reflect.get(terminal, "handleFocusOut") as (
      event: Pick<FocusEvent, "relatedTarget">,
    ) => void;

    handleFocusIn.call(terminal);
    handleFocusOut.call(terminal, { relatedTarget: null });

    expect(setFocused.mock.calls).toEqual([[true], [false]]);
    Reflect.set(terminal, "renderer", undefined);
    terminal.dispose();
  });

  it("writes strings, bytes, and lines", () => {
    const terminal = createTerminal();
    terminal.write("A");
    terminal.write(new TextEncoder().encode("B"));
    terminal.writeln("C");
    terminal.grid.update();
    expect(terminal.cursorY).toBe(1);
    expect(
      terminal.grid
        .readViewport()
        .slice(0, 3)
        .map((cell) => cell.text),
    ).toEqual(["A", "B", "C"]);
    terminal.dispose();
  });

  it("searches both scrollback history and the active screen", () => {
    const terminal = createTerminal(12, 2, { scrollbackLimit: 10 });
    terminal.write("first match\r\nsecond\r\nlast MATCH");
    const expected = [
      {
        text: "match",
        start: { column: 6, row: 0 },
        end: { column: 10, row: 0 },
      },
      {
        text: "MATCH",
        start: { column: 5, row: 2 },
        end: { column: 9, row: 2 },
      },
    ];
    expect(terminal.findAll("match")).toEqual(expected);
    expect(terminal.highlightSearch("match")).toEqual(expected);
    terminal.clearSearch();
    terminal.dispose();
  });

  it("navigates matches in Ghostty order and wraps in both directions", () => {
    const terminal = createTerminal(10, 2, { scrollbackLimit: 10 });
    terminal.write("Fizz\r\nBuzz\r\nFizz\r\nBang");
    let selection: {
      start: { x: number; y: number };
      end: { x: number; y: number };
    } | null = null;
    const selectAbsoluteRange = vi.fn<
      (start: { x: number; y: number }, end: { x: number; y: number }) => boolean
    >((start: { x: number; y: number }, end: { x: number; y: number }): boolean => {
      selection = { start, end };
      return true;
    });
    const clearSelection = vi.fn<() => void>(() => {
      selection = null;
    });
    Reflect.set(terminal, "selectionManager", {
      getSelectionPosition: () => selection,
      selectAbsoluteRange,
      clearSelection,
    });

    expect(terminal.findNext("Fizz")).toBe(true);
    expect(selection).toMatchObject({ start: { x: 0, y: 2 } });
    expect(terminal.findNext("Fizz")).toBe(true);
    expect(selection).toMatchObject({ start: { x: 0, y: 0 } });
    expect(terminal.findNext("Fizz")).toBe(true);
    expect(selection).toMatchObject({ start: { x: 0, y: 2 } });
    expect(terminal.findPrevious("Fizz")).toBe(true);
    expect(selection).toMatchObject({ start: { x: 0, y: 0 } });
    expect(terminal.findPrevious("Fizz")).toBe(true);
    expect(selection).toMatchObject({ start: { x: 0, y: 2 } });
    expect(clearSelection).not.toHaveBeenCalled();

    Reflect.set(terminal, "selectionManager", undefined);
    terminal.dispose();
  });

  it("drops the search selection after all matches disappear", () => {
    const terminal = createTerminal(10, 2);
    terminal.write("Fizz");
    const clearSelection = vi.fn<() => void>();
    const setSearchMatches = vi.fn<() => void>();
    Reflect.set(terminal, "selectionManager", {
      getSelectionPosition: () => ({ start: { x: 0, y: 0 }, end: { x: 3, y: 0 } }),
      selectAbsoluteRange: () => true,
      clearSelection,
    });
    Reflect.set(terminal, "renderer", { setSearchMatches });

    expect(terminal.findNext("missing")).toBe(false);
    expect(clearSelection).toHaveBeenCalledOnce();
    expect(setSearchMatches).toHaveBeenCalledWith([]);

    Reflect.set(terminal, "selectionManager", undefined);
    Reflect.set(terminal, "renderer", undefined);
    terminal.dispose();
  });

  it("recomputes search results after active-screen contents change", () => {
    const terminal = createTerminal(10, 3);
    terminal.write("Fizz\r\nBuzz");
    expect(terminal.findAll("Fizz")).toMatchObject([
      { start: { column: 0, row: 0 }, end: { column: 3, row: 0 } },
    ]);

    terminal.write("\x1b[2J\x1b[HBang\r\nFizz");
    expect(terminal.findAll("Fizz")).toMatchObject([
      { start: { column: 0, row: 1 }, end: { column: 3, row: 1 } },
    ]);
    terminal.dispose();
  });

  it("wraps previous search when every match is in scrollback", () => {
    const terminal = createTerminal(10, 2, { scrollbackLimit: 20 });
    terminal.write("Fizz\r\nblank\r\nblank\r\nblank");
    const matches = terminal.findAll("Fizz");
    expect(matches).toHaveLength(1);
    expect(matches[0]?.start.row).toBeLessThan(terminal.grid.getScrollbackLength());

    let selection: {
      start: { x: number; y: number };
      end: { x: number; y: number };
    } | null = null;
    Reflect.set(terminal, "selectionManager", {
      getSelectionPosition: () => selection,
      selectAbsoluteRange: (start: { x: number; y: number }, end: { x: number; y: number }) => {
        selection = { start, end };
        return true;
      },
      clearSelection: () => {
        selection = null;
      },
    });

    expect(terminal.findNext("Fizz")).toBe(true);
    expect(terminal.findPrevious("Fizz")).toBe(true);
    expect(selection).toMatchObject({ start: { x: 0, y: matches[0]?.start.row } });

    Reflect.set(terminal, "selectionManager", undefined);
    terminal.dispose();
  });

  it("searches a soft-wrapped match across the scrollback boundary", () => {
    const terminal = createTerminal(4, 2, { scrollbackLimit: 10 });
    terminal.write("A\r\nxxboo!\r\nC");

    expect(terminal.findAll("boo!")).toEqual([
      {
        text: "boo!",
        start: { column: 2, row: 1 },
        end: { column: 1, row: 2 },
      },
    ]);
    terminal.dispose();
  });

  it("searches only the active screen and never primary-screen history from alt screen", () => {
    const terminal = createTerminal(10, 2, { scrollbackLimit: 10 });
    terminal.write("Fizz\r\nblank\r\nblank");
    expect(terminal.findAll("Fizz")).toHaveLength(1);

    terminal.write("\x1b[?1049hAlt only");
    expect(terminal.findAll("Fizz")).toEqual([]);
    expect(terminal.findAll("Alt")).toHaveLength(1);

    terminal.write("\x1b[?1049l");
    expect(terminal.findAll("Fizz")).toHaveLength(1);
    terminal.dispose();
  });

  it("keeps search navigation valid after clearing scrollback", () => {
    const terminal = createTerminal(10, 2, { scrollbackLimit: 20 });
    terminal.write("error\r\nerror\r\nerror\r\nerror.");
    let selection: {
      start: { x: number; y: number };
      end: { x: number; y: number };
    } | null = null;
    Reflect.set(terminal, "selectionManager", {
      getSelectionPosition: () => selection,
      selectAbsoluteRange: (start: { x: number; y: number }, end: { x: number; y: number }) => {
        selection = { start, end };
        return true;
      },
      clearSelection: () => {
        selection = null;
      },
    });

    expect(terminal.findNext("error")).toBe(true);
    terminal.write("\x1b[3J");
    expect(() => terminal.findNext("error")).not.toThrow();
    expect(() => terminal.findPrevious("error")).not.toThrow();

    Reflect.set(terminal, "selectionManager", undefined);
    terminal.dispose();
  });

  it("resizes and safely disposes twice", () => {
    const terminal = createTerminal();
    terminal.resize(20, 5);
    expect([terminal.cols, terminal.rows]).toEqual([20, 5]);
    expect(() => terminal.dispose()).not.toThrow();
    expect(() => terminal.dispose()).not.toThrow();
  });

  it("disposes loaded addons", () => {
    const terminal = createTerminal();
    let activated = false;
    let disposed = false;
    terminal.loadAddon({
      activate: () => {
        activated = true;
      },
      dispose: () => {
        disposed = true;
      },
    });
    terminal.dispose();
    expect(activated).toBe(true);
    expect(disposed).toBe(true);
  });
});

describe("Terminal callbacks and state", () => {
  it("emits device status responses", () => {
    const terminal = createTerminal();
    const responses: string[] = [];
    const disposable = terminal.onData((data) => responses.push(data));
    terminal.write("\x1b[5n\x1b[6n");
    expect(responses).toEqual(["\x1b[0n", "\x1b[1;1R"]);
    disposable.dispose();
    terminal.dispose();
  });

  it("answers XTWINOPS queries from cached terminal geometry", () => {
    const terminal = createTerminal(10, 2);
    const responses: string[] = [];
    terminal.onData((data) => responses.push(data));

    terminal.write("\x1b[14t\x1b[16t\x1b[18t");

    expect(responses).toEqual(["\x1b[4;32;80t", "\x1b[6;16;8t", "\x1b[8;2;10t"]);
    terminal.dispose();
  });

  it("answers color-scheme queries from the active terminal background", () => {
    const dark = new Terminal({ cols: 10, rows: 2, theme: { background: "#111111" } });
    const light = new Terminal({ cols: 10, rows: 2, theme: { background: "#f5f5f5" } });
    const darkResponses: string[] = [];
    const lightResponses: string[] = [];
    dark.onData((data) => darkResponses.push(data));
    light.onData((data) => lightResponses.push(data));

    dark.write("\x1b[?996n");
    light.write("\x1b[?996n");

    expect(darkResponses).toEqual(["\x1b[?997;1n"]);
    expect(lightResponses).toEqual(["\x1b[?997;2n"]);
    dark.dispose();
    light.dispose();
  });

  it("reports the configured product identity through XTVERSION", () => {
    const terminal = new Terminal({ identity: { name: "orchbee-test", version: "2.4.0" } });
    const responses: string[] = [];
    terminal.onData((data) => responses.push(data));

    terminal.write("\x1b[>q");

    expect(responses).toEqual(["\x1bP>|orchbee-test 2.4.0\x1b\\"]);
    terminal.dispose();
  });

  it("uses a cached configurable ENQ answerback", () => {
    const terminal = new Terminal({ enquiryResponse: "orchbee-ready" });
    const responses: string[] = [];
    terminal.onData((data) => responses.push(data));

    terminal.write("\x05");

    expect(responses).toEqual(["orchbee-ready"]);
    terminal.dispose();
  });

  it("reports Ghostty device attributes without claiming disabled graphics", () => {
    const terminal = createTerminal();
    const responses: string[] = [];
    terminal.onData((data) => responses.push(data));

    terminal.write("\x1b[c\x1b[>c\x1b[=c");

    expect(responses).toEqual(["\x1b[?62;22c", "\x1b[>1;0;0c", "\x1bP!|00000000\x1b\\"]);
    terminal.dispose();
  });

  it("emits bell and title changes", () => {
    const terminal = createTerminal();
    let bells = 0;
    const titles: string[] = [];
    terminal.onBell(() => bells++);
    terminal.onTitleChange((title) => titles.push(title));
    terminal.write("\x07\x1b]2;Orchbee Agent\x07");
    expect(bells).toBe(1);
    expect(titles).toEqual(["Orchbee Agent"]);
    terminal.dispose();
  });

  it("emits working-directory changes with the raw Ghostty value", () => {
    const terminal = createTerminal();
    const directories: string[] = [];
    terminal.onPwdChange((directory) => directories.push(directory));

    terminal.write("\x1b]7;file://host/tmp/project\x07");

    expect(directories).toEqual(["file://host/tmp/project"]);
    expect(terminal.getPwd()).toBe("file://host/tmp/project");
    terminal.dispose();
  });

  it("tracks cursor visibility, style, and position", () => {
    const terminal = new Terminal({ cursorStyle: "underline", cursorBlink: false });
    terminal.write("AB\x1b[?25l\x1b[5 q");
    terminal.grid.update();
    expect(terminal.getCursor()).toEqual({ x: 2, y: 0, visible: false });
    expect(terminal.cursorStyle).toBe("bar");
    expect(terminal.cursorBlinking).toBe(true);
    terminal.dispose();
  });

  it("toggles terminal modes", () => {
    const terminal = createTerminal();
    terminal.write("\x1b[?2004h\x1b[?1004h");
    expect(terminal.getMode(DecMode.BRACKETED_PASTE)).toBe(true);
    expect(terminal.getMode(DecMode.FOCUS_EVENTS)).toBe(true);
    expect(terminal.getMode(AnsiMode.INSERT, true)).toBe(false);
    terminal.dispose();
  });

  it("restores primary content after alternate-screen output", () => {
    const terminal = createTerminal(20, 4);
    terminal.write("primary");
    terminal.write("\x1b[?1049h\x1b[2J\x1b[HALT");
    expect(terminal.isAlternateScreen()).toBe(true);
    terminal.write("\x1b[?1049l");
    terminal.grid.update();
    expect(terminal.grid.readViewport()[0]?.text).toBe("p");
    terminal.dispose();
  });

  it("updates Ghostty palette colors from theme options", () => {
    const terminal = new Terminal({ cols: 8, rows: 2, theme: { red: "#010203" } });
    terminal.write("\x1b[31mR");
    terminal.grid.update();
    expect(terminal.grid.readViewport()[0]?.fg).toEqual({ r: 1, g: 2, b: 3 });
    terminal.dispose();
  });

  it("exposes OSC default-color overrides and restores configured defaults", () => {
    const terminal = new Terminal({ cols: 8, rows: 2, theme: { background: "#010203" } });
    terminal.write("\x1b]11;rgb:12/34/56\x1b\\");
    terminal.grid.update();
    expect(terminal.getColors().background).toEqual({ r: 18, g: 52, b: 86 });
    terminal.write("\x1b]111\x1b\\");
    terminal.grid.update();
    expect(terminal.getColors().background).toEqual({ r: 1, g: 2, b: 3 });
    terminal.dispose();
  });

  it("keeps a scrolled viewport anchored when history grows", () => {
    const terminal = createTerminal(12, 3, { scrollbackLimit: 20 });
    for (let index = 0; index < 6; index++) terminal.writeln(`line-${index}`);
    terminal.scrollLines(-2);
    const before = terminal.buffer.active.viewportY;
    terminal.writeln("new-line");
    expect(terminal.buffer.active.viewportY).toBe(before);
    terminal.dispose();
  });

  it("confirms, sanitizes, and encodes unsafe paste", async () => {
    const confirmPaste = vi.fn<NonNullable<TerminalHost["confirmPaste"]>>(async () => true);
    const terminal = new Terminal({ cols: 10, rows: 2, host: { confirmPaste } });
    const data: string[] = [];
    terminal.onData((value) => data.push(value));

    const result = await terminal.pasteAsync("first\nsec\x1bond", "clipboard");

    expect(result).toMatchObject({
      kind: "emitted",
      source: "clipboard",
      inspection: {
        classification: "confirm",
        sanitized: "first\nsec ond",
        replacedControlBytes: 1,
      },
    });
    expect(confirmPaste).toHaveBeenCalledOnce();
    expect(data).toEqual(["first\rsec ond"]);
    terminal.dispose();
  });

  it("treats ordinary bracketed multiline paste as safe", async () => {
    const confirmPaste = vi.fn<NonNullable<TerminalHost["confirmPaste"]>>(async () => true);
    const terminal = new Terminal({ cols: 10, rows: 2, host: { confirmPaste } });
    const data: string[] = [];
    terminal.onData((value) => data.push(value));
    terminal.write("\x1b[?2004h");

    await expect(terminal.pasteAsync("first\nsecond")).resolves.toMatchObject({
      kind: "emitted",
      inspection: { bracketed: true, classification: "safe" },
    });
    expect(confirmPaste).not.toHaveBeenCalled();
    expect(data).toEqual(["\x1b[200~first\nsecond\x1b[201~"]);
    terminal.dispose();
  });

  it("does not emit paste data when confirmation is rejected", async () => {
    const terminal = new Terminal({
      cols: 10,
      rows: 2,
      host: { confirmPaste: async () => false },
    });
    const data: string[] = [];
    terminal.onData((value) => data.push(value));

    await expect(terminal.pasteAsync("first\nsecond")).resolves.toMatchObject({
      kind: "cancelled",
    });
    expect(data).toEqual([]);
    terminal.dispose();
  });

  it("preserves input order while an asynchronous paste confirmation is pending", async () => {
    let resolveConfirmation: ((confirmed: boolean) => void) | undefined;
    const confirmation = new Promise<boolean>((resolve) => {
      resolveConfirmation = resolve;
    });
    const terminal = new Terminal({
      cols: 10,
      rows: 2,
      host: { confirmPaste: () => confirmation },
    });
    const data: string[] = [];
    terminal.onData((value) => data.push(value));

    const paste = terminal.pasteAsync("first\nsecond", "clipboard");
    terminal.input("typed", true);
    const barrier = terminal.pasteAsync("");
    await Promise.resolve();
    expect(data).toEqual([]);

    resolveConfirmation?.(true);
    await Promise.all([paste, barrier]);
    expect(data).toEqual(["first\rsecond", "typed"]);
    terminal.dispose();
  });

  it("routes dropped files through host path policy before paste policy", async () => {
    const file = {
      name: "report final.txt",
      size: 12,
      type: "text/plain",
      lastModified: 1,
    } as File;
    const resolveDrop = vi.fn<NonNullable<TerminalHost["resolveDrop"]>>(async () => ({
      text: "'/tmp/report final.txt'",
    }));
    const terminal = new Terminal({ cols: 10, rows: 2, host: { resolveDrop } });
    const data: string[] = [];
    terminal.onData((value) => data.push(value));
    terminal.write("\x1b]7;file://localhost/tmp/project\x07");

    await expect(terminal.dropAsync("", [file])).resolves.toMatchObject({
      kind: "emitted",
      source: "drop",
    });

    expect(resolveDrop).toHaveBeenCalledWith({
      text: "",
      files: [file],
      cwd: "file://localhost/tmp/project",
    });
    expect(data).toEqual(["'/tmp/report final.txt'"]);
    terminal.dispose();
  });

  it("accepts dropped text but blocks files without a host path policy", async () => {
    const terminal = createTerminal();
    const data: string[] = [];
    terminal.onData((value) => data.push(value));

    await expect(terminal.dropAsync("dropped text")).resolves.toMatchObject({
      kind: "emitted",
      source: "drop",
    });
    await expect(terminal.dropAsync("", [{} as File])).resolves.toMatchObject({
      kind: "blocked",
      reason: "file-drop-unsupported",
    });
    expect(data).toEqual(["dropped text"]);
    terminal.dispose();
  });

  it("executes built-in paste through the action registry", async () => {
    const read = vi.fn<TerminalClipboardHost["read"]>(async () => ({ text: "from clipboard" }));
    const terminal = new Terminal({
      cols: 10,
      rows: 2,
      host: {
        clipboard: {
          read,
          write: async () => {},
        },
      },
    });
    const data: string[] = [];
    terminal.onData((value) => data.push(value));

    await expect(terminal.executeAction("terminal.paste", "keyboard")).resolves.toEqual({
      kind: "consumed",
    });
    expect(read).toHaveBeenCalledWith({ target: "standard", source: "paste" });
    expect(data).toEqual(["from clipboard"]);
    terminal.dispose();
  });

  it("exposes deterministic built-in action state", () => {
    const terminal = createTerminal();
    const context = { source: "api" as const, signal: new AbortController().signal };
    expect(terminal.actions.getState("terminal.copy", context)).toEqual({
      enabled: false,
      checked: false,
    });
    expect(terminal.actions.list().map((action) => action.id)).toEqual([
      "terminal.open-link",
      "terminal.copy",
      "terminal.paste",
      "terminal.select-all",
      "terminal.select-output",
      "terminal.clear-selection",
      "terminal.reset",
    ]);
    terminal.dispose();
  });

  it("requires the Ghostty link modifier before enabling hover activation", () => {
    const terminal = createTerminal();
    const isLinkActivationEnabled = Reflect.get(terminal, "isLinkActivationEnabled") as (
      link: { readonly activation?: "direct" | "modifier" },
      modifiers: {
        readonly alt: boolean;
        readonly control: boolean;
        readonly meta: boolean;
        readonly shift: boolean;
      },
    ) => boolean;
    const modifierLink = { activation: "modifier" as const };
    const noModifiers = { alt: false, control: false, meta: false, shift: false };

    expect(isLinkActivationEnabled.call(terminal, modifierLink, noModifiers)).toBe(false);
    expect(
      isLinkActivationEnabled.call(terminal, modifierLink, {
        ...noModifiers,
        control: true,
      }),
    ).toBe(true);
    expect(isLinkActivationEnabled.call(terminal, { activation: "direct" }, noModifiers)).toBe(
      true,
    );
    terminal.dispose();
  });

  it("sends serializable interaction context to the host and executes its selected action", async () => {
    const requests: ContextMenuRequest[] = [];
    const terminal = new Terminal({
      cols: 10,
      rows: 3,
      host: {
        showContextMenu: async (request) => {
          requests.push(request);
          return "terminal.reset";
        },
      },
    });
    (terminal as unknown as { canvas: HTMLCanvasElement }).canvas = {
      getBoundingClientRect: () => ({ left: 4, top: 8 }),
      removeEventListener: () => {},
      remove: () => {},
    } as unknown as HTMLCanvasElement;
    terminal.write("abc");
    terminal.grid.update();
    expect(terminal.cursorX).toBe(3);

    await expect(
      terminal.requestContextMenuAt(14, 28, {
        alt: false,
        control: true,
        meta: false,
        shift: true,
      }),
    ).resolves.toEqual({ kind: "consumed" });

    expect(requests).toHaveLength(1);
    expect(requests[0]).toMatchObject({
      x: 14,
      y: 28,
      context: {
        cell: { column: 1, row: 1, bufferRow: 1 },
        selection: null,
        link: null,
        semanticZone: "output",
        cwd: null,
        modifiers: { alt: false, control: true, meta: false, shift: true },
        mouseReporting: false,
      },
    });
    expect(requests[0]?.actionIds).toEqual([
      "terminal.copy",
      "terminal.paste",
      "terminal.select-all",
      "terminal.select-output",
      "terminal.clear-selection",
      "terminal.reset",
    ]);
    expect(requests[0]?.actions.find((action) => action.id === "terminal.copy")?.enabled).toBe(
      false,
    );
    terminal.grid.update();
    expect(terminal.cursorX).toBe(0);
    (terminal as unknown as { canvas: HTMLCanvasElement | undefined }).canvas = undefined;
    terminal.dispose();
  });
});
