# Ghostty Projected Surface Renderer Conformance

## Scope

Audit the projected client renderer against the pinned Ghostty renderer, font, and shaper sources.
The audit covers pixel geometry, glyph constraints, shaping boundaries, cursor presentation,
projected Kitty image composition, and other browser-renderer behavior that has a direct analogue.

It explicitly excludes VT parsing, PTY bytes, terminal modes, history and selection authority,
semantic input encoding, search ownership, windows, tabs, splits, updates, and other application
UI. Those exclusions are architectural boundaries: Ghostty VT authority lives only in
`apps/terminal-host`, while this package consumes packed Host projections.

| Baseline       | Value                                      |
| -------------- | ------------------------------------------ |
| Verified       | 2026-08-25                                 |
| Ghostty commit | `9c3ec931d64561a8407dde7ac984ce156ae91539` |

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

| Code         | Evidence                                                                                                                                  |
| ------------ | ----------------------------------------------------------------------------------------------------------------------------------------- |
| `E-RENDER`   | `src/rendering/__tests__/WebGPURenderer.unit.test.ts` and projected-terminal integration tests                                            |
| `E-FIT`      | `src/__tests__/surface-size.unit.test.ts`                                                                                                 |
| `E-SHAPE`    | `src/__tests__/buildTextShapingPlan.unit.test.ts` and projected renderer tests                                                            |
| `E-LINK`     | `src/__tests__/terminal-row-links.unit.test.ts`, projected-terminal tests, and projected renderer tests                                   |
| `E-IME`      | `src/input/__tests__/preedit.unit.test.ts` and projected-terminal tests                                                                   |
| `E-SELECT`   | `src/projected/__tests__/projected-terminal.unit.test.ts` and projected renderer tests                                                    |
| `E-FONT`     | font metrics, glyph constraints, complete Ghostty sprite routing, generated raster/constraint data, shaping, and projected renderer tests |
| `E-GRAPHICS` | strict OBKG/OBRW stores, `src/rendering/__tests__/WebGPURenderer.unit.test.ts`, and Terminal Host projection/animation differential tests |
| `E-ENV`      | exact headed `terminal-parity/CF-03-visual-environment` system witness and each named visual witness's asserted controls and limits       |

Each declaration below has exactly one disposition. `<unnamed@line>` identifies an unnamed
upstream test at the pinned commit.

## Controlled visual system environment

The stable headed `terminal-parity/CF-03-visual-environment` witness owns the common synthetic
Canvas evidence baseline. It registers the repository's product JetBrains Mono and Symbols Nerd
Font assets through the production application loader, forces Canvas 2D at the renderer capability
boundary, and asserts an exact `900x500` Chromium viewport, DPR `1`, `en-US` document and browser
locale, UTC time zone, light color scheme, and reduced-motion preference. The witness waits for the
font set and requires the selected product text face to be loaded before accepting the rendered
surface.

Evidence with a deliberate domain difference states and asserts that difference at its named
witness: the DPR trace changes only device metrics, visibility traces control their own animation
time, and WebGPU tests use deterministic owner-local adapters. Assertions compare exact semantic
state, geometry, ordering, colors, or bytes; there is no screenshot threshold or normalization that
may erase a semantic difference. This baseline does not claim identical operating-system
antialiasing, native-monitor transfer, WebGPU driver behavior, another browser engine, or native OS
effects.

## Provisioned product fonts

`@orchbee/terminal` keeps `monospace` as its source-neutral default. The shared Web application
registers these unmodified Ghostty font files and accompanying license texts before it constructs
any projected surface:

| Ghostty dependency            | Product face                         | Application assets                                                                            |
| ----------------------------- | ------------------------------------ | --------------------------------------------------------------------------------------------- |
| JetBrains Mono 2.304 variable | `Orchbee JetBrains Mono`             | `JetBrainsMono-Variable.ttf`, `JetBrainsMono-VariableItalic.ttf`, and `JetBrainsMono-OFL.txt` |
| Nerd Fonts Symbols Only 3.4.0 | `Orchbee Symbols Nerd Font` fallback | `SymbolsNerdFont-Regular.ttf` and `SymbolsNerdFont-LICENSE.txt`                               |

The files live under
`apps/web/src/workbench-client/projected-terminal-renderer/fonts/`. `FontFaceSet` preparation
includes U+F120 so initial Canvas and WebGPU draws cannot race the symbol fallback. Browser system
fonts remain responsible for unsupported CJK and emoji only after the provisioned stack. The
existing `pnpm audit:ghostty-panel` command verifies the Ghostty dependency versions and SHA-256 of
every font and license asset.

The shared Web product fixes its initial browser size at 13 CSS pixels to match Ghostty's default
13-point setting on the calibrated macOS scale. Every Canvas font context requests
`geometricPrecision`; Chromium's default `auto` legibility hinting otherwise thickens the
small-size raster even though the selected JetBrains Mono outline and weight are the same.
Ghostty's pinned CoreText path enables antialiasing and subpixel positioning but leaves font
smoothing disabled when `font-thicken = false`.

A native macOS 2x calibration against ZeroGH at the pinned Ghostty commit separates outline choice
from target-size rasterization. A true 10x render of regular `A` at 130 CSS pixels in Chromium and
130 points in ZeroGH produced the same 130-by-190-pixel ink bounds and 9,462.10 versus 9,469.57
pixels of alpha coverage. At the real size-13 setting, regular `1.jpg` measured 479.88 pixels under
Chromium `auto`, 369.71 after `geometricPrecision`, and 369.76 in ZeroGH. The product keeps the
size-13 raster as the release criterion; the 10x render is diagnostic evidence that the face,
outline, and weight agree.

The renderer samples the selected browser face at 1,000 CSS pixels, scales those unrounded ratios
back to the configured size, and rounds its baseline at the current physical device-pixel ratio.
The `rounds sampled font metrics at the physical pixel scale like Ghostty` regression covers the
one-physical-pixel baseline error that results from rounding at CSS scale before high-DPI raster
scaling. Cell width and height remain stable CSS geometry across a DPR transition.

The browser run planner keeps multi-cell Host graphemes outside natural-advance shaping runs. Each
wide grapheme is anchored to its projected head cell, its spacer tail remains non-painting, and a
following narrow complex run starts normally. The `anchors wide fallback graphemes to their Host
cells and resumes shaping after tails` regression covers both cumulative fallback drift and the
lost post-emoji combining run. Narrow-cell runs add the fractional face-to-grid advance difference
as Canvas letter spacing at the active raster scale. Selection and search overlays preserve that
run and recolor clipped cell-aligned portions instead of reshaping independently; the
`keeps grid-fitted shaping intact across selection boundaries` and `keeps a shaped run at the same
horizontal origin when selection recolors it` regressions cover the Canvas and WebGPU paths. The
WebGPU atlas also keeps device-aligned textured quads while rasterizing the horizontal crop
remainder; `keeps WebGPU run baselines on the cell anchor across cursor splits` covers a cursor
split whose new first glyph has a different signed ink bearing.

## Projected Kitty graphics

Pinned Ghostty remains the state owner for decoded images, frame selection, virtual and relative
placements, source intersection, and z ordering. Terminal Host resolves that state into bounded
OBKG `0.1.0` owned pixels and absolute placements. No path, file descriptor, shared-memory name or
mapping, compressed stream, encoded container, or unresolved pin reaches this package.

The packed store commits image resources, placements, resource revision, and monotonic animation
tick atomically. Canvas2D and WebGPU cache one converted resource per image generation, use the
intersected source rectangle and destination geometry, and paint at three matching seams:
`z < -1073741824` below cell backgrounds, `-1073741824 <= z < 0` between backgrounds and text, and
`z >= 0` above text. Replacement and failure paths cannot retain an obsolete Canvas or GPU
resource. Render-detail OBRW `0.1.0` carries an independent complete graphics snapshot rebased to
the selected history window; text detail carries none.

The upstream `src/renderer/image.zig` declarations map to the Host projection and client renderer
together: source intersection and relative/virtual placement are resolved before the wire;
pending/replaced images are covered by atomic store replacement and resource cleanup; current
animation frames are covered by Host generation/tick evidence plus generation-keyed renderer
caches. The package renderer never becomes a second Kitty parser or animation state machine.

## Host-derived projection corpus

`schemas/terminal/v1/fixtures/ghostty-surface.bin`,
`ghostty-alternate-surface.bin`, and `ghostty-render-row-window.bin` form one
Host-owned native surface evidence set. `apps/terminal-host` generates the OBTF pair from one pinned
native Ghostty VT and one Host Projection: it feeds the primary stream, captures its snapshot,
enters the alternate screen on the same terminal, and captures the second snapshot. A separate
native terminal replays the primary stream, appends DECSCA protected and restored-unprotected
sentinels, and generates the render-detail OBRW through the production `row_encoding.encode` to
`projection_buffer.encodeRenderSnapshot` path. No TypeScript encoder constructs any of the three.

Terminal Host verification regenerates and byte-compares all three fixtures. The projected
renderer test decodes the primary OBTF bytes for Canvas and WebGPU evidence, then commits both OBTF
snapshots through the packed store for transition evidence. The OBRW consumer independently fixes
the complete six-row by eight-cell oracle: row indices, flags, prompt classes, grapheme and
hyperlink strings plus raw byte ranges, all three color columns, every style flag and underline
kind, width topology, semantic contents, protected flags, and commit metadata. It requires strict
decode and trusted structural view to match that same matrix, commits the trusted frame through
`replaceProjectionBufferSnapshot`, and checks every store row against the same semantic columns.
Wire-valid in-memory mutations cover foreground/background exchange, a supported style change, a
supported semantic change, protected-cell clearing, canonical hyperlink removal, coordinated row
wrap, and valid width-topology replacement.

The corpus enables Ghostty grapheme-cluster mode `2027`, then contains narrow text, CJK, a
combining sequence, and a modifier-plus-ZWJ emoji inside one OSC 8 link. It places the final linked
wide grapheme at the right edge so Ghostty emits a wrapped spacer head followed by the wide head
and spacer tail on the next row. The link then closes before an explicit hard line break and one
final unlinked cell. Two more hard rows set non-black dynamic defaults and exercise explicit
foreground/background colors, inverse, faint, invisible, blink, overline, strikethrough, all five
underline kinds, an independent underline color, bold, and italic. The shared assertions therefore
cover these boundaries:

- Ghostty owns grapheme segmentation and terminal width.
- Host projection preserves each complete grapheme as one string and retains narrow, wide,
  spacer-tail, and wrapped spacer-head topology.
- The TypeScript decoder and packed store preserve the exact bytes and width codes.
- Canvas paints each non-spacer grapheme once; it never paints a spacer cell as an independent
  glyph.
- Search joins the soft-wrapped rows but does not cross the later hard boundary unless explicitly
  asked to span lines.
- Link detection preserves one OSC 8 range across the soft wrap and stops it before the hard line.
- Accessibility text and cell-offset mappings retain every physical row while the same row flags
  continue to distinguish soft and hard boundaries.
- Ghostty owns SGR parsing and style combinations; Host, the decoder, and the packed store preserve
  the resulting color encodings, flags, and underline kinds without reconstructing SGR state.
- Canvas and WebGPU consume the same style rows, including explicit RGB zero, default colors,
  independent underline color, hidden text, blink phases, and decoration geometry.

The three-fixture set is deterministic conformance evidence, not a second VT implementation or a
product sample. Terminal Host owns native generation, `@orchbee/terminal-protocol` owns tracked
bytes and exports, and this package owns consumer semantics. After a reviewed corpus, Ghostty pin,
Host encoder, OBTF/OBRW/OBPB format, decoder, packed-store, or draw-contract change, run
`pnpm --filter @orchbee/terminal-host surface-fixtures:generate` and review both producer and
consumer assertions. The set proves this bounded producer-to-consumer mapping; it does not prove
PTY delivery, history-anchor behavior, lifecycle policy, another platform's runtime, or a broader
trusted transport.

## Packed style projection

One packed cell presents the Ghostty style tuple `(foreground, background, underline color, style
flags, underline kind)`. RGB zero is a valid explicit color; `default` and `none` remain distinct
tagged encodings and must never be inferred from an RGB value. The client does not retain an SGR
state machine.

Canvas and WebGPU own equivalent presentation rules:

- only `TerminalColorEncoding.default` resolves through the client-effective default foreground or
  background; explicit RGB (including black), Host-projected palette RGB, and `none` retain their
  Host-projected behavior;
- inverse exchanges the resolved foreground and background before selection or search overrides;
- invisible and the hidden blink phase suppress text and decorations while preserving the cell
  background;
- faint applies half alpha to text and decorations, never to the background;
- single, double, curly, dotted, and dashed underlines retain their distinct geometry, and an
  explicit underline color overrides the effective foreground only for that underline;
- strikethrough and overline use the effective foreground and the same faint alpha;
- bold and italic affect glyph rasterization in both backends; and
- glyph telemetry counts submitted or painted glyphs, not hidden graphemes.

Changing surface transparency is a composition-policy change. It requests a render and forces one
complete Canvas repaint even when the packed store has no dirty rows, so explicit backgrounds are
preserved while the default background fill is removed. `src/projected/__tests__/ghostty-surface-parity.unit.test.ts`
owns the native-to-both-backends regression for these rules.

The same checked-in native fixture is decoded by `src/search/__tests__/terminal-search.unit.test.ts` and passed to the real search visitor to verify soft wraps, hard boundaries, and cross-line search. The projected owner retains Store and Canvas/WebGPU field parity.

## Packed presentation colors

The launch profile seeds Ghostty's default foreground, background, cursor color, and 256-color
palette. OSC 4, 10, 11, and 12 mutate that same Ghostty state. Host projection resolves palette
cell colors to RGB and publishes the current default foreground, background, and optional cursor
color in every frame. The packed client never reconstructs a palette or replaces those terminal
colors from `ITheme`.

Host cell projection preserves OSC 10/11 override provenance as explicit RGB, including values
equal to the configured defaults and colors on blank cells. Untouched or reset channels keep
default tokens; active DECSCNM materializes both effective swapped defaults. The maintained
fixtures and renderer expectations preserve these tokens exactly. An OSC background therefore
keeps otherwise blank rows visible to the conservative trailing-blank-row trimming policy and
remains painted after transparent clear. No RGB-equality normalization turns it back into a
default token. Reset restores ordinary default-token behavior.

An optional client `defaultColorOverride` is a narrower display projection, not a second terminal
color authority: it affects only packed `TerminalColorEncoding.default` foreground/background
tokens. It does not mutate or replace Host metadata, palette-projected RGB, VT state, OSC query
replies, or the Host cursor color. The renderer resolves each original foreground/background role
before applying inverse; default underline colors use the effective foreground, while an explicit
underline color remains explicit. The shared Orchbee Web/Desktop composition does not use this
compatibility projection: it keeps `defaultColorOverride` null and applies Light, Dark, or Custom
colors through the complete Host runtime-theme transaction.

Application presentation remains a separate, narrower layer:

- selection and active/inactive search colors come from the client theme;
- an explicitly configured cursor accent controls text painted inside a block cursor, while the
  Host background is the fallback accent;
- a missing Host cursor color falls back to the Host foreground;
- Canvas stays transparent before its first frame so the owning container supplies the loading
  surface, then opaque resize and clear reuse the latest effective default background;
- `allowTransparency` controls only renderer clear. `backgroundOpacity` remains a complete
  source-neutral configuration value for the composing terminal root, whose alpha affects neither
  glyphs nor explicit cell backgrounds; and
- the Workbench terminal root adopts each committed effective default background.

The native corpus sets OSC 4, 10, 11, and 12 values and proves the resulting metadata, resolved
palette cell, cursor paint, Canvas output, and WebGPU uploads. The Host runtime additionally sends
the matching OSC queries through the pinned Ghostty stream and verifies the exact replies; its
separate real-PTY trace verifies terminal-reply routing and accounting without relying on a
fixed-length color-reply read. Renderer unit tests prove that legacy terminal-color theme fields
cannot become a second packed palette and that application overlay colors remain configurable.

Every OBTF/OBPB projection also carries the Host theme revision captured with its rows and
metadata. `ProjectedTerminal` prepares overlay and cursor resources before the Host request, stages
incoming frames in a private packed store, and admits publication only when that store names the
exact committed revision. The application barrier synchronously preflights every mounted terminal,
commits their staged store and overlay state, and then changes the application design-token theme in
the same JavaScript task. A renderer preparation failure sends no Host mutation; superseded,
hidden, suspended, or mismatched stages cannot expose a mixed revision. The projected-terminal unit
traces cover exact commit, resume-latest, and renderer-failure preservation, while the Web
system witness `terminal-parity/SU-09-theme` observes the real runtime-presentation fixture holding the Host result
and revision-tagged frame before one complete release.

## Packed links and semantic boundaries

Ghostty owns both explicit hyperlinks and shell-integration semantics. OSC 8 targets enter the Host
style/string tables, while OSC 133 supplies each cell's `prompt`, `input`, or `output` semantic
content and each row's prompt class. OBTF and OBRW preserve those fields directly; the browser does
not parse either escape sequence.

The native corpus carries one OSC 8 link across a soft wrap, then crosses OSC 133 prompt, input, and
output boundaries around a palette-colored cell in its final row. The TypeScript decoder and packed
store assertions verify the original target, row prompt class, and all three cell semantics. A
separate projected-terminal trace reads a generation/revision-pinned Host row window, resolves the
explicit link at the exact pointer cell, revalidates that cell on modifier-assisted click, and emits
one typed URL request. The Workbench composition trace then routes that request through its
allowlisted external-link Action and environment-owned effect.

Explicit OSC 8 ranges retain precedence over detected URL or path text. Source-neutral application
policy limits activation to `http`, `https`, and `mailto`; Host metadata never grants filesystem or
generic scheme authority. A separate, default-off local attachment capability, which no composition
supplies today, may admit a typed file target after the same generation/revision-pinned
revalidation.

The headed Canvas 2D witnesses `terminal-parity/FS-05-file-links` and
`terminal-parity/FS-06-url-policy` feed generation-pinned synthetic Host row windows through the
production projected surface, Workbench Action, and Browser effect. They observe ordinary
allowlisted detection plus hover, context, and direct activation; automatic FTP text; unsupported
explicit OSC 8 metadata; explicit file metadata; and a detected path. File and unsupported targets
produce no external effect. These witnesses control Browser composition and policy behavior; they
do not invoke an operating-system URL handler, parse OSC bytes, claim filesystem existence, or add
file-opening authority.

The headed Canvas 2D witnesses `terminal-parity/FS-19-local-file-links` and
`terminal-parity/FS-20-image-preview` enable that optional capability with a fake local opener and
synthetic validated PNG bytes. They observe relative-path resolution, modifier activation, a 300 ms
hover dwell, cancellation when the target changes, rejection of the first request's late completion,
and the production anchored pointer-transparent card for the second target. They do not invoke an
operating-system application, read a real file, automate Finder, or grant Browser authority.

## Packed metadata transition

Title, current directory, active screen, cursor state, interaction modes, generation, revision, and
rows belong to one projected commit. Host captures the terminal state once and writes these values
into the same OBTF record; the packed client stages the complete frame and replaces its metadata and
rows together. It never applies title or mode callbacks independently of the rows they describe.

The primary fixture sets OSC 2 and OSC 7 metadata, a hidden steady underline cursor, application
cursor/keypad modes, bracketed paste, focus reporting, button-motion plus SGR mouse reporting,
modifyOtherKeys level 2, and every projected Kitty keyboard flag. The paired stream enters the
alternate screen with DECSET 1049, changes both metadata strings, exposes a blinking bar cursor,
changes mouse reporting to any-motion plus SGR pixels, clears the other interaction modes, selects
two Kitty keyboard flags, and replaces the grid with `ALT`.

The Host runtime regression independently feeds the same transition through its reader commit seam.
It proves that DECSET 1049 advances the generation and makes the next projection a complete
snapshot containing the new metadata. The TypeScript regression applies the two native frames in
sequence and proves that the retained primary metadata object stays unchanged while the live store
switches all targeted fields and rows in one commit.

## Packed cursor projection

`PackedRendererMetadata.cursor` retains the canonical Host cursor coordinate. The renderer derives
one presentation range without changing that coordinate so input, accessibility, and Host state do
not acquire a second cursor owner.

| Input                                                         | Presentation column     | Presentation width                                          |
| ------------------------------------------------------------- | ----------------------- | ----------------------------------------------------------- |
| Cursor cell is narrow and `wideTail` is false                 | Cursor column           | One cell                                                    |
| Cursor cell is a wide-cell head and `wideTail` is false       | Cursor column           | Two cells                                                   |
| `wideTail` is true and the cursor column is greater than zero | Cursor column minus one | Two cells                                                   |
| Invalid `wideTail` at column zero                             | Column zero             | Width derived from the cursor cell, never a negative column |

The projected range owns these invariants:

- Canvas 2D and WebGPU use the same presentation column and width for block, hollow-block, bar, and
  underline cursors.
- A bar stays on the leading edge; block, hollow-block, and underline geometry spans the complete
  wide character.
- Text shaping treats the complete presentation range as a cursor boundary, so a ligature cannot
  cross either cell of a wide cursor.
- Cursor text inversion reads and clips the wide-cell head grapheme; it never attempts to render a
  spacer tail as an independent glyph.
- Canonical cursor coordinates remain unchanged for IME anchoring, input encoding, accessibility,
  and Host acknowledgements.

The reference is `src/terminal/render.zig` and `src/renderer/generic.zig` at the pinned Ghostty
commit. `src/rendering/__tests__/WebGPURenderer.unit.test.ts` owns deterministic Canvas and WebGPU
regressions. This pure presentation projection has no long-lived state, async transition, or TLA+
property; `PR-03` is complete only when both backends exercise a real wide-head/spacer-tail row.

## Renderer manifest

- `src/renderer/State.zig` — Covered (`E-RENDER`): `preedit range covers exact cell width`; `preedit range shifts left at right edge`.
- `src/renderer/cell.zig` — Covered (`E-RENDER`): `Contents`; `Contents resize grows and shrinks`; `Contents clear retains other content`; `Contents clear last added content`; `Contents with zero-sized screen`.
- `src/renderer/cell.zig` — Covered (`E-FONT`): `Cell constraint widths`.
- `src/renderer/cursor.zig` — Covered (`E-RENDER`): `cursor: default uses configured style`; `cursor: blinking disabled`; `cursor: explicitly not visible`; `cursor: always block with preedit`.
- `src/renderer/link.zig` — Covered (`E-LINK`): `renderCellMap`; `renderCellMap hover links`; `renderCellMap mods no match`.
- `src/renderer/link.zig` — Browser product: `renderCellMap inactive links don't allocate`; browser-owned link indexing and garbage collection replace the native allocation assertion.
- `src/renderer/image.zig` — Covered (`E-GRAPHICS`): `kitty renderer ignores pending payloads and removes replaced placements`; `kitty renderer uses the intersected source rectangle`; `kitty renderer positions relative placements from the parent pin`; `kitty renderer relative placement with negative offsets`; `kitty renderer positions relative placements from virtual parent placeholders`; `kitty renderer uploads the current animation frame`; Host resolves canonical relationships and the packed store plus both renderer backends preserve their bounded presentation.
- `src/renderer/metal/shaders.zig` — Browser product: `<unnamed@278>`; WebGPU shader compilation replaces Metal shader compilation.
- `src/renderer/shadertoy.zig` — Gap: `spirv`; `spirv invalid`; `shadertoy to msl`; `shadertoy to glsl`.
- `src/renderer/size.zig` — Covered (`E-FIT`): `Size.balancePadding equal distributes whitespace equally`; `Size.balancePadding true shifts excess top to bottom`; `Padding balanced on zero`.
- `src/renderer/size.zig` — Covered (`E-FIT`, `E-SELECT`): `GridSize update exact`; `GridSize update rounding`; `coordinate conversion`.

## Font manifest

- `src/font/Atlas.zig` — Covered (`E-RENDER`): `happy path`; `exact fit`; `doesn't fit`; `fit multiple`; `writing data`; `grow`; `writing BGR data`; `grow BGR`.
- `src/font/Atlas.zig` — Browser product: `writing data from a larger source`; Canvas rasterization supplies the source crop.
- `src/font/Atlas.zig` — Upstream internal: `grow OOM`; `init error`; `reserve error`; `grow error`; Zig allocator failure injection has no browser equivalent.
- `src/font/CodepointMap.zig` — Browser product: `codepointmap`.
- `src/font/CodepointResolver.zig` — Browser product: `getIndex`; `getIndex disabled font style`.
- `src/font/CodepointResolver.zig` — Covered (`E-FONT`): `getIndex box glyph`; Orchbee routes every Unicode codepoint range declared by the pinned Ghostty sprite face before browser font fallback.
- `src/font/Collection.zig` — Upstream internal: `<unnamed@930>`.
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
- `src/font/face.zig` — Browser product: `Variation.Id: wght should be 2003265652`; `Variation.Id: slnt should be 1936486004`; numeric regular/bold weights and italic style use CSS font selection, so the browser owns variable-font axis identifiers. Arbitrary axes remain outside the complete eight-field presentation transaction.
- `src/font/face/coretext.zig` — Browser product: `<unnamed@971>`; `name`; `emoji`; `in-memory`; `variable`; `variable set variation`; `svg font table`; `glyphIndex colored vs text`.
- `src/font/face/freetype.zig` — Browser product: `face name`; `<unnamed@1112>`; `color emoji`; `mono to bgra`; `svg font table`; `bitmap glyph`; `bitmap glyph BDF`; `bitmap glyph PCF`; `bitmap glyph OTB`.
- `src/font/glyf_rasterize.zig` — Browser product: `<unnamed@459>`; `glyf_rasterize: empty outline returns empty bitmap`; `glyf_rasterize: square fills bitmap center`; `glyf_rasterize: quadratic contour renders`; `glyf_rasterize: consecutive off-curve points render`; `glyf_rasterize: units per em controls baseline scale`; `glyf_rasterize: degenerate outline returns full empty bitmap`; `glyf_rasterize: contour can start off curve with final on curve point`; `glyf_rasterize: contour can start with implied midpoint`; `glyf_rasterize: multiple contours render independently`; browser font rasterization replaces Ghostty's native outline rasterizer.
- `src/font/glyf_rasterize.zig` — Covered (`E-FONT`): `glyf_rasterize: non-zero bearings preserve declared whitespace`; `glyf_rasterize: negative y coordinates descend below baseline`; `glyf_rasterize: two-cell bitmap and constraint render within width`; `glyf_rasterize: line height does not change unconstrained em scale`; signed Canvas ink bounds, terminal baselines, and Ghostty-derived width constraints preserve the corresponding projected geometry.
- `src/font/glyf_rasterize_png_test.zig` — Gap: `glyf_rasterize: bubbletea glyph protocol examples match reference image`; Ghostty glyph graphics are not represented by the Host projection format and remain unsupported by the client renderer.
- `src/font/main.zig` — Upstream internal: `<unnamed@75>`.
- `src/font/opentype.zig` — Upstream internal: `<unnamed@17>`.
- `src/font/opentype/glyf.zig` — Browser product: `glyf`; `glyf: decode triangle`; `glyf: decode multiple contours`; `glyf: decode repeat and short vector flags`; `glyf: decode off curve and same coordinate flags`; `glyf: decode one-point contour`; `glyf: decode contour ending at max point index`; `glyf: reject glyphs with instructions and composite glyphs`; `glyf: reject truncated`; `glyf: reject endpoints out of order`; `glyf: reject too many points`; `glyf: zero-contour glyph can be header-only`; `glyf: zero-contour glyph can include instruction length`; `glyf: zero-contour glyph rejects instructions`.
- `src/font/opentype/head.zig` — Browser product: `head`.
- `src/font/opentype/hhea.zig` — Browser product: `hhea`.
- `src/font/opentype/os2.zig` — Browser product: `OS/2`.
- `src/font/opentype/post.zig` — Browser product: `post`.
- `src/font/opentype/sfnt.zig` — Browser product: `FixedPoint`; `parse font`; `get table`.
- `src/font/opentype/svg.zig` — Browser product: `SVG`; `SVG document list offset past end`; browser font loading and validation replace Ghostty's native SVG-table parser.
- `src/font/shape.zig` — Upstream internal: `<unnamed@96>`.
- `src/font/shaper/Cache.zig` — Covered (`E-FONT`): `Cache`.
- `src/font/shaper/coretext.zig` — Covered (`E-SHAPE`, `E-FONT`): `shape inconsolata ligs`; `shape monaspace ligs`; `shape emoji width`; `shape emoji width long`; `shape Combining characters`; `shape selection boundary`; `shape cursor boundary`; `shape cursor boundary and colored emoji`; `shape cell attribute change`; `shape nerd fonts`.
- `src/font/shaper/coretext.zig` — Browser product: `shape`; `shape U+3C9 with JB Mono`; `shape variation selector VS15`; `shape variation selector VS16`.
- `src/font/shaper/coretext.zig` — Covered (`E-SHAPE`): `run iterator`; `run iterator: empty cells with background set`; `shape left-replaced lig in last run`; `shape left-replaced lig in early run`; `shape with empty cells in between`; complete row-bounded style runs retain empty cells and stop at selection, cursor, preedit, hidden, constrained-symbol, colored-glyph, and sprite boundaries.
- `src/font/shaper/coretext.zig` — Browser product: `shape Devanagari string`; `shape Tai Tham vowels (position differs from advance)`; `shape Tai Tham letters (position.y differs from advance)`; `shape Javanese ligatures`; `shape Chakma vowel sign with ligature (vowel sign renders first)`; `shape Bengali ligatures with out of order vowels`; Canvas shapes the complete forced-LTR run instead of reproducing CoreText glyph placement.
- `src/font/shaper/coretext.zig` — Covered (`E-FONT`): `shape box glyphs`; `shape high plane sprite font codepoint`; the complete pinned Unicode sprite-face resolver bypasses browser font fallback.
- `src/font/shaper/feature.zig` — Application layer: `Feature.fromString`; `FeatureList.fromString`; the accepted presentation transaction exposes one font-ligature policy rather than arbitrary OpenType feature strings.
- `src/font/shaper/harfbuzz.zig` — Covered (`E-SHAPE`): `shape inconsolata ligs`; `shape monaspace ligs`; `shape emoji width`; `shape emoji width long`; `shape Combining characters`; `shape selection boundary`; `shape cursor boundary`; `shape cursor boundary and colored emoji`; `shape cell attribute change`.
- `src/font/shaper/harfbuzz.zig` — Browser product: `shape`; `shape variation selector VS15`; `shape variation selector VS16`.
- `src/font/shaper/harfbuzz.zig` — Covered (`E-SHAPE`): `run iterator`; `run iterator: empty cells with background set`; `shape arabic forced LTR`; `shape with empty cells in between`; complete row-bounded style runs are explicitly rasterized left-to-right.
- `src/font/shaper/harfbuzz.zig` — Covered (`E-SHAPE`, `E-FONT`): `shaper selects font for entire grapheme`; each Host-projected grapheme remains one shaping unit across browser font fallback.
- `src/font/shaper/harfbuzz.zig` — Browser product: `shape Devanagari string`; `shape Tai Tham vowels (position differs from advance)`; `shape Tibetan characters`; `shape Tai Tham letters (run_offset.y differs from zero)`; `shape Javanese ligatures`; `shape Chakma vowel sign with ligature (vowel sign renders first)`; `shape Bengali ligatures with out of order vowels`; Canvas replaces HarfBuzz glyph placement inside the bounded run.
- `src/font/shaper/harfbuzz.zig` — Covered (`E-FONT`): `shape box glyphs`; the complete pinned Unicode sprite-face resolver bypasses browser font fallback.
- `src/font/shaper/noop.zig` — Upstream internal: `<unnamed@141>`.
- `src/font/sprite.zig` — Upstream internal: `<unnamed@37>`; `<unnamed@43>`.
- `src/font/sprite/Face.zig` — Covered (`E-FONT`): `sprite face render all sprites`; deterministic generated data carries every declared Unicode sprite and all four complete padded odd/even cell-size and thickness reference rasters, while selected box, block, triangle, and Powerline shapes retain their procedural renderer between reference geometries.
- `src/font/sprite/Face.zig` — Covered (`E-RENDER`): `full height cursor sprites respect cursor height metric`; cursor geometry is renderer-owned and consumes the adjusted cursor-height metric rather than Unicode sprite data.
- `src/font/sprite/Face.zig` — Upstream internal: `<unnamed@666>`; compile-time declaration traversal has no browser analogue.

## Maintained limits

- Shadertoy and custom shader translation are not exposed.
- Arbitrary font variations and OpenType feature configuration are not exposed.
- Glyph graphics are not present in the Host projection format.
- General complex-script glyph placement is delegated to browser text rasterization; Orchbee owns
  complete row-bounded style runs, terminal boundaries, empty-cell retention, and forced-LTR
  direction.
- Unicode sprite coverage is complete for the pinned Ghostty ranges. Exact padded reference pixels
  and overhang offsets are generated for four upstream metric combinations; other physical
  geometries select the nearest reference and resample it, while selected procedural shapes
  continue to scale directly.

Kitty protocol parsing, VT behavior, and terminal query responses are outside this renderer audit;
the bounded Host-to-client graphics projection and composition described above are inside it. An
unmapped native behavior is not evidence about Terminal Host behavior.

## Upgrade verification

1. Compare the target Ghostty commit with the Terminal Host pin.
2. Run `pnpm audit:ghostty-panel` with a clean Ghostty checkout at the exact Terminal Host pin in
   `../ghostty`, or set `GHOSTTY_SOURCE` to that checkout root; the command rejects another HEAD or
   any staged, unstaged, or untracked source change.
3. Run `pnpm generate:ghostty-panel-data` when the upstream Nerd Font table, sprite ranges, or
   sprite reference rasters change.
4. Review every changed upstream declaration and update exactly one manifest disposition.
5. Add or revise projected-renderer evidence before marking an applicable declaration covered.
6. Run the terminal package checks and the repository merge gate.
