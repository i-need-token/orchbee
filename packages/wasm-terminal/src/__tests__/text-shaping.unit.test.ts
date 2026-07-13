import { describe, expect, it } from "vitest";
import {
  buildTextShapingPlan,
  type TextShapingCell,
  type TextShapingSpan,
} from "../text-shaping.js";

describe("buildTextShapingPlan", () => {
  it("selects the longest programming ligature at each column", () => {
    const plan = buildTextShapingPlan(cells("<==> !=="));

    expect(plan[0]).toEqual({ start: 0, end: 4, columns: 4, text: "<==>" });
    expect(plan[3]).toBe(plan[0]);
    expect(plan[5]).toEqual({ start: 5, end: 8, columns: 3, text: "!==" });
  });

  it("breaks a run at style, selection, cursor, or preedit boundaries", () => {
    const input = cells("=> -> =>");
    const selected = input[1];
    const cursor = input[4];
    const preedit = input[6];
    if (!selected || !cursor || !preedit) throw new Error("Missing shaping test cells");
    input[1] = { ...selected, styleKey: "selected" };
    input[4] = { ...cursor, eligible: false };
    input[6] = { ...preedit, eligible: false };
    const plan = buildTextShapingPlan(input);

    expect(plan.every((span) => span === null)).toBe(true);
  });

  it("does not join standard text ligatures that Ghostty deliberately splits", () => {
    const plan = buildTextShapingPlan(cells("fi fl st www"));

    expect(plan.slice(0, 8).every((span) => span === null)).toBe(true);
    expect((plan[9] as TextShapingSpan | null)?.text).toBe("www");
  });
});

function cells(text: string): TextShapingCell[] {
  return [...text].map((character) => ({
    text: character,
    width: 1,
    styleKey: "regular",
    eligible: character !== " ",
  }));
}
