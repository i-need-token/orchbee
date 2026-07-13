import { describe, expect, it, vi } from "vitest";
import { LinkDetector } from "../link-detector.js";
import type { ILinkProvider } from "../types.js";

describe("LinkDetector", () => {
  it("caches provider results and invalidates changed rows", async () => {
    const line = {
      length: 20,
      isWrapped: false,
      getCell: () => ({
        getCodepoint: () => 0,
        getHyperlinkId: () => 0,
        getWidth: () => 1,
        isBold: () => 0,
        isItalic: () => 0,
        isDim: () => false,
      }),
      translateToString: () => "",
    };
    const detector = new LinkDetector({ buffer: { active: { length: 1, getLine: () => line } } });
    const provideLinks = vi.fn<ILinkProvider["provideLinks"]>((_row, callback) =>
      callback([
        {
          text: "https://example.com",
          range: { start: { x: 0, y: 0 }, end: { x: 18, y: 0 } },
          activate: () => {},
        },
      ]),
    );
    detector.registerProvider({ provideLinks } as ILinkProvider);
    expect((await detector.getLinkAt(4, 0))?.text).toBe("https://example.com");
    await detector.getLinkAt(5, 0);
    expect(provideLinks).toHaveBeenCalledOnce();
    detector.invalidateRows(0, 0);
    await detector.getLinkAt(4, 0);
    expect(provideLinks).toHaveBeenCalledTimes(2);
    detector.dispose();
  });
});
