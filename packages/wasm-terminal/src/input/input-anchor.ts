export interface InputAnchorRequest {
  readonly canvasLeft: number;
  readonly canvasTop: number;
  readonly elementLeft: number;
  readonly elementTop: number;
  readonly cursorColumn: number;
  readonly cursorRow: number;
  readonly columns: number;
  readonly rows: number;
  readonly cellWidth: number;
  readonly cellHeight: number;
  readonly preeditCellWidth: number;
  readonly preeditCaretPixelOffset: number;
}

export interface InputAnchorLayout {
  readonly left: number;
  readonly top: number;
  readonly width: number;
  readonly height: number;
}

/** Keep the browser-reported IME caret at the midpoint of the terminal cursor cell. */
export function calculateInputAnchor(request: InputAnchorRequest): InputAnchorLayout {
  const cursorColumn = clamp(request.cursorColumn, 0, Math.max(0, request.columns - 1));
  const cursorRow = clamp(request.cursorRow, 0, Math.max(0, request.rows - 1));
  const anchorColumns = Math.max(1, Math.min(request.preeditCellWidth, request.columns));
  const caretOffset = Math.max(0, request.preeditCaretPixelOffset);
  return {
    left:
      request.canvasLeft -
      request.elementLeft +
      (cursorColumn + 0.5) * request.cellWidth -
      caretOffset,
    top: request.canvasTop - request.elementTop + cursorRow * request.cellHeight,
    width: anchorColumns * request.cellWidth,
    height: request.cellHeight,
  };
}

function clamp(value: number, minimum: number, maximum: number): number {
  return Math.max(minimum, Math.min(maximum, value));
}
