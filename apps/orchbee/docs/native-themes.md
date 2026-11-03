# Native themes

## Selection and ownership

The [shared device preferences](../../../crates/workbench-protocol/docs/user-preferences.md)
separate application appearance (Light, Dark, System) from terminal defaults (Host, Light, Dark,
Custom). Application System is the common default. System resolves TUI accents and overlays to One
Light or One Dark from an observed outer-terminal background; without that observation it uses
terminal-native contrast, even when the operating-system fallback reports a light or dark appearance.
The TUI does not change the outer host's font or appearance.

Terminal Host follows explicit application Light/Dark through the shared shipped terminal defaults.
With application System, Host paints terminal defaults with `Color::Reset`. System canvas,
persistent chrome and inactive surfaces also use the host-default background, including empty
cells, so the Client meets the host's surrounding padding without an RGB query or quantization.
Once the host background is observed, System derives the Tab strip and interaction fills from the
host instead: `surface-strip`, `surface-related`, `surface-hover`, `surface-current` and
`surface-pressed` mix 14, 10, 17, 21 and 28 percent of the host foreground into a dark background,
or 5, 7, 9, 12 and 16 percent into a light one. Until the foreground is observed, the matched named
theme's primary text stands in for it. The form-field band `surface-field` mixes the overlay
instead of the host background toward that foreground, 7 percent on dark and 5 on light, because
System keeps the palette overlay and a host background can match it. One Light and One Dark ship
the band as RGB 243,243,244 and 61,67,77. Derived values pass through the level's quantization and
collapsed-role fallbacks, so 16-color hosts may resolve hover and press to reverse video, a band
that collapses onto its dialog falls back to bracketed fields, and monochrome keeps its modifier
fallbacks. `surface-selection` keeps the named palette. A single
unframed terminal thus stays distinct from navigation on any host background. Without that
observation System resolves to terminal-native, which paints no strip or related fill.
Captured foreground/background values remain available as descriptive RGB in Settings; they never
replace host-relative painting. Retained child-frame launch defaults cannot replace this local choice. Explicit
terminal Light/Dark and Custom retain precedence; Custom uses the validated RGB pair. Only
default-marked cells are substituted.
Explicit program RGB, palette colors, OSC overrides, reversal, attributes and wide-cell geometry
retain their captured authority in live and retained rows. Empty canvas uses the selected terminal
defaults across the entire pane, including space beyond a smaller retained grid.

The one-shot OSC 10/11 probe retains each valid color once within its original 350 ms window.
Background settles polarity immediately; foreground can complete the same observation in either
reply order. Duplicates, malformed responses and late replies cannot replace settled colors.
These are Client-local display values, not shared terminal launch policy.

The shared `UserTerminalColors` resolver also supplies the Worker's new-terminal profile from the
same confirmed snapshot as scrollback and launch selection. OSC 10/11 queries then report those
captured defaults from terminal creation, before a program chooses its appearance. Host with System
launches with the colors a controlling Client, TUI or window, most recently reported, then the
Worker's OS light/dark fallback, then the shipped light baseline. Once both probe colors settle, the
Client reports them through the `host-colors` controller action; the Worker applies them to the
controlled terminal only while that terminal still follows Host defaults. Existing terminals
otherwise retain their query defaults until their runtime owner explicitly applies a terminal theme. Client appearance changes and reversible Settings previews recolor default-marked
presentation without mutating that shared query state, rewriting ANSI palette entries, or restarting
programs. Consequently a retained program's query-selected appearance can differ from a later local
preview; display recoloring alone does not establish runtime theme application.

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

Shared appearance preferences select terminal-native, one-light or one-dark rendering roles.
These roles map chrome, sidebars, menus, notices and selection overlays; renderers consume
them without changing focus, geometry or control.

The light/dark roles distinguish canvas, chrome and raised overlays. System uses matching Light/Dark
accents and overlays only with an observed host background; otherwise it uses terminal-native roles.
Its base surfaces remain host-relative in every color level. Quantization may collapse surface
differences while borders, labels and focus remain.
Overlay frames share theme corner and focus roles; titles use independent primary bold text.
Independent terminal popups use this complete overlay surface for the title, borders and pending
body, even on a dark-default outer host. A received program frame replaces only the inner body
with its independent terminal defaults and explicit program colors.

Each theme owns one accent per Tab group color. One Light and One Dark use their palette's hues;
terminal-native uses named ANSI colors, except orange, which has no ANSI name and uses RGB
`[255, 165, 0]`; a host at a lower color level quantizes it. A group header writes its name in the bold accent on a
resting tint that mixes 20% of the accent into the strip fill, including System's derived strip.
Terminal-native has no concrete strip color to mix, and 16-color hosts cannot express a faint fill,
so their headers rest on the strip with the accent text alone. A current, hover or pressed surface replaces only the tint; the accent name
stays readable on it, and leaving the header restores the tint or current surface. Group color
swatches in menus paint the same accent. Monochrome hosts use the ordinary role resolver and
interaction fallbacks.

The theme contract also provides `accent-action`, `surface-hover` and `surface-pressed`. For the
shared Light and Dark application appearances, these roles adapt the public product color intent in
[`tokens.css`](../../../packages/design-tokens/src/tokens.css): primary action, hovered surface and
pressed surface respectively. The Light cell-grid adapter makes the translucent hover and press
tokens opaque over its existing chrome surface; the Dark adapter resolves the declared neutral
references. A source-conformance test includes the CSS authority and pins both the semantic token
chain and its RGB projection. System reuses the matching named `accent-action` when a host background
is observed, derives its hover and pressed fills from the host as described above, and uses
terminal-native roles otherwise. These roles are available to renderers, but each surface
still requires consumer render evidence before it can claim adoption.

The sidebar's explicit red Delete intent uses independent `accent-destructive` metadata and palette
ownership, never `status-error`. Its shipped Light RGB is `[178, 52, 47]`, Dark is `[224, 108, 117]`,
and terminal-native uses ANSI red. Disabled content resolves to `text-disabled` before action tone;
it cannot acquire hover or pressed styling. Enabled destructive wording keeps its accent separately
from surrounding focus, navigation, current, hover and pressed tracks. At rest, terminal-native
destructive wording is ANSI red directly on the host default background without a backing fill:
a destructive menu row and a whole destructive confirmation button, including its brackets and key
hint, read as red text. Reference contrast for ANSI red on the host default is about 3.2:1 in
Campbell and VS Code Dark+ and 3.3:1 to 3.5:1 in One Half Light. System appearance, including the
terminal-native styles it adopts without an observed background, still backs destructive wording
on the host default with the resolver's pair, and every theme other than terminal-native keeps the
destructive key hint in `text-secondary`. Where reduced-color or native interaction uses reverse
video, or a known fill falls below 3:1, the theme resolver removes reversal from the destructive
content band and supplies its own contrasting backing pair. Components neither construct colors nor add
warning bold, underline or blink. Monochrome retains the localized Delete verb and ordinary markers.
Nominal palette tests cover the unpaired terminal-native rest state and the overlay, hover, pressed
and selection surfaces at truecolor, ANSI-256 and ANSI-16; customized host palette appearance still
requires real-window review.

Named-theme supporting text uses `[90, 93, 105]` in One Light and `[170, 177, 189]` in One Dark.
These audited secondary values clear 4.5:1 on every shipped surface, including pressed and
provisional-selection fills, and quantize separately from primary text at ANSI-256. The same
triplet feeds `text-disabled`, `hint-label` and `status-pending`; the pending modifier and disabled
availability remain independent. Secondary text is not dimmed to create priority at the expense
of its surface readability.

Named-theme muted text uses `[108, 110, 121]` in One Light and `[131, 138, 148]` in One Dark: the
quietest step from secondary toward chrome that still clears 4.5:1 on chrome. It is secondary text
on every fill, and under System on an observed host background where it would fall below 4.5:1,
such as a gray host lighter than about `#232323`. On a near-black host it keeps about 5.6:1.

One Dark primary text uses `[204, 211, 224]` with those secondary values and fills unchanged. Its
nominal sRGB primary/secondary contrast is approximately 1.434:1. Headings, primary values, key
glyphs and the existing primary surface foregrounds inherit this theme-owned value; renderers add
no local colors. Destructive intent, status, current identity, navigation, hover and pressed tracks
retain their separate semantics. Terminal preferences and explicit program colors keep the
independent authority described above.

Fixed RGB checks protect chosen palette values and the audited secondary boundary. A separate
regression uses a readable near-equal foreground pair to show why surface readability alone is
insufficient, then compares primary/secondary separation after content composition across resting,
current, selected, hover and pressed surfaces. Existing reduced-color and monochrome modifier
fallbacks still apply. These are nominal palette claims, not a universal hierarchy ratio or proof
of perceptual distinction; real-window light/dark, en-US/zh-CN and 80x24/120x40 review supplies that
separate evidence.

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
both named themes use a neutral fill against chrome and retain their primary/secondary
text colors. One Light uses `[224, 228, 235]` against `[240, 242, 245]` chrome so the persistent
row remains visible while input belongs to a terminal. This fill remains quieter than keyboard
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

The ordered Settings lane reads the shared authority on startup and polls once per second. Only a complete confirmed snapshot updates cached preferences and their resolved theme. Invalid sources and lost replies preserve last confirmed values and expose an issue. GUI edits and TUI saves use this same path without restarting terminals. Renderers read owned state, never files or environment. Native configuration reload remains responsible for its own fields and cannot replace confirmed common preferences.

[Settings appearance drafts](native-settings.md#apply-timing-and-capability-boundaries) supply a separate reversible palette and locale to the whole-frame compositor, including background chrome and terminal default markers. This projection never publishes runtime policy or replaces the confirmed cache; closing reveals the newest confirmed appearance, and program colors retain their independent authority.

## Correctness and evidence

| Claim                                                        | Evidence                                                                                                          | Bounds                                                                                                                                        |
| ------------------------------------------------------------ | ----------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------- |
| Complete role mapping and capability precedence              | Pure palette/resolver cases, quantized buffers, all consumer render cases                                         | Three themes, canonical palette, sampled environment; program colors excluded                                                                 |
| Shared Light/Dark interaction palette remains aligned        | CSS source-conformance test and exact foreign-input verification edge                                             | Public design tokens map shared Light/Dark roles; System uses observed-host accents or terminal-native roles with host-relative base surfaces |
| Dark text priority exceeds a readable near-equal control     | Resolved content separation and independent surface-readability cases                                             | Palette-specific truecolor evidence; reduced-color behavior and actual host appearance are separate                                           |
| Sidebar rows have one surface and distinct current ancestors | Workspace/Agent hover, press, navigation, collapse and supporting-line regression cases                           | Native, Light/Dark, ANSI-16 and monochrome; both locales; real-window acceptance remains separate                                             |
| Host-default launch colors follow explicit app appearance    | Shared resolver decisions, frozen preference snapshots and Worker launch-adapter profile cases                    | New terminal defaults and OSC query inputs; retained query state and Client previews remain separate                                          |
| Default markers follow the theme; explicit cells stay exact  | Live/retained cell rendering, mixed default/explicit RGB and empty canvas cases                                   | Native Reset and both named themes; monochrome, 16/256 colors and truecolor; physical host appearance is separate                             |
| Cursor contrast follows the displayed cell and input owner   | Contrast cases, native renderer/backend byte traces, movement, wide cells, visibility, recovery and failed writes | Host/Light/Dark/Custom defaults; unknown colors invert displayed defaults; real-window screenshots remain separate                            |
| Confirmed shared publication preserves complete appearance   | Server/Client publication traces and shared contract corpus                                                       | Shared settings lane and CAS traces; physical display capability remains separate                                                             |
| Startup and shared reload reach the real Client              | Isolated Windows ConPTY and root verification                                                                     | Native Windows process/byte evidence; physical emulator appearance is separate                                                                |

Run `cargo test --locked -p orchbee --all-targets --all-features`
and the affected `pnpm verify` root gate. Shared custom colors and System appearance are current behavior;
physical host appearance and untested platforms require their own execution evidence.

Monochrome selection surfaces explicitly pair terminal-default foreground and background with reverse video, keeping mixed label and marker roles on one displayed band.
