# Ghostty Projected Surface Renderer Conformance

## Scope

Audit the projected client renderer against the pinned Ghostty renderer, font, and shaper sources.
The audit covers pixel geometry, glyph constraints, shaping boundaries, cursor presentation, and
other browser-renderer behavior that has a direct analogue.

It explicitly excludes VT parsing, PTY bytes, terminal modes, history and selection authority,
semantic input encoding, search ownership, windows, tabs, splits, updates, and other application
UI. Those exclusions are architectural boundaries: Ghostty VT authority lives only in
`apps/terminal-host`, while this package consumes packed Host projections.

| Baseline       | Value                                      |
| -------------- | ------------------------------------------ |
| Verified       | 2026-08-02                                 |
| Ghostty commit | `91f66da24527fa02d92b5fd0b41cd020f553a64c` |

`pnpm audit:ghostty-panel` reports the authoritative declaration and disposition counts for the
checked Ghostty source. The audit accepts `test {}`, `test name {}`, and `test "name" {}` and
rejects missing, duplicate, stale, or out-of-scope manifest mappings.

## Dispositions

| Disposition       | Coverage treatment                                                                  |
| ----------------- | ----------------------------------------------------------------------------------- |
| Covered           | Applicable declaration with maintained projected-renderer evidence                  |
| Gap               | Applicable observable renderer capability absent or incomplete                      |
| Browser product   | Exact native algorithm replaced by `FontFaceSet`, Canvas text, or WebGPU            |
| Upstream internal | Allocator injection, native cache/container, or page-layout detail with no analogue |
| Application layer | Native configuration or product behavior outside the projected surface              |

Declaration counts are provenance metrics, not user-visible feature percentages.

## Orchbee evidence

| Code       | Evidence                                                                                                            |
| ---------- | ------------------------------------------------------------------------------------------------------------------- |
| `E-RENDER` | `src/rendering/__tests__/renderer.unit.test.ts` and projected-terminal integration tests                            |
| `E-FIT`    | `src/__tests__/surface-size.unit.test.ts`                                                                           |
| `E-SHAPE`  | `src/__tests__/buildTextShapingPlan.unit.test.ts` and projected renderer tests                                      |
| `E-LINK`   | `src/__tests__/terminal-row-links.unit.test.ts`, projected-terminal tests, and projected renderer tests             |
| `E-IME`    | `src/input/__tests__/preedit.unit.test.ts` and projected-terminal tests                                             |
| `E-SELECT` | `src/projected/__tests__/projected-terminal.unit.test.ts` and projected renderer tests                              |
| `E-FONT`   | font-metrics, glyph-constraints, Ghostty sprites, generated constraint-table, shaping, and projected renderer tests |

Each declaration below has exactly one disposition. `<unnamed@line>` identifies an unnamed
upstream test at the pinned commit.

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
- `src/font/CodepointResolver.zig` — Gap: `getIndex box glyph`; Orchbee routes a bounded sprite subset but does not implement the complete Ghostty sprite-face resolver.
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
- `src/font/glyf_rasterize_png_test.zig` — Gap: `glyf_rasterize: bubbletea glyph protocol examples match reference image`; Ghostty glyph graphics are not represented by the Host projection format and remain unsupported by the client renderer.
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
- `src/font/sprite/Face.zig` — Gap: `sprite face render all sprites`; `full height cursor sprites respect cursor height metric`; `<unnamed@664>`. Orchbee procedurally renders a bounded box, block, triangle, and Powerline subset, but not all sprites or sprite cursors.

## Maintained limits

- Shadertoy and custom shader translation are not exposed.
- Arbitrary font variations and OpenType feature configuration are not exposed.
- Glyph graphics are not present in the Host projection format.
- General complex-script shaping is delegated to browser text rasterization; only bounded
  terminal-cell shaping behavior is claimed.
- The procedural Ghostty sprite subset is intentionally incomplete.

Kitty graphics, VT behavior, and terminal query responses are outside this renderer audit. Their
absence here is not evidence about Terminal Host behavior.

## Upgrade verification

1. Compare the target Ghostty commit with the Terminal Host pin.
2. Run `pnpm audit:ghostty-panel` with Ghostty checked out at `../ghostty`, or set
   `GHOSTTY_SOURCE`.
3. Run `pnpm generate:ghostty-panel-data` when the upstream Nerd Font table changes.
4. Review every changed upstream declaration and update exactly one manifest disposition.
5. Add or revise projected-renderer evidence before marking an applicable declaration covered.
6. Run the terminal package checks and the repository merge gate.
