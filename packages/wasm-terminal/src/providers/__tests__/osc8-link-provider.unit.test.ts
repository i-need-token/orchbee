import { describe, expect, it } from "vitest";
import { OSC8LinkProvider } from "../osc8-link-provider.js";
import type { ILink } from "../../types.js";

describe("OSC8LinkProvider", () => {
  it("groups contiguous cells that resolve to the same URI", () => {
    const provider = new OSC8LinkProvider({
      buffer: {
        active: {
          length: 1,
          getLine: () => ({
            length: 4,
            getCell: (index) => ({ getHyperlinkId: () => (index < 3 ? 1 : 0) }),
          }),
        },
      },
      wasmTerm: {
        getHyperlinkUri: (_row, column) => (column < 3 ? "https://example.com" : null),
        getScrollbackHyperlinkUri: () => null,
        getScrollbackLength: () => 0,
      },
    });
    let links: ILink[] | undefined;
    provider.provideLinks(0, (value) => {
      links = value;
    });
    expect(links?.[0]).toMatchObject({
      text: "https://example.com",
      range: { start: { x: 0, y: 0 }, end: { x: 2, y: 0 } },
      target: { kind: "url", url: "https://example.com" },
      activation: "modifier",
    });
  });
});
