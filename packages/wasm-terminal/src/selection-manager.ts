/**
 * Selection manager for terminal text selection.
 *
 * Manages mouse-based text selection (drag, double-click word, triple-click line),
 * auto-scroll during drag, clipboard copy, and programmatic selection API.
 * Selection coordinates are stored in absolute buffer space so they persist
 * correctly across scrolling.
 */

import type { SelectionCoordinates, IBufferRange } from "./types.js";
import type { IEvent } from "./event-emitter.js";
import { EventEmitter } from "./event-emitter.js";
import type {
  ClipboardPayload,
  ClipboardWriteSource,
  TerminalClipboardHost,
} from "./host/terminal-host.js";
import type { GhosttySelectionAdjustment } from "./selection/ghostty-selection.js";

/** Interface the SelectionManager needs from the terminal. */
export interface ISelectionTerminal {
  getCellDimensions(): { width: number; height: number };
  getDimensions(): { cols: number; rows: number };
  getCanvasOffset(): { left: number; top: number };
  getScrollbackLength(): number;
  getViewportY(): number;
  getLine(y: number): SelectionCell[] | null;
  getScrollbackLine(offset: number): SelectionCell[] | null;
  getGraphemeString?(row: number, col: number): string;
  getScrollbackGraphemeString?(offset: number, col: number): string;
  isRowWrapped?(row: number): boolean;
  isScrollbackRowWrapped?(offset: number): boolean;
  scrollLines(amount: number): void;
  shouldSelect?(event: MouseEvent): boolean;
  requestRender(): void;
}

/** Cell data needed for selection text extraction. */
export interface SelectionCell {
  codepoint: number;
  width: number;
  grapheme_len: number;
}

export interface SelectionManagerOptions {
  readonly clipboard?: TerminalClipboardHost;
  readonly copyOnSelect?: boolean;
  readonly onCopyError?: (error: unknown) => void;
  readonly selectionModel?: SelectionModel;
}

/** Terminal-owned selection implementation, normally backed by Ghostty. */
export interface SelectionModel {
  hasSelection(): boolean;
  getSelection(): string;
  getRange(): {
    readonly start: { readonly x: number; readonly y: number };
    readonly end: { readonly x: number; readonly y: number };
    readonly rectangle?: boolean;
  } | null;
  clearSelection(): boolean;
  setRange(
    start: { readonly x: number; readonly y: number },
    end: { readonly x: number; readonly y: number },
    rectangle?: boolean,
  ): boolean;
  selectWordAt(column: number, row: number): boolean;
  selectLineAt(column: number, row: number): boolean;
  selectAll(): boolean;
  selectOutputAt?(column: number, row: number): boolean;
  adjustSelection?(adjustment: GhosttySelectionAdjustment): boolean;
  getClipboardPayload?(): ClipboardPayload;
  pressGesture?(input: {
    readonly column: number;
    readonly row: number;
    readonly surfaceX: number;
    readonly surfaceY: number;
    readonly timeNs: bigint;
    readonly repeatDistance: number;
    readonly repeatIntervalNs: bigint;
  }): boolean;
  dragGesture?(input: {
    readonly column: number;
    readonly row: number;
    readonly surfaceX: number;
    readonly surfaceY: number;
    readonly rectangle: boolean;
    readonly columns: number;
    readonly cellWidth: number;
    readonly paddingLeft: number;
    readonly screenHeight: number;
  }): boolean;
  releaseGesture?(column: number, row: number): boolean;
  resetGesture?(): void;
}

export type SelectionCopyResult =
  | { readonly kind: "written" }
  | { readonly kind: "empty" }
  | { readonly kind: "unsupported" }
  | { readonly kind: "failed"; readonly error: unknown };

export class SelectionManager {
  private readonly terminal: ISelectionTerminal;
  private readonly canvas: HTMLCanvasElement;
  private readonly options: SelectionManagerOptions;
  private copyOnSelect: boolean;

  // Selection state in absolute buffer coordinates
  private selectionStart: { col: number; absoluteRow: number } | null = null;
  private selectionEnd: { col: number; absoluteRow: number } | null = null;
  private isSelecting = false;
  private dragThresholdMet = false;

  // Mouse state
  private mouseDownX = 0;
  private mouseDownY = 0;
  private lastDragEvent: MouseEvent | undefined;

  // Dirty row tracking
  private dirtySelectionRows = new Set<number>();

  // Auto-scroll
  private autoScrollInterval: ReturnType<typeof setInterval> | undefined;
  private autoScrollDirection: "up" | "down" | null = null;

  // Event
  private readonly onSelectionChangeEmitter = new EventEmitter<void>();

  // Bound handlers for cleanup
  private boundHandlers: {
    mousedown: (e: MouseEvent) => void;
    mousemove: (e: MouseEvent) => void;
    mouseup: (e: MouseEvent) => void;
    click: (e: MouseEvent) => void;
    contextmenu: (e: MouseEvent) => void;
    docMousemove: (e: MouseEvent) => void;
    docMouseup: (e: MouseEvent) => void;
    docClick: (e: MouseEvent) => void;
  };

  constructor(
    terminal: ISelectionTerminal,
    canvas: HTMLCanvasElement,
    options: SelectionManagerOptions = {},
  ) {
    this.terminal = terminal;
    this.canvas = canvas;
    this.options = options;
    this.copyOnSelect = options.copyOnSelect ?? true;

    this.boundHandlers = {
      mousedown: this.handleMouseDown.bind(this),
      mousemove: this.handleMouseMove.bind(this),
      mouseup: this.handleMouseUp.bind(this),
      click: this.handleClick.bind(this),
      contextmenu: this.handleContextMenu.bind(this),
      docMousemove: this.handleDocMouseMove.bind(this),
      docMouseup: this.handleDocMouseUp.bind(this),
      docClick: this.handleDocClick.bind(this),
    };

    this.attachEventListeners();
  }

  // ==========================================================================
  // Public API
  // ==========================================================================

  get onSelectionChange(): IEvent<void> {
    return this.onSelectionChangeEmitter.event;
  }

  hasSelection(): boolean {
    if (this.options.selectionModel) return this.options.selectionModel.hasSelection();
    if (!this.selectionStart || !this.selectionEnd) return false;
    return this.dragThresholdMet;
  }

  getSelection(): string {
    if (this.options.selectionModel) return this.options.selectionModel.getSelection();
    if (!this.selectionStart || !this.selectionEnd || !this.dragThresholdMet) return "";

    const start = this.normalizeAbsoluteStart();
    const end = this.normalizeAbsoluteEnd();
    if (!start || !end) return "";

    let result = "";
    for (let row = start.absoluteRow; row <= end.absoluteRow; row++) {
      const line = this.getLineForAbsoluteRow(row);
      if (!line) continue;

      let lineText = "";
      const startCol = row === start.absoluteRow ? start.col : 0;
      const endCol = row === end.absoluteRow ? end.col : line.length - 1;

      for (let x = startCol; x <= endCol; x++) {
        const cell = line[x];
        if (!cell || cell.width === 0) continue;
        lineText += this.getCellText(row, x, cell);
      }

      result += lineText.trimEnd();
      if (row < end.absoluteRow && !this.isAbsoluteRowWrapped(row)) result += "\n";
    }

    return result;
  }

  getSelectionCoords(): SelectionCoordinates | null {
    const trackedRange = this.options.selectionModel?.getRange();
    if (this.options.selectionModel && !trackedRange) return null;
    if (trackedRange) {
      const viewportY = this.terminal.getViewportY();
      const scrollbackLength = this.terminal.getScrollbackLength();
      return {
        startCol: trackedRange.start.x,
        startRow: this.absoluteRowToViewport(trackedRange.start.y, scrollbackLength, viewportY),
        endCol: trackedRange.end.x,
        endRow: this.absoluteRowToViewport(trackedRange.end.y, scrollbackLength, viewportY),
        rectangle: trackedRange.rectangle ?? false,
      };
    }
    if (!this.selectionStart || !this.selectionEnd || !this.dragThresholdMet) return null;

    const start = this.normalizeAbsoluteStart();
    const end = this.normalizeAbsoluteEnd();
    if (!start || !end) return null;

    const viewportY = this.terminal.getViewportY();
    const scrollbackLength = this.terminal.getScrollbackLength();

    const startViewportRow = this.absoluteRowToViewport(
      start.absoluteRow,
      scrollbackLength,
      viewportY,
    );
    const endViewportRow = this.absoluteRowToViewport(end.absoluteRow, scrollbackLength, viewportY);

    if (startViewportRow < 0 || endViewportRow < 0) return null;

    return {
      startCol: start.col,
      startRow: startViewportRow,
      endCol: end.col,
      endRow: endViewportRow,
      rectangle: false,
    };
  }

  getSelectionPosition(): IBufferRange | null {
    const coords = this.getSelectionCoords();
    if (!coords) return null;
    return {
      start: { x: coords.startCol, y: coords.startRow },
      end: { x: coords.endCol, y: coords.endRow },
    };
  }

  clearSelection(): void {
    const hadSelection =
      this.hasSelection() || this.selectionStart !== null || this.selectionEnd !== null;
    if (!hadSelection) return;
    this.markCurrentRowsDirty();
    this.options.selectionModel?.clearSelection();
    this.selectionStart = null;
    this.selectionEnd = null;
    this.dragThresholdMet = false;
    this.onSelectionChangeEmitter.fire();
    this.terminal.requestRender();
  }

  selectAll(): void {
    const selectionModel = this.options.selectionModel;
    if (selectionModel) {
      this.markCurrentRowsDirty();
      if (!selectionModel.selectAll()) return;
      this.syncLocalRangeFromModel();
      this.finishSelectionChange();
      return;
    }
    const scrollbackLength = this.terminal.getScrollbackLength();
    const { cols, rows } = this.terminal.getDimensions();

    this.markCurrentRowsDirty();
    this.selectionStart = { col: 0, absoluteRow: 0 };
    this.selectionEnd = { col: cols - 1, absoluteRow: scrollbackLength + rows - 1 };
    this.dragThresholdMet = true;
    this.onSelectionChangeEmitter.fire();
    this.terminal.requestRender();
  }

  selectOutputAt(column: number, row: number): boolean {
    const selectionModel = this.options.selectionModel;
    if (!selectionModel?.selectOutputAt) return false;
    const absoluteRow = this.viewportRowToAbsolute(
      row,
      this.terminal.getScrollbackLength(),
      this.terminal.getViewportY(),
    );
    this.markCurrentRowsDirty();
    if (!selectionModel.selectOutputAt(column, absoluteRow)) return false;
    this.syncLocalRangeFromModel();
    this.finishSelectionChange();
    this.copySelectionIfConfigured();
    return true;
  }

  adjustSelection(adjustment: GhosttySelectionAdjustment): boolean {
    const selectionModel = this.options.selectionModel;
    if (!selectionModel?.adjustSelection) return false;
    this.markCurrentRowsDirty();
    if (!selectionModel.adjustSelection(adjustment)) return false;
    this.syncLocalRangeFromModel();
    this.finishSelectionChange();
    this.copySelectionIfConfigured();
    return true;
  }

  select(column: number, row: number, length: number): void {
    this.markCurrentRowsDirty();
    const scrollbackLength = this.terminal.getScrollbackLength();
    const viewportY = this.terminal.getViewportY();
    const absoluteRow = this.viewportRowToAbsolute(row, scrollbackLength, viewportY);

    const selectionModel = this.options.selectionModel;
    if (
      selectionModel &&
      !selectionModel.setRange(
        { x: column, y: absoluteRow },
        { x: column + length - 1, y: absoluteRow },
      )
    ) {
      return;
    }

    this.selectionStart = { col: column, absoluteRow };
    this.selectionEnd = { col: column + length - 1, absoluteRow };
    this.dragThresholdMet = true;
    this.markCurrentRowsDirty();
    this.onSelectionChangeEmitter.fire();
    this.terminal.requestRender();
  }

  selectAbsoluteRange(
    start: { readonly x: number; readonly y: number },
    end: { readonly x: number; readonly y: number },
  ): boolean {
    const selectionModel = this.options.selectionModel;
    if (!selectionModel) return false;
    this.markCurrentRowsDirty();
    if (!selectionModel.setRange(start, end)) return false;
    this.syncLocalRangeFromModel();
    this.finishSelectionChange();
    return true;
  }

  selectLines(start: number, end: number): void {
    this.markCurrentRowsDirty();
    const scrollbackLength = this.terminal.getScrollbackLength();
    const viewportY = this.terminal.getViewportY();

    const startAbsolute = this.viewportRowToAbsolute(start, scrollbackLength, viewportY);
    const endAbsolute = this.viewportRowToAbsolute(end, scrollbackLength, viewportY);
    const { cols } = this.terminal.getDimensions();

    const selectionModel = this.options.selectionModel;
    if (
      selectionModel &&
      !selectionModel.setRange({ x: 0, y: startAbsolute }, { x: cols - 1, y: endAbsolute })
    ) {
      return;
    }

    this.selectionStart = { col: 0, absoluteRow: startAbsolute };
    this.selectionEnd = { col: cols - 1, absoluteRow: endAbsolute };
    this.dragThresholdMet = true;
    this.markCurrentRowsDirty();
    this.onSelectionChangeEmitter.fire();
    this.terminal.requestRender();
  }

  copySelection(): boolean {
    const text = this.getSelection();
    if (!text || !this.options.clipboard) return false;
    void this.copySelectionAsync();
    return true;
  }

  setCopyOnSelect(enabled: boolean): void {
    this.copyOnSelect = enabled;
  }

  async copySelectionAsync(source: ClipboardWriteSource = "copy"): Promise<SelectionCopyResult> {
    const payload = this.options.selectionModel?.getClipboardPayload?.() ?? {
      text: this.getSelection(),
    };
    if (!payload.text) return { kind: "empty" };
    const clipboard = this.options.clipboard;
    if (!clipboard) return { kind: "unsupported" };
    try {
      await clipboard.write({
        target: "standard",
        source,
        payload,
      });
      return { kind: "written" };
    } catch (error) {
      this.options.onCopyError?.(error);
      return { kind: "failed", error };
    }
  }

  getDirtySelectionRows(): Set<number> {
    return this.dirtySelectionRows;
  }

  clearDirtySelectionRows(): void {
    this.dirtySelectionRows = new Set();
  }

  // ==========================================================================
  // Event Handlers
  // ==========================================================================

  private attachEventListeners(): void {
    this.canvas.addEventListener("mousedown", this.boundHandlers.mousedown);
    this.canvas.addEventListener("mousemove", this.boundHandlers.mousemove);
    this.canvas.addEventListener("click", this.boundHandlers.click);
    this.canvas.addEventListener("contextmenu", this.boundHandlers.contextmenu);
    document.addEventListener("mousemove", this.boundHandlers.docMousemove);
    document.addEventListener("mouseup", this.boundHandlers.docMouseup);
    document.addEventListener("click", this.boundHandlers.docClick);
  }

  private handleMouseDown(e: MouseEvent): void {
    if (e.button !== 0) return;
    if (this.terminal.shouldSelect && !this.terminal.shouldSelect(e)) return;

    const { col, row } = this.eventToCell(e);
    const scrollbackLength = this.terminal.getScrollbackLength();
    const viewportY = this.terminal.getViewportY();
    const absoluteRow = this.viewportRowToAbsolute(row, scrollbackLength, viewportY);

    this.isSelecting = true;
    this.dragThresholdMet = false;
    this.mouseDownX = e.clientX;
    this.mouseDownY = e.clientY;

    const selectionModel = this.options.selectionModel;
    if (selectionModel?.pressGesture) {
      const rect = this.canvas.getBoundingClientRect();
      const dimensions = this.terminal.getCellDimensions();
      selectionModel.pressGesture({
        column: col,
        row: absoluteRow,
        surfaceX: e.clientX - rect.left,
        surfaceY: e.clientY - rect.top,
        timeNs: BigInt(Math.max(0, Math.round(e.timeStamp * 1_000_000))),
        repeatDistance: dimensions.width,
        repeatIntervalNs: 500_000_000n,
      });
      this.syncLocalRangeFromModel();
      if (!this.selectionStart) this.selectionStart = { col, absoluteRow };
      if (!this.selectionEnd) this.selectionEnd = { col, absoluteRow };
      this.onSelectionChangeEmitter.fire();
      this.terminal.requestRender();
      return;
    }

    const hadSelection = this.hasSelection();
    this.markCurrentRowsDirty();
    this.options.selectionModel?.clearSelection();
    this.selectionStart = { col, absoluteRow };
    this.selectionEnd = { col, absoluteRow };
    if (hadSelection) {
      this.onSelectionChangeEmitter.fire();
      this.terminal.requestRender();
    }
  }

  private handleMouseMove(e: MouseEvent): void {
    if (!this.isSelecting) return;
    this.lastDragEvent = e;

    const selectionModel = this.options.selectionModel;
    if (selectionModel?.dragGesture) {
      const { col, row } = this.eventToCell(e);
      const absoluteRow = this.viewportRowToAbsolute(
        row,
        this.terminal.getScrollbackLength(),
        this.terminal.getViewportY(),
      );
      const rect = this.canvas.getBoundingClientRect();
      const cell = this.terminal.getCellDimensions();
      selectionModel.dragGesture({
        column: col,
        row: absoluteRow,
        surfaceX: e.clientX - rect.left,
        surfaceY: e.clientY - rect.top,
        rectangle: e.altKey,
        columns: this.terminal.getDimensions().cols,
        cellWidth: Math.max(1, Math.round(cell.width)),
        paddingLeft: 0,
        screenHeight: Math.max(1, Math.round(rect.height)),
      });
      this.syncLocalRangeFromModel();
      this.markCurrentRowsDirty();
      this.onSelectionChangeEmitter.fire();
      this.terminal.requestRender();
      this.checkAutoScroll(e);
      return;
    }

    // Check drag threshold (50% of cell width)
    const dims = this.terminal.getCellDimensions();
    const threshold = dims.width * 0.5;
    const dx = Math.abs(e.clientX - this.mouseDownX);
    const dy = Math.abs(e.clientY - this.mouseDownY);

    if (!this.dragThresholdMet && (dx > threshold || dy > threshold)) {
      this.dragThresholdMet = true;
    }

    if (!this.dragThresholdMet) return;

    const { col, row } = this.eventToCell(e);
    const scrollbackLength = this.terminal.getScrollbackLength();
    const viewportY = this.terminal.getViewportY();
    const absoluteRow = this.viewportRowToAbsolute(row, scrollbackLength, viewportY);
    const selectionStart = this.selectionStart;
    if (!selectionStart) return;

    this.markCurrentRowsDirty();
    this.selectionEnd = { col, absoluteRow };
    this.options.selectionModel?.setRange(
      { x: selectionStart.col, y: selectionStart.absoluteRow },
      { x: col, y: absoluteRow },
      e.altKey,
    );
    this.markCurrentRowsDirty();
    this.onSelectionChangeEmitter.fire();
    this.terminal.requestRender();

    // Auto-scroll near edges
    this.checkAutoScroll(e);
  }

  private handleMouseUp(e: MouseEvent): void {
    if (!this.isSelecting) return;
    const { col, row } = this.eventToCell(e);
    const absoluteRow = this.viewportRowToAbsolute(
      row,
      this.terminal.getScrollbackLength(),
      this.terminal.getViewportY(),
    );
    this.options.selectionModel?.releaseGesture?.(col, absoluteRow);
    this.isSelecting = false;
    this.lastDragEvent = undefined;
    this.stopAutoScroll();

    if (this.hasSelection()) {
      this.copySelectionIfConfigured();
    }
  }

  private handleClick(e: MouseEvent): void {
    if (this.options.selectionModel?.pressGesture) return;
    if (e.detail === 2) {
      // Double-click: word selection
      const { col, row } = this.eventToCell(e);
      this.selectWordAt(col, row);
    } else if (e.detail >= 3) {
      // Triple-click: line selection
      const { col, row } = this.eventToCell(e);
      this.selectLineAt(col, row);
    }
  }

  private handleContextMenu(_e: MouseEvent): void {
    // The host application owns context-menu rendering and actions.
  }

  private handleDocMouseMove(e: MouseEvent): void {
    if (!this.isSelecting) return;
    this.handleMouseMove(e);
  }

  private handleDocMouseUp(e: MouseEvent): void {
    this.handleMouseUp(e);
  }

  private handleDocClick(e: MouseEvent): void {
    // Deselect if click is outside canvas
    const target = e.target;
    const clickedOutside = !(target instanceof Node) || !this.canvas.contains(target);
    if (clickedOutside && this.hasSelection()) {
      this.clearSelection();
    }
  }

  // ==========================================================================
  // Coordinate Conversion
  // ==========================================================================

  private eventToCell(e: MouseEvent): { col: number; row: number } {
    const rect = this.canvas.getBoundingClientRect();
    const dims = this.terminal.getCellDimensions();
    const x = e.clientX - rect.left;
    const y = e.clientY - rect.top;
    const terminalDimensions = this.terminal.getDimensions();
    return {
      col: Math.max(0, Math.min(terminalDimensions.cols - 1, Math.floor(x / dims.width))),
      row: Math.max(0, Math.min(terminalDimensions.rows - 1, Math.floor(y / dims.height))),
    };
  }

  private viewportRowToAbsolute(
    viewportRow: number,
    scrollbackLength: number,
    viewportY: number,
  ): number {
    return scrollbackLength + viewportRow - Math.floor(viewportY);
  }

  private absoluteRowToViewport(
    absoluteRow: number,
    scrollbackLength: number,
    viewportY: number,
  ): number {
    return absoluteRow - scrollbackLength + Math.floor(viewportY);
  }

  // ==========================================================================
  // Selection Helpers
  // ==========================================================================

  private normalizeAbsoluteStart(): { col: number; absoluteRow: number } | null {
    if (!this.selectionStart || !this.selectionEnd) return null;
    if (
      this.selectionStart.absoluteRow < this.selectionEnd.absoluteRow ||
      (this.selectionStart.absoluteRow === this.selectionEnd.absoluteRow &&
        this.selectionStart.col <= this.selectionEnd.col)
    ) {
      return this.selectionStart;
    }
    return this.selectionEnd;
  }

  private normalizeAbsoluteEnd(): { col: number; absoluteRow: number } | null {
    if (!this.selectionStart || !this.selectionEnd) return null;
    if (
      this.selectionStart.absoluteRow < this.selectionEnd.absoluteRow ||
      (this.selectionStart.absoluteRow === this.selectionEnd.absoluteRow &&
        this.selectionStart.col <= this.selectionEnd.col)
    ) {
      return this.selectionEnd;
    }
    return this.selectionStart;
  }

  private getLineForAbsoluteRow(absoluteRow: number): SelectionCell[] | null {
    const scrollbackLength = this.terminal.getScrollbackLength();

    if (absoluteRow < scrollbackLength) {
      return this.terminal.getScrollbackLine(absoluteRow);
    }
    const screenRow = absoluteRow - scrollbackLength;
    return this.terminal.getLine(screenRow);
  }

  private getCellText(absoluteRow: number, col: number, cell: SelectionCell): string {
    const scrollbackLength = this.terminal.getScrollbackLength();
    const text =
      absoluteRow < scrollbackLength
        ? this.terminal.getScrollbackGraphemeString?.(absoluteRow, col)
        : this.terminal.getGraphemeString?.(absoluteRow - scrollbackLength, col);
    if (text && text !== " ") return text;
    return String.fromCodePoint(cell.codepoint || 32);
  }

  private isAbsoluteRowWrapped(absoluteRow: number): boolean {
    const scrollbackLength = this.terminal.getScrollbackLength();
    if (absoluteRow < scrollbackLength) {
      return this.terminal.isScrollbackRowWrapped?.(absoluteRow) ?? false;
    }
    const screenRow = absoluteRow - scrollbackLength;
    return this.terminal.isRowWrapped?.(screenRow) ?? false;
  }

  private selectWordAt(col: number, row: number): void {
    const selectionModel = this.options.selectionModel;
    if (selectionModel) {
      const absoluteRow = this.viewportRowToAbsolute(
        row,
        this.terminal.getScrollbackLength(),
        this.terminal.getViewportY(),
      );
      this.markCurrentRowsDirty();
      if (!selectionModel.selectWordAt(col, absoluteRow)) return;
      this.syncLocalRangeFromModel();
      this.finishSelectionChange();
      this.copySelectionIfConfigured();
      return;
    }
    const line = this.getLineForAbsoluteRow(
      this.viewportRowToAbsolute(
        row,
        this.terminal.getScrollbackLength(),
        this.terminal.getViewportY(),
      ),
    );
    if (!line) return;

    const wordChars = /[\w\-./~@+]/;
    let startCol = col;
    let endCol = col;

    while (
      startCol > 0 &&
      wordChars.test(String.fromCodePoint(line[startCol - 1]?.codepoint || 0))
    ) {
      startCol--;
    }
    while (
      endCol < line.length - 1 &&
      wordChars.test(String.fromCodePoint(line[endCol + 1]?.codepoint || 0))
    ) {
      endCol++;
    }

    this.markCurrentRowsDirty();
    const scrollbackLength = this.terminal.getScrollbackLength();
    const viewportY = this.terminal.getViewportY();
    const absoluteRow = this.viewportRowToAbsolute(row, scrollbackLength, viewportY);

    this.selectionStart = { col: startCol, absoluteRow };
    this.selectionEnd = { col: endCol, absoluteRow };
    this.dragThresholdMet = true;
    this.markCurrentRowsDirty();
    this.onSelectionChangeEmitter.fire();
    this.terminal.requestRender();
    this.copySelectionIfConfigured();
  }

  private selectLineAt(col: number, row: number): void {
    const selectionModel = this.options.selectionModel;
    if (!selectionModel) {
      this.selectLines(row, row);
      return;
    }
    const absoluteRow = this.viewportRowToAbsolute(
      row,
      this.terminal.getScrollbackLength(),
      this.terminal.getViewportY(),
    );
    this.markCurrentRowsDirty();
    if (!selectionModel.selectLineAt(col, absoluteRow)) return;
    this.syncLocalRangeFromModel();
    this.finishSelectionChange();
    this.copySelectionIfConfigured();
  }

  private copySelectionIfConfigured(): void {
    if (!this.copyOnSelect) return;
    void this.copySelectionAsync("copy-on-select");
  }

  private syncLocalRangeFromModel(): void {
    const range = this.options.selectionModel?.getRange();
    if (!range) {
      this.selectionStart = null;
      this.selectionEnd = null;
      this.dragThresholdMet = false;
      return;
    }
    this.selectionStart = { col: range.start.x, absoluteRow: range.start.y };
    this.selectionEnd = { col: range.end.x, absoluteRow: range.end.y };
    this.dragThresholdMet = true;
  }

  private finishSelectionChange(): void {
    this.markCurrentRowsDirty();
    this.onSelectionChangeEmitter.fire();
    this.terminal.requestRender();
  }

  private markCurrentRowsDirty(): void {
    const coords = this.getSelectionCoords();
    if (coords) {
      for (let row = coords.startRow; row <= coords.endRow; row++) {
        this.dirtySelectionRows.add(row);
      }
    }
  }

  // ==========================================================================
  // Auto-Scroll
  // ==========================================================================

  private checkAutoScroll(e: MouseEvent): void {
    const rect = this.canvas.getBoundingClientRect();
    const edgeZone = 30;

    if (e.clientY < rect.top + edgeZone) {
      this.startAutoScroll("up");
    } else if (e.clientY > rect.bottom - edgeZone) {
      this.startAutoScroll("down");
    } else {
      this.stopAutoScroll();
    }
  }

  private startAutoScroll(direction: "up" | "down"): void {
    if (this.autoScrollDirection === direction) return;
    this.stopAutoScroll();
    this.autoScrollDirection = direction;
    this.autoScrollInterval = setInterval(() => {
      this.terminal.scrollLines(direction === "up" ? -1 : 1);
      if (this.lastDragEvent) this.handleMouseMove(this.lastDragEvent);
      else this.terminal.requestRender();
    }, 50);
  }

  private stopAutoScroll(): void {
    if (this.autoScrollInterval !== undefined) {
      clearInterval(this.autoScrollInterval);
      this.autoScrollInterval = undefined;
    }
    this.autoScrollDirection = null;
  }

  // ==========================================================================
  // Lifecycle
  // ==========================================================================

  dispose(): void {
    this.stopAutoScroll();
    this.canvas.removeEventListener("mousedown", this.boundHandlers.mousedown);
    this.canvas.removeEventListener("mousemove", this.boundHandlers.mousemove);
    this.canvas.removeEventListener("click", this.boundHandlers.click);
    this.canvas.removeEventListener("contextmenu", this.boundHandlers.contextmenu);
    document.removeEventListener("mousemove", this.boundHandlers.docMousemove);
    document.removeEventListener("mouseup", this.boundHandlers.docMouseup);
    document.removeEventListener("click", this.boundHandlers.docClick);
    this.onSelectionChangeEmitter.dispose();
  }
}
