import { describe, expect, it } from "vitest";
import {
  AnsiMode,
  buildModeTag,
  DecMode,
  DirtyState,
  RenderStateData,
  Result,
  RowCellsData,
  RowData,
  TerminalData,
} from "../types";

describe("terminal constants", () => {
  it("keeps dirty-state values compatible with Ghostty", () => {
    expect(DirtyState.NONE).toBe(0);
    expect(DirtyState.PARTIAL).toBe(1);
    expect(DirtyState.FULL).toBe(2);
  });

  it("keeps result values compatible with Ghostty", () => {
    expect(Result.SUCCESS).toBe(0);
    expect(Result.OUT_OF_MEMORY).toBe(-1);
    expect(Result.INVALID_VALUE).toBe(-2);
    expect(Result.OUT_OF_SPACE).toBe(-3);
  });

  it("keeps terminal and render-state tags stable", () => {
    expect(TerminalData.COLS).toBe(1);
    expect(TerminalData.SCROLLBACK_ROWS).toBe(15);
    expect(RenderStateData.DIRTY).toBe(3);
    expect(RenderStateData.CURSOR_VIEWPORT_X).toBe(15);
  });

  it("keeps row tags stable", () => {
    expect(RowData.DIRTY).toBe(1);
    expect(RowData.CELLS).toBe(3);
    expect(RowCellsData.BG_COLOR).toBe(5);
    expect(RowCellsData.GRAPHEMES_UTF8).toBe(9);
  });

  it("builds DEC and ANSI mode tags", () => {
    expect(buildModeTag(DecMode.CURSOR_VISIBLE, false)).toBe(25);
    expect(buildModeTag(AnsiMode.INSERT, true)).toBe(4 | (1 << 15));
  });
});
