import { describe, expect, it, vi } from "vitest";
import { LinkDetector } from "../link-detector";
import type { ILink, ILinkBufferLine, ILinkProvider } from "../types";

describe("LinkDetector", () => {
  it("caches provider results and invalidates changed rows", async () => {
    const detector = createDetector(1);
    const provideLinks = vi.fn<ILinkProvider["provideLinks"]>((_row, callback) =>
      callback([link("https://example.com", 0, 0, 18, 0)]),
    );
    detector.registerProvider({ provideLinks });
    expect((await detector.getLinkAt(4, 0))?.text).toBe("https://example.com");
    await detector.getLinkAt(5, 0);
    expect(provideLinks).toHaveBeenCalledOnce();
    detector.invalidateRows(0, 0);
    await detector.getLinkAt(4, 0);
    expect(provideLinks).toHaveBeenCalledTimes(2);
    detector.dispose();
  });

  it("maps a multi-row link across its first, middle, and last rows", async () => {
    const detector = createDetector(3);
    detector.registerProvider({
      provideLinks: (_row, callback) => callback([link("wrapped", 5, 0, 2, 2)]),
    });

    await expect(detector.getLinkAt(4, 0)).resolves.toBeUndefined();
    await expect(detector.getLinkAt(5, 0)).resolves.toMatchObject({ text: "wrapped" });
    await expect(detector.getLinkAt(19, 1)).resolves.toMatchObject({ text: "wrapped" });
    await expect(detector.getLinkAt(2, 2)).resolves.toMatchObject({ text: "wrapped" });
    await expect(detector.getLinkAt(3, 2)).resolves.toBeUndefined();
    detector.dispose();
  });

  it("preserves earlier-provider precedence for overlapping links", async () => {
    const detector = createDetector(1);
    detector.registerProvider({
      provideLinks: (_row, callback) => callback([link("explicit", 2, 0, 8, 0)]),
    });
    detector.registerProvider({
      provideLinks: (_row, callback) => callback([link("regex", 0, 0, 12, 0)]),
    });

    await expect(detector.getLinkAt(4, 0)).resolves.toMatchObject({ text: "explicit" });
    await expect(detector.getLinkAt(10, 0)).resolves.toMatchObject({ text: "regex" });
    detector.dispose();
  });

  it("coalesces concurrent scans for one row", async () => {
    const detector = createDetector(1);
    let resolveScan: ((links: ILink[] | undefined) => void) | undefined;
    const provideLinks = vi.fn<ILinkProvider["provideLinks"]>((_row, callback) => {
      resolveScan = callback;
    });
    detector.registerProvider({ provideLinks });

    const first = detector.getLinkAt(3, 0);
    const second = detector.getLinkAt(4, 0);
    expect(provideLinks).toHaveBeenCalledOnce();
    resolveScan?.([link("shared", 2, 0, 6, 0)]);

    await expect(first).resolves.toMatchObject({ text: "shared" });
    await expect(second).resolves.toMatchObject({ text: "shared" });
    expect(provideLinks).toHaveBeenCalledOnce();
    detector.dispose();
  });

  it("discards an in-flight result after invalidation and scans again", async () => {
    const detector = createDetector(1);
    const callbacks: Array<(links: ILink[] | undefined) => void> = [];
    const provideLinks = vi.fn<ILinkProvider["provideLinks"]>((_row, callback) => {
      callbacks.push(callback);
    });
    detector.registerProvider({ provideLinks });

    const lookup = detector.getLinkAt(3, 0);
    detector.invalidateRows(0, 0);
    callbacks[0]?.([link("stale", 2, 0, 6, 0)]);
    await vi.waitFor(() => expect(provideLinks).toHaveBeenCalledTimes(2));
    callbacks[1]?.([link("fresh", 2, 0, 6, 0)]);

    await expect(lookup).resolves.toMatchObject({ text: "fresh" });
    detector.dispose();
  });

  it("invalidates links spanning an interior changed row and disposes providers", async () => {
    const detector = createDetector(3);
    const dispose = vi.fn<() => void>();
    const provideLinks = vi.fn<ILinkProvider["provideLinks"]>((_row, callback) =>
      callback([link("wrapped", 5, 0, 2, 2)]),
    );
    detector.registerProvider({ provideLinks, dispose });
    await detector.getLinkAt(1, 1);

    detector.invalidateRows(1, 1);
    await detector.getLinkAt(1, 1);
    expect(provideLinks).toHaveBeenCalledTimes(2);

    detector.dispose();
    expect(dispose).toHaveBeenCalledOnce();
  });

  it("cancels an in-flight lookup when disposed", async () => {
    const detector = createDetector(1);
    const dispose = vi.fn<() => void>();
    let resolveScan: ((links: ILink[] | undefined) => void) | undefined;
    detector.registerProvider({
      provideLinks: (_row, callback) => {
        resolveScan = callback;
      },
      dispose,
    });

    const lookup = detector.getLinkAt(3, 0);
    detector.dispose();
    resolveScan?.([link("stale", 2, 0, 6, 0)]);

    await expect(lookup).resolves.toBeUndefined();
    await expect(detector.getLinkAt(3, 0)).resolves.toBeUndefined();
    detector.dispose();
    expect(dispose).toHaveBeenCalledOnce();
  });

  it("does not invoke providers for positions outside the buffer", async () => {
    const detector = createDetector(1);
    const provideLinks = vi.fn<ILinkProvider["provideLinks"]>();
    detector.registerProvider({ provideLinks });

    await expect(detector.getLinkAt(-1, 0)).resolves.toBeUndefined();
    await expect(detector.getLinkAt(20, 0)).resolves.toBeUndefined();
    await expect(detector.getLinkAt(0, 1)).resolves.toBeUndefined();
    expect(provideLinks).not.toHaveBeenCalled();
    detector.dispose();
  });
});

function createDetector(rows: number): LinkDetector {
  const lines = Array.from({ length: rows }, () => createLine());
  return new LinkDetector({
    buffer: { active: { length: rows, getLine: (row) => lines[row] } },
  });
}

function createLine(): ILinkBufferLine {
  return {
    length: 20,
    isWrapped: false,
    getCell: (column) =>
      column >= 0 && column < 20
        ? {
            getCodepoint: () => 0,
            getHyperlinkId: () => 0,
            getWidth: () => 1,
            isBold: () => 0,
            isItalic: () => 0,
            isDim: () => false,
          }
        : undefined,
    translateToString: () => "",
  };
}

function link(text: string, startX: number, startY: number, endX: number, endY: number): ILink {
  return {
    text,
    range: { start: { x: startX, y: startY }, end: { x: endX, y: endY } },
    activate: () => {},
  };
}
