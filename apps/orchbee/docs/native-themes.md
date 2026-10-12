# Native themes

## Selection and ownership

The [shared device preferences](../../../crates/workbench-protocol/docs/user-preferences.md)
separate application appearance (Light, Dark, System) from terminal defaults (Host, Light, Dark,
Custom). Application Light is the common default. System maps TUI chrome to the outer terminal's
semantic defaults; the TUI does not change the outer host's font or appearance.

Terminal Host uses the captured frame/window foreground and background. Light/Dark use the shared
shipped defaults; Custom uses the validated RGB pair. Only default-marked cells are substituted.
Explicit program RGB, palette colors, OSC overrides, reversal, attributes and wide-cell geometry
retain their captured authority in live and retained rows. Cursor and empty canvas use the same
selected terminal defaults. This presentation does not change retained Host color-query replies.

Legacy Windows TOML accepts terminal-native, one-light and one-dark. These are migration inputs:
explicit legacy values seed unclaimed shared preferences once. Later TOML reloads cannot override
confirmed common appearance. Shipped theme roles still map chrome, sidebars, menus, notices and
selection overlays; renderers consume those roles without changing focus, geometry or control.

The light/dark roles distinguish canvas, chrome and raised overlays. System uses outer-host
defaults; quantization may collapse surface differences while borders, labels and focus remain.
Overlay frames share theme corner and focus roles; titles use independent primary bold text.

Persistent sidebar context uses `surface-current`, separate from the selection color used by
pickers. The native theme pairs ANSI gray with black body text, without DIM on host-relative text;
both named themes use a low-contrast neutral fill against chrome and retain their primary/secondary
text colors. This fill remains quieter than keyboard selection and preserves readable text contrast. Current names
are bold; status glyphs and explicit token colors remain semantic. When quantization collapses the
fill onto chrome, the gutter retains `*` even if configured text disables bold. Keyboard navigation
uses `surface-selection` and its paired text on a noncurrent identity line, with a separate gutter
marker. Current context keeps its neutral fill when both states coincide. Explicit token colors
and modifiers apply after the focus style. This does not infer host polarity or change a terminal's palette.

## Color capabilities

The Shell samples color environment inputs once before drawing. A pure resolver
uses this order: nonempty `NO_COLOR`, `FORCE_COLOR`, `COLORTERM`, then `TERM`.
`NO_COLOR` and `FORCE_COLOR=0` suppress chrome colors. Force values `1`, `2`, and
`3` select 16 colors, 256 colors, and truecolor. An invalid force value is unknown,
which means 16 colors. `COLORTERM=truecolor` or `24bit` selects truecolor; a TERM
containing `256color` selects 256 colors, and `TERM=dumb` suppresses colors.
When `COLORTERM` and `TERM` are absent or empty, a plain GUID-shaped `WT_SESSION`
identifies Windows Terminal and selects truecolor. Presence of `TMUX`, `ZELLIJ`
or `STY` prevents this outer-host fallback. Explicit descriptors, including unknown
ones, retain their existing decisions; malformed session indicators remain unknown
and use 16 colors. Values are not logged. This is a color capability decision,
not an appearance or background-polarity decision.

Windows Terminal sets its [plain session GUID](https://github.com/microsoft/terminal/blob/main/src/cascadia/TerminalConnection/ConptyConnection.cpp)
when launching a connection. A host without that indicator retains the conservative
fallback; no terminal query or per-frame environment read is added.

Theme role quantization runs when an admitted theme is installed, outside rendering. It uses
the existing canonical xterm palette and perceptual distance. Text roles exclude
bright black and preserve secondary/disabled hierarchy with modifiers at 16 colors.
Monochrome keeps text, glyphs and modifiers. Host palettes may differ; this is not a
measurement of physical contrast or a negotiation of terminal appearance.
The named RGB themes use their muted foreground without an additional DIM for
disabled text at truecolor and 256 colors. Faint intensity can darken an RGB
foreground until disabled text looks like primary text on a light surface.
At 16 colors and monochrome, quantization retains DIM when muted and primary
foregrounds coincide; `terminal-native` keeps its host-relative DIM.
At low color levels, a host may render DIM black like ordinary black. Visible
unavailability reasons remain necessary even when the disabled modifier is present.

## Publication and reload

The ordered Settings lane reads the shared authority on startup and polls once per second. Only a complete confirmed snapshot updates cached preferences and theme roles. Invalid sources and lost replies preserve last confirmed values and expose an issue. GUI edits and TUI saves use this same path without restarting terminals. Renderers read cached values, never files or environment. Legacy configuration reload remains responsible for its unrelated fields and cannot replace confirmed common preferences.

## Correctness and evidence

| Claim                                                       | Evidence                                                                        | Bounds                                                                                                            |
| ----------------------------------------------------------- | ------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------- |
| Legacy names and migration admission                        | Settings decision cases and CLI warning/schema checks                           | Windows TOML owner and existing fatal source boundary                                                             |
| Complete role mapping and capability precedence             | Pure palette/resolver cases, quantized buffers, all consumer render cases       | Three themes, canonical palette, sampled environment; program colors excluded                                     |
| Default markers follow the theme; explicit cells stay exact | Live/retained cell rendering, mixed default/explicit RGB and empty canvas cases | Native Reset and both named themes; monochrome, 16/256 colors and truecolor; physical host appearance is separate |
| Confirmed shared publication preserves complete appearance  | Server/Client publication traces and shared contract corpus                     | Shared settings lane and CAS traces; physical display capability remains separate                                 |
| Startup and shared reload reach the real Client             | Isolated Windows ConPTY and root verification                                   | Native Windows process/byte evidence; physical emulator appearance is separate                                    |

Run `cargo test --locked -p orchbee --all-targets --all-features`
and the affected `pnpm verify` root gate. Shared custom colors and System appearance are current behavior;
physical host appearance and untested platforms require their own execution evidence.
