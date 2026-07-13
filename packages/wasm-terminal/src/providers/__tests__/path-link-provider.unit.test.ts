import { describe, expect, it } from "vitest";
import { PathLinkProvider } from "../path-link-provider.js";
import type { ILink, ILinkBufferLine } from "../../types.js";

describe("PathLinkProvider", () => {
  it("resolves a relative path and source position against OSC PWD", () => {
    const provider = createProvider([line("error at src/main.ts:12:4")]);
    expect(provide(provider, 0)?.[0]).toMatchObject({
      text: "src/main.ts:12:4",
      range: { start: { x: 9, y: 0 }, end: { x: 24, y: 0 } },
      target: {
        kind: "file",
        path: "/Users/test/project/src/main.ts",
        line: 12,
        column: 4,
      },
    });
  });

  it("detects an absolute path across a soft wrap", () => {
    const provider = createProvider([line("open /Users/test/pro", true), line("ject/file.ts")]);
    expect(provide(provider, 1)?.[0]).toMatchObject({
      text: "/Users/test/project/file.ts",
      range: { start: { x: 5, y: 0 }, end: { x: 11, y: 1 } },
      target: { kind: "file", path: "/Users/test/project/file.ts" },
    });
  });
});

function createProvider(lines: readonly ILinkBufferLine[]): PathLinkProvider {
  return new PathLinkProvider({
    buffer: { active: { length: lines.length, getLine: (row) => lines[row] } },
    getPwd: () => "file:///Users/test/project",
    getSemanticZone: () => "output",
  });
}

function provide(provider: PathLinkProvider, row: number): ILink[] | undefined {
  let links: ILink[] | undefined;
  provider.provideLinks(row, (value) => {
    links = value;
  });
  return links;
}

function line(text: string, isWrapped = false): ILinkBufferLine {
  return {
    length: text.length,
    isWrapped,
    getCell: (index) => ({
      getCodepoint: () => text.codePointAt(index) ?? 0,
      getWidth: () => 1,
      getHyperlinkId: () => 0,
      isBold: () => 0,
      isItalic: () => 0,
      isDim: () => false,
    }),
    translateToString: () => text,
  };
}
