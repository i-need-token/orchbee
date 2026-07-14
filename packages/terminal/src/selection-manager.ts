/**
 * DOM and clipboard adapter for Ghostty-owned terminal selection.
 *
 * Ghostty owns selection ranges, click behavior, drag thresholds, word and line
 * boundaries, rectangular selection, and autoscroll. This class only translates
 * browser events and mirrors Ghostty viewport movement into the web renderer.
 */

import type { IEvent } from "./event-emitter";
import { EventEmitter } from "./event-emitter";
import type {
  ClipboardPayload,
  ClipboardWriteSource,
  TerminalClipboardHost,
} from "./host/terminal-host";
import type {
  GhosttySelectionAdjustment,
  GhosttySelectionAutoscroll,
} from "./selection/ghostty-selection";
import type { IBufferRange, SelectionCoordinates } from "./types";

/** Interface the SelectionManager needs from the terminal surface. */
export interface ISelectionTerminal {
  getCellDimensions(): { width: number; height: number };
  getDimensions(): { cols: number; rows: number };
  getCanvasOffset(): { left: number; top: number };
  getScrollbackLength(): number;
  getViewportY(): number;
  getLine?(row: number): SelectionCell[] | null;
  getScrollbackLine?(offset: number): SelectionCell[] | null;
  getGraphemeString?(row: number, column: number): string;
  getScrollbackGraphemeString?(offset: number, column: number): string;
  isRowWrapped?(row: number): boolean;
  isScrollbackRowWrapped?(offset: number): boolean;
  scrollLines(amount: number): void;
  shouldSelect?(event: MouseEvent): boolean;
  requestRender(): void;
}

/** Retained for API compatibility; text extraction is owned by Ghostty. */
export interface SelectionCell {
  codepoint: number;
  width: number;
  grapheme_len: number;
}

export interface SelectionManagerOptions {
  readonly clipboard?: TerminalClipboardHost;
  readonly copyOnSelect?: boolean;
  readonly onCopyError?: (error: unknown) => void;
  readonly selectionModel: SelectionModel;
}

/** Terminal-owned selection implementation backed by Ghostty. */
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
  pressGesture(input: {
    readonly column: number;
    readonly row: number;
    readonly surfaceX: number;
    readonly surfaceY: number;
    readonly timeNs: bigint;
    readonly repeatDistance: number;
    readonly repeatIntervalNs: bigint;
    readonly selectOutput: boolean;
  }): boolean;
  dragGesture(input: {
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
  releaseGesture(column: number, row: number): boolean;
  getAutoscroll(): GhosttySelectionAutoscroll;
  autoscrollTick(input: {
    readonly viewportColumn: number;
    readonly viewportRow: number;
    readonly surfaceX: number;
    readonly surfaceY: number;
    readonly rectangle: boolean;
    readonly columns: number;
    readonly cellWidth: number;
    readonly paddingLeft: number;
    readonly screenHeight: number;
  }): boolean;
  resetGesture(): void;
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
  private readonly model: SelectionModel;
  private copyOnSelect: boolean;
  private isSelecting = false;
  private lastDragEvent: MouseEvent | undefined;
  private dirtySelectionRows = new Set<number>();
  private autoScrollInterval: ReturnType<typeof setInterval> | undefined;
  private autoScrollDirection: GhosttySelectionAutoscroll = null;
  private readonly onSelectionChangeEmitter = new EventEmitter<void>();
  private readonly boundHandlers: {
    mousedown: (event: MouseEvent) => void;
    mousemove: (event: MouseEvent) => void;
    contextmenu: (event: MouseEvent) => void;
    docMouseup: (event: MouseEvent) => void;
    docClick: (event: MouseEvent) => void;
  };

  constructor(
    terminal: ISelectionTerminal,
    canvas: HTMLCanvasElement,
    options: SelectionManagerOptions,
  ) {
    this.terminal = terminal;
    this.canvas = canvas;
    this.options = options;
    this.model = options.selectionModel;
    this.copyOnSelect = options.copyOnSelect ?? true;
    this.boundHandlers = {
      mousedown: this.handleMouseDown.bind(this),
      mousemove: this.handleMouseMove.bind(this),
      contextmenu: this.handleContextMenu.bind(this),
      docMouseup: this.handleDocMouseUp.bind(this),
      docClick: this.handleDocClick.bind(this),
    };
    this.attachEventListeners();
  }

  get onSelectionChange(): IEvent<void> {
    return this.onSelectionChangeEmitter.event;
  }

  hasSelection(): boolean {
    return this.model.hasSelection();
  }

  getSelection(): string {
    return this.model.getSelection();
  }

  getSelectionCoords(): SelectionCoordinates | null {
    const range = this.model.getRange();
    if (!range) return null;
    const viewportY = this.terminal.getViewportY();
    const scrollbackLength = this.terminal.getScrollbackLength();
    return {
      startCol: range.start.x,
      startRow: this.absoluteRowToViewport(range.start.y, scrollbackLength, viewportY),
      endCol: range.end.x,
      endRow: this.absoluteRowToViewport(range.end.y, scrollbackLength, viewportY),
      rectangle: range.rectangle ?? false,
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
    if (!this.model.hasSelection()) return;
    this.markCurrentRowsDirty();
    if (!this.model.clearSelection()) return;
    this.finishSelectionChange();
  }

  selectAll(): void {
    this.markCurrentRowsDirty();
    if (this.model.selectAll()) this.finishSelectionChange();
  }

  selectOutputAt(column: number, row: number): boolean {
    if (!this.model.selectOutputAt) return false;
    const absoluteRow = this.viewportRowToAbsolute(
      row,
      this.terminal.getScrollbackLength(),
      this.terminal.getViewportY(),
    );
    this.markCurrentRowsDirty();
    if (!this.model.selectOutputAt(column, absoluteRow)) return false;
    this.finishSelectionChange();
    this.copySelectionIfConfigured();
    return true;
  }

  adjustSelection(adjustment: GhosttySelectionAdjustment): boolean {
    if (!this.model.adjustSelection) return false;
    this.markCurrentRowsDirty();
    if (!this.model.adjustSelection(adjustment)) return false;
    this.finishSelectionChange();
    this.copySelectionIfConfigured();
    return true;
  }

  select(column: number, row: number, length: number): void {
    const absoluteRow = this.viewportRowToAbsolute(
      row,
      this.terminal.getScrollbackLength(),
      this.terminal.getViewportY(),
    );
    this.markCurrentRowsDirty();
    if (
      this.model.setRange({ x: column, y: absoluteRow }, { x: column + length - 1, y: absoluteRow })
    ) {
      this.finishSelectionChange();
    }
  }

  selectAbsoluteRange(
    start: { readonly x: number; readonly y: number },
    end: { readonly x: number; readonly y: number },
  ): boolean {
    this.markCurrentRowsDirty();
    if (!this.model.setRange(start, end)) return false;
    this.finishSelectionChange();
    return true;
  }

  selectLines(start: number, end: number): void {
    const scrollbackLength = this.terminal.getScrollbackLength();
    const viewportY = this.terminal.getViewportY();
    const startAbsolute = this.viewportRowToAbsolute(start, scrollbackLength, viewportY);
    const endAbsolute = this.viewportRowToAbsolute(end, scrollbackLength, viewportY);
    const { cols } = this.terminal.getDimensions();
    this.markCurrentRowsDirty();
    if (this.model.setRange({ x: 0, y: startAbsolute }, { x: cols - 1, y: endAbsolute })) {
      this.finishSelectionChange();
    }
  }

  copySelection(): boolean {
    if (!this.getSelection() || !this.options.clipboard) return false;
    void this.copySelectionAsync();
    return true;
  }

  setCopyOnSelect(enabled: boolean): void {
    this.copyOnSelect = enabled;
  }

  async copySelectionAsync(source: ClipboardWriteSource = "copy"): Promise<SelectionCopyResult> {
    const payload = this.model.getClipboardPayload?.() ?? { text: this.getSelection() };
    if (!payload.text) return { kind: "empty" };
    const clipboard = this.options.clipboard;
    if (!clipboard) return { kind: "unsupported" };
    try {
      await clipboard.write({ target: "standard", source, payload });
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

  private attachEventListeners(): void {
    this.canvas.addEventListener("mousedown", this.boundHandlers.mousedown);
    this.canvas.addEventListener("mousemove", this.boundHandlers.mousemove);
    this.canvas.addEventListener("contextmenu", this.boundHandlers.contextmenu);
    document.addEventListener("mouseup", this.boundHandlers.docMouseup);
    document.addEventListener("click", this.boundHandlers.docClick);
  }

  private handleMouseDown(event: MouseEvent): void {
    if (event.button !== 0) return;
    if (this.terminal.shouldSelect && !this.terminal.shouldSelect(event)) return;
    const { col, row } = this.eventToCell(event);
    const absoluteRow = this.viewportRowToAbsolute(
      row,
      this.terminal.getScrollbackLength(),
      this.terminal.getViewportY(),
    );
    const rect = this.canvas.getBoundingClientRect();
    const dimensions = this.terminal.getCellDimensions();
    this.isSelecting = true;
    this.lastDragEvent = undefined;
    this.stopAutoScroll();
    this.markCurrentRowsDirty();
    if (
      this.model.pressGesture({
        column: col,
        row: absoluteRow,
        surfaceX: event.clientX - rect.left,
        surfaceY: event.clientY - rect.top,
        timeNs: BigInt(Math.max(0, Math.round(event.timeStamp * 1_000_000))),
        repeatDistance: dimensions.width,
        repeatIntervalNs: 500_000_000n,
        selectOutput: event.ctrlKey || event.metaKey,
      })
    ) {
      this.finishSelectionChange();
    }
  }

  private handleMouseMove(event: MouseEvent): void {
    if (!this.isSelecting) return;
    this.lastDragEvent = event;
    const { col, row } = this.eventToCell(event);
    const absoluteRow = this.viewportRowToAbsolute(
      row,
      this.terminal.getScrollbackLength(),
      this.terminal.getViewportY(),
    );
    const rect = this.canvas.getBoundingClientRect();
    const cell = this.terminal.getCellDimensions();
    this.markCurrentRowsDirty();
    if (
      this.model.dragGesture({
        column: col,
        row: absoluteRow,
        surfaceX: event.clientX - rect.left,
        surfaceY: event.clientY - rect.top,
        rectangle: event.altKey,
        columns: this.terminal.getDimensions().cols,
        cellWidth: Math.max(1, Math.round(cell.width)),
        paddingLeft: 0,
        screenHeight: Math.max(1, Math.round(rect.height)),
      })
    ) {
      this.finishSelectionChange();
    }
    this.refreshAutoScroll();
  }

  private handleMouseUp(event: MouseEvent): void {
    if (!this.isSelecting) return;
    const { col, row } = this.eventToCell(event);
    const absoluteRow = this.viewportRowToAbsolute(
      row,
      this.terminal.getScrollbackLength(),
      this.terminal.getViewportY(),
    );
    this.model.releaseGesture(col, absoluteRow);
    this.isSelecting = false;
    this.lastDragEvent = undefined;
    this.stopAutoScroll();
    if (this.hasSelection()) this.copySelectionIfConfigured();
  }

  private handleContextMenu(_event: MouseEvent): void {
    // The host application owns context-menu rendering and actions.
  }

  private handleDocMouseUp(event: MouseEvent): void {
    this.handleMouseUp(event);
  }

  private handleDocClick(event: MouseEvent): void {
    const target = event.target;
    const clickedOutside = !(target instanceof Node) || !this.canvas.contains(target);
    if (clickedOutside && this.hasSelection()) this.clearSelection();
  }

  private eventToCell(event: MouseEvent): { col: number; row: number } {
    const rect = this.canvas.getBoundingClientRect();
    const cell = this.terminal.getCellDimensions();
    const dimensions = this.terminal.getDimensions();
    return {
      col: Math.max(
        0,
        Math.min(dimensions.cols - 1, Math.floor((event.clientX - rect.left) / cell.width)),
      ),
      row: Math.max(
        0,
        Math.min(dimensions.rows - 1, Math.floor((event.clientY - rect.top) / cell.height)),
      ),
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

  private copySelectionIfConfigured(): void {
    if (this.copyOnSelect) void this.copySelectionAsync("copy-on-select");
  }

  private finishSelectionChange(): void {
    this.markCurrentRowsDirty();
    this.onSelectionChangeEmitter.fire();
    this.terminal.requestRender();
  }

  private markCurrentRowsDirty(): void {
    const coordinates = this.getSelectionCoords();
    if (!coordinates) return;
    const start = Math.min(coordinates.startRow, coordinates.endRow);
    const end = Math.max(coordinates.startRow, coordinates.endRow);
    for (let row = start; row <= end; row++) this.dirtySelectionRows.add(row);
  }

  private refreshAutoScroll(): void {
    const direction = this.model.getAutoscroll();
    if (!direction) {
      this.stopAutoScroll();
      return;
    }
    this.startAutoScroll(direction);
  }

  private startAutoScroll(direction: Exclude<GhosttySelectionAutoscroll, null>): void {
    if (this.autoScrollDirection === direction) return;
    this.stopAutoScroll();
    this.autoScrollDirection = direction;
    this.autoScrollInterval = setInterval(() => this.runAutoScrollTick(), 16);
  }

  private runAutoScrollTick(): void {
    const event = this.lastDragEvent;
    const direction = this.model.getAutoscroll();
    if (!event || !direction) {
      this.stopAutoScroll();
      return;
    }
    const { col, row } = this.eventToCell(event);
    const rect = this.canvas.getBoundingClientRect();
    const cell = this.terminal.getCellDimensions();
    this.markCurrentRowsDirty();
    const changed = this.model.autoscrollTick({
      viewportColumn: col,
      viewportRow: row,
      surfaceX: event.clientX - rect.left,
      surfaceY: event.clientY - rect.top,
      rectangle: event.altKey,
      columns: this.terminal.getDimensions().cols,
      cellWidth: Math.max(1, Math.round(cell.width)),
      paddingLeft: 0,
      screenHeight: Math.max(1, Math.round(rect.height)),
    });
    if (changed) {
      this.terminal.scrollLines(direction === "up" ? -1 : 1);
      this.finishSelectionChange();
    }
    this.refreshAutoScroll();
  }

  private stopAutoScroll(): void {
    if (this.autoScrollInterval !== undefined) {
      clearInterval(this.autoScrollInterval);
      this.autoScrollInterval = undefined;
    }
    this.autoScrollDirection = null;
  }

  dispose(): void {
    this.stopAutoScroll();
    this.model.resetGesture();
    this.canvas.removeEventListener("mousedown", this.boundHandlers.mousedown);
    this.canvas.removeEventListener("mousemove", this.boundHandlers.mousemove);
    this.canvas.removeEventListener("contextmenu", this.boundHandlers.contextmenu);
    document.removeEventListener("mouseup", this.boundHandlers.docMouseup);
    document.removeEventListener("click", this.boundHandlers.docClick);
    this.onSelectionChangeEmitter.dispose();
  }
}
