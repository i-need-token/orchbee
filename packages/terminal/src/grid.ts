/**
 * Grid data reader — iterator-based viewport and cell access.
 *
 * Uses the upstream Ghostty API's row/cell iterator pattern for
 * reading viewport data. Unlike the patched API which returned
 * a flat 16-byte cell struct array, the upstream API requires
 * iterating rows and cells individually.
 *
 * Performance optimizations:
 * - Reuse iterator/cells handles across calls
 * - Pre-allocate output buffers once
 * - Batch field reads with row_cells_get_multi where possible
 */

import type { WasmBridge } from "./wasm-bridge.js";
import {
  type CellData,
  CellDataField,
  CellSemanticContent,
  CellFlags,
  CellWide,
  CELL_VALUE_SIZE,
  COLOR_RGB_SIZE,
  DirtyState,
  GRID_REF_SIZE,
  POINT_SIZE,
  PointTag,
  RawRowData,
  RENDER_STATE_COLORS_SIZE,
  type RenderStateHandle,
  RenderStateData,
  RowCellsData,
  RowData,
  STYLE_SIZE,
  type TerminalHandle,
  UnderlineStyle,
} from "./types.js";

// Re-export types for the public API
export type { CellData } from "./types.js";

/** Maximum grapheme codepoints to buffer per lookup. */
const GRAPHEME_BUFFER_CODEPOINTS = 16;
/** Size of grapheme buffer in bytes (16 codepoints × 4 bytes). */
const GRAPHEME_BUFFER_BYTES = GRAPHEME_BUFFER_CODEPOINTS * 4;
/** Size of UTF-8 buffer for grapheme text. */
const UTF8_BUFFER_SIZE = 32;

/** A render-state snapshot captured without mutating the terminal. */
export interface GridFrame {
  readonly lines: readonly (readonly CellData[])[];
  readonly dirtyRows: readonly boolean[];
  readonly wrappedRows: readonly boolean[];
}

/** A line resolved from Ghostty's scrollback history. */
export interface ScrollbackLine {
  readonly cells: readonly CellData[];
  readonly wrapped: boolean;
}

/**
 * Grid data reader for a terminal instance.
 *
 * Usage pattern (per frame):
 *   1. grid.update() → DirtyState
 *   2. grid.forEachRow((row, cells) => { ... })
 *   3. grid.markClean()
 */
export class Grid {
  private readonly bridge: WasmBridge;
  private readonly termHandle: TerminalHandle;
  private readonly rsHandle: RenderStateHandle;
  private _cols: number;
  private _rows: number;

  /** Reusable row iterator handle (created once, reused). */
  private rowIterHandle = 0;
  /** Reusable row cells handle (created once, reused). */
  private cellsHandle = 0;

  /** Pre-allocated output buffers. */
  private u16Buf = 0;
  private colorBuf = 0;
  private graphemeBuf = 0;
  private utf8DataBuf = 0;
  private ghosttyBuf = 0; // GhosttyBuffer struct (12 bytes)
  private dirtyValBuf = 0; // 4 bytes for DirtyState value
  private boolBuf = 0; // 1 byte for bool values
  private u32Buf = 0; // 4 bytes for u32 values
  private rawCellBuf = 0; // 8-byte opaque GhosttyCell value
  private rawRowBuf = 0; // 8-byte opaque GhosttyRow value
  private styleBuf = 0; // GhosttyStyle
  private handleSlot = 0; // reusable pointer-sized output slot
  private pointBuf = 0; // GhosttyPoint
  private gridRefBuf = 0; // GhosttyGridRef
  private colorsBuf = 0; // GhosttyRenderStateColors

  /** Whether buffers have been allocated. */
  private buffersAllocated = false;

  constructor(
    bridge: WasmBridge,
    termHandle: TerminalHandle,
    rsHandle: RenderStateHandle,
    cols: number,
    rows: number,
  ) {
    this.bridge = bridge;
    this.termHandle = termHandle;
    this.rsHandle = rsHandle;
    this._cols = cols;
    this._rows = rows;
  }

  get cols(): number {
    return this._cols;
  }

  get rows(): number {
    return this._rows;
  }

  // ==========================================================================
  // Render State
  // ==========================================================================

  /** Sync render state from terminal. Call once per frame. */
  update(): DirtyState {
    const exp = this.bridge.getExports();
    const result = exp.ghostty_render_state_update(this.rsHandle, this.termHandle);
    if (result !== 0) return DirtyState.NONE;

    // Read dirty state
    this.ensureBuffers();
    const dirtyResult = exp.ghostty_render_state_get(
      this.rsHandle,
      RenderStateData.DIRTY,
      this.dirtyValBuf,
    );
    if (dirtyResult !== 0) return DirtyState.NONE;
    const dirty = this.bridge.createView(this.dirtyValBuf, 4).getUint32(0, true);
    if (dirty === DirtyState.PARTIAL || dirty === DirtyState.FULL) return dirty;
    return DirtyState.NONE;
  }

  /** Check if a specific row is dirty. Requires row iteration. */
  isRowDirty(row: number): boolean {
    this.ensureBuffers();
    this.populateRowIterator();

    const exp = this.bridge.getExports();
    let currentRow = -1;
    while (exp.ghostty_render_state_row_iterator_next(this.rowIterHandle)) {
      currentRow++;
      if (currentRow === row) {
        const result = exp.ghostty_render_state_row_get(
          this.rowIterHandle,
          RowData.DIRTY,
          this.boolBuf,
        );
        if (result !== 0) return false;
        return this.bridge.createView(this.boolBuf, 1).getUint8(0) !== 0;
      }
    }
    return false;
  }

  /** Mark all render state as clean (call after rendering). */
  markClean(): void {
    this.ensureBuffers();
    const exp = this.bridge.getExports();
    // Set dirty to NONE (0)
    const view = this.bridge.createView(this.dirtyValBuf, 4);
    view.setUint32(0, 0, true);
    exp.ghostty_render_state_set(this.rsHandle, 0 /* RenderStateOption.DIRTY */, this.dirtyValBuf);
  }

  // ==========================================================================
  // Viewport Access (Iterator-Based)
  // ==========================================================================

  /**
   * Read all viewport cells into a flat array.
   *
   * This is the primary method for rendering. It iterates all rows
   * and cells using the upstream iterator API and returns parsed
   * CellData objects.
   *
   * NOTE: This allocates new objects. For maximum performance,
   * prefer forEachRow() which reuses the cells iterator.
   */
  readViewport(): CellData[] {
    return this.captureFrame().lines.flat();
  }

  /** Capture all rows and their dirty state in one iterator traversal. */
  captureFrame(): GridFrame {
    this.ensureBuffers();
    this.populateRowIterator();

    const exp = this.bridge.getExports();
    const lines: CellData[][] = [];
    const dirtyRows: boolean[] = [];
    const wrappedRows: boolean[] = [];

    while (exp.ghostty_render_state_row_iterator_next(this.rowIterHandle)) {
      const dirtyResult = exp.ghostty_render_state_row_get(
        this.rowIterHandle,
        RowData.DIRTY,
        this.boolBuf,
      );
      dirtyRows.push(
        dirtyResult === 0 && this.bridge.createView(this.boolBuf, 1).getUint8(0) !== 0,
      );
      wrappedRows.push(this.readCurrentRowWrapped(exp));

      this.populateCellsForCurrentRow();
      const line: CellData[] = [];

      while (exp.ghostty_render_state_row_cells_next(this.cellsHandle)) {
        line.push(this.readCurrentCell());
      }
      lines.push(line);
    }

    return { lines, dirtyRows, wrappedRows };
  }

  /**
   * Iterate all rows, calling the callback for each row with
   * the row index and a CellReader for accessing cells.
   *
   * More efficient than readViewport() because it avoids creating
   * CellData objects — the caller reads only the fields it needs.
   */
  forEachRow(callback: (row: number, reader: CellReader) => void): void {
    this.ensureBuffers();
    this.populateRowIterator();

    const exp = this.bridge.getExports();
    let row = 0;

    while (exp.ghostty_render_state_row_iterator_next(this.rowIterHandle)) {
      this.populateCellsForCurrentRow();

      const reader = new CellReader(this.bridge, this.cellsHandle, this);
      callback(row, reader);
      row++;
    }
  }

  /**
   * Get a single cell at (row, col).
   *
   * NOTE: This is expensive (requires full row iteration to reach
   * the target row). Prefer forEachRow() for bulk access.
   */
  getCell(row: number, col: number): CellData {
    this.ensureBuffers();
    this.populateRowIterator();

    const exp = this.bridge.getExports();
    let currentRow = -1;

    while (exp.ghostty_render_state_row_iterator_next(this.rowIterHandle)) {
      currentRow++;
      if (currentRow === row) {
        this.populateCellsForCurrentRow();

        // Select the column
        exp.ghostty_render_state_row_cells_select(this.cellsHandle, col);
        return this.readCurrentCell();
      }
    }

    return EMPTY_CELL;
  }

  // ==========================================================================
  // Scrollback
  // ==========================================================================

  /** Get number of scrollback lines (not including active screen). */
  getScrollbackLength(): number {
    this.ensureBuffers();
    const exp = this.bridge.getExports();
    const result = exp.ghostty_terminal_get(
      this.termHandle,
      15, // TerminalData.SCROLLBACK_ROWS
      this.u32Buf,
    );
    if (result !== 0) return 0;
    return this.bridge.createView(this.u32Buf, 4).getUint32(0, true);
  }

  /** Read a scrollback line by oldest-first history offset. */
  readScrollbackLine(offset: number): ScrollbackLine | null {
    if (!Number.isInteger(offset) || offset < 0 || offset >= this.getScrollbackLength()) {
      return null;
    }

    this.ensureBuffers();
    this.refreshColors();
    const exp = this.bridge.getExports();
    const cells: CellData[] = [];
    let wrapped = false;

    for (let x = 0; x < this._cols; x++) {
      this.writePoint(PointTag.HISTORY, x, offset);
      const result = exp.ghostty_terminal_grid_ref(this.termHandle, this.pointBuf, this.gridRefBuf);
      if (result !== 0) return null;

      if (x === 0) wrapped = this.readGridRefRowWrapped(exp);
      cells.push(this.readGridRefCell(exp));
    }

    return { cells, wrapped };
  }

  /** Resolve an OSC 8 URI at a viewport or scrollback coordinate. */
  getHyperlinkUri(tag: PointTag, row: number, col: number): string | null {
    if (col < 0 || col >= this._cols || row < 0) return null;
    this.ensureBuffers();
    const exp = this.bridge.getExports();
    this.writePoint(tag, col, row);
    if (exp.ghostty_terminal_grid_ref(this.termHandle, this.pointBuf, this.gridRefBuf) !== 0) {
      return null;
    }

    this.bridge.createView(this.u32Buf, 4).setUint32(0, 0, true);
    const probe = exp.ghostty_grid_ref_hyperlink_uri(this.gridRefBuf, 0, 0, this.u32Buf);
    const length = this.bridge.createView(this.u32Buf, 4).getUint32(0, true);
    if (length === 0) return null;
    if (probe === 0) return null;

    const uriPtr = this.bridge.allocU8Array(length);
    try {
      if (exp.ghostty_grid_ref_hyperlink_uri(this.gridRefBuf, uriPtr, length, this.u32Buf) !== 0) {
        return null;
      }
      return new TextDecoder().decode(this.bridge.readBytes(uriPtr, length));
    } finally {
      this.bridge.freeU8Array(uriPtr, length);
    }
  }

  // ==========================================================================
  // Lifecycle
  // ==========================================================================

  /** Resize grid. Call after terminal resize. */
  resize(cols: number, rows: number): void {
    this._cols = cols;
    this._rows = rows;
  }

  /** Free all allocated WASM buffers. */
  dispose(): void {
    const exp = this.bridge.getExports();
    if (this.cellsHandle !== 0) {
      exp.ghostty_render_state_row_cells_free(this.cellsHandle);
      this.cellsHandle = 0;
    }
    if (this.rowIterHandle !== 0) {
      exp.ghostty_render_state_row_iterator_free(this.rowIterHandle);
      this.rowIterHandle = 0;
    }
    this.freeBuffers();
  }

  // ==========================================================================
  // Private: Buffer Management
  // ==========================================================================

  /** Ensure all pre-allocated buffers exist. */
  private ensureBuffers(): void {
    if (this.buffersAllocated) return;

    const exp = this.bridge.getExports();

    // Create row iterator
    this.rowIterHandle = this.bridge.allocAndReadOpaque(exp.ghostty_render_state_row_iterator_new);

    // Create row cells
    this.cellsHandle = this.bridge.allocAndReadOpaque(exp.ghostty_render_state_row_cells_new);

    // Allocate output buffers
    this.u16Buf = this.bridge.allocU8Array(2);
    this.colorBuf = this.bridge.allocU8Array(COLOR_RGB_SIZE);
    this.graphemeBuf = this.bridge.allocU8Array(GRAPHEME_BUFFER_BYTES);
    this.utf8DataBuf = this.bridge.allocU8Array(UTF8_BUFFER_SIZE);
    this.ghosttyBuf = this.bridge.allocU8Array(12); // GhosttyBuffer
    this.dirtyValBuf = this.bridge.allocU8Array(4);
    this.boolBuf = this.bridge.allocU8();
    this.u32Buf = this.bridge.allocU8Array(4);
    this.rawCellBuf = this.bridge.allocU8Array(CELL_VALUE_SIZE);
    this.rawRowBuf = this.bridge.allocU8Array(CELL_VALUE_SIZE);
    this.styleBuf = this.bridge.allocU8Array(STYLE_SIZE);
    this.handleSlot = this.bridge.allocOpaque();
    this.pointBuf = this.bridge.allocU8Array(POINT_SIZE);
    this.gridRefBuf = this.bridge.allocU8Array(GRID_REF_SIZE);
    this.colorsBuf = this.bridge.allocU8Array(RENDER_STATE_COLORS_SIZE);

    this.buffersAllocated = true;
  }

  /** Free all pre-allocated buffers. */
  private freeBuffers(): void {
    if (!this.buffersAllocated) return;

    this.bridge.freeU8Array(this.u16Buf, 2);
    this.bridge.freeU8Array(this.colorBuf, COLOR_RGB_SIZE);
    this.bridge.freeU8Array(this.graphemeBuf, GRAPHEME_BUFFER_BYTES);
    this.bridge.freeU8Array(this.utf8DataBuf, UTF8_BUFFER_SIZE);
    this.bridge.freeU8Array(this.ghosttyBuf, 12);
    this.bridge.freeU8Array(this.dirtyValBuf, 4);
    this.bridge.freeU8(this.boolBuf);
    this.bridge.freeU8Array(this.u32Buf, 4);
    this.bridge.freeU8Array(this.rawCellBuf, CELL_VALUE_SIZE);
    this.bridge.freeU8Array(this.rawRowBuf, CELL_VALUE_SIZE);
    this.bridge.freeU8Array(this.styleBuf, STYLE_SIZE);
    this.bridge.freeOpaque(this.handleSlot);
    this.bridge.freeU8Array(this.pointBuf, POINT_SIZE);
    this.bridge.freeU8Array(this.gridRefBuf, GRID_REF_SIZE);
    this.bridge.freeU8Array(this.colorsBuf, RENDER_STATE_COLORS_SIZE);

    this.buffersAllocated = false;
  }

  /** Populate row iterator from render state. */
  private populateRowIterator(): void {
    const exp = this.bridge.getExports();
    // Pass pointer to iterator handle for render_state_get to populate
    this.bridge.createView(this.handleSlot, 4).setUint32(0, this.rowIterHandle, true);
    exp.ghostty_render_state_get(this.rsHandle, RenderStateData.ROW_ITERATOR, this.handleSlot);
  }

  /** Populate cells handle for the current row. */
  private populateCellsForCurrentRow(): void {
    const exp = this.bridge.getExports();
    this.bridge.createView(this.handleSlot, 4).setUint32(0, this.cellsHandle, true);
    exp.ghostty_render_state_row_get(this.rowIterHandle, RowData.CELLS, this.handleSlot);
  }

  /** Read the current cell from the cells iterator. */
  private readCurrentCell(): CellData {
    const exp = this.bridge.getExports();

    // Read grapheme length
    const lenResult = exp.ghostty_render_state_row_cells_get(
      this.cellsHandle,
      RowCellsData.GRAPHEMES_LEN,
      this.u32Buf,
    );
    const graphemeLen =
      lenResult === 0 ? this.bridge.createView(this.u32Buf, 4).getUint32(0, true) : 0;

    // Read UTF-8 text using GhosttyBuffer
    let text = " ";
    if (graphemeLen > 0) {
      const bufView = this.bridge.createView(this.ghosttyBuf, 12);
      bufView.setUint32(0, this.utf8DataBuf, true); // ptr
      bufView.setUint32(4, UTF8_BUFFER_SIZE, true); // cap
      bufView.setUint32(8, 0, true); // len
      const utf8Result = exp.ghostty_render_state_row_cells_get(
        this.cellsHandle,
        RowCellsData.GRAPHEMES_UTF8,
        this.ghosttyBuf,
      );
      if (utf8Result === 0) {
        const writtenLen = this.bridge.createView(this.ghosttyBuf + 8, 4).getUint32(0, true);
        if (writtenLen > 0) {
          text = new TextDecoder().decode(
            new Uint8Array(this.bridge.getExports().memory.buffer, this.utf8DataBuf, writtenLen),
          );
        }
      }
    }

    // Read fg color
    const fgResult = exp.ghostty_render_state_row_cells_get(
      this.cellsHandle,
      RowCellsData.FG_COLOR,
      this.colorBuf,
    );
    const fg = fgResult === 0 ? readRgb(this.bridge, this.colorBuf) : null;

    // Read bg color
    const bgResult = exp.ghostty_render_state_row_cells_get(
      this.cellsHandle,
      RowCellsData.BG_COLOR,
      this.colorBuf,
    );
    const bg = bgResult === 0 ? readRgb(this.bridge, this.colorBuf) : null;

    // Read selected
    const selResult = exp.ghostty_render_state_row_cells_get(
      this.cellsHandle,
      RowCellsData.SELECTED,
      this.boolBuf,
    );
    const selected =
      selResult === 0 ? this.bridge.createView(this.boolBuf, 1).getUint8(0) !== 0 : false;

    // Read hasStyling
    const styleResult = exp.ghostty_render_state_row_cells_get(
      this.cellsHandle,
      RowCellsData.HAS_STYLING,
      this.boolBuf,
    );
    const hasStyling =
      styleResult === 0 ? this.bridge.createView(this.boolBuf, 1).getUint8(0) !== 0 : false;

    const { flags, underlineStyle, underlineColor, overline } = this.readCurrentStyle(exp);
    const { width, hyperlinkId, semanticContent } = this.readRawCellMetadata(exp);

    // Grapheme length distinguishes a real space from an empty cell.
    const codepoint = graphemeLen > 0 ? (text.codePointAt(0) ?? 0) : 0;

    return {
      codepoint,
      text,
      fg,
      bg,
      selected,
      hasStyling,
      flags,
      width,
      hyperlinkId,
      graphemeLen,
      semanticContent,
      underlineStyle,
      underlineColor,
      overline,
    };
  }

  private readCurrentRowWrapped(exp: ReturnType<WasmBridge["getExports"]>): boolean {
    if (exp.ghostty_render_state_row_get(this.rowIterHandle, RowData.RAW, this.rawRowBuf) !== 0) {
      return false;
    }
    const raw = this.bridge.createView(this.rawRowBuf, CELL_VALUE_SIZE).getBigUint64(0, true);
    if (exp.ghostty_row_get(raw, RawRowData.WRAP, this.boolBuf) !== 0) return false;
    return this.bridge.createView(this.boolBuf, 1).getUint8(0) !== 0;
  }

  private readGridRefRowWrapped(exp: ReturnType<WasmBridge["getExports"]>): boolean {
    if (exp.ghostty_grid_ref_row(this.gridRefBuf, this.rawRowBuf) !== 0) return false;
    const raw = this.bridge.createView(this.rawRowBuf, CELL_VALUE_SIZE).getBigUint64(0, true);
    if (exp.ghostty_row_get(raw, RawRowData.WRAP, this.boolBuf) !== 0) return false;
    return this.bridge.createView(this.boolBuf, 1).getUint8(0) !== 0;
  }

  private readGridRefCell(exp: ReturnType<WasmBridge["getExports"]>): CellData {
    if (exp.ghostty_grid_ref_cell(this.gridRefBuf, this.rawCellBuf) !== 0) return EMPTY_CELL;
    const raw = this.bridge.createView(this.rawCellBuf, CELL_VALUE_SIZE).getBigUint64(0, true);

    const wideResult = exp.ghostty_cell_get(raw, CellDataField.WIDE, this.u32Buf);
    const wide =
      wideResult === 0
        ? this.bridge.createView(this.u32Buf, 4).getUint32(0, true)
        : CellWide.NARROW;
    const hyperlinkResult = exp.ghostty_cell_get(raw, CellDataField.HAS_HYPERLINK, this.boolBuf);
    const hyperlinkId =
      hyperlinkResult === 0 && this.bridge.createView(this.boolBuf, 1).getUint8(0) !== 0 ? 1 : 0;

    const styleResult = exp.ghostty_grid_ref_style(this.gridRefBuf, this.styleBuf);
    const flags = styleResult === 0 ? this.readFlagsFromStyleBuffer() : 0;
    const fg = styleResult === 0 ? this.readStyleColor(8) : null;
    let bg = styleResult === 0 ? this.readStyleColor(24) : null;
    const underlineStyle = styleResult === 0 ? this.readUnderlineStyle() : UnderlineStyle.NONE;
    const underlineColor = styleResult === 0 ? this.readStyleColor(40) : null;
    const overline =
      styleResult === 0 && this.bridge.createView(this.styleBuf, STYLE_SIZE).getUint8(63) !== 0;
    const contentTag = this.readCellU32(exp, raw, CellDataField.CONTENT_TAG);
    if (contentTag === 2) {
      if (exp.ghostty_cell_get(raw, CellDataField.COLOR_PALETTE, this.boolBuf) === 0) {
        bg = this.readPaletteColor(this.bridge.createView(this.boolBuf, 1).getUint8(0));
      }
    } else if (contentTag === 3) {
      if (exp.ghostty_cell_get(raw, CellDataField.COLOR_RGB, this.colorBuf) === 0) {
        bg = readRgb(this.bridge, this.colorBuf);
      }
    }
    const text = this.readGridRefText(exp);
    const hasStyling = this.readCellBool(exp, raw, CellDataField.HAS_STYLING);
    const semanticContent = this.readSemanticContent(exp, raw);

    return {
      codepoint: text.codePointAt(0) ?? 0,
      text: text || " ",
      fg,
      bg,
      selected: false,
      hasStyling,
      flags,
      width: resolveCellWidth(wide),
      hyperlinkId,
      graphemeLen: text ? [...text].length : 0,
      semanticContent,
      underlineStyle,
      underlineColor,
      overline,
    };
  }

  private readGridRefText(exp: ReturnType<WasmBridge["getExports"]>): string {
    const result = exp.ghostty_grid_ref_graphemes(
      this.gridRefBuf,
      this.graphemeBuf,
      GRAPHEME_BUFFER_CODEPOINTS,
      this.u32Buf,
    );
    const length = this.bridge.createView(this.u32Buf, 4).getUint32(0, true);
    if (length === 0) return "";

    if (result === 0) return this.readCodepoints(this.graphemeBuf, length);
    const ptr = this.bridge.allocU8Array(length * 4);
    try {
      if (exp.ghostty_grid_ref_graphemes(this.gridRefBuf, ptr, length, this.u32Buf) !== 0) {
        return "";
      }
      return this.readCodepoints(ptr, length);
    } finally {
      this.bridge.freeU8Array(ptr, length * 4);
    }
  }

  private readCodepoints(ptr: number, length: number): string {
    const view = this.bridge.createView(ptr, length * 4);
    let result = "";
    for (let index = 0; index < length; index++) {
      result += String.fromCodePoint(view.getUint32(index * 4, true));
    }
    return result;
  }

  private writePoint(tag: PointTag, x: number, y: number): void {
    const view = this.bridge.createView(this.pointBuf, POINT_SIZE);
    for (let index = 0; index < POINT_SIZE; index++) view.setUint8(index, 0);
    view.setUint32(0, tag, true);
    view.setUint16(8, x, true);
    view.setUint32(12, y, true);
  }

  private refreshColors(): void {
    const view = this.bridge.createView(this.colorsBuf, RENDER_STATE_COLORS_SIZE);
    view.setUint32(0, RENDER_STATE_COLORS_SIZE, true);
    this.bridge.getExports().ghostty_render_state_colors_get(this.rsHandle, this.colorsBuf);
  }

  private readStyleColor(offset: number): { r: number; g: number; b: number } | null {
    const style = this.bridge.createView(this.styleBuf, STYLE_SIZE);
    const tag = style.getUint32(offset, true);
    if (tag === 0) return null;
    if (tag === 2) {
      return {
        r: style.getUint8(offset + 8),
        g: style.getUint8(offset + 9),
        b: style.getUint8(offset + 10),
      };
    }
    if (tag === 1) {
      const paletteIndex = style.getUint8(offset + 8);
      return this.readPaletteColor(paletteIndex);
    }
    return null;
  }

  private readPaletteColor(index: number): { r: number; g: number; b: number } {
    const paletteOffset = 14 + index * COLOR_RGB_SIZE;
    const colors = this.bridge.createView(this.colorsBuf, RENDER_STATE_COLORS_SIZE);
    return {
      r: colors.getUint8(paletteOffset),
      g: colors.getUint8(paletteOffset + 1),
      b: colors.getUint8(paletteOffset + 2),
    };
  }

  private readCellU32(
    exp: ReturnType<WasmBridge["getExports"]>,
    raw: bigint,
    field: CellDataField,
  ): number {
    if (exp.ghostty_cell_get(raw, field, this.u32Buf) !== 0) return 0;
    return this.bridge.createView(this.u32Buf, 4).getUint32(0, true);
  }

  private readCellBool(
    exp: ReturnType<WasmBridge["getExports"]>,
    raw: bigint,
    field: CellDataField,
  ): boolean {
    if (exp.ghostty_cell_get(raw, field, this.boolBuf) !== 0) return false;
    return this.bridge.createView(this.boolBuf, 1).getUint8(0) !== 0;
  }

  private readFlagsFromStyleBuffer(): number {
    const view = this.bridge.createView(this.styleBuf, STYLE_SIZE);
    let flags = 0;
    if (view.getUint8(56) !== 0) flags |= CellFlags.BOLD;
    if (view.getUint8(57) !== 0) flags |= CellFlags.ITALIC;
    if (view.getUint8(58) !== 0) flags |= CellFlags.FAINT;
    if (view.getUint8(59) !== 0) flags |= CellFlags.BLINK;
    if (view.getUint8(60) !== 0) flags |= CellFlags.INVERSE;
    if (view.getUint8(61) !== 0) flags |= CellFlags.INVISIBLE;
    if (view.getUint8(62) !== 0) flags |= CellFlags.STRIKETHROUGH;
    if (view.getInt32(64, true) !== 0) flags |= CellFlags.UNDERLINE;
    return flags;
  }

  private readCurrentStyle(exp: ReturnType<WasmBridge["getExports"]>): {
    readonly flags: number;
    readonly underlineStyle: UnderlineStyle;
    readonly underlineColor: { readonly r: number; readonly g: number; readonly b: number } | null;
    readonly overline: boolean;
  } {
    const result = exp.ghostty_render_state_row_cells_get(
      this.cellsHandle,
      RowCellsData.STYLE,
      this.styleBuf,
    );
    if (result !== 0) {
      return {
        flags: 0,
        underlineStyle: UnderlineStyle.NONE,
        underlineColor: null,
        overline: false,
      };
    }
    return {
      flags: this.readFlagsFromStyleBuffer(),
      underlineStyle: this.readUnderlineStyle(),
      underlineColor: this.readStyleColor(40),
      overline: this.bridge.createView(this.styleBuf, STYLE_SIZE).getUint8(63) !== 0,
    };
  }

  private readUnderlineStyle(): UnderlineStyle {
    const value = this.bridge.createView(this.styleBuf, STYLE_SIZE).getInt32(64, true);
    switch (value) {
      case UnderlineStyle.SINGLE:
        return UnderlineStyle.SINGLE;
      case UnderlineStyle.DOUBLE:
        return UnderlineStyle.DOUBLE;
      case UnderlineStyle.CURLY:
        return UnderlineStyle.CURLY;
      case UnderlineStyle.DOTTED:
        return UnderlineStyle.DOTTED;
      case UnderlineStyle.DASHED:
        return UnderlineStyle.DASHED;
      default:
        return UnderlineStyle.NONE;
    }
  }

  private readRawCellMetadata(exp: ReturnType<WasmBridge["getExports"]>): {
    width: number;
    hyperlinkId: number;
    semanticContent: CellSemanticContent;
  } {
    const rawResult = exp.ghostty_render_state_row_cells_get(
      this.cellsHandle,
      RowCellsData.RAW,
      this.rawCellBuf,
    );
    if (rawResult !== 0) {
      return { width: 1, hyperlinkId: 0, semanticContent: CellSemanticContent.OUTPUT };
    }

    const raw = this.bridge.createView(this.rawCellBuf, CELL_VALUE_SIZE).getBigUint64(0, true);
    const wideResult = exp.ghostty_cell_get(raw, CellDataField.WIDE, this.u32Buf);
    const wide =
      wideResult === 0
        ? this.bridge.createView(this.u32Buf, 4).getUint32(0, true)
        : CellWide.NARROW;
    const hyperlinkResult = exp.ghostty_cell_get(raw, CellDataField.HAS_HYPERLINK, this.boolBuf);
    const hyperlinkId =
      hyperlinkResult === 0 && this.bridge.createView(this.boolBuf, 1).getUint8(0) !== 0 ? 1 : 0;
    const semanticContent = this.readSemanticContent(exp, raw);

    const width = resolveCellWidth(wide);
    return { width, hyperlinkId, semanticContent };
  }

  private readSemanticContent(
    exp: ReturnType<WasmBridge["getExports"]>,
    raw: bigint,
  ): CellSemanticContent {
    const value = this.readCellU32(exp, raw, CellDataField.SEMANTIC_CONTENT);
    if (value === CellSemanticContent.INPUT || value === CellSemanticContent.PROMPT) return value;
    return CellSemanticContent.OUTPUT;
  }
}

function resolveCellWidth(wide: number): number {
  if (wide === CellWide.WIDE) return 2;
  if (wide === CellWide.NARROW) return 1;
  return 0;
}

// ============================================================================
// CellReader — efficient per-cell access within a row
// ============================================================================

/**
 * Cell reader for efficient iteration within a row.
 *
 * Created by Grid.forEachRow(). The reader advances through cells
 * and reads only the fields the caller needs.
 */
export class CellReader {
  private readonly bridge: WasmBridge;
  private readonly cellsHandle: number;
  private readonly grid: Grid;

  constructor(bridge: WasmBridge, cellsHandle: number, grid: Grid) {
    this.bridge = bridge;
    this.cellsHandle = cellsHandle;
    this.grid = grid;
  }

  /** Advance to the next cell. Returns false at end of row. */
  next(): boolean {
    return this.bridge.getExports().ghostty_render_state_row_cells_next(this.cellsHandle) !== 0;
  }

  /** Select a specific column (0-based). */
  select(col: number): boolean {
    return (
      this.bridge.getExports().ghostty_render_state_row_cells_select(this.cellsHandle, col) === 0
    );
  }

  /** Read the full CellData for the current cell. */
  read(): CellData {
    return this.grid["readCurrentCell"]();
  }

  /** Read just the text for the current cell. */
  readText(): string {
    const exp = this.bridge.getExports();
    const grid = this.grid;

    // Read grapheme length
    const lenResult = exp.ghostty_render_state_row_cells_get(
      this.cellsHandle,
      RowCellsData.GRAPHEMES_LEN,
      grid["u32Buf"],
    );
    const graphemeLen =
      lenResult === 0 ? this.bridge.createView(grid["u32Buf"], 4).getUint32(0, true) : 0;

    if (graphemeLen === 0) return " ";

    // Read UTF-8 text
    const bufView = this.bridge.createView(grid["ghosttyBuf"], 12);
    bufView.setUint32(0, grid["utf8DataBuf"], true);
    bufView.setUint32(4, UTF8_BUFFER_SIZE, true);
    bufView.setUint32(8, 0, true);
    const utf8Result = exp.ghostty_render_state_row_cells_get(
      this.cellsHandle,
      RowCellsData.GRAPHEMES_UTF8,
      grid["ghosttyBuf"],
    );
    if (utf8Result === 0) {
      const writtenLen = this.bridge.createView(grid["ghosttyBuf"] + 8, 4).getUint32(0, true);
      if (writtenLen > 0) {
        return new TextDecoder().decode(
          new Uint8Array(exp.memory.buffer, grid["utf8DataBuf"], writtenLen),
        );
      }
    }
    return " ";
  }
}

// ============================================================================
// Helpers
// ============================================================================

/** Read an RGB color from a 3-byte WASM buffer. */
function readRgb(bridge: WasmBridge, ptr: number): { r: number; g: number; b: number } {
  const view = bridge.createView(ptr, COLOR_RGB_SIZE);
  return {
    r: view.getUint8(0),
    g: view.getUint8(1),
    b: view.getUint8(2),
  };
}

/** Sentinel empty cell. */
const EMPTY_CELL: CellData = {
  codepoint: 0,
  text: " ",
  fg: null,
  bg: null,
  selected: false,
  hasStyling: false,
  flags: 0,
  width: 1,
  hyperlinkId: 0,
  graphemeLen: 0,
  semanticContent: CellSemanticContent.OUTPUT,
  underlineStyle: UnderlineStyle.NONE,
  underlineColor: null,
  overline: false,
};
