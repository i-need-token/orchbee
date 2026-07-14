import { describe, expect, it, vi } from "vitest";
import { drawGhosttySprite, isGhosttySprite, physicalSpriteGeometry } from "../ghostty-sprites";

function createContext() {
  const context = {
    beginPath: vi.fn<CanvasRenderingContext2D["beginPath"]>(),
    closePath: vi.fn<CanvasRenderingContext2D["closePath"]>(),
    fill: vi.fn<CanvasRenderingContext2D["fill"]>(),
    fillRect: vi.fn<CanvasRenderingContext2D["fillRect"]>(),
    globalAlpha: 1,
    lineTo: vi.fn<CanvasRenderingContext2D["lineTo"]>(),
    moveTo: vi.fn<CanvasRenderingContext2D["moveTo"]>(),
    restore: vi.fn<CanvasRenderingContext2D["restore"]>(),
    save: vi.fn<CanvasRenderingContext2D["save"]>(),
    stroke: vi.fn<CanvasRenderingContext2D["stroke"]>(),
  };
  return context as typeof context & CanvasRenderingContext2D;
}

describe("Ghostty sprites", () => {
  it("recognizes only the implemented sprite subset", () => {
    expect(isGhosttySprite(0x2500)).toBe(true);
    expect(isGhosttySprite(0x2551)).toBe(true);
    expect(isGhosttySprite(0x2581)).toBe(true);
    expect(isGhosttySprite(0x25e5)).toBe(true);
    expect(isGhosttySprite(0xe0b3)).toBe(true);
    expect(isGhosttySprite(0x2504)).toBe(false);
    expect(isGhosttySprite(0x41)).toBe(false);
  });

  it("draws a light corner with Ghostty's integer-centered boxes", () => {
    const context = createContext();

    expect(drawGhosttySprite(context, 0x250c, 10, 20, 8, 17, 1)).toBe(true);

    expect(context.fillRect).toHaveBeenCalledTimes(2);
    expect(context.fillRect).toHaveBeenNthCalledWith(1, 13, 28, 5, 1);
    expect(context.fillRect).toHaveBeenNthCalledWith(2, 13, 28, 1, 9);
  });

  it("joins the upper and lower halves of a double vertical", () => {
    const context = createContext();

    drawGhosttySprite(context, 0x2551, 0, 0, 8, 17, 1);

    expect(context.fillRect).toHaveBeenCalledTimes(4);
    expect(context.fillRect).toHaveBeenNthCalledWith(1, 2, 0, 1, 9);
    expect(context.fillRect).toHaveBeenNthCalledWith(2, 4, 0, 1, 9);
    expect(context.fillRect).toHaveBeenNthCalledWith(3, 2, 8, 1, 9);
    expect(context.fillRect).toHaveBeenNthCalledWith(4, 4, 8, 1, 9);
  });

  it("uses full-cell triangles and Ghostty's box thickness for Powerline", () => {
    const filled = createContext();
    const outlined = createContext();
    outlined.fillStyle = "#abcdef";

    drawGhosttySprite(filled, 0xe0b0, 2, 3, 8, 16, 2);
    drawGhosttySprite(outlined, 0xe0b3, 2, 3, 8, 16, 2);

    expect(filled.moveTo).toHaveBeenCalledWith(2, 3);
    expect(filled.lineTo).toHaveBeenNthCalledWith(1, 10, 11);
    expect(filled.lineTo).toHaveBeenNthCalledWith(2, 2, 19);
    expect(filled.closePath).toHaveBeenCalledOnce();
    expect(filled.fill).toHaveBeenCalledOnce();
    expect(outlined.moveTo).toHaveBeenCalledWith(10, 3);
    expect(outlined.lineTo).toHaveBeenNthCalledWith(1, 2, 11);
    expect(outlined.lineTo).toHaveBeenNthCalledWith(2, 10, 19);
    expect(outlined.lineWidth).toBe(2);
    expect(outlined.lineCap).toBe("butt");
    expect(outlined.strokeStyle).toBe("#abcdef");
    expect(outlined.stroke).toHaveBeenCalledOnce();
    expect(outlined.save).toHaveBeenCalledOnce();
    expect(outlined.restore).toHaveBeenCalledOnce();
  });

  it("scales the cell and box thickness into physical pixels", () => {
    expect(
      physicalSpriteGeometry(
        {
          cellWidth: 8,
          cellHeight: 17,
          cellBaseline: 4,
          underlinePosition: 15,
          underlineThickness: 1,
          strikethroughPosition: 8,
          strikethroughThickness: 1,
          overlinePosition: 0,
          overlineThickness: 1,
          boxThickness: 1,
          cursorThickness: 1,
          cursorHeight: 17,
          iconHeight: 17,
          iconHeightSingle: 15,
          faceWidth: 8,
          faceHeight: 17,
          faceY: 0,
        },
        2,
        2,
      ),
    ).toEqual({ width: 32, height: 34, boxThickness: 2 });
  });

  it("draws block fractions and shades with Ghostty's rounded geometry", () => {
    const lowerEighth = createContext();
    const leftQuarter = createContext();
    const shade = createContext();

    drawGhosttySprite(lowerEighth, 0x2581, 2, 3, 8, 17, 1);
    drawGhosttySprite(leftQuarter, 0x258e, 2, 3, 8, 17, 1);
    drawGhosttySprite(shade, 0x2592, 2, 3, 8, 17, 1);

    expect(lowerEighth.fillRect).toHaveBeenCalledWith(2, 18, 8, 2);
    expect(leftQuarter.fillRect).toHaveBeenCalledWith(2, 3, 2, 17);
    expect(shade.fillRect).toHaveBeenCalledWith(2, 3, 8, 17);
    expect(shade.globalAlpha).toBe(0.5);
    expect(shade.save).toHaveBeenCalledOnce();
    expect(shade.restore).toHaveBeenCalledOnce();
  });

  it("draws overlapping odd-cell quadrants and full-cell corner triangles", () => {
    const quadrant = createContext();
    const triangle = createContext();

    drawGhosttySprite(quadrant, 0x259a, 2, 3, 7, 17, 1);
    drawGhosttySprite(triangle, 0x25e2, 2, 3, 8, 17, 1);

    expect(quadrant.fillRect.mock.calls).toEqual([
      [2, 3, 4, 9],
      [5, 11, 4, 9],
    ]);
    expect(triangle.moveTo).toHaveBeenCalledWith(2, 20);
    expect(triangle.lineTo).toHaveBeenNthCalledWith(1, 10, 20);
    expect(triangle.lineTo).toHaveBeenNthCalledWith(2, 10, 3);
    expect(triangle.closePath).toHaveBeenCalledOnce();
    expect(triangle.fill).toHaveBeenCalledOnce();
  });

  it("leaves unsupported characters for font rasterization", () => {
    expect(drawGhosttySprite(createContext(), 0x41, 0, 0, 8, 17, 1)).toBe(false);
  });
});
