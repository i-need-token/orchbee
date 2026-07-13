import { beforeAll, describe, expect, it } from "vitest";
import { GhosttySelectionController } from "../ghostty-selection.js";
import type { TerminalHandle } from "../../types.js";
import type { WasmBridge } from "../../wasm-bridge.js";
import { createTerminal, initializeWasm } from "../../__tests__/test-helpers.js";

describe("GhosttySelectionController", () => {
  let bridge: WasmBridge;

  beforeAll(async () => {
    bridge = await initializeWasm();
  });

  it("installs, formats, tracks, and clears a range", () => {
    const terminal = createTerminal(20, 3);
    const selection = createController(bridge, terminal);
    try {
      terminal.write("hello world");

      expect(selection.setRange({ x: 0, y: 0 }, { x: 4, y: 0 })).toBe(true);
      expect(selection.hasSelection()).toBe(true);
      expect(selection.getSelection()).toBe("hello");
      const payload = selection.getClipboardPayload();
      expect(payload.text).toBe("hello");
      expect(payload.vt).toContain("hello");
      expect(payload.html).toContain("hello");
      expect(selection.getRange()).toEqual({
        start: { x: 0, y: 0 },
        end: { x: 4, y: 0 },
        rectangle: false,
      });
      terminal.grid.update();
      const selectedCells = terminal.grid
        .readViewport()
        .slice(0, 6)
        .map((cell) => cell.selected);
      expect(selectedCells).toEqual([true, true, true, true, true, false]);

      terminal.write("\r\nsecond");
      expect(selection.getSelection()).toBe("hello");
      expect(selection.clearSelection()).toBe(true);
      expect(selection.hasSelection()).toBe(false);
    } finally {
      selection.dispose();
      terminal.dispose();
    }
  });

  it("uses Ghostty word, line, and select-all semantics", () => {
    const terminal = createTerminal(20, 3);
    const selection = createController(bridge, terminal);
    try {
      terminal.write("hello world\r\nsecond line");

      expect(selection.selectWordAt(7, 0)).toBe(true);
      expect(selection.getSelection()).toBe("world");

      expect(selection.selectLineAt(3, 1)).toBe(true);
      expect(selection.getSelection()).toBe("second line");

      expect(selection.selectAll()).toBe(true);
      expect(selection.getSelection()).toContain("hello world");
      expect(selection.getSelection()).toContain("second line");
    } finally {
      selection.dispose();
      terminal.dispose();
    }
  });

  it("adjusts the active endpoint using Ghostty selection semantics", () => {
    const terminal = createTerminal(20, 3);
    const selection = createController(bridge, terminal);
    try {
      terminal.write("hello world");
      expect(selection.setRange({ x: 0, y: 0 }, { x: 4, y: 0 })).toBe(true);
      expect(selection.adjustSelection("right")).toBe(true);
      expect(selection.getRange()).toEqual({
        start: { x: 0, y: 0 },
        end: { x: 5, y: 0 },
        rectangle: false,
      });
      expect(selection.getSelection()).toBe("hello");
      expect(selection.adjustSelection("right")).toBe(true);
      expect(selection.getSelection()).toBe("hello w");
      expect(selection.adjustSelection("left")).toBe(true);
      expect(selection.getSelection()).toBe("hello");
      expect(selection.adjustSelection("endOfLine")).toBe(true);
      expect(selection.getSelection()).toBe("hello world");
    } finally {
      selection.dispose();
      terminal.dispose();
    }
  });

  it("selects a semantic command output block", () => {
    const terminal = createTerminal(20, 8);
    const selection = createController(bridge, terminal);
    try {
      terminal.write(
        "\x1b]133;A\x07$ \x1b]133;B\x07printf hi\r\n" +
          "\x1b]133;C\x07first\r\nsecond\r\n\x1b]133;D;0\x07" +
          "\x1b]133;A\x07$ ",
      );
      expect(selection.selectOutputAt(2, 1)).toBe(true);
      expect(selection.getSelection()).toBe("first\nsecond");
      expect(selection.selectOutputAt(1, 0)).toBe(false);
    } finally {
      selection.dispose();
      terminal.dispose();
    }
  });

  it("uses Ghostty click counting and word-granular drag gestures", () => {
    const terminal = createTerminal(20, 3);
    const selection = createController(bridge, terminal);
    const firstPress = {
      column: 7,
      row: 0,
      surfaceX: 60,
      surfaceY: 8,
      timeNs: 1_000_000_000n,
      repeatDistance: 8,
      repeatIntervalNs: 500_000_000n,
    };
    try {
      terminal.write("hello world");

      expect(selection.pressGesture(firstPress)).toBe(true);
      expect(selection.hasSelection()).toBe(false);
      expect(selection.releaseGesture(7, 0)).toBe(true);

      expect(
        selection.pressGesture({ ...firstPress, timeNs: firstPress.timeNs + 100_000_000n }),
      ).toBe(true);
      expect(selection.getSelection()).toBe("world");

      expect(
        selection.dragGesture({
          column: 1,
          row: 0,
          surfaceX: 8,
          surfaceY: 8,
          rectangle: false,
          columns: 20,
          cellWidth: 8,
          paddingLeft: 0,
          screenHeight: 51,
        }),
      ).toBe(true);
      expect(selection.getSelection()).toBe("hello world");
      expect(selection.releaseGesture(1, 0)).toBe(true);
    } finally {
      selection.dispose();
      terminal.dispose();
    }
  });

  it("preserves rectangular selection metadata", () => {
    const terminal = createTerminal(10, 3);
    const selection = createController(bridge, terminal);
    try {
      terminal.write("abcdef\r\n123456");
      expect(selection.setRange({ x: 1, y: 0 }, { x: 3, y: 1 }, true)).toBe(true);
      expect(selection.getRange()).toEqual({
        start: { x: 1, y: 0 },
        end: { x: 3, y: 1 },
        rectangle: true,
      });
      expect(selection.getSelection()).toBe("bcd\n234");
    } finally {
      selection.dispose();
      terminal.dispose();
    }
  });
});

function createController(
  bridge: WasmBridge,
  terminal: ReturnType<typeof createTerminal>,
): GhosttySelectionController {
  const handle = (terminal as unknown as { readonly handle: TerminalHandle }).handle;
  return new GhosttySelectionController(bridge, handle);
}
