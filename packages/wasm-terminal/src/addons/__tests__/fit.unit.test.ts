import { afterEach, describe, expect, it, vi } from "vitest";
import { FitAddon } from "../fit.js";
import type { ITerminalCore } from "../../types.js";

afterEach(() => vi.unstubAllGlobals());

describe("FitAddon", () => {
  it("calculates dimensions from renderer metrics and container size", () => {
    vi.stubGlobal("window", {
      getComputedStyle: () => ({ getPropertyValue: () => "0" }),
    });
    const element = { clientWidth: 815, clientHeight: 340 } as HTMLElement;
    const terminal = {
      cols: 80,
      rows: 24,
      element,
      renderer: { getMetrics: () => ({ width: 8, height: 17 }) },
    } as ITerminalCore & { renderer: { getMetrics(): { width: number; height: number } } };
    const addon = new FitAddon();
    addon.activate(terminal);
    expect(addon.proposeDimensions()).toEqual({ cols: 100, rows: 20 });
    addon.dispose();
  });

  it("resizes only when proposed dimensions change", () => {
    vi.stubGlobal("window", {
      getComputedStyle: () => ({ getPropertyValue: () => "0" }),
    });
    const resize = vi.fn<(cols: number, rows: number) => void>();
    const terminal = {
      cols: 80,
      rows: 24,
      element: { clientWidth: 655, clientHeight: 340 } as HTMLElement,
      renderer: { getMetrics: () => ({ width: 8, height: 17 }) },
      resize,
    } as ITerminalCore & {
      renderer: { getMetrics(): { width: number; height: number } };
      resize(cols: number, rows: number): void;
    };
    const addon = new FitAddon();
    addon.activate(terminal);
    addon.fit();
    expect(resize).toHaveBeenCalledWith(80, 20);
    addon.dispose();
  });
});
