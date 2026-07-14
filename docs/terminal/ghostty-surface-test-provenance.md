# Ghostty Terminal Panel Conformance

## Scope

Audit one terminal panel against Ghostty renderer, font, shaper, search, surface, and related Swift tests; exclude VT behavior supplied by the identical WASM artifact and exclude windows, tabs, splits, updates, command palettes, and other application UI.

| Baseline              | Value                                                              |
| --------------------- | ------------------------------------------------------------------ |
| Verified              | 2026-07-13                                                         |
| Ghostty commit        | `91f66da24527fa02d92b5fd0b41cd020f553a64c`                         |
| Orchbee commit        | `f13224bba131ad7cdebc2c7c4ddeca498da8c718` plus working-tree audit |
| WASM SHA-256          | `b0f39cfe981af36745c6b9c6919e9ac9cdf20aa507764e78bfb7a63ddc945c5b` |
| Artifact comparison   | Byte-identical to Ghostty `zig-out/bin/ghostty-vt.wasm`            |
| Full upstream tests   | 3,343 Zig declarations under `src/`; 246 Swift declarations        |
| Panel audit set       | 261 Zig declarations plus 13 related Swift declarations            |
| Orchbee package tests | 237 tests in 34 files                                              |

The Zig count accepts `test {}`, `test name {}`, and `test "name" {`; earlier counts that accepted only quoted and unnamed tests were incomplete.

## Dispositions

| Disposition       | Count | Coverage treatment                                                                  |
| ----------------- | ----: | ----------------------------------------------------------------------------------- |
| Covered           |   114 | Applicable declaration with Orchbee unit or browser evidence                        |
| Gap               |    52 | Applicable observable capability absent or incomplete in the terminal panel         |
| Browser product   |    70 | Exact native algorithm replaced by `FontFaceSet`, Canvas text, or WebGPU            |
| Upstream internal |    24 | Allocator injection, native cache/container, or page-layout detail with no analogue |
| Application layer |    14 | Configuration parsing, pasteboard search UI, transfer UI, or native search focus    |
| Total             |   274 | Every declaration in the audit set                                                  |

Declaration-weighted applicable coverage is `114 / (114 + 52) = 68.7%`; this is a test-provenance metric, not a user-visible feature percentage.

## Orchbee evidence

| Code       | Evidence                                                                                      |
| ---------- | --------------------------------------------------------------------------------------------- |
| `E-RENDER` | `src/__tests__/renderer.unit.test.ts`                                                         |
| `E-FIT`    | fit tests and `src/__tests__/surface-size.unit.test.ts`                                       |
| `E-SHAPE`  | `src/__tests__/text-shaping.unit.test.ts`, renderer tests, and `browser/surface-test.html`    |
| `E-SEARCH` | `src/search/__tests__/terminal-search.unit.test.ts` and `src/__tests__/terminal.unit.test.ts` |
| `E-LINK`   | link detector, renderer, terminal, and browser surface tests                                  |
| `E-MOUSE`  | `src/input/__tests__/surface-mouse.unit.test.ts` and browser surface tests                    |
| `E-IME`    | `src/input/__tests__/preedit.unit.test.ts` and input-handler tests                            |
| `E-SELECT` | `src/__tests__/selection-manager.unit.test.ts`                                                |
| `E-FONT`   | font-metrics, glyph-constraints, renderer, generated-table, DPR, and browser surface tests    |

Each declaration named below has the row's disposition and evidence; `<unnamed@line>` identifies an unnamed upstream test at the baseline commit.

## Renderer manifest

- `src/renderer/State.zig` — Covered (`E-RENDER`): `preedit range covers exact cell width`; `preedit range shifts left at right edge`.
- `src/renderer/cell.zig` — Covered (`E-RENDER`): `Contents`; `Contents clear retains other content`; `Contents clear last added content`; `Contents with zero-sized screen`.
- `src/renderer/cell.zig` — Covered (`E-FONT`): `Cell constraint widths`.
- `src/renderer/cursor.zig` — Covered (`E-RENDER`): `cursor: default uses configured style`; `cursor: blinking disabled`; `cursor: explicitly not visible`; `cursor: always block with preedit`.
- `src/renderer/link.zig` — Covered (`E-LINK`): `renderCellMap`; `renderCellMap hover links`; `renderCellMap mods no match`.
- `src/renderer/metal/shaders.zig` — Browser product: `<unnamed@283>`; WebGPU shader compilation replaces Metal shader compilation.
- `src/renderer/shadertoy.zig` — Gap: `spirv`; `spirv invalid`; `shadertoy to msl`; `shadertoy to glsl`.
- `src/renderer/size.zig` — Covered (`E-FIT`): `Size.balancePadding equal distributes whitespace equally`; `Size.balancePadding true shifts excess top to bottom`; `Padding balanced on zero`.
- `src/renderer/size.zig` — Covered (`E-FIT`, `E-SELECT`): `GridSize update exact`; `GridSize update rounding`; `coordinate conversion`.

## Font manifest

- `src/font/Atlas.zig` — Covered (`E-RENDER`): `happy path`; `exact fit`; `doesn't fit`; `fit multiple`; `writing data`; `grow`; `writing BGR data`; `grow BGR`.
- `src/font/Atlas.zig` — Browser product: `writing data from a larger source`; Canvas rasterization supplies the source crop.
- `src/font/Atlas.zig` — Upstream internal: `grow OOM`; `init error`; `reserve error`; `grow error`; Zig allocator failure injection has no browser equivalent.
- `src/font/CodepointMap.zig` — Browser product: `codepointmap`.
- `src/font/CodepointResolver.zig` — Browser product: `getIndex`; `getIndex disabled font style`.
- `src/font/CodepointResolver.zig` — Gap: `getIndex box glyph`; Orchbee has no Ghostty sprite-face resolver.
- `src/font/Collection.zig` — Upstream internal: `<unnamed@929>`.
- `src/font/Collection.zig` — Browser product: `init`; `add full`; `add deferred without loading options`; `getFace`; `getIndex`; `completeStyles`; `hasCodepoint`; `hasCodepoint emoji default graphical`; `face metrics`.
- `src/font/Collection.zig` — Covered (`E-FONT`): `setSize`; `metrics`.
- `src/font/Collection.zig` — Covered (`E-FONT`): `adjusted sizes`.
- `src/font/DeferredFace.zig` — Browser product: `fontconfig`; `coretext`.
- `src/font/Glyph.zig` — Covered (`E-FONT`): `Constraints`.
- `src/font/Metrics.zig` — Application layer: `formatConfig percent`; `formatConfig absolute`; `Modifier: parse absolute`; `Modifier: parse percent`.
- `src/font/Metrics.zig` — Covered (`E-FONT`): `Metrics: apply modifiers`; `Metrics: adjust cell height smaller`; `Metrics: adjust cell height larger`; `Metrics: adjust icon height by percentage`; `Metrics: adjust icon height by absolute pixels`; `Modifier: percent`; `Modifier: absolute`.
- `src/font/SharedGrid.zig` — Covered (`E-FONT`): `getIndex`; `renderGlyph error after cache insert rolls back cache entry`.
- `src/font/SharedGrid.zig` — Upstream internal: `init error`.
- `src/font/SharedGridSet.zig` — Covered (`E-FONT`): `Key`; `Key different font points`; `Key different font DPI`.
- `src/font/SharedGridSet.zig` — Upstream internal: `SharedGridSet`.
- `src/font/discovery.zig` — Browser product: `descriptor hash`; `descriptor hash family names`; `fontconfig`; `fontconfig codepoint`; `coretext`; `coretext codepoint`; `coretext sorting`; `windows`.
- `src/font/face.zig` — Upstream internal: `<unnamed@96>`.
- `src/font/face.zig` — Gap: `Variation.Id: wght should be 2003265652`; `Variation.Id: slnt should be 1936486004`; arbitrary font variations are not exposed.
- `src/font/face/coretext.zig` — Browser product: `<unnamed@970>`; `name`; `emoji`; `in-memory`; `variable`; `variable set variation`; `svg font table`; `glyphIndex colored vs text`.
- `src/font/face/freetype.zig` — Browser product: `face name`; `<unnamed@1111>`; `color emoji`; `mono to bgra`; `svg font table`; `bitmap glyph`; `bitmap glyph BDF`; `bitmap glyph PCF`; `bitmap glyph OTB`.
- `src/font/glyf_rasterize.zig` — Gap: `<unnamed@457>`; `glyf_rasterize: empty outline returns empty bitmap`; `glyf_rasterize: square fills bitmap center`; `glyf_rasterize: quadratic contour renders`; `glyf_rasterize: consecutive off-curve points render`; `glyf_rasterize: units per em controls baseline scale`; `glyf_rasterize: degenerate outline returns full empty bitmap`; `glyf_rasterize: contour can start off curve with final on curve point`; `glyf_rasterize: contour can start with implied midpoint`; `glyf_rasterize: multiple contours render independently`; `glyf_rasterize: non-zero bearings preserve declared whitespace`; `glyf_rasterize: negative y coordinates descend below baseline`; `glyf_rasterize: two-cell bitmap and constraint render within width`; `glyf_rasterize: line height does not change unconstrained em scale`.
- `src/font/glyf_rasterize_png_test.zig` — Gap: `glyf_rasterize: bubbletea glyph protocol examples match reference image`; Ghostty glyph graphics are disabled in the packaged WASM and unsupported by the renderer.
- `src/font/main.zig` — Upstream internal: `<unnamed@75>`.
- `src/font/opentype.zig` — Upstream internal: `<unnamed@17>`.
- `src/font/opentype/glyf.zig` — Browser product: `glyf`; `glyf: decode triangle`; `glyf: decode multiple contours`; `glyf: decode repeat and short vector flags`; `glyf: decode off curve and same coordinate flags`; `glyf: decode one-point contour`; `glyf: decode contour ending at max point index`; `glyf: reject glyphs with instructions and composite glyphs`; `glyf: reject truncated`; `glyf: reject endpoints out of order`; `glyf: reject too many points`; `glyf: zero-contour glyph can be header-only`; `glyf: zero-contour glyph can include instruction length`; `glyf: zero-contour glyph rejects instructions`.
- `src/font/opentype/head.zig` — Browser product: `head`.
- `src/font/opentype/hhea.zig` — Browser product: `hhea`.
- `src/font/opentype/os2.zig` — Browser product: `OS/2`.
- `src/font/opentype/post.zig` — Browser product: `post`.
- `src/font/opentype/sfnt.zig` — Browser product: `FixedPoint`; `parse font`; `get table`.
- `src/font/opentype/svg.zig` — Browser product: `SVG`.
- `src/font/shape.zig` — Upstream internal: `<unnamed@96>`.
- `src/font/shaper/Cache.zig` — Covered (`E-FONT`): `Cache`.
- `src/font/shaper/coretext.zig` — Covered (`E-SHAPE`, `E-FONT`): `shape inconsolata ligs`; `shape monaspace ligs`; `shape emoji width`; `shape emoji width long`; `shape Combining characters`; `shape selection boundary`; `shape cursor boundary`; `shape cursor boundary and colored emoji`; `shape cell attribute change`; `shape nerd fonts`.
- `src/font/shaper/coretext.zig` — Browser product: `shape`; `shape U+3C9 with JB Mono`; `shape variation selector VS15`; `shape variation selector VS16`.
- `src/font/shaper/coretext.zig` — Gap: `run iterator`; `run iterator: empty cells with background set`; `shape left-replaced lig in last run`; `shape left-replaced lig in early run`; `shape with empty cells in between`; `shape Devanagari string`; `shape Tai Tham vowels (position differs from advance)`; `shape Tai Tham letters (position.y differs from advance)`; `shape Javanese ligatures`; `shape Chakma vowel sign with ligature (vowel sign renders first)`; `shape Bengali ligatures with out of order vowels`; `shape box glyphs`; `shape high plane sprite font codepoint`.
- `src/font/shaper/feature.zig` — Gap: `Feature.fromString`; `FeatureList.fromString`; only the font-ligature boolean is exposed.
- `src/font/shaper/harfbuzz.zig` — Covered (`E-SHAPE`): `shape inconsolata ligs`; `shape monaspace ligs`; `shape emoji width`; `shape emoji width long`; `shape Combining characters`; `shape selection boundary`; `shape cursor boundary`; `shape cursor boundary and colored emoji`; `shape cell attribute change`.
- `src/font/shaper/harfbuzz.zig` — Browser product: `shape`; `shape variation selector VS15`; `shape variation selector VS16`.
- `src/font/shaper/harfbuzz.zig` — Gap: `run iterator`; `run iterator: empty cells with background set`; `shape arabic forced LTR`; `shape with empty cells in between`; `shape Devanagari string`; `shape Tai Tham vowels (position differs from advance)`; `shape Tibetan characters`; `shape Tai Tham letters (run_offset.y differs from zero)`; `shape Javanese ligatures`; `shape Chakma vowel sign with ligature (vowel sign renders first)`; `shape Bengali ligatures with out of order vowels`; `shape box glyphs`.
- `src/font/shaper/noop.zig` — Upstream internal: `<unnamed@141>`.
- `src/font/sprite.zig` — Upstream internal: `<unnamed@37>`; `<unnamed@43>`.
- `src/font/sprite/Face.zig` — Gap: `sprite face render all sprites`; `full height cursor sprites respect cursor height metric`; `<unnamed@664>`.

## Search manifest

- `src/terminal/search/Thread.zig` — Upstream internal: `<unnamed@848>`; Orchbee search is synchronous and has no worker-owned queue.
- `src/terminal/search/active.zig` — Covered (`E-SEARCH`): `simple search`; `clear screen and search`.
- `src/terminal/search/pagelist.zig` — Covered (`E-SEARCH`): `simple search`; `feed multiple pages with matches`; `feed multiple pages no matches`; `feed iteratively through multiple matches`; `feed with match spanning page boundary`; `feed with match spanning page boundary with newline`.
- `src/terminal/search/pagelist.zig` — Upstream internal: `feed with pruned page`.
- `src/terminal/search/screen.zig` — Covered (`E-SEARCH`): `simple search`; `simple search with history`; `reload active with history change`; `active change contents`; `select next`; `select in active changes contents completely`; `select into history`; `select prev`; `select prev then next`; `select prev with history`; `select prev wraps when all matches are in history`; `select after all matches disappear drops the selection`; `screen search no scrollback has no history`; `select after clearing scrollback`.
- `src/terminal/search/screen.zig` — Upstream internal: `reloadActive partial history cleanup on appendSlice error`; `reloadActive partial history cleanup on loop append error`.
- `src/terminal/search/sliding_window.zig` — Covered (`E-SEARCH`): `SlidingWindow empty on init`; `SlidingWindow single append`; `SlidingWindow single append case insensitive ASCII`; `SlidingWindow single append single char`; `SlidingWindow single append no match`; `SlidingWindow two pages`; `SlidingWindow two pages single char`; `SlidingWindow two pages match across boundary`; `SlidingWindow two pages no match across boundary with newline`; `SlidingWindow two pages no match across boundary with newline reverse`; `SlidingWindow single append match on boundary`; `SlidingWindow single append reversed`; `SlidingWindow single append no match reversed`; `SlidingWindow two pages reversed`; `SlidingWindow two pages match across boundary reversed`; `SlidingWindow single append match on boundary reversed`; `SlidingWindow single append soft wrapped`; `SlidingWindow single append reversed soft wrapped`; `SlidingWindow append whitespace only node`.
- `src/terminal/search/sliding_window.zig` — Upstream internal: `SlidingWindow two pages no match prunes first page`; `SlidingWindow two pages no match keeps both pages`; `SlidingWindow single append across circular buffer boundary`; `SlidingWindow two pages no match prunes first page reversed`; `SlidingWindow two pages no match keeps both pages reversed`; `SlidingWindow single append across circular buffer boundary reversed`.
- `src/terminal/search/viewport.zig` — Covered (`E-SEARCH`): `simple search`; `clear screen and search`; `clear screen and search dirty tracking`; `history search, no active area`.

## Surface and Swift manifest

- `src/surface_mouse.zig` — Covered (`E-MOUSE`): `keyToMouseShape`, including its 13 state-table scenarios on macOS and other platforms.
- `src/apprt/gtk/class/surface.zig` — Application layer: `computeFraction`; this is GTK file-transfer progress.
- `macos/Tests/Ghostty/Surface View/SurfaceView+SearchStateTests.swift` — Application layer: `init_withNilNeedle_readsPasteboardNeedle`; `init_withEmptyNeedle_readsPasteboardNeedle`; `init_withNeedle_setsNeedle`; `init_withNeedle_writesPasteboard`; `writePasteboardNeedle_writesPasteboard`; `readPasteboardNeedle_whenPasteboardNeedleIsNil`; `readPasteboardNeedle_whenPasteboardNeedleIsValid`; `readPasteboardNeedle_setsNeedleSelectionRange`.
- `macos/Tests/Ghostty/SurfaceViewAppKitTests.swift` — Covered (`E-IME`): `suppressesOnlySingleC0ControlTextWhileComposing`; `doesNotSuppressControlTextWhenNotComposing`; `doesNotSuppressMissingText`. The parameterized declaration expands to eight cases, so all ten runtime cases are represented.
- `macos/GhosttyUITests/GhosttyMouseStateTests.swift` — Covered (`E-SELECT`): `testSelectionFocusChange`.
- `macos/GhosttyUITests/GhosttyMouseStateTests.swift` — Application layer: `testSearchFocusState`.

## Remaining capability gaps

| Capability                             | Missing declarations | Current limit                                                                            |
| -------------------------------------- | -------------------: | ---------------------------------------------------------------------------------------- |
| Shadertoy/custom shaders               |                    4 | No shader API or SPIR-V translation                                                      |
| Font variations and arbitrary features |                    4 | Only family, size, style flags, and ligature enablement exist                            |
| Glyph graphics rasterization           |                   15 | Packaged WASM and renderer advertise no glyph graphics                                   |
| General complex-script shaping runs    |                   26 | Only bounded programming ligatures and per-grapheme Canvas rasterization are implemented |
| Built-in Ghostty sprite font           |                    3 | Box drawing and cursor sprites depend on browser fonts                                   |
| Total                                  |                   52 |                                                                                          |

Kitty graphics are also absent because Ghostty disables them for `wasm32-freestanding`; no scoped upstream renderer/font declaration measures that additional capability gap.

## Upgrade verification

1. Compare the target Ghostty commit and packaged WASM SHA-256 with this baseline.
2. Run `pnpm audit:ghostty-panel` from the repository root with Ghostty checked out at `../ghostty`, or set `GHOSTTY_SOURCE` to its root.
3. Run `pnpm generate:ghostty-panel-data` when the upstream Nerd Font table changes; the audit rejects stale generated output.
4. The audit must enumerate `test {}`, `test name {}`, and `test "name" {` syntax and reject missing, duplicate, or stale mappings.
5. Port assertions only for Orchbee-owned behavior; retain browser-product, internal, and application exclusions with an explicit reason.
6. Run unit, Node integration, browser surface, typecheck, build, lint, format, and artifact checks.
