import { beforeAll, describe, expect, it } from "vitest";
import { CellFlags, CellSemanticContent, DirtyState, PointTag, UnderlineStyle } from "../types.js";
import { createTerminal, initializeWasm } from "./test-helpers.js";

beforeAll(initializeWasm);

describe("Grid render state", () => {
  it("reports and clears dirty state", () => {
    const terminal = createTerminal();
    terminal.grid.update();
    terminal.grid.markClean();
    expect(terminal.grid.update()).toBe(DirtyState.NONE);
    terminal.write("test");
    expect(terminal.grid.update()).not.toBe(DirtyState.NONE);
    expect(terminal.grid.isRowDirty(0)).toBe(true);
    terminal.dispose();
  });

  it("reads viewport cells and codepoints", () => {
    const terminal = createTerminal(8, 2);
    terminal.write("Hi");
    terminal.grid.update();
    const cells = terminal.grid.readViewport();
    expect(cells).toHaveLength(16);
    expect(cells[0]).toMatchObject({ text: "H", codepoint: 72 });
    expect(cells[1]).toMatchObject({ text: "i", codepoint: 105 });
    terminal.dispose();
  });

  it("iterates and selects cells by row", () => {
    const terminal = createTerminal(8, 2);
    terminal.write("ABCDE");
    terminal.grid.update();
    let selected = "";
    terminal.grid.forEachRow((row, reader) => {
      if (row === 0 && reader.select(2)) selected = reader.readText();
    });
    expect(selected).toBe("C");
    terminal.dispose();
  });

  it("preserves SGR flags", () => {
    const terminal = createTerminal();
    terminal.write("\x1b[1;2;3;4;5;7;8;9mX\x1b[0m");
    terminal.grid.update();
    const cell = terminal.grid.readViewport()[0];
    if (!cell) throw new Error("Expected the styled cell");
    expect(cell.flags & CellFlags.BOLD).toBeTruthy();
    expect(cell.flags & CellFlags.FAINT).toBeTruthy();
    expect(cell.flags & CellFlags.ITALIC).toBeTruthy();
    expect(cell.flags & CellFlags.UNDERLINE).toBeTruthy();
    expect(cell.flags & CellFlags.STRIKETHROUGH).toBeTruthy();
    terminal.dispose();
  });

  it("preserves underline variants, underline color, and overline", () => {
    const terminal = createTerminal();
    terminal.write("\x1b[4:2;58:2::10:20:30;53mA\x1b[4:3mB\x1b[4:4mC\x1b[4:5mD");
    terminal.grid.update();
    const cells = terminal.grid.readViewport();
    expect(cells[0]).toMatchObject({
      underlineStyle: UnderlineStyle.DOUBLE,
      underlineColor: { r: 10, g: 20, b: 30 },
      overline: true,
    });
    expect(cells[1]?.underlineStyle).toBe(UnderlineStyle.CURLY);
    expect(cells[2]?.underlineStyle).toBe(UnderlineStyle.DOTTED);
    expect(cells[3]?.underlineStyle).toBe(UnderlineStyle.DASHED);
    terminal.dispose();
  });

  it("represents wide characters as head and tail cells", () => {
    const terminal = createTerminal();
    terminal.write("你好");
    terminal.grid.update();
    const cells = terminal.grid.readViewport();
    expect(cells[0]).toMatchObject({ text: "你", width: 2 });
    expect(cells[1]).toMatchObject({ width: 0 });
    expect(cells[2]).toMatchObject({ text: "好", width: 2 });
    terminal.dispose();
  });

  it("reads scrollback oldest-first", () => {
    const terminal = createTerminal(8, 2, { scrollbackLimit: 10 });
    terminal.write("one\r\ntwo\r\nthree\r\nfour");
    terminal.grid.update();
    expect(terminal.grid.getScrollbackLength()).toBe(2);
    expect(terminal.grid.readScrollbackLine(0)?.cells[0]?.text).toBe("o");
    expect(terminal.grid.readScrollbackLine(1)?.cells[0]?.text).toBe("t");
    terminal.dispose();
  });

  it("resolves OSC 8 hyperlink references", () => {
    const terminal = createTerminal();
    terminal.write("\x1b]8;;https://example.com/path\x1b\\link\x1b]8;;\x1b\\");
    terminal.grid.update();
    expect(terminal.grid.getHyperlinkUri(PointTag.VIEWPORT, 0, 0)).toBe("https://example.com/path");
    terminal.dispose();
  });

  it("preserves OSC 133 semantic content for every cell", () => {
    const terminal = createTerminal(20, 2);
    terminal.write("\x1b]133;A\x07$ \x1b]133;B\x07echo\x1b]133;C\x07result");
    terminal.grid.update();
    const cells = terminal.grid.readViewport();
    expect(cells[0]?.semanticContent).toBe(CellSemanticContent.PROMPT);
    expect(cells[2]?.semanticContent).toBe(CellSemanticContent.INPUT);
    expect(cells[6]?.semanticContent).toBe(CellSemanticContent.OUTPUT);
    terminal.dispose();
  });

  it("updates dimensions after resize", () => {
    const terminal = createTerminal(80, 24);
    terminal.resize(40, 10);
    terminal.grid.update();
    expect(terminal.grid).toMatchObject({ cols: 40, rows: 10 });
    expect(terminal.grid.readViewport()).toHaveLength(400);
    terminal.dispose();
  });
});
