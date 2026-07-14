import { describe, expect, it } from "vitest";
import type { DetailedFontMetrics } from "../font-metrics.js";
import {
  cellConstraintWidth,
  constrainGlyph,
  getNerdFontConstraint,
  NO_GLYPH_CONSTRAINT,
  type ConstraintCell,
  type GlyphSize,
} from "../glyph-constraints.js";

const SYMBOL = "";
const POWERLINE_LEFT = "";
const POWERLINE_RIGHT = "";

describe("Ghostty glyph constraints", () => {
  it("calculates symbol constraint widths from adjacent cells", () => {
    expect(widthFor(SYMBOL, 0)).toBe(2);
    expect(widthFor(`${SYMBOL}z`, 0)).toBe(1);
    expect(widthFor(`${SYMBOL} z`, 0)).toBe(2);
    expect(widthFor(`${SYMBOL}\u00a0z`, 0)).toBe(1);
    expect(widthFor(`   ${SYMBOL}`, 3)).toBe(1);
    expect(widthFor(`z${SYMBOL}`, 1)).toBe(2);
    expect(widthFor(`${SYMBOL}${SYMBOL}`, 0)).toBe(1);
    expect(widthFor(`${SYMBOL}${SYMBOL}`, 1)).toBe(1);
    expect(widthFor(`${SYMBOL} ${SYMBOL}`, 0)).toBe(2);
    expect(widthFor(`${SYMBOL} ${SYMBOL}`, 2)).toBe(2);
    expect(widthFor(`${SYMBOL}${POWERLINE_RIGHT}`, 0)).toBe(1);
    expect(widthFor(`${POWERLINE_LEFT}${SYMBOL}`, 1)).toBe(2);
    expect(widthFor(POWERLINE_LEFT, 0)).toBe(2);
    expect(widthFor(`${POWERLINE_LEFT} z`, 0)).toBe(2);
  });

  it("matches Ghostty's symbol, emoji, Nerd Font, and Powerline geometry", () => {
    const ascii: GlyphSize = { width: 6.784, height: 15.28, x: 1.408, y: 4.84 };
    expect(constrainGlyph(ascii, metrics, NO_GLYPH_CONSTRAINT, 1)).toEqual(ascii);
    expect(constrainGlyph(ascii, metrics, NO_GLYPH_CONSTRAINT, 2)).toEqual(ascii);

    const fit = { ...NO_GLYPH_CONSTRAINT, size: "fit" } as const;
    const square = { width: 10.272, height: 10.272, x: 2.864, y: 5.304 };
    expectGlyph(constrainGlyph(square, metrics, fit, 1), {
      width: metrics.faceWidth,
      height: metrics.faceWidth,
      x: 0,
      y: 5.64,
    });
    expectGlyph(constrainGlyph(square, metrics, fit, 2), square);

    const emoji = {
      ...NO_GLYPH_CONSTRAINT,
      size: "cover",
      alignHorizontal: "center",
      alignVertical: "center",
      padLeft: 0.025,
      padRight: 0.025,
    } as const;
    expectGlyph(constrainGlyph({ width: 20, height: 20, x: 0.46, y: 1 }, metrics, emoji, 2), {
      width: 18.72,
      height: 18.72,
      x: 0.44,
      y: 1.4,
    });

    const nerd = getNerdFontConstraint(0xea61);
    expect(nerd).toMatchObject({
      size: "fit_cover1",
      height: "icon",
      alignHorizontal: "center1",
      alignVertical: "center1",
    });
    if (!nerd) throw new Error("Missing generated Nerd Font constraint");
    const lightbulb = { width: 9.015625, height: 13.015625, x: 3.015625, y: 3.76525 };
    expectGlyph(constrainGlyph(lightbulb, metrics, nerd, 1), {
      width: 7.2125,
      height: 10.4125,
      x: 0.8125,
      y: 5.950695224719102,
    });
    expectGlyph(constrainGlyph(lightbulb, metrics, nerd, 2), {
      width: lightbulb.width,
      height: lightbulb.height,
      x: 1.015625,
      y: 4.7483690308988775,
    });

    const powerline = getNerdFontConstraint(0xe0c0);
    expect(powerline).toMatchObject({
      size: "stretch",
      height: "cell",
      alignHorizontal: "start",
      alignVertical: "center1",
    });
    if (!powerline) throw new Error("Missing generated Powerline constraint");
    const flame = { width: 16.796875, height: 16.46875, x: -0.796875, y: 1.7109375 };
    expectGlyph(constrainGlyph(flame, metrics, powerline, 1), {
      width: metrics.cellWidth,
      height: metrics.cellHeight,
      x: 0,
      y: 0,
    });
    expectGlyph(constrainGlyph(flame, metrics, powerline, 2), {
      width: 2 * metrics.cellWidth,
      height: metrics.cellHeight,
      x: 0,
      y: 0,
    });
  });
});

const metrics: DetailedFontMetrics = {
  cellWidth: 10,
  cellHeight: 22,
  cellBaseline: 5,
  underlinePosition: 19,
  underlineThickness: 1,
  strikethroughPosition: 12,
  strikethroughThickness: 1,
  overlinePosition: 0,
  overlineThickness: 1,
  boxThickness: 1,
  cursorThickness: 1,
  cursorHeight: 22,
  iconHeight: 21.12,
  iconHeightSingle: 44.48 / 3,
  faceWidth: 9.6,
  faceHeight: 21.12,
  faceY: 0.2,
};

function widthFor(text: string, column: number): 1 | 2 {
  const cells: ConstraintCell[] = [...text].map((character) => ({
    codepoint: character.codePointAt(0) ?? 0,
    width: 1,
  }));
  while (cells.length < 4) cells.push({ codepoint: 0, width: 1 });
  return cellConstraintWidth(cells, column, 4);
}

function expectGlyph(actual: GlyphSize, expected: GlyphSize): void {
  expect(actual.width).toBeCloseTo(expected.width, 10);
  expect(actual.height).toBeCloseTo(expected.height, 10);
  expect(actual.x).toBeCloseTo(expected.x, 10);
  expect(actual.y).toBeCloseTo(expected.y, 10);
}
