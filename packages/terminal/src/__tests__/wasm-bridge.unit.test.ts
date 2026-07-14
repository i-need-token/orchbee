import { beforeAll, describe, expect, it } from "vitest";
import { getBridge } from "../wasm-bridge.js";
import type { WasmBridge } from "../wasm-bridge.js";
import { initializeWasm } from "./test-helpers.js";

let bridge: WasmBridge;

beforeAll(async () => {
  bridge = await initializeWasm();
});

describe("WasmBridge", () => {
  it("exposes the upstream terminal exports", () => {
    const exports = bridge.getExports();
    expect(exports.memory).toBeInstanceOf(WebAssembly.Memory);
    expect(exports.ghostty_terminal_new).toBeTypeOf("function");
    expect(exports.ghostty_terminal_vt_write).toBeTypeOf("function");
  });

  it("returns the initialized singleton", () => {
    expect(getBridge()).toBe(bridge);
  });

  it("creates fresh views over WASM memory", () => {
    const view = bridge.createView(0, 4);
    expect(view).toBeInstanceOf(DataView);
    expect(view.byteLength).toBe(4);
  });

  it("round-trips allocated bytes", () => {
    const pointer = bridge.allocU8Array(4);
    const data = new Uint8Array([1, 2, 3, 4]);
    bridge.writeBytes(pointer, data);
    expect(bridge.readBytes(pointer, data.length)).toEqual(data);
    bridge.freeU8Array(pointer, data.length);
  });

  it("registers callbacks that return an i32 to Ghostty", () => {
    const callback = bridge.registerCallback3ReturningI32((a, b, c) => a + b + c);
    const trampoline = bridge.getExports().__indirect_function_table.get(callback.index);
    expect(trampoline).toBeTypeOf("function");
    expect((trampoline as (a: number, b: number, c: number) => number)(1, 2, 3)).toBe(6);
    callback.dispose();
    expect(bridge.getExports().__indirect_function_table.get(callback.index)).toBeNull();
  });

  it("registers three-argument callbacks used by struct-return effects", () => {
    let values: readonly number[] = [];
    const callback = bridge.registerCallback3((a, b, c) => {
      values = [a, b, c];
    });
    const trampoline = bridge.getExports().__indirect_function_table.get(callback.index);
    (trampoline as (a: number, b: number, c: number) => void)(4, 5, 6);
    expect(values).toEqual([4, 5, 6]);
    callback.dispose();
  });
});
