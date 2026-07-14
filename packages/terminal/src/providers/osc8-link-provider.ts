/**
 * OSC 8 Hyperlink Provider
 *
 * Detects hyperlinks created with OSC 8 escape sequences.
 * The Ghostty WASM automatically assigns hyperlink_id to cells,
 * so we scan for contiguous regions with the same ID.
 */

import type { ILink, ILinkProvider, ILinkRange } from "../types";

/** Terminal interface needed by OSC8LinkProvider. */
export interface ITerminalForOSC8Provider {
  buffer: {
    active: {
      length: number;
      getLine(y: number):
        | {
            length: number;
            getCell(x: number): { getHyperlinkId(): number } | undefined;
          }
        | undefined;
    };
  };
  wasmTerm?: {
    getHyperlinkUri(row: number, col: number): string | null;
    getScrollbackHyperlinkUri(offset: number, col: number): string | null;
    getScrollbackLength(): number;
  };
}

export class OSC8LinkProvider implements ILinkProvider {
  private terminal: ITerminalForOSC8Provider;

  constructor(terminal: ITerminalForOSC8Provider) {
    this.terminal = terminal;
  }

  provideLinks(y: number, callback: (links: ILink[] | undefined) => void): void {
    const links: ILink[] = [];
    const visited = new Set<number>();

    const line = this.terminal.buffer.active.getLine(y);
    if (!line) {
      callback(undefined);
      return;
    }

    for (let x = 0; x < line.length; x++) {
      if (visited.has(x)) continue;

      const cell = line.getCell(x);
      if (!cell) continue;

      const hyperlinkId = cell.getHyperlinkId();
      if (hyperlinkId === 0) continue;

      const wasmTerm = this.terminal.wasmTerm;
      if (!wasmTerm) continue;
      const scrollbackLength = wasmTerm.getScrollbackLength();
      const viewportRow = y - scrollbackLength;

      let uri: string | null;
      if (viewportRow < 0) {
        uri = wasmTerm.getScrollbackHyperlinkUri(y, x);
      } else {
        uri = wasmTerm.getHyperlinkUri(viewportRow, x);
      }

      if (uri) {
        let endX = x;
        for (let col = x + 1; col < line.length; col++) {
          const nextCell = line.getCell(col);
          if (!nextCell || nextCell.getHyperlinkId() === 0) break;

          const nextUri =
            viewportRow < 0
              ? wasmTerm.getScrollbackHyperlinkUri(y, col)
              : wasmTerm.getHyperlinkUri(viewportRow, col);
          if (nextUri !== uri) break;

          endX = col;
        }

        for (let col = x; col <= endX; col++) visited.add(col);

        const range: ILinkRange = { start: { x, y }, end: { x: endX, y } };
        links.push({
          text: uri,
          range,
          target: { kind: "url", url: uri },
          activation: "modifier",
        });
      }
    }

    callback(links.length > 0 ? links : undefined);
  }

  dispose(): void {}
}
