# Color Themes

| Field              | Value                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| ------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Status             | Steps 1, 2, 4, 6, and 8 and the GUI half of step 7 are delivered; the rest is a proposal under review. The inventory and measurements below are verified against `main` at `439bcc4b4` and against the cited upstream theme files as fetched on 2026-11-05                                                                                                                                                                                                                                                             |
| Owner              | Theme data and resolver maintainers, the native theme module in `apps/orchbee`, `@orchbee/design-tokens`, `@orchbee/terminal`, the terminal protocol and Host, and the user-preference owners                                                                                                                                                                                                                                                                                                                          |
| Current authority  | [`crates/color-themes`](../../crates/color-themes/README.md) owns theme data, the resolver, and the gates; the [Worker](../../apps/orchbee-worker/README.md#terminal-launch-palette) owns the launch palette; [`docs/tui/visual-foundations.md`](../../docs/tui/visual-foundations.md) owns TUI roles and the theme contract; [`docs/frontend/design-tokens.md`](../../docs/frontend/design-tokens.md) owns the CSS registry; [`docs/INTERFACE-DESIGN.md`](../../docs/INTERFACE-DESIGN.md) owns GUI/TUI correspondence |
| Decision authority | The product owner decides the default pair, the shipped list, the TUI default color source, and settings presentation; the terminal-protocol owner decides the palette-index cell encoding; this proposal decides the theme data model, the resolver, and the gates                                                                                                                                                                                                                                                    |
| Exit condition     | Both Clients offer the same named themes through shared light and dark preferences, every shipped theme passes the gates as a repository check, notices are recorded, the owning standards describe the result, and this proposal is deleted                                                                                                                                                                                                                                                                           |

## Purpose and scope

Users can choose among 16 named color themes, 10 dark and 6 light, that recolor the GUI chrome, the
TUI chrome, and the terminal canvas consistently. Every shipped theme passes measured legibility
and distinguishability gates before it is ranked, and every value either comes from a permissively
licensed upstream or is derived by one documented resolver.

This proposal covers theme data, its single source, generation into each medium, the
palette-to-role resolver, selection and polarity, the terminal ANSI path, settings, licensing,
evidence, and delivery order. It does not cover user-authored or imported themes, a render-time
minimum-contrast filter, the 256-color cube, fonts, or the fixed Agent status colors, which no
theme changes.

## Current state

### TUI

- `tui_theme/name.rs` defines `ThemeName::{TerminalNative, OneLight, OneDark}`. `terminal-native`
  is the default and the only theme users reach; the One pair exists for tests.
- `tui_theme/shipped.rs` gives each RGB theme a `Palette` of 26 values plus 9 Tab-group colors:
  text (`primary`, `secondary`, `muted`, `inverse`), accents, status, `frame`/`divider`/`guide`, and
  surfaces from `background` through `related`. `application_with_host` maps that palette to the
  role families in the visual-foundations standard.
- `tui_theme.rs` fixes `AGENT_ATTENTION #febc2e`, `AGENT_DONE #28c840`, and `AGENT_ERROR #ff5f57`,
  applies `MUTED_CONTRAST = 4.5`, derives host-relative surfaces from reported outer-terminal colors,
  and quantizes once to truecolor, 256, or 16 colors.

### GUI

- `packages/design-tokens/src/tokens.css` maps 105 `--ob-sys-color-*` tokens under
  `:root`, `[data-theme='light']`, and `[data-theme='dark']`. The registry accepts only `light`
  and `dark` and admits another theme "only when a concrete product consumer needs it". This
  proposal is that consumer.
- `ApplicationAppearance::{System, Light, Dark}` is a device-scoped preference shared by both
  Clients and surfaced as `appearance.application-theme`.

### Terminal colors

- The Worker launches each terminal with the cursor and ANSI 0–15 of the Orchbee theme whose
  polarity matches the terminal's default colors, and applies the complete theme again when it
  recolors a Host-following terminal. See the
  [terminal launch palette](../../apps/orchbee-worker/README.md#terminal-launch-palette).
- The Host resolves palette-coded cells to RGB in `render_encoding.zig`. Only default-coded cells
  carry the `color_default` sentinel, which each Client projects through
  `TerminalDisplayAppearance::{Host, Light, Dark, Custom}` and its own default colors.
- The Host already accepts `ApplyTerminalTheme` with revisions. The Worker uses it so that a
  Host-following terminal tracks the controlling Client's outer-terminal default colors.

### Measured defects

Measured with the [gates](../../crates/color-themes/README.md#gates):

- **Fixed by steps 1 and 2.** Every terminal launched with the light-tuned palette, so on the dark
  default canvas `#1e1e1e` blue `#0451a5` was 2.16:1 and white `#555555` 2.24:1, below the 3:1
  floor for normal ANSI colors. The cursor `#57bef8` was 2.02:1 on the light canvas `#fcfcfc`,
  below the 3:1 non-text floor; Orchbee Light now ships `#2e9cd4` at 3:1.
- **Open until step 3.** The Host keeps one palette per terminal, chosen at launch, and resolves
  palette-coded cells with it for every Client. A Client whose polarity differs from that launch,
  including one whose appearance changed afterward, paints default-coded cells with its own
  canvas but palette-coded cells with the other polarity's ANSI colors: dark yellow `#e5e510` is
  1.32:1 on the light canvas `#fcfcfc`.

## What a theme owns

| Concern                                                         | GUI                                    | TUI                               | Terminal canvas                   |
| --------------------------------------------------------------- | -------------------------------------- | --------------------------------- | --------------------------------- |
| Canvas background, foreground, cursor, selection                | Specified                              | Specified                         | Specified                         |
| ANSI 0–15                                                       | Specified (terminal renderer)          | Specified (pane cells)            | Specified                         |
| Chrome, strip, overlay, field, and interaction surfaces         | Derived, or family-specified           | Derived, or family-specified      | Not applicable                    |
| Text hierarchy, frames, dividers, guides                        | Derived                                | Derived                           | Not applicable                    |
| Accent, link, focus, status text, destructive                   | Derived from ANSI with contrast floors | Same values                       | Not applicable                    |
| Tab-group colors                                                | Derived from ANSI 1–6 and 9–14         | Same values                       | Not applicable                    |
| Agent `attention`, `done`, `error` glyphs; `working` and `idle` | Fixed; never themed                    | Fixed; never themed               | Not applicable                    |
| ANSI 16–255                                                     | Standard xterm cube and gray ramp      | Standard xterm cube and gray ramp | Standard xterm cube and gray ramp |

"Specified" values come from the theme file. "Derived" values come from the resolver, and the
same resolved values reach both Clients. "Family-specified" means a theme may pin chrome surfaces
only to values its upstream publishes for UI use, such as Catppuccin `mantle`/`crust` or Rosé Pine
`surface`/`overlay`. The gates apply to resolved values either way.

## Single source and generation

One new Rust library, `crates/color-themes`, is the only authority for theme data and resolution.

- **Data.** `crates/color-themes/themes/<id>.toml` holds one theme in the
  [theme file schema](../../crates/color-themes/README.md#theme-files): identity, declared
  polarity, canvas colors and ANSI 0–15 with each gated correction beside the value it replaced,
  and provenance. Optional family-specified chrome joins the schema with its first consumer in
  step 5 or 6. Display names are product copy and go through localization like every other label.

- **Resolver.** The crate resolves a theme into the complete role set below. It owns the OKLab
  mixing, the lightness-only contrast correction, and the role proportions that are now spread
  across `tui_theme.rs` and the host-relative derivation.
- **TUI.** The native theme module consumes resolved roles from the crate instead of
  hand-written palettes, then quantizes as it does today.
- **Worker.** The Worker takes the launch palette from the device's effective theme instead of a
  constant.
- **GUI.** A deterministic generator in the crate writes checked-in
  `packages/design-tokens/src/color-themes.generated.css`, with one block per non-default theme that
  overrides every `--ob-sys-color-*` token, and a typed module of terminal colors per theme for the
  `@orchbee/terminal` renderer. Following `docs/REPOSITORY-HYGIENE.md`, the generator is committed
  and verification fails when the generated output is stale.
- **Default pair.** The Orchbee pair keeps its reviewed hand-authored GUI mapping. Its theme files
  pin chrome to those values, so the TUI resolves the same surfaces and no default-pair CSS is
  generated. Until then its chrome is derived; open decision 13 covers its dark canvas.

A Rust crate is the authority because the TUI needs the resolver at run time for host-relative
surfaces. A JSON source under `packages/` would force Rust to read across workspaces, and a CSS
source cannot drive the TUI or the Worker.

## Palette-to-role resolver

The [resolver table](../../crates/color-themes/README.md#resolver) in the crate README owns every
role rule and the lightness-only correction, and every validation figure in this proposal used
those rules. A proportion may change only by re-running the gates. Family-specified chrome, when it
joins the schema, replaces the derived chrome and strip.

The GUI's status `surface`, `border`, `hover`, and `pressed`
variants come from the same status hue by the mixes the current hand-authored tokens use, so each
generated block fills the entire inventory that `docs/frontend/design-tokens.md` requires.

## Selection, polarity, and System

- **Preferences.** Two new device-scoped preferences, `theme.light` and `theme.dark`, each hold a
  theme id and default to `orchbee-light` and `orchbee-dark`. Both Clients read them. The existing
  Appearance preference (System, Light, or Dark) picks which slot is active. Ghostty's
  `theme = light:X,dark:Y` and Herdr's `auto_switch` with `light_name`/`dark_name` use the same
  model.
- **System.** System follows the Client's own polarity signal: the OS appearance in the GUI, and the
  outer terminal's reported background in the TUI. A change of that signal switches slots without
  a restart.
- **Picking a theme.** Picking a theme sets the slot of its polarity. If Appearance is fixed to the
  other polarity, the pick also switches Appearance, so the choice is visible at once. Under System,
  a theme for the inactive polarity says when it applies, for example "Used when the system is
  dark".
- **Polarity check.** A theme declares its polarity, and the gate check confirms it: canvas OKLab
  L < 0.6 means dark.
- **Default terminal colors.** `TerminalDisplayAppearance` becomes `Theme` (the default; the
  active theme's colors), `Host` (unchanged), and `Custom` (unchanged). Delivered without a
  migration: before release the preference record is current-only, so a record that still stores
  `light` or `dark` is refused like any other invalid record and the defaults apply.
- **Unknown ids.** The record admits any lowercase kebab-case id of at most 64 bytes, so a record
  written by a build with more themes stays valid. Resolution is the only fallback point:
  `orchbee_color_themes::select` gives the Orchbee theme of the slot's polarity for an id the build
  does not ship or one of the other polarity, and the Worker reaches it through
  `UserColorThemes::theme` in `local-runtime`. The protocol and the launch wire check syntax and
  bounds only.

### `terminal-native`

`terminal-native` remains. It stays the TUI's theme whenever host appearance or color level is
unknown, and it becomes an explicit TUI color source, "Terminal colors", beside "Theme".

- With Terminal colors, chrome keeps today's host-relative mapping, and pane cells emit `Reset` for
  default-coded colors and `Indexed(n)` for palette-coded colors 0–15, so the user's own terminal
  palette shows. That requires the palette-index encoding below.
- The color source is a TUI-only preference, because the GUI has no outer terminal. The shared theme
  ids stay meaningful to both Clients.
- Named themes need truecolor in the TUI, as grok-build's `requires_truecolor` does. A tinted canvas
  and its 4–14% interaction mixes do not survive 256-color quantization without collapsing surfaces.
  On a 256-color or 16-color host the TUI uses Terminal colors, and the picker shows named themes as
  unavailable, with the reason.

## Terminal ANSI path

Each Client must paint palette-coded cells with its own active theme. A desktop GUI in light mode
and a phone in dark mode can view one terminal at the same time, and the Host's single palette
cannot serve both. That mismatch is the open measured defect.

- **Recommended: tag palette-coded cells (option A).** The Host encodes palette indexes 0–15 as a
  new tagged code (for example `0x02000000 | n`) beside `color_default`. Each Client maps the index
  through its theme, and the TUI under Terminal colors maps it to `Indexed(n)`. Indexes 16–255 and
  direct RGB stay resolved by the Host. This extends the per-Client projection that default-coded
  cells already use. It is a wire change, so it needs a protocol revision and Client support before
  the Host emits the tag.
- **Kept: Host palette for queries.** The Worker applies the controlling Client's effective palette
  through the existing `ApplyTerminalTheme` path. Programs that query OSC 4, 10, or 11 to choose
  light or dark output then get answers that match what that Client shows.
- **Rejected: Host-only recoloring (option B).** Recoloring through `ApplyTerminalTheme` alone needs
  no wire change, but it leaves every non-controlling Client with another Client's palette and makes
  each theme switch a Host round trip.

## Validation method

The scratch scoring script is not tracked (vetoable). The durable form is the gate check in
`crates/color-themes`, described under Evidence. The method below is what the check must
reproduce.

### Candidates

- 82 external candidates from 37 families were scored, plus the proposed Orchbee pair. The current
  Orchbee defaults were scored as baselines.
- Sources:
  - the Ghostty bundle from iTerm2-Color-Schemes;
  - official upstream terminal ports for Catppuccin, Tokyo Night, Kanagawa, and Rosé Pine (the
    iTerm2 ports deviate; the iTerm2 Catppuccin Latte selection, for example, puts `#dc8a78` under
    `#eff1f5` text at 2.34:1);
  - the Windows Terminal built-ins and Warp's default themes;
  - themes Zellij, Herdr, and grok-build bundle or default to.

### Gates

A failing theme is not ranked. The [gate table](../../crates/color-themes/README.md#gates) in the
crate README owns the thresholds and exemptions. A failure within 5% of its threshold is reported
as borderline, but it still fails.

Four rules changed after the first pass over the data. Each change is disclosed with its reason:

1. **Agent-glyph gate.** The gate is relative instead of an absolute 1.5:1. The fixed attention
   yellow is 1.20:1 on Orchbee Light's current-row surface, a surface the product already accepts.
   An absolute floor would reject every light theme, and the glyph hues may not change for
   contrast.
2. **Selection visibility.** The threshold fell from ΔE 10 to 6, because selection is a large-area
   fill.
3. **Correctable legibility.** An ANSI slot, the cursor, or the selection below its floor is
   corrected by OKLCH lightness only, within the correction budget. A theme whose palette needs
   more than that is rejected rather than rewritten.
4. **Polarity exemptions.** ANSI 0 is exempt on dark canvases. ANSI 0, 7, and 15 are exempt on light
   canvases, because light palettes disagree on which end of the gray axis they invert.

### Ranking

The score is a weighted mean of transparent proxies. Differences below 0.03 are within their
resolution and are not treated as meaningful.

| Proxy (weight)    | Measure                                                                                                         |
| ----------------- | --------------------------------------------------------------------------------------------------------------- |
| Comfort (0.12)    | Text contrast in a 7–14:1 band, ramping down toward 4.5:1 and toward 21:1                                       |
| Legibility (0.12) | Mean ANSI contrast, capped at 4.5:1                                                                             |
| Distinct (0.15)   | Minimum cross-hue ΔE among chromatic ANSI, and red/green ΔE under CVD                                           |
| Harmony (0.15)    | Low spread of accent lightness and chroma, and hue fidelity to the canonical ANSI hues                          |
| Surface (0.08)    | Canvas lightness in a comfortable band (dark L 0.18–0.32, light 0.93–0.985), and low canvas chroma              |
| Community (0.18)  | Adoption across Ghostty, Zellij, Warp, Herdr, grok-build, and Windows Terminal, plus GitHub stars on 2026-11-05 |
| Pairing (0.10)    | The family has a passing sibling of the other polarity                                                          |
| Fidelity (0.10)   | Small total correction                                                                                          |

The harmony proxy penalizes deliberately muted palettes, and community favors incumbents. Neither
measures beauty directly. Weight stability (4,000 Dirichlet draws around these weights) reports how
often a family stays in the dark top nine or the light top five, so a rank that depends on one
weight choice is visible.

### Curation

- One representative per family and polarity: the upstream's default variant when it is within
  0.03 of the family's best, otherwise the best.
- The Orchbee pair is seeded as the default.
- The remaining slots are filled by score. Near-ties of 0.03 or less go to the representative
  farthest from the themes already chosen, by mean CIEDE2000 over background, foreground, and ANSI
  1–6.
- The minimum distance between two chosen themes is 8.8 among dark themes and 7.0 among light
  themes.

## Proposed themes

In the tables below:

- Text is WCAG ratio / APCA |Lc|.
- ANSI is the minimum contrast of ANSI 1–6 after correction.
- Cross is the minimum cross-hue ΔE among chromatic ANSI colors.
- CVD is red/green ΔE under deuteranopia or protanopia, whichever is lower.
- Stability is the family's share of weight draws in the top set.

All dark themes keep every fixed Agent glyph at 1.4× or more of its accepted ratio. The light
themes range from 0.90× to 1.00×.

### Dark

| Theme            | Source                                               | License    | Text      | ANSI | Cross | CVD  | Corrections                 | Score | Stability |
| ---------------- | ---------------------------------------------------- | ---------- | --------- | ---- | ----- | ---- | --------------------------- | ----- | --------- |
| Orchbee Dark     | Current defaults; VS Code Dark Modern ANSI           | MIT        | 11.2 / 79 | 3.24 | 16.8  | 13.4 | None                        | 0.683 | Default   |
| Gruvbox Dark     | morhetz/gruvbox via iTerm2-Color-Schemes             | MIT        | 10.7 / 82 | 3.00 | 12.6  | 8.2  | Red 2.69:1, ΔE 2.9          | 0.771 | 0.92      |
| Rosé Pine        | rose-pine/ghostty                                    | MIT        | 13.4 / 87 | 3.38 | 19.9  | 11.8 | None                        | 0.751 | 0.81      |
| Catppuccin Mocha | catppuccin/ghostty                                   | MIT        | 11.3 / 80 | 7.08 | 15.2  | 13.1 | None                        | 0.751 | 1.00      |
| Flexoki Dark     | kepano/flexoki via iTerm2-Color-Schemes              | MIT        | 12.0 / 76 | 4.42 | 18.7  | 5.2  | None                        | 0.749 | 0.75      |
| Nightfox         | EdenEast/nightfox.nvim via iTerm2-Color-Schemes      | MIT        | 10.1 / 74 | 3.64 | 12.9  | 9.5  | None                        | 0.741 | 0.87      |
| Dracula          | dracula via iTerm2-Color-Schemes                     | MIT        | 13.4 / 99 | 4.53 | 12.8  | 13.6 | None                        | 0.734 | 0.93      |
| Tokyo Night      | folke/tokyonight.nvim extras                         | Apache-2.0 | 10.6 / 74 | 6.46 | 12.2  | 12.5 | Bright black 1.91:1, ΔE 1.0 | 0.733 | 0.95      |
| Tomorrow Night   | chriskempson/tomorrow-theme via iTerm2-Color-Schemes | MIT        | 9.8 / 71  | 4.46 | 15.3  | 14.9 | Bright black 1.93:1, ΔE 0.8 | 0.692 | 0.47      |
| Kanagawa Wave    | rebelot/kanagawa.nvim extras                         | MIT        | 11.3 / 80 | 3.22 | 10.9  | 9.9  | None                        | 0.676 | 0.29      |

- **Orchbee Dark.** The current neutral canvas `#1e1e1e` with VS Code Dark Modern ANSI, the
  sibling of today's light palette. It fixes both measured dark defects without new chrome.
- **Gruvbox Dark.** The top score. A warm retro palette with the strongest pairing, and the
  family's default variant (Hard scores 0.790).
- **Rosé Pine.** The widest hue spread among the dark picks (cross ΔE 19.9) on a deep violet canvas.
- **Catppuccin Mocha.** The most adopted family, and every accent clears 7:1.
- **Flexoki Dark.** A warm ink-on-paper look with balanced accents. Its CVD margin, 5.2, is the
  thinnest that passes.
- **Nightfox.** A cool blue-slate look; its sibling Dayfox leads the light set.
- **Dracula.** The highest text Lc among the dark picks (99) and very high adoption.
- **Tokyo Night.** Indigo and neon accents, all at 6.4:1 or more. It needs one invisible-gray
  correction.
- **Tomorrow Night.** A quiet classic with the best CVD margin of the set (14.9).
- **Kanagawa Wave.** Muted ukiyo-e tones. It was chosen over Nord on the near-tie rule, because
  Nightfox already covers Nord's cool-blue space (Nord scored 0.705 and passed).

### Light

| Theme          | Source                                          | License       | Text       | ANSI | Cross | CVD | Corrections                                         | Score | Stability |
| -------------- | ----------------------------------------------- | ------------- | ---------- | ---- | ----- | --- | --------------------------------------------------- | ----- | --------- |
| Orchbee Light  | Current defaults and ANSI                       | MIT           | 18.0 / 103 | 3.03 | 20.1  | 7.1 | Cursor 2.02:1, ΔE 9.98                              | 0.538 | Default   |
| Dayfox         | EdenEast/nightfox.nvim via iTerm2-Color-Schemes | MIT           | 11.1 / 91  | 4.56 | 13.5  | 7.9 | None                                                | 0.733 | 1.00      |
| Gruvbox Light  | morhetz/gruvbox via iTerm2-Color-Schemes        | MIT           | 10.2 / 88  | 3.00 | 15.9  | 8.7 | Green, yellow (2.19:1, ΔE 8.2), cyan; total ΔE 12.6 | 0.715 | 1.00      |
| Flexoki Light  | kepano/flexoki via iTerm2-Color-Schemes         | MIT           | 18.6 / 104 | 3.39 | 18.7  | 5.2 | Bright black 2.00:1, ΔE < 0.1                       | 0.651 | 0.87      |
| Rosé Pine Dawn | rose-pine/ghostty                               | MIT           | 6.7 / 79   | 3.00 | 11.6  | 9.6 | Yellow (2.05:1, ΔE 9.7), cyan; total ΔE 13.4        | 0.622 | 0.70      |
| Tango Light    | Tango Desktop Project via iTerm2-Color-Schemes  | Public domain | 21.0 / 106 | 3.00 | 20.0  | 7.7 | Yellow and three brights; total ΔE 11.9             | 0.558 | 0.34      |

- **Orchbee Light.** The current neutral default. The cursor is darkened to `#2e9cd4` for 3:1.
- **Dayfox.** The only light theme that passes without a correction.
- **Gruvbox Light.** Pairs with the top dark theme. Its yellow needs the largest correction in the
  set. Its glyph margin is exactly 0.90×.
- **Flexoki Light.** A paper-white canvas with balanced accents. It pairs with Flexoki Dark.
- **Rosé Pine Dawn.** The softest light theme (6.7:1). It pairs with Rosé Pine.
- **Tango Light.** The high-contrast light option, black on white. It is the lowest-scoring pick
  and the first to drop if the list shrinks.

Light is thinner because light palettes fail more often. Yellow and cyan near a light canvas,
selection fills, and the fixed glyphs on light surfaces are the common causes.

Paired families are Orchbee, Gruvbox, Rosé Pine, Nightfox/Dayfox, and Flexoki.

### Passing alternates

These themes passed but were not chosen. They are candidates for a later family-variant menu:

- Gruvbox Dark Hard and Light Hard
- Catppuccin Frappé and Macchiato
- Rosé Pine Moon
- Tokyo Night Moon and Storm
- Nordfox, Terafox, Carbonfox, and Duskfox
- Nord
- One Half Dark
- Ayu Dark and Mirage
- Iceberg Dark
- Tango Dark
- Gruvbox Material Dark
- VS Code Dark Modern and Light Modern
- Campbell
- Warp Dark
- Melange Light

### Near-misses

| Theme                                                            | Polarity | Failed gate                                                                                 |
| ---------------------------------------------------------------- | -------- | ------------------------------------------------------------------------------------------- |
| Catppuccin Latte                                                 | Light    | Correction budget, borderline: yellow, magenta, cursor 2.34:1, and selection; total ΔE 21.0 |
| Atom One Dark                                                    | Dark     | Body text Lc 58, borderline                                                                 |
| GitHub Dark Default                                              | Dark     | Red/green under CVD ΔE 4.8, borderline                                                      |
| GitHub Light Colorblind                                          | Light    | Bright red ≈ bright yellow ΔE 9.6, borderline                                               |
| Zenbones Light                                                   | Light    | CVD ΔE 5.0, borderline; error glyph at 0.88× on chrome                                      |
| Iceberg Light                                                    | Light    | Done glyph at 0.84× on chrome                                                               |
| Everforest Dark                                                  | Dark     | Green ≈ cyan ΔE 9.4                                                                         |
| Kanagawa Dragon                                                  | Dark     | Blue ≈ cyan ΔE 6.7; CVD ΔE 4.0                                                              |
| Night Owl                                                        | Dark     | Selection text 3.37:1                                                                       |
| WezTerm default                                                  | Dark     | Selection text 3.88:1                                                                       |
| Tokyo Night Day                                                  | Light    | Selection text 3.28:1; done glyph at 0.79×                                                  |
| One Half Light                                                   | Light    | Cursor 1.96:1 needs ΔE 10.7; CVD ΔE 4.1                                                     |
| GitHub Dark Dimmed, GitHub Light Default, Gruvbox Material Light | Both     | CVD ΔE 4.1, 4.2, 4.3                                                                        |
| Dawnfox, Kanagawa Lotus                                          | Light    | CVD ΔE 1.8                                                                                  |
| Solarized Dark, Light                                            | Both     | Text Lc 40 and 4.13:1; bright blue ≈ bright cyan ΔE 4.5; selection text                     |
| Everforest Light, Ayu Light, Atom One Light, Warp Light          | Light    | Correction budget exceeded (ΔE 24.8 to 79.7 in total)                                       |
| Monokai Pro, Xcode                                               | Both     | Proprietary license                                                                         |
| Modus                                                            | Both     | GPL-3.0                                                                                     |
| Material, Nord Light                                             | Both     | Unknown origin of the iTerm2 port                                                           |

Of the 84 themes scored, 39 passed.

### The fixed Agent glyphs under CVD

The fixed glyph colors are not distinguishable by hue alone for every viewer:

- Failed against Completed is ΔE 2.8 under deuteranopia.
- Needs attention against Completed is ΔE 3.5 under protanopia.

At every color level above monochrome, the TUI draws Needs attention, Completed, and Failed with
the same filled hexagon `⬢` (`Theme::agent_glyph`); the distinct `#`, `+`, and `x` shapes appear only
in ASCII, no-color, and monochrome output. Needs attention also pulses, so in a sidebar row Completed
and Failed differ by hue alone, which a deuteranope sees as ΔE 2.8. The state word appears in
tooltips and legends, not in the row. No theme can repair this, because the hues are fixed in every
theme, and this proposal does not change them. Open decision 12 proposes a shape cue.

## Settings

Both Clients present the same structure and setting ids. Differences are limited to what the medium
requires.

- **Rows.** The Appearance section holds Appearance (System, Light, Dark; the existing
  `appearance.application-theme`), Light theme, and Dark theme. The TUI adds Colors (Theme, Terminal
  colors).
- **Picker.** Each picker lists the themes of its polarity. Every row shows the name and a swatch
  strip of canvas, foreground, and ANSI 1–5.
- **Preview.** Moving focus previews the theme in place without committing. Enter or click commits,
  and Escape restores the committed theme. This follows the preview, commit, and reset rules in
  `docs/tui/settings.md` and `docs/frontend/settings.md`.
- **Reset.** Reset returns both slots to the Orchbee pair.
- **TUI on reduced color.** On a 256-color or 16-color host, the TUI shows named themes as
  unavailable, with the reason. It never offers a choice that would quantize a canvas into
  collapsed surfaces.
- **Localization.** Theme names and every new label ship in `en-US` and `zh-CN`.

## Licensing and notices

- Each shipped family gets a `THIRD_PARTY_NOTICES.md` entry in the existing format: copyright
  holder, source, pinned revision, license, and use ("terminal and interface color values, modified
  by lightness correction where noted").
- The source of each theme file is recorded too. Where an iTerm2-Color-Schemes port supplied the
  values, that MIT compilation is cited beside the upstream family.
- Tokyo Night is Apache-2.0. Its entry reproduces the license notice and states the modification
  (one lightness-corrected slot).
- Tango is public domain; it is credited without a license obligation.
- Families shipped: Gruvbox, Rosé Pine, Catppuccin, Flexoki, Nightfox, Dracula, Tomorrow, and
  Kanagawa (MIT), plus Tokyo Night (Apache-2.0) and Tango (public domain). The Orchbee pair's ANSI
  comes from VS Code (MIT, Microsoft).
- Licenses were read from each repository's license file. Implementation re-verifies them at the
  pinned revision.
- Proprietary themes (Monokai Pro, Xcode), GPL themes (Modus), and ports of unknown origin
  (iTerm2 Material, Nord Light) are excluded.

## Evidence

Evidence follows `docs/TESTING.md`. Every check answers a named question.

- **Do all shipped themes meet the gates?** One data-driven test in `crates/color-themes` evaluates
  every theme file against the gate table and the declared polarity. It asserts the rules, not any
  theme's values, so adding, removing, or retuning a theme never edits test expectations. The color
  math (WCAG 2, APCA, CIEDE2000, Machado CVD) is checked once against published reference pairs.
- **Is the generated CSS or terminal module stale?** A verification step regenerates both and fails
  on a difference.
- **Does every generated block cover the full token inventory?** The existing design-token
  inventory tests already answer this once generated blocks join the registry. They need no new
  companion.
- **Do palette-index cells render through the right theme in each Client?** A protocol-level
  encode/decode case covers the new tag. Real-window render evidence covers one dark and one light
  theme in each Client, plus Terminal colors in the TUI.
- **No other tests.** There are no per-theme snapshot tests and no tests restating resolver
  proportions.

## Implementation plan

Each step is one commit, or a small series, that leaves `main` working:

1. **Theme crate. Done.** [`crates/color-themes`](../../crates/color-themes/README.md) holds the
   data schema, the resolver, the gate test, and the Orchbee pair with its dark ANSI and light
   cursor fixes.
2. **Worker. Done.** The
   [terminal launch palette](../../apps/orchbee-worker/README.md#terminal-launch-palette) follows
   the polarity of the effective default colors, so dark terminals no longer inherit the light ANSI.
3. **Protocol.** Add the palette-index cell tag: schema, Host encoding behind a negotiated revision,
   and decoding in both Clients, which map indexes through their active theme.
4. **Preferences. Done.** The shared device record holds `lightTheme` and `darkTheme`
   (`theme.light` and `theme.dark`), `TerminalDisplayAppearance` is Theme/Host/Custom, and the
   launch capture carries both slots so the Worker's palette takes the selected theme's cursor and
   ANSI colors. A Host-following terminal keeps the slots captured at launch until step 3 lets
   Clients map ANSI colors themselves.
5. **TUI.** Resolve roles from the crate, add the Colors source with truecolor gating, retire the
   test-only One pair, and keep `terminal-native` emitting `Reset` and `Indexed`.
6. **GUI. Done.** The crate's [GUI files](../../crates/color-themes/README.md#gui-files) test
   generates the theme CSS and the `@orchbee/terminal` color module and fails when either is stale;
   `pnpm lint:styles` checks each block against the registry. The application boundary sets
   `data-color-theme` for a named theme only, and the projected terminal's Theme mode publishes the
   selected theme's canvas, cursor, selection, and ANSI 0–15. Server-backed terminals take the
   selected canvas; their ANSI arrives with the launch palette until step 3.
7. **Settings.** Add the pickers with preview, commit, and reset in both Clients, with `en-US` and
   `zh-CN` copy. **GUI done:** the [color themes](../../apps/web/docs/terminal-settings.md#color-themes)
   pickers preview on arrow keys, commit on Enter or click, and restore on Escape, on leaving the
   list, or on close. Under System both pickers say when they apply. The TUI half remains.
8. **Named themes. Done.** The 14 named themes ship in
   [`crates/color-themes`](../../crates/color-themes/README.md#shipped-themes) with their notice
   entries, one commit per family. Each passes the gates with exactly the corrections listed above
   and no other. Measured after 8-bit rounding, a few correction sizes differ slightly from the
   tables: Tomorrow Night ΔE 1.0, Flexoki Light ΔE 0.5, and totals of 12.9 for Gruvbox Light, 13.5
   for Rosé Pine Dawn, and 12.6 for Tango Light, all within the budget. Tango Light takes the
   original Tango palette (`Builtin Tango Light`), whose corrections match the list. The GUI pickers
   stay expanded lists beside their labels.
9. **Documentation.** Update `docs/tui/visual-foundations.md`, `docs/frontend/design-tokens.md`
   (multiple themes, the polarity attribute), and the package READMEs, then delete this proposal.

### GUI theme boundary

`data-theme` keeps its polarity meaning (`light` or `dark`), which sets `color-scheme`, the shadcn
`dark` variant, and the hand-authored base mapping. A named theme adds `data-color-theme="<id>"` on
the same element, and generated blocks match `[data-theme='dark'][data-color-theme='<id>']` to
override every system color token. The Orchbee pair sets no `data-color-theme`.

## Open decisions

Each decision is listed with a recommendation and is vetoable:

1. **Default pair.** Keep the neutral Orchbee pair as the default, with its two fixes. It is the
   reviewed GUI look and the most neutral canvas for Agent output. The alternative, a popular family
   as default, would recolor every existing user.
2. **TUI default color source.** Keep Terminal colors as the TUI default, as the standard requires
   today. Picking any named theme in the TUI switches the source to Theme. The alternative, default
   to Theme on known truecolor hosts, gives stronger cross-client sameness but overrides users' own
   terminal palettes.
3. **ANSI path.** Option A (palette-index tags) plus the Host palette for queries. Option B alone
   cannot serve two Clients with different polarities.
4. **Corrected palettes.** Ship the lightness-corrected values under the upstream name, record the
   upstream values in the theme file, and note the modification in the notice. The alternative,
   verbatim palettes, would ship unreadable colors, such as Gruvbox Light's yellow at 2.19:1.
5. **Render-time minimum contrast.** Do not add one now. VS Code's `minimumContrastRatio`, Warp's
   enforcement, and Ghostty's `minimum-contrast` alter program-chosen colors, and the gates already
   make shipped palettes legible. Revisit as an opt-in accessibility setting.
6. **List size.** Ship the 16 representatives. Offer family variants such as Catppuccin Frappé or
   Gruvbox Hard later, from the passing alternates, only if users ask.
7. **Catppuccin Latte.** Leave it out. It misses the correction budget by ΔE 1.0. Revisit if the
   upstream fixes its cursor and selection; Catppuccin Mocha then gains a sibling.
8. **Family-specified chrome.** Allow it only from values the upstream publishes for UI surfaces,
   and gate it like derived chrome. This keeps signature looks, such as Catppuccin's mantle, without
   free-form tuning.
9. **Tinted GUI chrome.** Tint GUI chrome with the theme, as the TUI already does. Theming only the
   terminal canvas would break the cross-client match the product values most.
10. **Imported themes.** Defer Ghostty and iTerm2 file import. When it comes, show gate failures as
    warnings rather than refusing the import.
11. **Scoring script.** Keep it out of the repository. The gate test in `crates/color-themes`
    becomes the durable check, and the ranking proxies remain proposal evidence only.
12. **Outcome shape in color modes.** Give Completed and Failed distinct shapes at every color
    level, for example by reusing the existing outcome fallbacks or a checked and crossed hexagon
    pair, while keeping the fixed hues. The alternative, an opt-in accessibility setting, leaves the
    default sidebar ambiguous for viewers with red-green color vision deficiency. This decision
    belongs to the TUI and GUI status owners rather than to theming.
13. **Orchbee Dark canvas. Decided in step 6.** The GUI's dark canvas is `oklch(14.5% 0 none)`,
    about `#0a0a0a`, while Orchbee Dark and the shipped dark terminal default use `#1e1e1e`. Step 6
    keeps both: `#1e1e1e` stays the gated terminal canvas, and the Orchbee pair keeps its reviewed
    hand-authored GUI mapping with no generated block, so the default look does not move. Named
    themes take their GUI canvas from the theme. Moving either Orchbee value, or pinning Orchbee
    chrome in the theme files, remains a later change that re-runs the gates.
