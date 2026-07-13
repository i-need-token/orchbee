# Terminal Surface Gap Analysis

> Status: Active implementation investigation
> Last verified: 2026-07-13 against Ghostty and `ghostty-web`
> Exit condition: Promote durable conclusions and tracked work, then delete this file.

## Scope

`wasm-terminal` targets Ghostty terminal-surface behavior with a WebGPU renderer; windows, tabs, splits, shell process lifecycle, and application chrome belong to Orchbee.

| Layer            | Owns                                                                                    |
| ---------------- | --------------------------------------------------------------------------------------- |
| Ghostty WASM     | VT parsing, modes, screen state, tracked selection, semantic cells, protocol responses  |
| Terminal surface | DOM input arbitration, search, links, actions, host requests, snapshots, viewport state |
| Renderer         | WebGPU/Canvas drawing, glyph atlas, preedit, decorations, cursor, scrollbar             |
| Host application | Clipboard, paste confirmation, menus, URL policy, notifications, native pointer effects |

## Implemented

| Area                 | Current behavior                                                                                           |
| -------------------- | ---------------------------------------------------------------------------------------------------------- |
| VT foundation        | Upstream `libghostty-vt`, primary/alternate screens, scrollback, modes, palette, cursor, title, PWD        |
| Protocol identity    | DA1/DA2/DA3, XTVERSION, ENQ, DSR, XTWINOPS, color-scheme query, focus and mouse reports                    |
| Rendering            | WebGPU plus Canvas fallback, ink-bounds/color atlas, physical-pixel cursors, DPR rebuild, frame telemetry  |
| Text style           | Bold, italic, faint, blink, inverse, invisible, strike, overline, underline variants and underline color   |
| Bounded shaping      | Optional programming-ligature runs with style, cursor, selection, search, and preedit boundaries           |
| Dynamic colors       | OSC default-color overrides synchronize empty background, foreground, and cursor rendering                 |
| IME                  | CJK commit, single-underlined preedit, stable cursor-midpoint candidate anchor, transition cancellation    |
| Keyboard             | Ghostty encoding, prioritized scoped hooks, named actions, Kitty releases, focus-loss release repair       |
| Input ordering       | Async paste confirmation cannot reorder later keyboard, focus, or wheel input                              |
| Paste safety         | Control filtering, newline policy, bracket breakout detection, bracketed paste, host confirmation          |
| Selection            | Ghostty tracked pins, clear-on-input, screen-transition clearing, word/line/rectangle/all/output selection |
| Rich copy            | Ghostty plain, HTML, and VT selection formats through a host clipboard capability                          |
| Search               | Literal/regex/case/whole-word, scrollback/soft-wrap, all-match decoration, active navigation               |
| Shell semantics      | OSC 133 cell metadata, prompt/input/output context, semantic output selection                              |
| Links                | OSC 8, wrapped URLs, CWD-resolved file paths/positions, semantic filtering, host-owned opening             |
| Mouse                | X10, UTF-8, SGR, URXVT, SGR pixels, motion, vertical/horizontal wheel, modifiers                           |
| Drag and drop        | Text uses paste safety; files require host path/quoting resolution before entering the input queue         |
| Actions and menus    | Named action registry, deterministic state, serializable provider contributions, application-owned UI      |
| Accessibility        | Visible-row mirror, stable UTF-16/cell ranges, two-way DOM selection, cursor and live output               |
| Lifecycle            | Transactional open rollback, hidden-surface suspension, disposal, disposed-open rejection                  |
| Capability reporting | Renderer/host capabilities plus actual Ghostty SIMD, graphics, and tmux compile-time features              |

## Remaining gaps

### P0

| Gap                         | Current limit                                                                                      | Required outcome                                                                 |
| --------------------------- | -------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------- |
| IME platform completeness   | Candidate geometry is browser-tested; native input-method windows are not fully conformance-tested | Verify Pinyin, Japanese, Korean, dictation, emoji, and mobile behavior           |
| Protocol-owned host effects | Upstream terminal callbacks do not expose OSC 52, desktop notification, or mouse-shape payloads    | Add upstream typed callbacks before advertising or handling these effects        |
| Graphics protocols          | Ghostty disables Kitty graphics for `wasm32-freestanding`; renderer advertises none                | Add browser time support upstream, enable storage/placements, render image quads |

### P1

| Gap          | Current limit                                                                                     | Required outcome                                                      |
| ------------ | ------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------- |
| Text shaping | Canvas shapes a bounded programming-ligature set; it has no deterministic raw-font fallback chain | Add raw-font shaping, fallback discovery, and arbitrary font features |

### P2

| Gap                       | Current limit                                                                   |
| ------------------------- | ------------------------------------------------------------------------------- |
| Minimum contrast          | Application colors are rendered without configurable contrast correction        |
| Decoration contributions  | Links and selections are built in; addons cannot add renderer overlays          |
| Scrollbar presentation    | Scrollbar policy and rendering remain coupled to the terminal/renderer          |
| Selection policy breadth  | Copy trimming/unwrap formats use fixed Ghostty formatter options                |
| Semantic command metadata | Cell zones and output ranges exist; command line, command id, and status do not |

## `ghostty-web` relationship

| Concern       | `ghostty-web`                          | `wasm-terminal`                                                              |
| ------------- | -------------------------------------- | ---------------------------------------------------------------------------- |
| VT API        | Historical patched WASM API            | Upstream stable C API through `WasmBridge`                                   |
| Renderer      | Canvas                                 | WebGPU with Canvas 2D fallback                                               |
| Font pipeline | Browser Canvas shaping and fallback    | Browser Canvas shaping/fallback feeding a physical-pixel atlas               |
| Demos         | Colors, shell, scrollbar               | Equivalent colors, PTY shell, scrollbar, and bundled-font Ghostty comparison |
| Selection     | TypeScript numeric ranges              | Ghostty terminal-owned tracked selections                                    |
| Host effects  | Browser globals in providers/managers  | Typed host capabilities and requests                                         |
| Paste         | Direct browser input path              | Sanitized, confirmable, ordered pipeline                                     |
| Search/a11y   | Not a complete native-surface analogue | Search API and accessible text mirror                                        |

## Verification boundary

Unit tests use the real Ghostty WASM for protocol, grid, selection, Unicode, color, and semantic behavior; browser tests remain required for WebGPU output, native IME candidate UI, clipboard permissions, screen readers, and GPU loss.

## Native Ghostty rasterization boundary

| Concern       | Native Ghostty on macOS                  | `wasm-terminal`                                                    |
| ------------- | ---------------------------------------- | ------------------------------------------------------------------ |
| Primary font  | Embedded JetBrains Mono variable faces   | The same variable faces bundled in the comparison demos            |
| Fallback      | CoreText codepoint discovery             | CSS list plus browser fallback                                     |
| Shaping       | CoreText text runs and glyph positions   | Canvas shaping for graphemes and bounded programming-ligature runs |
| Rasterization | CoreText alpha/color glyph bitmaps       | Canvas 2D alpha/color glyph bitmaps uploaded to WebGPU             |
| Compositing   | Native Metal renderer and alpha blending | Browser WebGPU and browser color management                        |
