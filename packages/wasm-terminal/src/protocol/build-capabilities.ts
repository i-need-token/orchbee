import { BuildInfo, Result } from "../types.js";
import type { WasmBridge } from "../wasm-bridge.js";

export interface GhosttyBuildCapabilities {
  readonly simd: boolean;
  readonly kittyGraphics: boolean;
  readonly tmuxControlMode: boolean;
}

/** Read compile-time Ghostty features from the loaded WASM binary. */
export function readGhosttyBuildCapabilities(bridge: WasmBridge): GhosttyBuildCapabilities {
  return {
    simd: readBoolean(bridge, BuildInfo.SIMD),
    kittyGraphics: readBoolean(bridge, BuildInfo.KITTY_GRAPHICS),
    tmuxControlMode: readBoolean(bridge, BuildInfo.TMUX_CONTROL_MODE),
  };
}

function readBoolean(bridge: WasmBridge, field: BuildInfo): boolean {
  const output = bridge.allocU8();
  try {
    const result = bridge.getExports().ghostty_build_info(field, output);
    return result === Result.SUCCESS && bridge.createView(output, 1).getUint8(0) !== 0;
  } finally {
    bridge.freeU8(output);
  }
}
