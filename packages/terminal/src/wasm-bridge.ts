/**
 * WASM bridge — loads the Ghostty VT parser module and manages memory.
 *
 * All WASM memory access goes through this class. No other file
 * should touch `memory.buffer` directly. Every `createView()` call
 * creates a fresh DataView to handle WASM memory growth.
 */

import type { WasmExports } from "./types.js";

export interface WasmTypeFieldLayout {
  readonly offset: number;
  readonly size: number;
  readonly type: string;
}

export interface WasmTypeLayout {
  readonly size: number;
  readonly align: number;
  readonly fields: Readonly<Record<string, WasmTypeFieldLayout>>;
}

/** Options for WASM module initialization. */
export interface InitOptions {
  /** WASM source: URL string, URL object, or pre-fetched ArrayBuffer. */
  readonly source: string | URL | ArrayBuffer;
}

/** Module-level singleton (set by init()). */
let bridgeInstance: WasmBridge | null = null;

/**
 * Initialize the Ghostty WASM VT parser.
 *
 * Must be called before creating Terminal instances.
 * Returns the WasmBridge singleton for advanced usage.
 *
 * @example
 * ```ts
 * import { init } from "@orchbee/terminal";
 * const bridge = await init({ source: "/ghostty-vt.wasm" });
 * ```
 */
export async function init(
  options: InitOptions = { source: new URL("../ghostty-vt.wasm", import.meta.url) },
): Promise<WasmBridge> {
  if (bridgeInstance) return bridgeInstance;
  bridgeInstance = await WasmBridge.load(options.source);
  return bridgeInstance;
}

/**
 * Get the initialized WasmBridge.
 * Throws if init() hasn't been called yet.
 */
export function getBridge(): WasmBridge {
  if (!bridgeInstance) {
    throw new Error(
      "WASM not initialized. Call init() before using Terminal.\n" +
        'Example: await init({ source: "/ghostty-vt.wasm" })',
    );
  }
  return bridgeInstance;
}

/**
 * WASM bridge — central point for all WASM memory operations.
 *
 * Design: every memory access creates a fresh DataView to handle
 * memory growth (when WASM allocates beyond current pages, the
 * underlying ArrayBuffer is replaced, detaching any cached views).
 */
export class WasmBridge {
  private readonly exports: WasmExports;
  private readonly freeCallbackSlots: number[] = [];
  private typeLayouts: Readonly<Record<string, WasmTypeLayout>> | undefined;
  readonly memory: WebAssembly.Memory;

  private constructor(wasmInstance: WebAssembly.Instance) {
    if (!isWasmExports(wasmInstance.exports)) {
      throw new Error("WASM module does not expose the Ghostty terminal API");
    }
    this.exports = wasmInstance.exports;
    this.memory = this.exports.memory;
  }

  // ==========================================================================
  // WASM Memory Helpers
  // ==========================================================================

  /** Allocate a u8 array in WASM memory. Returns pointer. */
  allocU8Array(len: number): number {
    return this.exports.ghostty_wasm_alloc_u8_array(len);
  }

  /** Free a u8 array in WASM memory. */
  freeU8Array(ptr: number, len: number): void {
    this.exports.ghostty_wasm_free_u8_array(ptr, len);
  }

  /** Allocate a single u8 in WASM memory. Returns pointer. */
  allocU8(): number {
    return this.exports.ghostty_wasm_alloc_u8();
  }

  /** Free a single u8 in WASM memory. */
  freeU8(ptr: number): void {
    this.exports.ghostty_wasm_free_u8(ptr);
  }

  /** Allocate an opaque pointer in WASM memory. */
  allocOpaque(): number {
    return this.exports.ghostty_wasm_alloc_opaque();
  }

  /** Free an opaque pointer in WASM memory. */
  freeOpaque(ptr: number): void {
    this.exports.ghostty_wasm_free_opaque(ptr);
  }

  /** Allocate a usize in WASM memory. */
  allocUsize(): number {
    return this.exports.ghostty_wasm_alloc_usize();
  }

  /** Free a usize in WASM memory. */
  freeUsize(ptr: number): void {
    this.exports.ghostty_wasm_free_usize(ptr);
  }

  // ==========================================================================
  // Data Transfer
  // ==========================================================================

  /** Write bytes into WASM memory at the given pointer. */
  writeBytes(ptr: number, data: Uint8Array): void {
    new Uint8Array(this.memory.buffer).set(data, ptr);
  }

  /** Read bytes from WASM memory. Returns a fresh copy (safe after free). */
  readBytes(ptr: number, len: number): Uint8Array {
    return new Uint8Array(this.memory.buffer, ptr, len).slice();
  }

  /** Read a null-terminated UTF-8 string from WASM memory. */
  readCString(ptr: number): string {
    const bytes = new Uint8Array(this.memory.buffer);
    let end = ptr;
    while (end < bytes.length && bytes[end] !== 0) end++;
    if (end === bytes.length) throw new Error("WASM string is not null-terminated");
    return new TextDecoder().decode(bytes.subarray(ptr, end));
  }

  /**
   * Create a DataView over WASM memory.
   *
   * IMPORTANT: Creates a fresh view each call to handle memory growth.
   * Never cache DataView or TypedArray references across WASM calls
   * that might allocate — they become detached when memory grows.
   */
  createView(byteOffset: number, byteLength: number): DataView {
    return new DataView(this.memory.buffer, byteOffset, byteLength);
  }

  // ==========================================================================
  // Raw Export Access
  // ==========================================================================

  /** Get the raw WASM exports (for advanced usage). */
  getExports(): WasmExports {
    return this.exports;
  }

  /** Return a C ABI layout reported by Ghostty for this WASM build. */
  getTypeLayout(name: string): WasmTypeLayout {
    this.typeLayouts ??= parseTypeLayouts(this.readCString(this.exports.ghostty_type_json()));
    const layout = this.typeLayouts[name];
    if (!layout) throw new Error(`Ghostty type layout is missing: ${name}`);
    return layout;
  }

  /** Encode terminal focus reporting through Ghostty. */
  encodeFocus(focused: boolean): Uint8Array {
    return this.encodeIntoBuffer((buffer, length, written) =>
      this.exports.ghostty_focus_encode(focused ? 0 : 1, buffer, length, written),
    );
  }

  /** Return whether Ghostty considers text safe to paste without confirmation. */
  isPasteSafe(text: string): boolean {
    const data = new TextEncoder().encode(text);
    const pointer =
      data.length === 0
        ? 0
        : requireWasmPointer(this.allocU8Array(data.length), "paste input buffer");
    try {
      if (data.length > 0) this.writeBytes(pointer, data);
      return this.exports.ghostty_paste_is_safe(pointer, data.length) !== 0;
    } finally {
      if (data.length > 0) this.freeU8Array(pointer, data.length);
    }
  }

  /** Encode paste bytes through Ghostty's sanitization and bracketed-paste implementation. */
  encodePaste(text: string, bracketed: boolean): Uint8Array {
    const data = new TextEncoder().encode(text);
    const pointer =
      data.length === 0
        ? 0
        : requireWasmPointer(this.allocU8Array(data.length), "paste input buffer");
    try {
      if (data.length > 0) this.writeBytes(pointer, data);
      return this.encodeIntoBuffer((buffer, length, written) =>
        this.exports.ghostty_paste_encode(
          pointer,
          data.length,
          bracketed ? 1 : 0,
          buffer,
          length,
          written,
        ),
      );
    } finally {
      if (data.length > 0) this.freeU8Array(pointer, data.length);
    }
  }

  /** Register a table-compatible `(i32, i32, i32, i32) -> void` callback. */
  registerCallback4(callback: (a: number, b: number, c: number, d: number) => void): {
    readonly index: number;
    dispose(): void;
  } {
    const module = new WebAssembly.Module(CALLBACK4_TRAMPOLINE);
    const instance = new WebAssembly.Instance(module, { env: { callback } });
    const trampoline = instance.exports["trampoline"];
    if (typeof trampoline !== "function") {
      throw new Error("WASM callback trampoline export is missing");
    }

    const table = this.exports.__indirect_function_table;
    const index = this.freeCallbackSlots.pop() ?? table.grow(1);
    table.set(index, trampoline);
    let active = true;
    return {
      index,
      dispose: (): void => {
        if (!active) return;
        table.set(index, null);
        this.freeCallbackSlots.push(index);
        active = false;
      },
    };
  }

  /** Register a table-compatible `(i32, i32) -> void` callback. */
  registerCallback2(callback: (a: number, b: number) => void): {
    readonly index: number;
    dispose(): void;
  } {
    const module = new WebAssembly.Module(CALLBACK2_TRAMPOLINE);
    const instance = new WebAssembly.Instance(module, { env: { callback } });
    const trampoline = instance.exports["trampoline"];
    if (typeof trampoline !== "function") {
      throw new Error("WASM callback trampoline export is missing");
    }

    const table = this.exports.__indirect_function_table;
    const index = this.freeCallbackSlots.pop() ?? table.grow(1);
    table.set(index, trampoline);
    let active = true;
    return {
      index,
      dispose: (): void => {
        if (!active) return;
        table.set(index, null);
        this.freeCallbackSlots.push(index);
        active = false;
      },
    };
  }

  /** Register a table-compatible `(i32, i32, i32) -> i32` callback. */
  registerCallback3ReturningI32(callback: (a: number, b: number, c: number) => number): {
    readonly index: number;
    dispose(): void;
  } {
    const module = new WebAssembly.Module(CALLBACK3_I32_TRAMPOLINE);
    const instance = new WebAssembly.Instance(module, { env: { callback } });
    const trampoline = instance.exports["trampoline"];
    if (typeof trampoline !== "function") {
      throw new Error("WASM callback trampoline export is missing");
    }

    const table = this.exports.__indirect_function_table;
    const index = this.freeCallbackSlots.pop() ?? table.grow(1);
    table.set(index, trampoline);
    let active = true;
    return {
      index,
      dispose: (): void => {
        if (!active) return;
        table.set(index, null);
        this.freeCallbackSlots.push(index);
        active = false;
      },
    };
  }

  /** Register a table-compatible `(i32, i32, i32) -> void` callback. */
  registerCallback3(callback: (a: number, b: number, c: number) => void): {
    readonly index: number;
    dispose(): void;
  } {
    const module = new WebAssembly.Module(CALLBACK3_TRAMPOLINE);
    const instance = new WebAssembly.Instance(module, { env: { callback } });
    const trampoline = instance.exports["trampoline"];
    if (typeof trampoline !== "function") {
      throw new Error("WASM callback trampoline export is missing");
    }

    const table = this.exports.__indirect_function_table;
    const index = this.freeCallbackSlots.pop() ?? table.grow(1);
    table.set(index, trampoline);
    let active = true;
    return {
      index,
      dispose: (): void => {
        if (!active) return;
        table.set(index, null);
        this.freeCallbackSlots.push(index);
        active = false;
      },
    };
  }

  // ==========================================================================
  // Double-Pointer Helpers
  // ==========================================================================

  /**
   * Allocate an opaque slot, call a constructor function that writes
   * the handle to the slot, read the handle, and free the slot.
   *
   * Used for ghostty_terminal_new, ghostty_render_state_new, etc.
   *
   * @param fn Function that takes (allocator, ptrPtr) and returns a result code.
   * @returns The handle read from the opaque slot.
   * @throws If the function returns a non-zero result.
   */
  allocAndReadOpaque(fn: (allocator: number, ptrPtr: number) => number): number {
    const ptrPtr = requireWasmPointer(this.allocOpaque(), "opaque output slot");
    try {
      const result = fn(0, ptrPtr);
      if (result !== 0) {
        throw new Error(`WASM constructor failed with result code: ${result}`);
      }
      return requireWasmPointer(
        this.createView(ptrPtr, 4).getUint32(0, true),
        "constructed Ghostty handle",
      );
    } finally {
      this.freeOpaque(ptrPtr);
    }
  }

  /**
   * Query a u16 value from ghostty_terminal_get or ghostty_render_state_get.
   *
   * @param fn Function that takes (handle, fieldId, outPtr) and returns a result code.
   * @param handle The opaque handle (terminal or render state).
   * @param fieldId The data field identifier.
   * @returns The u16 value, or 0 if the query failed.
   */
  getU16(
    fn: (handle: number, field: number, out: number) => number,
    handle: number,
    fieldId: number,
  ): number {
    const outPtr = this.allocU8Array(2);
    try {
      const result = fn(handle, fieldId, outPtr);
      if (result !== 0) return 0;
      return this.createView(outPtr, 2).getUint16(0, true);
    } finally {
      this.freeU8Array(outPtr, 2);
    }
  }

  /**
   * Query a bool value from ghostty_terminal_get or ghostty_render_state_get.
   *
   * @param fn Function that takes (handle, fieldId, outPtr) and returns a result code.
   * @param handle The opaque handle.
   * @param fieldId The data field identifier.
   * @returns The bool value, or false if the query failed.
   */
  getBool(
    fn: (handle: number, field: number, out: number) => number,
    handle: number,
    fieldId: number,
  ): boolean {
    const outPtr = this.allocU8();
    try {
      const result = fn(handle, fieldId, outPtr);
      if (result !== 0) return false;
      return this.createView(outPtr, 1).getUint8(0) !== 0;
    } finally {
      this.freeU8(outPtr);
    }
  }

  /**
   * Query a u32 value from ghostty_terminal_get.
   *
   * @param fn Function that takes (handle, fieldId, outPtr) and returns a result code.
   * @param handle The opaque handle.
   * @param fieldId The data field identifier.
   * @returns The u32 value, or 0 if the query failed.
   */
  getU32(
    fn: (handle: number, field: number, out: number) => number,
    handle: number,
    fieldId: number,
  ): number {
    const outPtr = this.allocU8Array(4);
    try {
      const result = fn(handle, fieldId, outPtr);
      if (result !== 0) return 0;
      return this.createView(outPtr, 4).getUint32(0, true);
    } finally {
      this.freeU8Array(outPtr, 4);
    }
  }

  private encodeIntoBuffer(
    encode: (buffer: number, length: number, written: number) => number,
  ): Uint8Array {
    let bufferSize = 64;
    let buffer = requireWasmPointer(this.allocU8Array(bufferSize), "encoding buffer");
    let written = 0;
    try {
      written = requireWasmPointer(this.allocUsize(), "encoding length slot");
      let result = encode(buffer, bufferSize, written);
      if (result === -3) {
        const required = this.createView(written, 4).getUint32(0, true);
        const replacement = requireWasmPointer(
          this.allocU8Array(required),
          "expanded encoding buffer",
        );
        this.freeU8Array(buffer, bufferSize);
        bufferSize = required;
        buffer = replacement;
        result = encode(buffer, bufferSize, written);
      }
      if (result !== 0) throw new Error(`Ghostty encoding failed with result code: ${result}`);
      const length = this.createView(written, 4).getUint32(0, true);
      return this.readBytes(buffer, length);
    } finally {
      this.freeU8Array(buffer, bufferSize);
      if (written !== 0) this.freeUsize(written);
    }
  }

  // ==========================================================================
  // Loading
  // ==========================================================================

  /**
   * Load and instantiate the Ghostty WASM module.
   *
   * @param source - URL string, URL object, or pre-fetched ArrayBuffer.
   */
  static async load(source: string | URL | ArrayBuffer): Promise<WasmBridge> {
    let wasmBytes: ArrayBuffer;

    if (source instanceof ArrayBuffer) {
      wasmBytes = source;
    } else {
      wasmBytes = await fetchWasm(source);
    }

    if (wasmBytes.byteLength === 0) {
      throw new Error("WASM file is empty (0 bytes)");
    }

    const wasmModule = await WebAssembly.compile(wasmBytes);

    // We need the instance reference for the log import closure.
    // Use a mutable binding that gets set after instantiation.
    let wasmInstance: WebAssembly.Instance | undefined;

    const importObject = {
      env: {
        log: (ptr: number, len: number): void => {
          if (!wasmInstance || !isWasmExports(wasmInstance.exports)) return;
          const mem = wasmInstance.exports.memory.buffer;
          const bytes = new Uint8Array(mem, ptr, len);
          console.log("[ghostty-vt]", new TextDecoder().decode(bytes));
        },
      },
    };

    wasmInstance = await WebAssembly.instantiate(wasmModule, importObject);
    return new WasmBridge(wasmInstance);
  }
}

function parseTypeLayouts(json: string): Readonly<Record<string, WasmTypeLayout>> {
  const value: unknown = JSON.parse(json);
  if (!isUnknownRecord(value)) throw new Error("Ghostty type metadata is invalid");
  const layouts: Record<string, WasmTypeLayout> = {};
  for (const [name, candidate] of Object.entries(value)) {
    if (!isUnknownRecord(candidate)) throw new Error(`Ghostty type layout is invalid: ${name}`);
    const { align, fields, size } = candidate;
    if (typeof align !== "number" || typeof size !== "number" || !isUnknownRecord(fields)) {
      throw new Error(`Ghostty type layout is invalid: ${name}`);
    }
    const parsedFields: Record<string, WasmTypeFieldLayout> = {};
    for (const [fieldName, field] of Object.entries(fields)) {
      if (!isUnknownRecord(field)) {
        throw new Error(`Ghostty type field layout is invalid: ${name}.${fieldName}`);
      }
      const { offset, size: fieldSize, type } = field;
      if (typeof offset !== "number" || typeof fieldSize !== "number" || typeof type !== "string") {
        throw new Error(`Ghostty type field layout is invalid: ${name}.${fieldName}`);
      }
      parsedFields[fieldName] = { offset, size: fieldSize, type };
    }
    layouts[name] = { align, fields: parsedFields, size };
  }
  return layouts;
}

function isUnknownRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function requireWasmPointer(pointer: number, purpose: string): number {
  if (pointer === 0) throw new Error(`WASM allocation failed: ${purpose}`);
  return pointer;
}

function isWasmExports(value: unknown): value is WasmExports {
  if (!value || typeof value !== "object") return false;
  return (
    "memory" in value &&
    value.memory instanceof WebAssembly.Memory &&
    "ghostty_terminal_new" in value &&
    typeof value.ghostty_terminal_new === "function"
  );
}

/** A tiny module that turns a JavaScript callback into a WASM function reference. */
const CALLBACK4_TRAMPOLINE = new Uint8Array([
  0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x01, 0x60, 0x04, 0x7f, 0x7f, 0x7f,
  0x7f, 0x00, 0x02, 0x10, 0x01, 0x03, 0x65, 0x6e, 0x76, 0x08, 0x63, 0x61, 0x6c, 0x6c, 0x62, 0x61,
  0x63, 0x6b, 0x00, 0x00, 0x03, 0x02, 0x01, 0x00, 0x07, 0x0e, 0x01, 0x0a, 0x74, 0x72, 0x61, 0x6d,
  0x70, 0x6f, 0x6c, 0x69, 0x6e, 0x65, 0x00, 0x01, 0x0a, 0x0e, 0x01, 0x0c, 0x00, 0x20, 0x00, 0x20,
  0x01, 0x20, 0x02, 0x20, 0x03, 0x10, 0x00, 0x0b,
]);

/** `(i32, i32) -> void` import/export trampoline. */
const CALLBACK2_TRAMPOLINE = new Uint8Array([
  0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, 0x01, 0x06, 0x01, 0x60, 0x02, 0x7f, 0x7f, 0x00,
  0x02, 0x10, 0x01, 0x03, 0x65, 0x6e, 0x76, 0x08, 0x63, 0x61, 0x6c, 0x6c, 0x62, 0x61, 0x63, 0x6b,
  0x00, 0x00, 0x03, 0x02, 0x01, 0x00, 0x07, 0x0e, 0x01, 0x0a, 0x74, 0x72, 0x61, 0x6d, 0x70, 0x6f,
  0x6c, 0x69, 0x6e, 0x65, 0x00, 0x01, 0x0a, 0x0a, 0x01, 0x08, 0x00, 0x20, 0x00, 0x20, 0x01, 0x10,
  0x00, 0x0b,
]);

/** `(i32, i32, i32) -> i32` import/export trampoline. */
const CALLBACK3_I32_TRAMPOLINE = new Uint8Array([
  0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x01, 0x60, 0x03, 0x7f, 0x7f, 0x7f,
  0x01, 0x7f, 0x02, 0x10, 0x01, 0x03, 0x65, 0x6e, 0x76, 0x08, 0x63, 0x61, 0x6c, 0x6c, 0x62, 0x61,
  0x63, 0x6b, 0x00, 0x00, 0x03, 0x02, 0x01, 0x00, 0x07, 0x0e, 0x01, 0x0a, 0x74, 0x72, 0x61, 0x6d,
  0x70, 0x6f, 0x6c, 0x69, 0x6e, 0x65, 0x00, 0x01, 0x0a, 0x0c, 0x01, 0x0a, 0x00, 0x20, 0x00, 0x20,
  0x01, 0x20, 0x02, 0x10, 0x00, 0x0b,
]);

/** `(i32, i32, i32) -> void` import/export trampoline. */
const CALLBACK3_TRAMPOLINE = new Uint8Array([
  0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, 0x01, 0x07, 0x01, 0x60, 0x03, 0x7f, 0x7f, 0x7f,
  0x00, 0x02, 0x10, 0x01, 0x03, 0x65, 0x6e, 0x76, 0x08, 0x63, 0x61, 0x6c, 0x6c, 0x62, 0x61, 0x63,
  0x6b, 0x00, 0x00, 0x03, 0x02, 0x01, 0x00, 0x07, 0x0e, 0x01, 0x0a, 0x74, 0x72, 0x61, 0x6d, 0x70,
  0x6f, 0x6c, 0x69, 0x6e, 0x65, 0x00, 0x01, 0x0a, 0x0c, 0x01, 0x0a, 0x00, 0x20, 0x00, 0x20, 0x01,
  0x20, 0x02, 0x10, 0x00, 0x0b,
]);

/**
 * Fetch WASM bytes from a URL or path.
 * Tries Node.js fs first, then falls back to fetch (browser).
 */
async function fetchWasm(source: string | URL): Promise<ArrayBuffer> {
  const url = source instanceof URL ? source : new URL(source, import.meta.url);

  // Try Node.js fs module first (for SSR / test environments)
  if (typeof process !== "undefined" && typeof process.versions?.node === "string") {
    try {
      const fs = await import("node:fs/promises");
      const path = url.protocol === "file:" ? url.pathname : url.href;
      const buffer = await fs.readFile(path);
      return buffer.buffer.slice(buffer.byteOffset, buffer.byteOffset + buffer.byteLength);
    } catch {
      // fs failed, fall through to fetch
    }
  }

  // Browser / fetch fallback
  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(`Failed to fetch WASM: ${response.status} ${response.statusText}`);
  }
  const buffer = await response.arrayBuffer();
  if (buffer.byteLength === 0) {
    throw new Error(`WASM file is empty (0 bytes). Check URL: ${url.href}`);
  }
  return buffer;
}
