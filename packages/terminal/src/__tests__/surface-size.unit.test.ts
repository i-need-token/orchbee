import { describe, expect, it } from "vitest";
import { balanceSurfacePadding } from "../surface-size";

describe("balanceSurfacePadding", () => {
  it("distributes whitespace equally after explicit padding", () => {
    const padding = balanceSurfacePadding(
      { width: 1_050, height: 850 },
      { width: 10, height: 20 },
      { top: 4, bottom: 4, left: 4, right: 4 },
      "equal",
    );

    expect(padding.left).toBe(padding.right);
    expect(padding.top).toBe(padding.bottom);
    expect(padding.top).toBeGreaterThan(0);
    expect(padding).toEqual({ top: 5, bottom: 5, left: 5, right: 5 });
  });

  it("shifts excess top padding to the bottom in the capped mode", () => {
    const padding = balanceSurfacePadding(
      { width: 1_090, height: 1_070 },
      { width: 20, height: 40 },
      { top: 0, bottom: 0, left: 0, right: 0 },
      true,
    );

    expect(padding.left).toBe(padding.right);
    expect(padding.top).toBeLessThan(padding.bottom);
    expect(padding).toEqual({ top: 10, bottom: 20, left: 5, right: 5 });
  });

  it("never returns negative padding for a zero-sized surface", () => {
    expect(
      balanceSurfacePadding(
        { width: 0, height: 0 },
        { width: 10, height: 20 },
        { top: 0, bottom: 0, left: 0, right: 0 },
        "equal",
      ),
    ).toEqual({ top: 0, bottom: 0, left: 0, right: 0 });
  });
});
