# Native themes

## Selection and ownership

The Windows configuration owner admits `[theme] name = "terminal-native"`,
`"one-light"`, or `"one-dark"`. The default is `terminal-native`. Names are exact;
an invalid name or type warns and defaults only that field. Other theme fields,
including `custom` and automatic appearance selection, remain unsupported.
The offline default configuration includes the default name.

These themes color Orchbee chrome, sidebars, menus, notices and selection overlays.
Default-marked terminal foreground and background follow the selected theme;
explicit program colors and the program palette remain unchanged. Themes do not
change focus, geometry or control ownership. The theme module maps semantic roles;
renderers consume roles.
For live and retained rows, `terminal-native` projects default markers to host-relative
`Color::Reset`, preserving either a light or dark host. The Worker's light launch-profile
RGB defaults are not the Client host's defaults. Explicit cell RGB, including resolved
palette entries and values equal to the frame defaults, remains exact. The same theme
defaults cover empty canvas cells. The Host projects program-overridden OSC foreground and
background defaults as explicit cell RGB, even when an override equals its configured default.
Those colors remain exact in live and retained rows. Untouched or reset defaults keep their
markers and follow the Client theme. Program-requested screen reversal projects both effective
swapped defaults explicitly; cell inverse and explicit program colors retain their normal priority.
The retained terminal still owns color-query responses; this presentation does not query or
change the Client host's colors.
Both RGB themes declare their polarity and distinguish the terminal canvas,
persistent chrome and raised overlays. `surface-canvas` supplies the themed
terminal default background; `surface-chrome` paints sidebars, bars and pane-title
bands; `surface-overlay` paints the entire overlay, including padding and empty
cells. Text remains readable on every surface. Terminal-native maps all three
backgrounds to the host default; quantization may collapse these differences at
lower color levels, while borders, labels and focus emphasis remain.
Overlay frames share the theme's corner style and focused/unfocused frame roles.
Their titles use primary bold text, independently of the border's focus color.

Persistent sidebar context uses `surface-current`, separate from the selection color used by
pickers. The native theme pairs ANSI gray with black body text, without DIM on host-relative text;
both named themes use a neutral fill and retain their primary/secondary text colors. Current names
are bold; status glyphs and explicit token colors remain semantic. When quantization collapses the
fill onto chrome, the gutter retains `*` even if configured text disables bold. Keyboard navigation
keeps a separate gutter marker, using the paired foreground when it
coincides with the current row. This does not infer host polarity or change a terminal's palette.

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

The Server publishes the named theme in the existing complete configuration
snapshot. A theme-only change advances that publication revision. Clients validate
the entire snapshot before applying any field and accept only a newer revision,
including the initial authoritative baseline. A fatal reload, malformed snapshot,
or stale publication preserves the installed theme. Startup can draw the existing
provisional local configuration before the authoritative baseline arrives.

`orchbee site reload-config`, the existing reload binding, and the global menu
reload the same owner. The next complete frame uses one cached theme for all chrome;
rendering never reads the configuration file or environment. Background tasks
continue and no user input is replayed. Configuration-driven menu cancellation
retains its existing behavior.

The closed wire snapshot requires a named theme, so Clients and Server must come
from the same native cohort. New CLI output versions retain all older closed
receipt versions. Roll back the complete cohort and select `terminal-native`;
no checkpoint, terminal history, or configuration file is migrated or deleted.

## Correctness and evidence

| Claim                                                       | Evidence                                                                         | Bounds                                                                                                            |
| ----------------------------------------------------------- | -------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------- |
| Exact names and local fallback                              | Settings decision cases and CLI warning/schema checks                            | Windows TOML owner and existing fatal source boundary                                                             |
| Complete role mapping and capability precedence             | Pure palette/resolver cases, quantized buffers, all consumer render cases        | Three themes, canonical palette, sampled environment; program colors excluded                                     |
| Default markers follow the theme; explicit cells stay exact | Live/retained cell rendering, mixed default/explicit RGB and empty canvas cases  | Native Reset and both named themes; monochrome, 16/256 colors and truecolor; physical host appearance is separate |
| Reload is atomic and stale/fatal input preserves the theme  | Configuration publication model, Server/Client traces and shared contract corpus | Existing finite payload/revision model; no new fairness or liveness claim                                         |
| Startup and shared reload reach the real Client             | Isolated Windows ConPTY and root verification                                    | Native Windows process/byte evidence; physical emulator appearance is separate                                    |

Run `node scripts/tla/run-models.mjs --models windows-configuration-publication`
and the required root `pnpm verify`. Custom colors, automatic appearance reports,
and the remaining UI layout options are separate slices.
