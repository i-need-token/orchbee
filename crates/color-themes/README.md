# Color Themes

`orchbee-color-themes` is the single authority for Orchbee color themes: the theme file schema,
the shipped theme data, the palette-to-role resolver, and the gates every shipped theme passes.
The [Worker](../../apps/orchbee-worker/README.md#terminal-launch-palette) takes each terminal's
launch palette from the device's selected theme slots, and the GUI's [generated files](#gui-files)
come from this crate. The native Client theme module is a planned consumer under the
[color-theme proposal](../../proposals/workbench/color-themes.md).

It is admitted as one deliberately independent contract: the Worker, the native Client, and the
GUI generator are separate applications that must read one theme authority, and applications never
import each other's source.

The crate does not choose which theme a device shows, store preferences, quantize colors, compose
styles or modifiers, localize theme names, or change the fixed Agent glyph colors.

## Public boundary

| Item                     | Guarantee                                                                                 |
| ------------------------ | ----------------------------------------------------------------------------------------- |
| `shipped()`              | Every shipped theme, in a stable order, each structurally valid                           |
| `orchbee(Polarity)`      | The Orchbee theme of that polarity, the default pair                                      |
| `select(Polarity, id)`   | The shipped theme `id` of that polarity; an unshipped or other-polarity id gives Orchbee  |
| `Theme`                  | Identity, declared polarity, `Canvas`, ANSI 0-15, and `Source`; read-only                 |
| `Theme::parse(&str)`     | Admits one [theme file](#theme-files), or a `ThemeFileError`; gates are not checked here  |
| `PaletteColor`           | A correctable color: the shipped `value()` and, when corrected, the replaced `original()` |
| `Roles::resolve(&Theme)` | Every role in the [resolver table](#resolver), with each floor met on `surface_chrome`    |
| `Rgb`                    | An 8-bit sRGB color; `u32::from` packs it as `0xRRGGBB`, `Display` writes `#rrggbb`       |

Theme files are embedded at build time and admitted on first use. An embedded file that fails to
parse panics; the gate test admits every embedded file, so a verified build cannot reach it.

## Theme files

`themes/<id>.toml` holds one theme; `src/catalog.rs` lists every shipped file. Unknown keys are
rejected, so a misspelled optional key fails instead of silently falling back to a derived color.

| Key                                                                                | Contract                                                                                                                                                 |
| ---------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `id`, `family`                                                                     | Kebab-case. `id` is unique among shipped themes; `family` pairs a light and a dark sibling                                                               |
| `name`                                                                             | The source's own name. Clients show a localized name keyed by `id`, never this field                                                                     |
| `polarity`                                                                         | `light` or `dark`; the gate test confirms it                                                                                                             |
| `[source]` `url`, `path`, `revision`, `license`, `copyright`                       | The `https` repository, the file that publishes the values, an immutable revision, the license identifier, and the copyright notice the license requires |
| `[canvas]` `background`, `foreground`, `cursor_text`                               | `#rrggbb`                                                                                                                                                |
| `[canvas]` `cursor`, `selection_background`, `selection_foreground`; `[ansi]` 0-15 | Correctable: `#rrggbb`, or `{ value, original }` when a gated correction replaced `original`. The two values must differ                                 |
| `[canvas]` `selection_background`, `selection_foreground`                          | Optional. Absent selection background is derived by the resolver; absent selection foreground is the foreground                                          |
| `[ansi]`                                                                           | `black`, `red`, `green`, `yellow`, `blue`, `magenta`, `cyan`, `white`, then the same eight with a `bright_` prefix                                       |

A correction moves OKLCH lightness only, away from the canvas, keeping hue and reducing chroma only
to stay in the sRGB gamut, and stays within the correction budget below. A theme that needs more is
rejected rather than rewritten. `original` is the source's value or, for a slot the source does
not publish, such as the Orchbee cursor, the value Orchbee shipped before the correction.

Every shipped family needs a [`THIRD_PARTY_NOTICES.md`](../../THIRD_PARTY_NOTICES.md) entry naming
the copyright holder, source, pinned revision, license, and use, before its file ships.

## Shipped themes

| Theme              | Values                                                                                   | Correction                                                                                      |
| ------------------ | ---------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------- |
| `orchbee-light`    | Orchbee canvas, cursor, and selection; VS Code terminal registry light ANSI              | Cursor `#57bef8` to `#2e9cd4`, 3:1                                                              |
| `orchbee-dark`     | Orchbee canvas, cursor, and selection; VS Code terminal registry dark ANSI (Dark Modern) | None                                                                                            |
| `gruvbox-dark`     | morhetz/gruvbox via the iTerm2-Color-Schemes Ghostty export                              | ANSI 1 `#cc241d` to `#d53026`, 3:1                                                              |
| `rose-pine`        | rose-pine/ghostty                                                                        | None                                                                                            |
| `catppuccin-mocha` | catppuccin/ghostty                                                                       | None                                                                                            |
| `flexoki-dark`     | kepano/flexoki via the iTerm2-Color-Schemes Ghostty export                               | None                                                                                            |
| `nightfox`         | EdenEast/nightfox.nvim via the iTerm2-Color-Schemes Ghostty export                       | None                                                                                            |
| `dayfox`           | EdenEast/nightfox.nvim via the iTerm2-Color-Schemes Ghostty export                       | None                                                                                            |
| `gruvbox-light`    | morhetz/gruvbox via the iTerm2-Color-Schemes Ghostty export                              | ANSI 2 `#98971a` to `#918f08`, 3 `#d79921` to `#b98000`, and 6 `#689d6a` to `#639765`, each 3:1 |
| `flexoki-light`    | kepano/flexoki via the iTerm2-Color-Schemes Ghostty export                               | ANSI 8 `#b7b5ac` to `#b6b5ac`, 2:1                                                              |
| `rose-pine-dawn`   | rose-pine/ghostty                                                                        | ANSI 3 `#ea9d34` to `#c67e00` and 6 `#d7827e` to `#ca7773`, each 3:1                            |

## Resolver

`Roles::resolve` derives every role from the declared polarity, the canvas, and the shipped ANSI
values. Mixes and lightness steps are in OKLab; a floor correction is the lightness-only correction
above, measured with WCAG 2 contrast on `surface_chrome`. A proportion may change only after the
gate test passes again.

| Role                                                | Rule                                                                                                                     |
| --------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------ |
| `surface-canvas`                                    | Theme background                                                                                                         |
| `surface-chrome`, `surface-strip`                   | Canvas lightness −0.025 on dark (+0.03 when canvas lightness is below 0.16, so near-black canvases lift), −0.02 on light |
| `surface-overlay`                                   | Canvas lightness +0.045 on dark, +0.02 on light; the canvas itself when its lightness exceeds 0.98                       |
| `surface-related`, `-hover`, `-current`, `-pressed` | Chrome mixed toward the foreground at 5, 8, 11, and 14%                                                                  |
| `frame-guide`, `frame-divider`, `frame-unfocused`   | Chrome mixed toward the foreground at 10, 18, and 24%                                                                    |
| `surface-field`                                     | Overlay mixed toward the foreground at 7% on dark, 5% on light                                                           |
| `surface-selection`                                 | Theme selection background; canvas mixed 30% toward ANSI 4 when absent                                                   |
| `text-primary`                                      | Theme foreground                                                                                                         |
| `text-secondary`                                    | Foreground mixed 28% toward chrome, raised to at least 4.5:1                                                             |
| `text-muted`                                        | The step from secondary toward chrome closest to chrome that still holds 4.5:1                                           |
| `accent-action`, `accent-focus`                     | ANSI 4, raised to at least 3:1                                                                                           |
| `accent-link`                                       | ANSI 4, raised to at least 4.5:1                                                                                         |
| `status-success`, `-warning`, `-error`, `-info`     | ANSI 2, 3, 1, and 6, each raised to at least 4.5:1                                                                       |
| `accent-destructive`                                | ANSI 1, raised to at least 4.5:1; a separate role from `status-error`                                                    |
| `status-unread`                                     | `accent-action`                                                                                                          |
| `status-working`, `status-idle`                     | `text-secondary` and `text-muted`                                                                                        |
| `agent-attention`, `agent-done`, `agent-error`      | Fixed `#febc2e`, `#28c840`, and `#ff5f57` in every theme                                                                 |

## Gates

Every shipped theme passes every gate. A failure within 5% of its threshold is reported as
borderline and still fails. Contrast is WCAG 2, `Lc` is APCA 0.0.98G-4g, ΔE is CIEDE2000, and
color-vision deficiency is Machado 2009 at severity 1.0.

| Gate                    | Threshold                                                                                          |
| ----------------------- | -------------------------------------------------------------------------------------------------- |
| License                 | `MIT`, `Apache-2.0`, `BSD-2-Clause`, `BSD-3-Clause`, `ISC`, or `public-domain`                     |
| Polarity                | Canvas OKLab lightness below 0.6 exactly when the theme declares `dark`                            |
| Body text               | Foreground on canvas at least 4.5:1 and \|Lc\| at least 60                                         |
| Selection text          | Selection foreground on the resolved selection at least 4.5:1                                      |
| Selection visible       | Resolved selection at least ΔE 6 from canvas                                                       |
| Cursor                  | At least 3:1 on canvas                                                                             |
| ANSI legibility         | ANSI 1–6 at least 3:1 and 8–14 at least 2:1 on canvas; on dark, 7 at least 3:1 and 15 at least 2:1 |
| Correction budget       | Each correction at most ΔE 10 from its original; at most ΔE 20 in total                            |
| Collisions              | Within each of ANSI 1–6 and 9–14, every pair at least ΔE 10                                        |
| Red and green under CVD | ANSI 1 against 2, and 9 against 10, at least ΔE 5 under deuteranopia and protanopia                |
| Agent glyphs            | Each fixed glyph on canvas, chrome, and current at least 0.9× its contrast in Orchbee Light        |
| Resolved floors         | Every floor in the [resolver table](#resolver) holds on chrome                                     |

ANSI 0 is exempt on dark canvases; ANSI 0, 7, and 15 are exempt on light canvases, because light
palettes disagree on which end of the gray axis they invert. The glyph gate is relative because
the fixed attention yellow is about 1.2:1 on Orchbee Light's current surface, which the product
accepts, and the glyph hues never change for contrast.

## GUI files

The GUI cannot run Rust, so `tests/client_files.rs` generates two tracked files from the shipped
themes and fails when either is stale:

| File                                                                                                                                 | Content                                                                                                                                            |
| ------------------------------------------------------------------------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------- |
| [`packages/design-tokens/src/color-themes.generated.css`](../../packages/design-tokens/src/color-themes.generated.css)               | One `[data-theme='<polarity>'][data-color-theme='<id>']` block per named theme, overriding every system color token. The Orchbee pair has no block |
| [`packages/terminal/src/color-themes/color-themes.generated.ts`](../../packages/terminal/src/color-themes/color-themes.generated.ts) | Every shipped theme's id, family, polarity, canvas, cursor, resolved selection, and ANSI 0-15 for `@orchbee/terminal`                              |

A block takes surfaces, text, frames, accents, and statuses from the resolver: canvas, chrome as
`surface-raised`, overlay, the text steps, the frame steps as borders, `accent-action` and
`accent-link`, and the four status roles. States follow the hand-authored mapping's proportions,
with hover lighter and pressed darker in both polarities. The brand honey, the scrim, and every
Agent status glyph color keep their polarity's hand-authored reference; `pnpm lint:styles` checks
that each block declares exactly its polarity's system colors and keeps those fixed values.

Regenerate after changing a theme file or the generator, and review an unshipped theme file
without adding it:

```sh
pnpm generate:color-themes
pnpm generate:color-themes --out-dir <directory> --theme <file.toml>
```

## Verification

`tests/gates.rs` evaluates every shipped theme against the gates; it asserts rules, never a
theme's values, so adding, removing, or retuning a theme never edits test expectations.
`tests/colorimetry.rs` implements the gate color science independently of the resolver and checks
it against published WCAG, APCA, CIEDE2000, Machado, and OKLab reference values.
`src/theme/tests.rs` checks that malformed theme files are rejected. `tests/client_files.rs` fails
when a generated GUI file is stale; verification selects this crate when either file changes. Run
from the repository root:

```sh
cargo test -p orchbee-color-themes --all-targets --locked
cargo clippy -p orchbee-color-themes --all-targets --all-features --locked -- -D warnings
```
