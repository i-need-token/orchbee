import type { IBufferRange, TerminalHandle } from "../types";
import { PointTag, Result, TerminalData, TerminalOption } from "../types";
import type { WasmBridge } from "../wasm-bridge";
import type { ClipboardPayload } from "../host/terminal-host";

const POINT_SIZE = 24;
const POINT_COORDINATE_SIZE = 8;
const GRID_REF_SIZE = 12;
const SELECTION_SIZE = 32;
const SELECT_WORD_OPTIONS_SIZE = 24;
const SELECT_LINE_OPTIONS_SIZE = 28;
const FORMAT_OPTIONS_SIZE = 16;
const SELECTION_START_OFFSET = 4;
const SELECTION_END_OFFSET = SELECTION_START_OFFSET + GRID_REF_SIZE;
const SELECTION_RECTANGLE_OFFSET = SELECTION_END_OFFSET + GRID_REF_SIZE;
const GESTURE_VALUE_SIZE = 16;

const GestureEventType = {
  PRESS: 0,
  RELEASE: 1,
  DRAG: 2,
  AUTOSCROLL_TICK: 3,
} as const;

const GestureData = {
  AUTOSCROLL: 2,
} as const;

const GestureEventOption = {
  REFERENCE: 0,
  POSITION: 1,
  REPEAT_DISTANCE: 2,
  TIME_NS: 3,
  REPEAT_INTERVAL_NS: 4,
  BEHAVIORS: 6,
  RECTANGLE: 7,
  GEOMETRY: 8,
  VIEWPORT: 9,
} as const;

/** Ghostty-owned selection state exposed in stable screen coordinates. */
export interface GhosttySelectionRange {
  readonly start: { readonly x: number; readonly y: number };
  readonly end: { readonly x: number; readonly y: number };
  readonly rectangle: boolean;
}

export interface GhosttySelectionGesturePoint {
  readonly column: number;
  readonly row: number;
  readonly surfaceX: number;
  readonly surfaceY: number;
}

export interface GhosttySelectionGesturePress extends GhosttySelectionGesturePoint {
  readonly timeNs: bigint;
  readonly repeatDistance: number;
  readonly repeatIntervalNs: bigint;
  readonly selectOutput?: boolean;
}

export interface GhosttySelectionGestureDrag extends GhosttySelectionGesturePoint {
  readonly rectangle: boolean;
  readonly columns: number;
  readonly cellWidth: number;
  readonly paddingLeft: number;
  readonly screenHeight: number;
}

export type GhosttySelectionAutoscroll = "up" | "down" | null;

export interface GhosttySelectionGestureAutoscrollTick {
  readonly viewportColumn: number;
  readonly viewportRow: number;
  readonly surfaceX: number;
  readonly surfaceY: number;
  readonly rectangle: boolean;
  readonly columns: number;
  readonly cellWidth: number;
  readonly paddingLeft: number;
  readonly screenHeight: number;
}

/** Keyboard-style adjustment applied to the active selection endpoint. */
export type GhosttySelectionAdjustment =
  | "left"
  | "right"
  | "up"
  | "down"
  | "home"
  | "end"
  | "pageUp"
  | "pageDown"
  | "beginningOfLine"
  | "endOfLine";

const SelectionAdjustmentValue: Readonly<Record<GhosttySelectionAdjustment, number>> = {
  left: 0,
  right: 1,
  up: 2,
  down: 3,
  home: 4,
  end: 5,
  pageUp: 6,
  pageDown: 7,
  beginningOfLine: 8,
  endOfLine: 9,
};

/**
 * Adapts Ghostty's snapshot selection ABI to terminal-owned tracked selection.
 *
 * Snapshot grid references never escape this class. Every derived snapshot is
 * installed immediately, after which Ghostty owns tracked endpoint references.
 */
export class GhosttySelectionController {
  private readonly bridge: WasmBridge;
  private readonly terminal: TerminalHandle;
  private readonly pointBuffer: number;
  private readonly referenceBuffer: number;
  private readonly selectionBuffer: number;
  private readonly orderedSelectionBuffer: number;
  private readonly optionsBuffer: number;
  private readonly outputLengthBuffer: number;
  private readonly coordinateBuffer: number;
  private readonly gestureValueBuffer: number;
  private gesture = 0;
  private pressEvent = 0;
  private releaseEvent = 0;
  private dragEvent = 0;
  private autoscrollEvent = 0;
  private disposed = false;

  constructor(bridge: WasmBridge, terminal: TerminalHandle) {
    this.bridge = bridge;
    this.terminal = terminal;
    let pointBuffer = 0;
    let referenceBuffer = 0;
    let selectionBuffer = 0;
    let orderedSelectionBuffer = 0;
    let optionsBuffer = 0;
    let outputLengthBuffer = 0;
    let coordinateBuffer = 0;
    let gestureValueBuffer = 0;
    try {
      pointBuffer = requireSelectionAllocation(bridge.allocU8Array(POINT_SIZE), "point buffer");
      referenceBuffer = requireSelectionAllocation(
        bridge.allocU8Array(GRID_REF_SIZE),
        "grid reference buffer",
      );
      selectionBuffer = requireSelectionAllocation(
        bridge.allocU8Array(SELECTION_SIZE),
        "selection buffer",
      );
      orderedSelectionBuffer = requireSelectionAllocation(
        bridge.allocU8Array(SELECTION_SIZE),
        "ordered selection buffer",
      );
      optionsBuffer = requireSelectionAllocation(
        bridge.allocU8Array(SELECT_LINE_OPTIONS_SIZE),
        "selection options buffer",
      );
      outputLengthBuffer = requireSelectionAllocation(
        bridge.allocUsize(),
        "selection output length slot",
      );
      coordinateBuffer = requireSelectionAllocation(
        bridge.allocU8Array(POINT_COORDINATE_SIZE),
        "selection coordinate buffer",
      );
      gestureValueBuffer = requireSelectionAllocation(
        bridge.allocU8Array(GESTURE_VALUE_SIZE),
        "selection gesture value buffer",
      );
    } catch (error) {
      if (gestureValueBuffer !== 0) bridge.freeU8Array(gestureValueBuffer, GESTURE_VALUE_SIZE);
      if (coordinateBuffer !== 0) bridge.freeU8Array(coordinateBuffer, POINT_COORDINATE_SIZE);
      if (outputLengthBuffer !== 0) bridge.freeUsize(outputLengthBuffer);
      if (optionsBuffer !== 0) bridge.freeU8Array(optionsBuffer, SELECT_LINE_OPTIONS_SIZE);
      if (orderedSelectionBuffer !== 0) {
        bridge.freeU8Array(orderedSelectionBuffer, SELECTION_SIZE);
      }
      if (selectionBuffer !== 0) bridge.freeU8Array(selectionBuffer, SELECTION_SIZE);
      if (referenceBuffer !== 0) bridge.freeU8Array(referenceBuffer, GRID_REF_SIZE);
      if (pointBuffer !== 0) bridge.freeU8Array(pointBuffer, POINT_SIZE);
      throw error;
    }
    this.pointBuffer = pointBuffer;
    this.referenceBuffer = referenceBuffer;
    this.selectionBuffer = selectionBuffer;
    this.orderedSelectionBuffer = orderedSelectionBuffer;
    this.optionsBuffer = optionsBuffer;
    this.outputLengthBuffer = outputLengthBuffer;
    this.coordinateBuffer = coordinateBuffer;
    this.gestureValueBuffer = gestureValueBuffer;
    try {
      this.gesture = this.createGesture();
      this.pressEvent = this.createGestureEvent(GestureEventType.PRESS);
      this.releaseEvent = this.createGestureEvent(GestureEventType.RELEASE);
      this.dragEvent = this.createGestureEvent(GestureEventType.DRAG);
      this.autoscrollEvent = this.createGestureEvent(GestureEventType.AUTOSCROLL_TICK);
    } catch (error) {
      this.dispose();
      throw error;
    }
  }

  hasSelection(): boolean {
    return this.readActiveSelection(this.selectionBuffer);
  }

  clearSelection(): boolean {
    const hadSelection = this.hasSelection();
    this.resetGesture();
    if (!hadSelection) return false;
    const result = this.bridge
      .getExports()
      .ghostty_terminal_set(this.terminal, TerminalOption.SELECTION, 0);
    return result === Result.SUCCESS;
  }

  setRange(start: IBufferRange["start"], end: IBufferRange["end"], rectangle = false): boolean {
    this.initializeSelection(this.selectionBuffer);
    if (!this.resolvePoint(start.x, start.y, this.selectionBuffer + SELECTION_START_OFFSET)) {
      return false;
    }
    if (!this.resolvePoint(end.x, end.y, this.selectionBuffer + SELECTION_END_OFFSET)) {
      return false;
    }
    this.bridge
      .createView(this.selectionBuffer + SELECTION_RECTANGLE_OFFSET, 1)
      .setUint8(0, rectangle ? 1 : 0);
    return this.installSelection(this.selectionBuffer);
  }

  selectWordAt(column: number, row: number): boolean {
    if (!this.resolvePoint(column, row, this.referenceBuffer)) return false;
    this.clearBuffer(this.optionsBuffer, SELECT_WORD_OPTIONS_SIZE);
    const options = this.bridge.createView(this.optionsBuffer, SELECT_WORD_OPTIONS_SIZE);
    options.setUint32(0, SELECT_WORD_OPTIONS_SIZE, true);
    this.copyBytes(this.referenceBuffer, this.optionsBuffer + 4, GRID_REF_SIZE);
    this.initializeSelection(this.selectionBuffer);
    const result = this.bridge
      .getExports()
      .ghostty_terminal_select_word(this.terminal, this.optionsBuffer, this.selectionBuffer);
    return result === Result.SUCCESS && this.installSelection(this.selectionBuffer);
  }

  selectLineAt(column: number, row: number): boolean {
    if (!this.resolvePoint(column, row, this.referenceBuffer)) return false;
    this.clearBuffer(this.optionsBuffer, SELECT_LINE_OPTIONS_SIZE);
    const options = this.bridge.createView(this.optionsBuffer, SELECT_LINE_OPTIONS_SIZE);
    options.setUint32(0, SELECT_LINE_OPTIONS_SIZE, true);
    this.copyBytes(this.referenceBuffer, this.optionsBuffer + 4, GRID_REF_SIZE);
    options.setUint8(24, 1);
    this.initializeSelection(this.selectionBuffer);
    const result = this.bridge
      .getExports()
      .ghostty_terminal_select_line(this.terminal, this.optionsBuffer, this.selectionBuffer);
    return result === Result.SUCCESS && this.installSelection(this.selectionBuffer);
  }

  selectAll(): boolean {
    this.initializeSelection(this.selectionBuffer);
    const result = this.bridge
      .getExports()
      .ghostty_terminal_select_all(this.terminal, this.selectionBuffer);
    return result === Result.SUCCESS && this.installSelection(this.selectionBuffer);
  }

  selectOutputAt(column: number, row: number): boolean {
    if (!this.resolvePoint(column, row, this.referenceBuffer)) return false;
    this.initializeSelection(this.selectionBuffer);
    const result = this.bridge
      .getExports()
      .ghostty_terminal_select_output(this.terminal, this.referenceBuffer, this.selectionBuffer);
    return result === Result.SUCCESS && this.installSelection(this.selectionBuffer);
  }

  adjustSelection(adjustment: GhosttySelectionAdjustment): boolean {
    if (!this.readActiveSelection(this.selectionBuffer)) return false;
    const result = this.bridge
      .getExports()
      .ghostty_terminal_selection_adjust(
        this.terminal,
        this.selectionBuffer,
        SelectionAdjustmentValue[adjustment],
      );
    return result === Result.SUCCESS && this.installSelection(this.selectionBuffer);
  }

  pressGesture(input: GhosttySelectionGesturePress): boolean {
    if (!this.resolvePoint(input.column, input.row, this.referenceBuffer)) return false;
    if (
      !this.setGestureOption(this.pressEvent, GestureEventOption.REFERENCE, this.referenceBuffer)
    ) {
      return false;
    }
    this.writeSurfacePosition(input.surfaceX, input.surfaceY);
    if (!this.setGestureOption(this.pressEvent, GestureEventOption.POSITION)) return false;
    this.bridge
      .createView(this.gestureValueBuffer, GESTURE_VALUE_SIZE)
      .setFloat64(0, input.repeatDistance, true);
    if (!this.setGestureOption(this.pressEvent, GestureEventOption.REPEAT_DISTANCE)) return false;
    this.bridge
      .createView(this.gestureValueBuffer, GESTURE_VALUE_SIZE)
      .setBigUint64(0, input.timeNs, true);
    if (!this.setGestureOption(this.pressEvent, GestureEventOption.TIME_NS)) return false;
    this.bridge
      .createView(this.gestureValueBuffer, GESTURE_VALUE_SIZE)
      .setBigUint64(0, input.repeatIntervalNs, true);
    if (!this.setGestureOption(this.pressEvent, GestureEventOption.REPEAT_INTERVAL_NS))
      return false;
    this.clearBuffer(this.gestureValueBuffer, GESTURE_VALUE_SIZE);
    const behaviors = this.bridge.createView(this.gestureValueBuffer, GESTURE_VALUE_SIZE);
    behaviors.setUint32(0, 0, true);
    behaviors.setUint32(4, 1, true);
    behaviors.setUint32(8, input.selectOutput ? 3 : 2, true);
    if (!this.setGestureOption(this.pressEvent, GestureEventOption.BEHAVIORS)) return false;
    return this.applyGestureEvent(this.pressEvent, true);
  }

  dragGesture(input: GhosttySelectionGestureDrag): boolean {
    if (!this.resolvePoint(input.column, input.row, this.referenceBuffer)) return false;
    if (
      !this.setGestureOption(this.dragEvent, GestureEventOption.REFERENCE, this.referenceBuffer)
    ) {
      return false;
    }
    this.writeSurfacePosition(input.surfaceX, input.surfaceY);
    if (!this.setGestureOption(this.dragEvent, GestureEventOption.POSITION)) return false;
    this.clearBuffer(this.gestureValueBuffer, GESTURE_VALUE_SIZE);
    this.bridge
      .createView(this.gestureValueBuffer, GESTURE_VALUE_SIZE)
      .setUint8(0, input.rectangle ? 1 : 0);
    if (!this.setGestureOption(this.dragEvent, GestureEventOption.RECTANGLE)) return false;
    const geometry = this.bridge.createView(this.gestureValueBuffer, GESTURE_VALUE_SIZE);
    geometry.setUint32(0, input.columns, true);
    geometry.setUint32(4, input.cellWidth, true);
    geometry.setUint32(8, input.paddingLeft, true);
    geometry.setUint32(12, input.screenHeight, true);
    if (!this.setGestureOption(this.dragEvent, GestureEventOption.GEOMETRY)) return false;
    return this.applyGestureEvent(this.dragEvent, true);
  }

  releaseGesture(column: number, row: number): boolean {
    const hasReference = this.resolvePoint(column, row, this.referenceBuffer);
    if (
      !this.setGestureOption(
        this.releaseEvent,
        GestureEventOption.REFERENCE,
        hasReference ? this.referenceBuffer : 0,
      )
    ) {
      return false;
    }
    const result = this.bridge
      .getExports()
      .ghostty_selection_gesture_event(this.gesture, this.terminal, this.releaseEvent, 0);
    return result === Result.NO_VALUE || result === Result.SUCCESS;
  }

  getAutoscroll(): GhosttySelectionAutoscroll {
    this.clearBuffer(this.gestureValueBuffer, GESTURE_VALUE_SIZE);
    const result = this.bridge
      .getExports()
      .ghostty_selection_gesture_get(
        this.gesture,
        this.terminal,
        GestureData.AUTOSCROLL,
        this.gestureValueBuffer,
      );
    if (result !== Result.SUCCESS) return null;
    const value = this.bridge.createView(this.gestureValueBuffer, 4).getUint32(0, true);
    if (value === 1) return "up";
    if (value === 2) return "down";
    return null;
  }

  autoscrollTick(input: GhosttySelectionGestureAutoscrollTick): boolean {
    this.clearBuffer(this.coordinateBuffer, POINT_COORDINATE_SIZE);
    const viewport = this.bridge.createView(this.coordinateBuffer, POINT_COORDINATE_SIZE);
    viewport.setUint16(0, input.viewportColumn, true);
    viewport.setUint32(4, input.viewportRow, true);
    if (
      !this.setGestureOption(
        this.autoscrollEvent,
        GestureEventOption.VIEWPORT,
        this.coordinateBuffer,
      )
    ) {
      return false;
    }
    this.writeSurfacePosition(input.surfaceX, input.surfaceY);
    if (!this.setGestureOption(this.autoscrollEvent, GestureEventOption.POSITION)) return false;
    this.clearBuffer(this.gestureValueBuffer, GESTURE_VALUE_SIZE);
    this.bridge
      .createView(this.gestureValueBuffer, GESTURE_VALUE_SIZE)
      .setUint8(0, input.rectangle ? 1 : 0);
    if (!this.setGestureOption(this.autoscrollEvent, GestureEventOption.RECTANGLE)) return false;
    const geometry = this.bridge.createView(this.gestureValueBuffer, GESTURE_VALUE_SIZE);
    geometry.setUint32(0, input.columns, true);
    geometry.setUint32(4, input.cellWidth, true);
    geometry.setUint32(8, input.paddingLeft, true);
    geometry.setUint32(12, input.screenHeight, true);
    if (!this.setGestureOption(this.autoscrollEvent, GestureEventOption.GEOMETRY)) return false;
    return this.applyGestureEvent(this.autoscrollEvent, false);
  }

  resetGesture(): void {
    this.bridge.getExports().ghostty_selection_gesture_reset(this.gesture, this.terminal);
  }

  getSelection(): string {
    return this.formatSelection(0);
  }

  getClipboardPayload(): ClipboardPayload {
    const text = this.formatSelection(0);
    if (!text) return { text: "" };
    const vt = this.formatSelection(1);
    const html = this.formatSelection(2);
    return { text, ...(html ? { html } : {}), ...(vt ? { vt } : {}) };
  }

  private formatSelection(format: 0 | 1 | 2): string {
    if (!this.hasSelection()) return "";
    this.clearBuffer(this.optionsBuffer, FORMAT_OPTIONS_SIZE);
    const options = this.bridge.createView(this.optionsBuffer, FORMAT_OPTIONS_SIZE);
    options.setUint32(0, FORMAT_OPTIONS_SIZE, true);
    options.setUint32(4, format, true);
    options.setUint8(8, 1);
    options.setUint8(9, 1);
    options.setUint32(12, 0, true);

    const exports = this.bridge.getExports();
    this.bridge.createView(this.outputLengthBuffer, 4).setUint32(0, 0, true);
    const probe = exports.ghostty_terminal_selection_format_buf(
      this.terminal,
      this.optionsBuffer,
      0,
      0,
      this.outputLengthBuffer,
    );
    const length = this.bridge.createView(this.outputLengthBuffer, 4).getUint32(0, true);
    if (length === 0) return "";
    if (probe !== Result.OUT_OF_SPACE) return "";

    const output = requireSelectionAllocation(
      this.bridge.allocU8Array(length),
      "formatted selection buffer",
    );
    try {
      const result = exports.ghostty_terminal_selection_format_buf(
        this.terminal,
        this.optionsBuffer,
        output,
        length,
        this.outputLengthBuffer,
      );
      if (result !== Result.SUCCESS) return "";
      const written = this.bridge.createView(this.outputLengthBuffer, 4).getUint32(0, true);
      return new TextDecoder().decode(this.bridge.readBytes(output, written));
    } finally {
      this.bridge.freeU8Array(output, length);
    }
  }

  getRange(): GhosttySelectionRange | null {
    if (!this.readActiveSelection(this.selectionBuffer)) return null;
    this.initializeSelection(this.orderedSelectionBuffer);
    const result = this.bridge
      .getExports()
      .ghostty_terminal_selection_ordered(
        this.terminal,
        this.selectionBuffer,
        0,
        this.orderedSelectionBuffer,
      );
    if (result !== Result.SUCCESS) return null;

    const start = this.readScreenPoint(this.orderedSelectionBuffer + SELECTION_START_OFFSET);
    const end = this.readScreenPoint(this.orderedSelectionBuffer + SELECTION_END_OFFSET);
    const rectangle =
      this.bridge
        .createView(this.orderedSelectionBuffer + SELECTION_RECTANGLE_OFFSET, 1)
        .getUint8(0) !== 0;
    return start && end ? { start, end, rectangle } : null;
  }

  dispose(): void {
    if (this.disposed) return;
    const exports = this.bridge.getExports();
    exports.ghostty_selection_gesture_event_free(this.pressEvent);
    exports.ghostty_selection_gesture_event_free(this.releaseEvent);
    exports.ghostty_selection_gesture_event_free(this.dragEvent);
    exports.ghostty_selection_gesture_event_free(this.autoscrollEvent);
    exports.ghostty_selection_gesture_free(this.gesture, this.terminal);
    this.pressEvent = 0;
    this.releaseEvent = 0;
    this.dragEvent = 0;
    this.autoscrollEvent = 0;
    this.gesture = 0;
    this.bridge.freeU8Array(this.pointBuffer, POINT_SIZE);
    this.bridge.freeU8Array(this.referenceBuffer, GRID_REF_SIZE);
    this.bridge.freeU8Array(this.selectionBuffer, SELECTION_SIZE);
    this.bridge.freeU8Array(this.orderedSelectionBuffer, SELECTION_SIZE);
    this.bridge.freeU8Array(this.optionsBuffer, SELECT_LINE_OPTIONS_SIZE);
    this.bridge.freeUsize(this.outputLengthBuffer);
    this.bridge.freeU8Array(this.coordinateBuffer, POINT_COORDINATE_SIZE);
    this.bridge.freeU8Array(this.gestureValueBuffer, GESTURE_VALUE_SIZE);
    this.disposed = true;
  }

  private resolvePoint(column: number, row: number, outputReference: number): boolean {
    if (!Number.isInteger(column) || !Number.isInteger(row) || column < 0 || row < 0) return false;
    this.writePoint(column, row);
    this.clearBuffer(outputReference, GRID_REF_SIZE);
    this.bridge.createView(outputReference, GRID_REF_SIZE).setUint32(0, GRID_REF_SIZE, true);
    const result = this.bridge
      .getExports()
      .ghostty_terminal_grid_ref(this.terminal, this.pointBuffer, outputReference);
    return result === Result.SUCCESS;
  }

  private writePoint(column: number, row: number): void {
    this.clearBuffer(this.pointBuffer, POINT_SIZE);
    const point = this.bridge.createView(this.pointBuffer, POINT_SIZE);
    point.setUint32(0, PointTag.SCREEN, true);
    point.setUint16(8, column, true);
    point.setUint32(12, row, true);
  }

  private readActiveSelection(output: number): boolean {
    this.initializeSelection(output);
    const result = this.bridge
      .getExports()
      .ghostty_terminal_get(this.terminal, TerminalData.SELECTION, output);
    return result === Result.SUCCESS;
  }

  private installSelection(selection: number): boolean {
    return (
      this.bridge
        .getExports()
        .ghostty_terminal_set(this.terminal, TerminalOption.SELECTION, selection) === Result.SUCCESS
    );
  }

  private createGesture(): number {
    return this.bridge.allocAndReadOpaque((allocator, output) =>
      this.bridge.getExports().ghostty_selection_gesture_new(allocator, output),
    );
  }

  private createGestureEvent(type: number): number {
    const output = requireSelectionAllocation(
      this.bridge.allocOpaque(),
      "selection gesture event output slot",
    );
    try {
      const result = this.bridge.getExports().ghostty_selection_gesture_event_new(0, output, type);
      if (result !== Result.SUCCESS) {
        throw new Error(`Failed to create Ghostty selection gesture event: ${result}`);
      }
      return requireSelectionAllocation(
        this.bridge.createView(output, 4).getUint32(0, true),
        "selection gesture event",
      );
    } finally {
      this.bridge.freeOpaque(output);
    }
  }

  private setGestureOption(
    event: number,
    option: number,
    value = this.gestureValueBuffer,
  ): boolean {
    return (
      this.bridge.getExports().ghostty_selection_gesture_event_set(event, option, value) ===
      Result.SUCCESS
    );
  }

  private applyGestureEvent(event: number, clearWhenEmpty: boolean): boolean {
    this.initializeSelection(this.selectionBuffer);
    const result = this.bridge
      .getExports()
      .ghostty_selection_gesture_event(this.gesture, this.terminal, event, this.selectionBuffer);
    if (result === Result.SUCCESS) return this.installSelection(this.selectionBuffer);
    if (result === Result.NO_VALUE && clearWhenEmpty) {
      this.bridge.getExports().ghostty_terminal_set(this.terminal, TerminalOption.SELECTION, 0);
      return true;
    }
    return false;
  }

  private writeSurfacePosition(x: number, y: number): void {
    this.clearBuffer(this.gestureValueBuffer, GESTURE_VALUE_SIZE);
    const position = this.bridge.createView(this.gestureValueBuffer, GESTURE_VALUE_SIZE);
    position.setFloat64(0, x, true);
    position.setFloat64(8, y, true);
  }

  private readScreenPoint(reference: number): { readonly x: number; readonly y: number } | null {
    this.clearBuffer(this.coordinateBuffer, POINT_COORDINATE_SIZE);
    const result = this.bridge
      .getExports()
      .ghostty_terminal_point_from_grid_ref(
        this.terminal,
        reference,
        PointTag.SCREEN,
        this.coordinateBuffer,
      );
    if (result !== Result.SUCCESS) return null;
    const coordinate = this.bridge.createView(this.coordinateBuffer, POINT_COORDINATE_SIZE);
    return { x: coordinate.getUint16(0, true), y: coordinate.getUint32(4, true) };
  }

  private initializeSelection(pointer: number): void {
    this.clearBuffer(pointer, SELECTION_SIZE);
    const selection = this.bridge.createView(pointer, SELECTION_SIZE);
    selection.setUint32(0, SELECTION_SIZE, true);
    selection.setUint32(SELECTION_START_OFFSET, GRID_REF_SIZE, true);
    selection.setUint32(SELECTION_END_OFFSET, GRID_REF_SIZE, true);
  }

  private clearBuffer(pointer: number, length: number): void {
    const view = this.bridge.createView(pointer, length);
    for (let index = 0; index < length; index++) view.setUint8(index, 0);
  }

  private copyBytes(source: number, destination: number, length: number): void {
    this.bridge.writeBytes(destination, this.bridge.readBytes(source, length));
  }
}

function requireSelectionAllocation(pointer: number, purpose: string): number {
  if (pointer === 0) throw new Error(`WASM allocation failed: ${purpose}`);
  return pointer;
}
