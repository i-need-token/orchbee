import { describe, expect, it } from "vitest";
import {
  classifySurfaceMousePlatform,
  isSurfaceMouseModifierKey,
  resolveSurfaceMouseCursor,
  type SurfaceMouseCursorInput,
  type SurfaceMouseModifiers,
} from "../surface-mouse.js";

const NONE: SurfaceMouseModifiers = {
  alt: false,
  control: false,
  meta: false,
  shift: false,
};

describe("surface mouse cursor", () => {
  it("ignores non-modifier keys and preserves link or hidden cursor states", () => {
    expect(isSurfaceMouseModifierKey("KeyA")).toBe(false);
    expect(isSurfaceMouseModifierKey("ShiftLeft")).toBe(true);
    expect(resolveSurfaceMouseCursor(input({ overLink: true }))).toBeNull();
    expect(resolveSurfaceMouseCursor(input({ hidden: true }))).toBeNull();
  });

  it("uses an arrow for mouse tracking and Shift for text selection override", () => {
    expect(resolveSurfaceMouseCursor(input({ mouseTracking: true }))).toBe("default");
    expect(
      resolveSurfaceMouseCursor(
        input({ mouseTracking: true, modifiers: { ...NONE, shift: true } }),
      ),
    ).toBe("text");
  });

  it("uses a crosshair for rectangular selection on non-macOS platforms", () => {
    const rectangle = { ...NONE, alt: true, control: true };
    expect(resolveSurfaceMouseCursor(input({ modifiers: rectangle }))).toBe("crosshair");
    expect(
      resolveSurfaceMouseCursor(
        input({ mouseTracking: true, modifiers: { ...rectangle, shift: true } }),
      ),
    ).toBe("crosshair");
    expect(resolveSurfaceMouseCursor(input({ modifiers: { ...NONE, alt: true } }))).toBe("text");
  });

  it("uses Option alone for rectangular selection on macOS", () => {
    expect(
      resolveSurfaceMouseCursor(input({ platform: "macos", modifiers: { ...NONE, alt: true } })),
    ).toBe("crosshair");
    expect(
      resolveSurfaceMouseCursor(
        input({
          platform: "macos",
          mouseTracking: true,
          modifiers: { ...NONE, alt: true },
        }),
      ),
    ).toBe("default");
  });

  it("classifies Apple browser platforms", () => {
    expect(classifySurfaceMousePlatform("MacIntel")).toBe("macos");
    expect(classifySurfaceMousePlatform("iPhone")).toBe("macos");
    expect(classifySurfaceMousePlatform("Linux x86_64")).toBe("other");
  });
});

function input(overrides: Partial<SurfaceMouseCursorInput>): SurfaceMouseCursorInput {
  return {
    hidden: false,
    modifiers: NONE,
    mouseTracking: false,
    overLink: false,
    platform: "other",
    ...overrides,
  };
}
