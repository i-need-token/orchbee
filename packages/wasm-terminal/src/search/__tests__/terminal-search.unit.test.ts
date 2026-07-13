import { describe, expect, it } from "vitest";
import { findTerminalMatches, searchMatchContainsCell } from "../terminal-search.js";
import type { CellData } from "../../types.js";

describe("findTerminalMatches", () => {
  it("supports case, whole-word, and regular-expression searches", () => {
    const lines = [line("Error error_error error 42")];
    expect(findTerminalMatches(lines, "error")).toHaveLength(4);
    expect(findTerminalMatches(lines, "error", { caseSensitive: true })).toHaveLength(3);
    expect(findTerminalMatches(lines, "error", { wholeWord: true })).toHaveLength(2);
    expect(findTerminalMatches(lines, "e(?:rror)?", { regex: true })).toHaveLength(4);
  });

  it("maps UTF-16 matches to wide terminal cell columns", () => {
    const lines = [line("A你好B")];
    expect(findTerminalMatches(lines, "你好")).toEqual([
      {
        text: "你好",
        start: { column: 1, row: 0 },
        end: { column: 3, row: 0 },
      },
    ]);
  });

  it("does not search terminal padding after the last content cell", () => {
    expect(findTerminalMatches([[...line("text"), emptyCell(), emptyCell()]], "  ")).toEqual([]);
  });

  it("searches a logical line across soft-wrapped physical rows", () => {
    expect(
      findTerminalMatches([line("wrapped"), line("value")], "edva", {}, [true, false]),
    ).toEqual([
      {
        text: "edva",
        start: { column: 5, row: 0 },
        end: { column: 1, row: 1 },
      },
    ]);
  });

  it("tests cells inside a multi-row match", () => {
    const match = {
      text: "wrapped",
      start: { column: 4, row: 2 },
      end: { column: 2, row: 4 },
    };
    expect(searchMatchContainsCell(match, 3, 2)).toBe(false);
    expect(searchMatchContainsCell(match, 4, 2)).toBe(true);
    expect(searchMatchContainsCell(match, 0, 3)).toBe(true);
    expect(searchMatchContainsCell(match, 2, 4)).toBe(true);
    expect(searchMatchContainsCell(match, 3, 4)).toBe(false);
  });
});

function line(value: string): CellData[] {
  const cells: CellData[] = [];
  for (const character of value) {
    const width = /[你好吗]/u.test(character) ? 2 : 1;
    cells.push(cell(character, width));
    if (width === 2) cells.push(cell("", 0));
  }
  return cells;
}

function cell(text: string, width: number): CellData {
  return {
    codepoint: text.codePointAt(0) ?? 0,
    text: text || " ",
    fg: null,
    bg: null,
    selected: false,
    hasStyling: false,
    flags: 0,
    width,
    hyperlinkId: 0,
    graphemeLen: text ? [...text].length : 0,
    semanticContent: 0,
    underlineStyle: 0,
    underlineColor: null,
    overline: false,
  };
}

function emptyCell(): CellData {
  return cell("", 1);
}
