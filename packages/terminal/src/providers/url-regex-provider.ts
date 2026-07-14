/**
 * URL Regex Link Provider
 *
 * Detects plain text URLs using regex pattern matching.
 * Runs after OSC8LinkProvider, so explicit hyperlinks take precedence.
 */

import type { ILink, ILinkBuffer, ILinkBufferLine, ILinkProvider } from "../types";

/** Terminal interface needed by UrlRegexProvider. */
export interface ITerminalForUrlProvider {
  buffer: {
    active: ILinkBuffer;
  };
}

const URL_REGEX =
  /(?:https?:\/\/|mailto:|ftp:\/\/|ssh:\/\/|git:\/\/|tel:|magnet:|gemini:\/\/|gopher:\/\/|news:)[\w\-.~:/?#@!$&*+,;=%]+/gi;

const TRAILING_PUNCTUATION = /[.,;!?)\]]+$/;

export class UrlRegexProvider implements ILinkProvider {
  private terminal: ITerminalForUrlProvider;

  constructor(terminal: ITerminalForUrlProvider) {
    this.terminal = terminal;
  }

  provideLinks(y: number, callback: (links: ILink[] | undefined) => void): void {
    const links: ILink[] = [];

    const logicalLine = this.readLogicalLine(y);
    if (!logicalLine) {
      callback(undefined);
      return;
    }

    URL_REGEX.lastIndex = 0;

    let match: RegExpExecArray | null = URL_REGEX.exec(logicalLine.text);
    while (match !== null) {
      let url = match[0];
      const startIndex = match.index;
      let endIndex = match.index + url.length - 1;

      const stripped = url.replace(TRAILING_PUNCTUATION, "");
      if (stripped.length < url.length) {
        url = stripped;
        endIndex = startIndex + url.length - 1;
      }

      const start = logicalLine.positions[startIndex];
      const end = logicalLine.positions[endIndex];
      if (url.length > 8 && start && end) {
        links.push({
          text: url,
          range: { start, end },
          target: { kind: "url", url },
          activation: "modifier",
        });
      }

      match = URL_REGEX.exec(logicalLine.text);
    }

    callback(links.length > 0 ? links : undefined);
  }

  private readLogicalLine(y: number): {
    readonly text: string;
    readonly positions: readonly { readonly x: number; readonly y: number }[];
  } | null {
    const buffer = this.terminal.buffer.active;
    if (!buffer.getLine(y)) return null;
    let start = y;
    while (start > 0 && buffer.getLine(start - 1)?.isWrapped) start--;

    let text = "";
    const positions: Array<{ readonly x: number; readonly y: number }> = [];
    for (let row = start; row < buffer.length; row++) {
      const line = buffer.getLine(row);
      if (!line) break;
      const segment = this.readLine(line, row);
      text += segment.text;
      positions.push(...segment.positions);
      if (!line.isWrapped) break;
    }
    return { text, positions };
  }

  private readLine(
    line: ILinkBufferLine,
    row: number,
  ): {
    readonly text: string;
    readonly positions: readonly { readonly x: number; readonly y: number }[];
  } {
    let text = "";
    const positions: Array<{ readonly x: number; readonly y: number }> = [];
    for (let column = 0; column < line.length; column++) {
      const cell = line.getCell(column);
      if (!cell || cell.getWidth() === 0) continue;
      const codepoint = cell.getCodepoint();
      const value =
        codepoint === 0 || codepoint < 32 || codepoint > 0x10ffff
          ? " "
          : String.fromCodePoint(codepoint);
      text += value;
      for (let index = 0; index < value.length; index++) positions.push({ x: column, y: row });
    }
    return { text, positions };
  }

  dispose(): void {}
}
