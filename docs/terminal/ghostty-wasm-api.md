# Ghostty WASM API

`packages/terminal` uses upstream `libghostty-vt`; do not apply the historical `ghostty-web` patch.

Use [Ghostty Terminal Panel Conformance](./ghostty-surface-test-provenance.md) to decide whether an upstream test belongs in Orchbee.

## Build

```sh
cd /path/to/ghostty
zig build lib-vt -Dtarget=wasm32-freestanding -Doptimize=ReleaseSmall
cp zig-out/bin/ghostty-vt.wasm /path/to/orchbee/packages/terminal/
```

The WASM module must export `memory` and `__indirect_function_table`.

## ABI

Read generated ABI metadata from `ghostty_type_json()` when upgrading Ghostty; the current wasm32 layouts used by the binding are:

| Type                               | Size | Important offsets                        |
| ---------------------------------- | ---: | ---------------------------------------- |
| `GhosttyTerminalOptions`           |    8 | `cols=0`, `rows=2`, `max_scrollback=4`   |
| `GhosttyPoint`                     |   24 | `tag=0`, coordinate `x=8`, `y=12`        |
| `GhosttyGridRef`                   |   12 | `size=0`, `node=4`, `x=8`, `y=10`        |
| `GhosttyPointCoordinate`           |    8 | `x=0`, `y=4`                             |
| `GhosttyMousePosition`             |    8 | `x=0`, `y=4`                             |
| `GhosttyMouseEncoderSize`          |   36 | `size=0`, dimensions and padding `4..32` |
| `GhosttySelectionGestureBehaviors` |   12 | click behaviors `0`, `4`, `8`            |
| `GhosttySelectionGestureGeometry`  |   16 | geometry fields `0`, `4`, `8`, `12`      |
| `GhosttyStyle`                     |   72 | flags `56..64`                           |
| `GhosttyRenderStateColors`         |  784 | palette starts at `14`                   |

WASM `i64` parameters such as `GhosttyCell` and `GhosttyRow` are JavaScript `bigint` values.

## Integration

| Capability                        | Upstream API                                                                     |
| --------------------------------- | -------------------------------------------------------------------------------- |
| Lifecycle and VT input            | `ghostty_terminal_new/free/reset/resize/vt_write`                                |
| Terminal responses                | `GHOSTTY_TERMINAL_OPT_WRITE_PTY` callback                                        |
| Bell and title events             | `GHOSTTY_TERMINAL_OPT_BELL/TITLE_CHANGED` callbacks                              |
| Frame snapshot                    | `ghostty_render_state_update` plus row/cell iterators                            |
| Cell text and resolved colors     | `ghostty_render_state_row_cells_get`                                             |
| Cell width and hyperlink presence | `ghostty_cell_get` on the raw cell                                               |
| Scrollback and OSC 8 URIs         | `ghostty_terminal_grid_ref` plus `ghostty_grid_ref_*`                            |
| Modes                             | `ghostty_terminal_mode_get`                                                      |
| Keyboard encoding                 | `ghostty_key_encoder_setopt_from_terminal` plus `ghostty_key_encoder_encode`     |
| Mouse encoding                    | `ghostty_mouse_encoder_setopt_from_terminal` plus `ghostty_mouse_encoder_encode` |
| Focus and paste encoding          | `ghostty_focus_encode`, `ghostty_paste_is_safe`, `ghostty_paste_encode`          |
| Selection gestures                | `ghostty_selection_gesture_*`                                                    |

Call `ghostty_render_state_update` once for each requested frame, capture the iterator data once, and serve renderer/buffer consumers from that snapshot.

Grid references are for scrollback, links, and other non-frame lookups; do not use them for the active render loop.

## JavaScript callbacks

Browser and Node runtimes cannot portably insert plain JavaScript functions into a `WebAssembly.Table`; `WasmBridge` creates typed WASM trampolines, grows the exported table, and clears each slot on terminal disposal.

Callbacks run synchronously during `ghostty_terminal_vt_write`; copy borrowed bytes before returning and never re-enter `vt_write` on the same terminal.

## Upgrade check

1. Replace `ghostty-vt.wasm` with the new upstream build.
2. Compare `ghostty_type_json()` with constants in `types.ts`.
3. Verify enum values against upstream headers.
4. Run typecheck, unit tests, Node integration tests, and the WebGPU browser demo.
