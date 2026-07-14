import type { CellData } from "../types.js";

export interface TerminalSearchOptions {
  readonly caseSensitive?: boolean;
  readonly regex?: boolean;
  readonly wholeWord?: boolean;
}

export interface TerminalSearchMatch {
  readonly text: string;
  readonly start: { readonly column: number; readonly row: number };
  readonly end: { readonly column: number; readonly row: number };
}

export function searchMatchContainsCell(
  match: TerminalSearchMatch,
  column: number,
  row: number,
): boolean {
  if (row < match.start.row || row > match.end.row) return false;
  if (match.start.row === match.end.row) {
    return row === match.start.row && column >= match.start.column && column <= match.end.column;
  }
  if (row === match.start.row) return column >= match.start.column;
  if (row === match.end.row) return column <= match.end.column;
  return true;
}

/** Find row-local matches and map JavaScript string offsets back to terminal cells. */
export function findTerminalMatches(
  lines: readonly (readonly CellData[])[],
  query: string,
  options: TerminalSearchOptions = {},
  wrappedRows: readonly boolean[] = [],
): readonly TerminalSearchMatch[] {
  if (!query) return [];
  const expression = createExpression(query, options);
  const matches: TerminalSearchMatch[] = [];
  let logicalText = "";
  let logicalPositions: Array<{ readonly column: number; readonly row: number }> = [];
  for (let row = 0; row < lines.length; row++) {
    const searchable = buildSearchableLine(lines[row] ?? []);
    logicalText += searchable.text;
    logicalPositions = logicalPositions.concat(
      searchable.columns.map((column) => ({ column, row })),
    );
    if (wrappedRows[row]) continue;
    appendMatches(matches, logicalText, logicalPositions, expression, options);
    logicalText = "";
    logicalPositions = [];
  }
  if (logicalText) appendMatches(matches, logicalText, logicalPositions, expression, options);
  return matches;
}

function appendMatches(
  output: TerminalSearchMatch[],
  text: string,
  positions: readonly { readonly column: number; readonly row: number }[],
  expression: RegExp,
  options: TerminalSearchOptions,
): void {
  for (const match of text.matchAll(expression)) {
    const value = match[0];
    const index = match.index;
    if (!value || index === undefined || !isWholeWord(text, index, value, options)) continue;
    const start = positions[index];
    const end = positions[index + value.length - 1];
    if (!start || !end) continue;
    output.push({ text: value, start, end });
  }
}

function createExpression(query: string, options: TerminalSearchOptions): RegExp {
  const source = options.regex ? query : escapeRegExp(query);
  return new RegExp(source, options.caseSensitive ? "gu" : "giu");
}

function buildSearchableLine(cells: readonly CellData[]): {
  readonly text: string;
  readonly columns: readonly number[];
} {
  let lastContentColumn = -1;
  for (let column = cells.length - 1; column >= 0; column--) {
    if ((cells[column]?.graphemeLen ?? 0) > 0) {
      lastContentColumn = column;
      break;
    }
  }
  let text = "";
  const columns: number[] = [];
  for (let column = 0; column <= lastContentColumn; column++) {
    const cell = cells[column];
    if (!cell || cell.width === 0) continue;
    const value = cell.graphemeLen > 0 ? cell.text : " ";
    text += value;
    for (let index = 0; index < value.length; index++) columns.push(column);
  }
  return { text, columns };
}

function isWholeWord(
  line: string,
  index: number,
  match: string,
  options: TerminalSearchOptions,
): boolean {
  if (!options.wholeWord) return true;
  const before = index === 0 ? "" : (Array.from(line.slice(0, index)).at(-1) ?? "");
  const after = Array.from(line.slice(index + match.length))[0] ?? "";
  return !isWordCharacter(before) && !isWordCharacter(after);
}

function isWordCharacter(value: string): boolean {
  return value ? /[\p{L}\p{N}_]/u.test(value) : false;
}

function escapeRegExp(value: string): string {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}
