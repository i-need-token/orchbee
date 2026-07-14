import { describe, expect, it } from "vitest";
import { buildPreeditState, shouldSuppressComposingControlInput } from "../preedit";

describe("buildPreeditState", () => {
  it("maps a UTF-16 selection to wide terminal cell offsets", () => {
    expect(buildPreeditState("A你好", { start: 1, end: 2 }, width)).toEqual({
      cells: [
        { text: "A", width: 1 },
        { text: "你", width: 2 },
        { text: "好", width: 2 },
      ],
      width: 5,
      selection: { start: 1, end: 3 },
    });
  });

  it("keeps a caret after a surrogate-pair grapheme on a valid cell boundary", () => {
    expect(buildPreeditState("🎉a", { start: 2, end: 2 }, width).selection).toEqual({
      start: 2,
      end: 2,
    });
  });

  it("defaults the marked-text caret to the end", () => {
    expect(buildPreeditState("个人", undefined, width).selection).toEqual({ start: 4, end: 4 });
  });

  it("keeps combining marks and zero-width joiners in their grapheme cell", () => {
    const state = buildPreeditState("e\u0301👩‍💻", undefined, (codepoint) => {
      if (codepoint === 0x301 || codepoint === 0x200d) return 0;
      return codepoint > 0x7f ? 2 : 1;
    });
    expect(state.cells).toEqual([
      { text: "e\u0301", width: 1 },
      { text: "👩‍💻", width: 2 },
    ]);
    expect(state.width).toBe(3);
  });
});

describe("shouldSuppressComposingControlInput", () => {
  it.each([
    ["\u0008", true],
    ["\u001f", true],
    ["\u007f", false],
    [" ", false],
    ["h", false],
    ["", false],
    ["\u0009x", false],
    ["\u0009\u0009", false],
  ])("classifies %j while composing", (text, expected) => {
    expect(shouldSuppressComposingControlInput(text, true)).toBe(expected);
  });

  it("preserves control input outside composition and missing text", () => {
    expect(shouldSuppressComposingControlInput("\u0008", false)).toBe(false);
    expect(shouldSuppressComposingControlInput(null, true)).toBe(false);
  });
});

function width(codepoint: number): number {
  return codepoint > 0x7f ? 2 : 1;
}
