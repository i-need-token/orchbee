# Native themes

## Selection and ownership

The [appearance settings](../../../crates/local-runtime/README.md#tables-and-keys) choose an application appearance (Light, Dark, System), one theme for each of the light and dark
slots, the native Client's color source (Theme or Terminal colors) and terminal defaults (Theme,
Host, Custom). The TUI does not change the outer host's font or configured theme. While it owns the screen,
its named canvas temporarily sets the default background on admitted hosts, as described below.

The [source resolver](../crates/workbench-client/src/tui_theme/source.rs) decides what the TUI
paints. The first matching row wins:

| Host and settings                                    | The TUI paints                    |
| ---------------------------------------------------- | --------------------------------- |
| The host is below truecolor                          | Terminal colors                   |
| Colors is Terminal colors                            | Terminal colors                   |
| Appearance is Light or Dark                          | The theme in that slot            |
| Appearance is System and the host appearance settled | The slot matching that appearance |
| Appearance is System and it has not settled          | Terminal colors                   |

System takes the outer terminal's OSC 11 background. When the terminal does not answer before the
probe deadline, it takes the operating system's appearance where an adapter exists (Windows
today), as the GUI follows the operating system; a named theme paints its own canvas, so the
terminal's unreported colors need not match it. A slot id this build does not ship, or one of
the other polarity, paints that slot's Orchbee theme and keeps the stored id. Below truecolor the
named themes are unavailable: a quantized canvas and ANSI palette would show a different theme, so
Settings disables the theme rows and says why. Reaching truecolor, or the host appearance
settling, repaints the confirmed settings without a restart.

Named themes take every role from the [shipped catalog](../../../crates/color-themes/README.md)
through its role resolver. The [named-theme adapter](../crates/workbench-client/src/tui_theme/shipped.rs)
puts primary text on each resolved surface, `text-selection` on `surface-selection`, bold primary
key hints and secondary hint labels, and it uses plain borders and UTF-8 glyphs. Orchbee Light and
Orchbee Dark then take the shared theme colors described below, as the GUI's Orchbee themes do.
Terminal colors are the `terminal-native` mapping: host default foreground and background
(`Color::Reset`), named ANSI accents and the paired and modifier fallbacks of the
[visual foundations](../../../docs/tui/visual-foundations.md). The traffic-light Agent colors are
the same under every theme and under Terminal colors.

Terminal defaults decide the colors of default-marked cells. While a named theme paints, Theme
takes that theme's canvas foreground and background, and Host does the same while Appearance is
Light or Dark. Host under System, and Theme or Host under Terminal colors, defer to the host:
default cells emit `Color::Reset`, and the outer terminal's observed colors describe them in RGB.
Custom always uses its validated pair. No terminal choice changes the chrome, and retained
child-frame launch defaults never replace this local choice. Default-marked cells use that pair.
Unmodified ANSI 0–15 cells use the active named theme's ANSI palette; Terminal colors emits
`Color::Indexed(n)` at truecolor and ANSI-256, and retains the existing ANSI-16 and monochrome
quantization below them. Theme and polarity changes repaint the same retained cells without a
Host round trip. Direct RGB and indexes 16–255 retain Host resolution. Child OSC 4 overrides stay
RGB until OSC 104 resets their slots. Bold is a separate attribute, never a Host bright-index
rewrite; Terminal colors leaves the outer terminal's bold-color policy in control. Inverse and
faint retain their existing attributes. The TUI retains its existing underline-kind support;
separate underline colors remain a backend limitation. Selection/search and software-cursor
contrast remain Client-owned layers over the resolved cell colors. Empty canvas uses the selected
terminal defaults across the entire pane, including space beyond a smaller retained grid.

The one-shot OSC 10/11/12 and OSC 4 (slots 0–15) probe retains each valid color once within its
original 350 ms window. Background settles polarity immediately; foreground, cursor and palette
replies can complete the same observation in any order. Duplicates, malformed responses and late
replies cannot replace captured colors.

Each controller grant reports the Client's effective confirmed defaults, cursor and ANSI colors
through `host-colors`, including named themes before any outer-terminal reply. The Worker applies
known colors through revision-fenced `ApplyTerminalTheme` for every controlled terminal, regardless
of launch display mode. Subsequent live/history polling also synchronizes changed confirmed
settings without waiting for keyboard input. A read-only Client changes only its own rendering.
The Worker's OS fallback applies before any Client reports; the latest reported defaults seed new
Host-following launches. Reversible Settings previews remain local until committed.

**Vetoable decision: partial outer palette.** Under Terminal colors, only observed OSC 4 slots and
an observed cursor are reported; unsupported or missing replies stay explicitly unknown. The Worker
applies known entries and retains the current query fallback for unknown entries. This cannot prove
that an unobserved query slot matches a customized outer palette, while `Indexed(n)` still paints
that actual outer slot. No missing reply is replaced with an invented outer color.

The native backend paints a steady software block at the current input owner's reported position,
after composing text and graphics in the synchronized frame. Its theme-owned contrast policy uses
the cell's displayed background, including program reversal, and chooses the higher-contrast black
or white fill in that color encoding. The covered glyph uses the former background as its foreground;
dim, hidden and blink attributes cannot hide the block, and concealed program glyphs remain blank.
This also handles equal custom foreground/background values and blank cells. Unknown default colors
invert the displayed host-default pair, accounting for an already reversed cell. Indexed and ANSI
contrast uses the canonical palette and still requires physical host review.

The backend keeps unadorned cells, erases the previous block before every diff (including an empty
diff), and paints only the final requested cursor. Covering surfaces hide it or supply their own
editor position; panel switches, live cropping, retained-tail offsets and recovery keep the renderer's
input coordinates. A wide-cell tail paints its complete glyph without changing the exact native
input-method anchor. Cursor output stays in the same frame and flush, and a failed write never
replays a pending cursor. Host cursor color and shape are untouched. The native cursor stays hidden
while the Client draws; the existing terminal mode guard restores its visibility on exit and failure.

The selected roles map chrome, sidebars, menus, notices and selection overlays; renderers consume
them without changing focus, geometry or control. Named themes distinguish canvas, chrome and
raised overlays. Overlay frames share theme corner and focus roles; titles use independent primary
bold text. Independent terminal popups use this complete overlay surface for the title, borders
and pending body, even on a dark-default outer host. A received program frame replaces only the
inner body with its independent terminal defaults and explicit program colors.

Each theme owns one accent per Tab group color. Named themes take the catalog's Tab-group roles;
terminal-native uses named ANSI colors, except orange, which has no ANSI name and uses RGB
`[255, 165, 0]`; a host at a lower color level quantizes it. A resting group header is a solid
block of the accent with black or white text, whichever contrasts more with it, chosen in the
accent's own color space. An expanded group's members and the gaps between them rest on a band
that mixes 20% of the accent into the theme's strip fill. Terminal-native has no concrete strip
color to mix, and 16-color hosts cannot express a faint fill, so they paint no band and a resting
member's label takes the accent instead. A current, hover or pressed surface replaces the block or
the band on its own token, with the accent name readable on it, and leaving restores the block or
band. Group color swatches in menus paint the same accent. Monochrome hosts show the header in
reverse video and otherwise use the ordinary role resolver and interaction fallbacks.

The theme contract also provides `accent-action`, `surface-hover` and `surface-pressed`. In
Orchbee Light and Orchbee Dark these roles carry the
[shared theme colors](../../../schemas/workbench/README.md#cross-client-parity-sources), which the
GUI's CSS registry also resolves to: primary action, hover and press respectively, beside the Agent
state colors. The Light cell-grid adapter makes the translucent hover and press colors opaque over
its chrome surface. A conformance test reads that file, as the CSS registry's test does, and
neither reads the other's source. Every other shipped theme keeps the catalog resolver's roles.
These roles are available to renderers, but each surface still requires consumer render evidence
before it can claim adoption.

The sidebar's explicit red Delete intent uses independent `accent-destructive` metadata and palette
ownership, never `status-error`. Named themes take the catalog's destructive accent, and
terminal-native uses ANSI red. Disabled content resolves to `text-disabled` before action tone; it
cannot acquire hover or pressed styling. Enabled destructive wording keeps its accent separately
from surrounding focus, navigation, current, hover and pressed tracks. At rest, terminal-native
destructive wording is ANSI red directly on the host default background without a backing fill:
a destructive menu row and a whole destructive confirmation button, including its brackets and key
hint, read as red text. Reference contrast for ANSI red on the host default is about 3.2:1 in
Campbell and VS Code Dark+ and 3.3:1 to 3.5:1 in One Half Light. Every named theme keeps the
destructive key hint in `text-secondary`. Where reduced-color or native interaction uses reverse
video, or a known fill falls below 3:1, the theme resolver removes reversal from the destructive
content band and supplies its own contrasting backing pair. Components neither construct colors
nor add warning bold, underline or blink. Monochrome retains the localized Delete verb and ordinary
markers. Nominal palette tests cover the unpaired terminal-native rest state and the overlay,
hover, pressed and selection surfaces at truecolor, ANSI-256 and ANSI-16; customized host palette
appearance still requires real-window review.

Named-theme supporting text, hint labels and pending status are the resolver's secondary text,
which stays below primary text on the canvas, chrome, overlay and inactive surfaces. Muted text is
its quietest step toward chrome that still clears 4.5:1 there, and it rises to secondary text on
every fill. Disabled text stays quieter than primary text without faint intensity at truecolor and
256 colors. Secondary text is not dimmed to create priority at the expense of its surface
readability. The catalog's contrast floors cover chrome; its [README](../../../crates/color-themes/README.md)
records them. Real-window light/dark, en-US/zh-CN and 80x24/120x40 review supplies perceptual
evidence.

## Outer-terminal background

On native Windows, a plain GUID-shaped `WT_SESSION` admits OSC 11/111 background control for
Windows Terminal only when no nested host is marked: `TMUX`, `ZELLIJ`, `STY`, `HERDR_ENV`,
`ORCHBEE_PANE_ID` and `TERM_PROGRAM` are all absent. Windows Terminal sets no `TERM_PROGRAM`, so its
presence names another host between the TUI and the window, whose panes would otherwise retint the
whole outer window. Other hosts remain unchanged until
an adapter contract admits them. A named theme sets the host default background to `surface-canvas`
between frames, so window padding, the scrollbar gutter and fractional cells match the TUI.
Runtime theme changes, including Settings previews and their cancellation, update that background.
Terminal-native and no-color presentation leave the host background alone; returning to either
after a named theme releases the override with OSC 111 (the host's configured default).

The lifecycle records cleanup before writing the override, including an unknown or partial write,
and restores it before leaving the alternate screen on normal exit, detach, fatal error and panic.
The same lease must be released before any future shell suspension or external-program handoff.
A failed effect fences frame output and is never replayed. Existing bounded cleanup attempts still
run after another cleanup fails. Forced process termination cannot restore in process.
This boundary has no GUI equivalent; Desktop owns its entire window surface.

## Sidebar row state

The [Workspace row compositor](../crates/workbench-client/src/tui_render/workspace_sidebar.rs) owns one background per row,
including padding, blank cells and continuation lines. The [text compositor](../crates/workbench-client/src/tui_render/sidebar_text.rs)
clears inherited whole-row bold before applying identity emphasis, so supporting lines keep their
hierarchy. Monochrome's provisional selection is the exception: it is a bold reverse band, so every
cell that is not explicitly plain shares its bold and drops DIM, and Windows Terminal, which
brightens bold text and applies DIM before reverse video, still shows one background. Pressed,
provisional, hover, current and base surfaces retain that order; focus,
operational state and unread markers remain independent. Hover and press do not select an entity.

The theme's `paired_pointer_surfaces` projection, consumed by the sidebar and Settings, pairs
reverse-video pointer fallbacks with primary text on chrome. In terminal-native and Monochrome that
pair is the host's own, so hover and press form the same host-relative reverse band as the Tab
strip; collapsed 256- and 16-color fills pair the quantized primary text and chrome the same way.
Every cell that stays in the band takes its foreground without DIM: ordinary and secondary text,
navigation markers, resting accents, explicit token colors and blank cells. Token foregrounds and
inherited span backgrounds therefore never become separate displayed backgrounds, and a host that
applies DIM before reverse video swaps the colors cannot give a token a darker background. Status
and Agent glyphs with a sampled color keep the contrast pair described for the action band below,
and a destructive label keeps its own pair; Monochrome reverses them with the band. Named RGB
surfaces keep their palette. Shared controls consume this projection without inspecting the active
palette.

Persistent sidebar context uses `surface-current`, separate from the selection color used by
pickers. The native theme pairs ANSI gray with black body text, without DIM on host-relative text;
named themes use the resolver's neutral current fill against chrome and retain their
primary/secondary text colors, so the persistent row remains visible while input belongs to a
terminal. This fill remains quieter than keyboard
selection and preserves readable text contrast. A current Workspace and its visible active
Agent share this one fill rather than two neighboring shades; folding the Agents leaves the fill
on the Workspace alone. The fill alone
marks the current sidebar row in the named themes: names keep their normal weight and no marker is
added; status glyphs and explicit token colors remain semantic. Terminal-native keeps the ANSI gray
fill and also underlines the complete current row, as Monochrome does, because ANSI 7 can sit
within a few steps of a light host's default background, as One Half Light shows. The same
underline marks every terminal-native current surface: the active Tab, which also keeps its bold
label, a Settings current row, and navigator or picker current rows. When quantization collapses a
named theme's fill onto chrome, the current row is underlined instead, as in Monochrome; the
configured token modifiers remain unchanged. Keyboard navigation uses `surface-selection`, its paired text and the separate navigation
marker. When the provisional target is also current, `surface-selection` wins the row background
until navigation leaves that row, and pointer hover likewise replaces the current fill on its own
row; Agent state and unread glyphs remain in their own cells. Explicit token colors and modifiers
apply after the focus style. This does not infer host polarity or change a terminal's palette.

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

Real-host comparisons record these descriptors alongside the image. An automation-launched preview
can inherit tool-only `NO_COLOR=1` or `TERM=dumb`; clear only those preview-local overrides when
reviewing ordinary color capability, and keep intentional monochrome runs as separate evidence.
Never change the user's persistent environment to prepare a preview.

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
Monochrome current context uses an underline without bold, plus a current marker where its owner
keeps one; provisional selection and checked accents use bold reverse video plus their distinct
markers. Focused frames retain a bold
reverse outline, including shared split boundaries, and focus labels use bold underline. Provider
choices use a fixed checked-glyph track and the selection role rather than primary-action styling.
At low color levels, a host may render DIM black like ordinary black. Visible
unavailability reasons remain necessary even when the disabled modifier is present.

Terminal-native maps `accent-action` to ANSI cyan without a modifier. Hover and press use the
theme resolver's reverse-video interaction band; the component, not the surface role, adds bold to
only the primary label while it is pressed. If 256-color or 16-color quantization collapses a named
hover or press fill onto another interaction, base, current or selection fill, the resolver applies
the same reverse-video fallback. Monochrome does likewise. It does not make an at-rest primary
action bold or reversed: the control's stable action grammar, verb and grouping preserve its
affordance while focus and press keep independent cues.
Inside that band, the resolver takes reversal off a glyph whose status or Agent role has a sampled
color, such as a Tab's traffic-light state glyph or its unread bullet, and backs that foreground
with ANSI black or white, whichever contrasts more. The glyph keeps its hue instead of becoming a
block of it; host-relative Working and Idle glyphs, and monochrome, reverse with the band. Gaps
beside such a glyph stay in the band.

## Color diagnosis

The retained Host inherits its launch environment, then applies each terminal’s launch delta.
The default Worker launch declares `TERM=xterm-256color`, `COLORTERM=truecolor` and
`TERM_PROGRAM=ghostty`; it leaves inherited application color policy intact. A user’s explicit
`NO_COLOR` must remain effective. Chrome color capability never recolors explicit program cells.

For a controlled color comparison, remove only known automation-injected color policy from the
fixture environment **before** starting its isolated Server, Worker, Host and Client. Opening a
new Client against an existing Session cannot update the retained Host’s environment. Do not restart
an unrelated user Session to obtain a clean fixture. Read the resulting environment inside a fresh
child terminal; changing just the frontend environment is insufficient.

Compare the same application version, configuration and content in GUI, TUI and a reference
terminal. Record terminal defaults, palette, theme and color-capability variables. Include a
negative `NO_COLOR` control and explicit ANSI, indexed and RGB foreground/background output,
with bold, faint and other supported attributes. First inspect captured cell styles to distinguish
an application emitting no colors from loss during parsing or transport; then inspect real window
captures in light and dark themes, including a theme switch. Cell snapshots alone do not establish
final drawing correctness. Never add application-specific text coloring to hide missing output.

## Publication and reload

The ordered Settings lane observes the Server's pushed settings snapshots in revision order. Only a complete snapshot updates the cached values and their resolved theme; a file the Server cannot parse keeps the last valid values, and a lost reply keeps the last confirmed ones. GUI and TUI edits use this same path without restarting terminals. Renderers read owned state, never files or environment.

[Settings appearance previews](native-settings.md#apply-timing-and-capability-boundaries) supply a separate reversible palette and locale to the whole-frame compositor, including background chrome and terminal default markers. This projection never publishes runtime policy or replaces the confirmed cache; ending the preview reveals the newest confirmed appearance, and program colors retain their independent authority.

## Correctness and evidence

| Claim                                                        | Evidence                                                                                                          | Bounds                                                                                                                |
| ------------------------------------------------------------ | ----------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------- |
| Complete role mapping and capability precedence              | Pure palette/resolver cases, quantized buffers, all consumer render cases                                         | Every shipped theme and terminal-native, canonical palette, sampled environment; program colors excluded              |
| Source resolution and the truecolor gate                     | Resolution matrix over appearance, reported host polarity, color level and source; Settings availability cases    | Pure decisions and Settings state; physical host detection is separate                                                |
| Shared Orchbee interaction palette remains aligned           | Conformance tests in both Clients against the shared theme colors, with exact shared-input verification edges     | The shared colors cover Orchbee action, hover, press and Agent states; other themes keep the catalog resolver's roles |
| Agent state colors never follow the theme                    | Agent glyph styles compared under every shipped theme and Terminal colors                                         | Truecolor; quantization maps the fixed values per level                                                               |
| Sidebar rows have one surface and distinct current ancestors | Workspace/Agent hover, press, navigation, collapse and supporting-line regression cases                           | Native, Light/Dark, ANSI-16 and monochrome; both locales; real-window acceptance remains separate                     |
| Host-default launch colors follow explicit app appearance    | Shared resolver decisions, frozen settings snapshots and Worker launch-adapter profile cases                      | New terminal defaults and OSC query inputs; retained query state and Client previews remain separate                  |
| Default markers follow the theme; explicit cells stay exact  | Live/retained cell rendering, mixed default/explicit RGB and empty canvas cases                                   | Native Reset and Orchbee Light/Dark; monochrome, 16/256 colors and truecolor; physical host appearance is separate    |
| Cursor contrast follows the displayed cell and input owner   | Contrast cases, native renderer/backend byte traces, movement, wide cells, visibility, recovery and failed writes | Theme/Host/Custom defaults; unknown colors invert displayed defaults; real-window screenshots remain separate         |
| Confirmed shared publication preserves complete appearance   | Server/Client publication traces and shared contract corpus                                                       | Shared settings lane and edit traces; physical display capability remains separate                                    |
| Startup and shared reload reach the real Client              | Isolated Windows ConPTY and root verification                                                                     | Native Windows process/byte evidence; physical emulator appearance is separate                                        |

Run `cargo test --locked -p orchbee --all-targets --all-features`
and the affected `pnpm verify` root gate. Named themes, Terminal colors and System appearance are current behavior;
physical host appearance and untested platforms require their own execution evidence.

Monochrome selection surfaces explicitly pair terminal-default foreground and background with reverse video, keeping mixed label and marker roles on one displayed band.
