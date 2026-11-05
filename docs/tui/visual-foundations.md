# TUI Visual Foundations

## Authority and scope

This standard owns color roles, the theme contract, text hierarchy, frames, glyphs, and display-width
rules for Orchbee-authored terminal interfaces: every surface a Rust Client paints into a character
cell grid, including chrome around retained terminal panes. Read it before authoring or reviewing
rendering code in a native Client.

Region composition and size tiers follow [`layout.md`](layout.md); status, hint, and overlay
behavior follows [`overlays-and-status.md`](overlays-and-status.md); host capability detection
follows [`terminal-environment.md`](terminal-environment.md). Shared GUI/TUI meaning, hierarchy,
and theme correspondence follow [`INTERFACE-DESIGN.md`](../INTERFACE-DESIGN.md). The frontend CSS
registry in [`frontend/design-tokens.md`](../frontend/design-tokens.md) and the native theme module
implement those roles in their respective media; rendering consumes roles and themes supply values.
Reusable controls and their state combinations follow [`components.md`](components.md).

The role inventory below is the required contract for new and migrated rendering code. Its presence
does not assert that every existing native theme or surface has completed adoption; implementation
support is established only by source and the required render evidence.

The content of a terminal pane belongs to the program running inside it. These rules govern only
Orchbee-drawn cells.

## Selection rules

- Choose a color or modifier by meaning, never by resemblance to another element or because the
  current value looks right on one terminal.
- Rendering code references semantic roles only. `ratatui::style::Color::Rgb`,
  `Color::Indexed`, and named ANSI variants appear only inside the theme module that maps roles to
  values. Review enforces this rule; no automated check exists yet.
- Each visual channel of a visible state has one role. A control may combine independent channels,
  such as a background for pointer interaction, a marker for keyboard focus, and a glyph for a
  chosen value at the control's declared boundary. Do not combine two roles in one channel, borrow a
  status color for emphasis, or bypass the `text` roles with a component-local color or modifier.
- Explicit user-configured token foreground/bold/dim overrides are presentation data. The theme
  boundary applies them over each token's semantic role; rendering code still does not construct
  colors. State tokens keep state roles, ordinary values keep text roles, and focus/ordinal
  markers retain their separate roles. User overrides do not change state or identity.
- If no role fits, stop and propose a role change through the theme owner. Do not add a local
  constant.

## Color roles and themes

A theme maps every role below to one concrete style. A style may include a foreground, background,
or permitted modifier. A theme with a missing role is invalid. Roles are stable names; themes
change values.

| Role family | Roles                                                                                                              | Use when                                                                                                                                                                                                                                        |
| ----------- | ------------------------------------------------------------------------------------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `text`      | `primary`, `secondary`, `muted`, `disabled`, `inverse`                                                             | Content and chrome text by information priority; `inverse` only on a filled surface                                                                                                                                                             |
| `accent`    | `action`, `destructive`, `focus`, `selection`, `link`                                                              | The primary action at rest, explicit destructive intent, the focused frame or control, provisional or checked indicators, selected-range boundaries, and activatable references                                                                 |
| `status`    | `success`, `warning`, `error`, `info`, `pending`, `working`, `idle`, `unread`                                      | Outcome and state meaning only; never decoration or hierarchy                                                                                                                                                                                   |
| `agent`     | `attention`, `done`, `error`                                                                                       | Agent state glyphs for Needs attention, Completed, and Failed only; never text, status words, or emphasis                                                                                                                                       |
| `frame`     | `focused`, `unfocused`, `divider`, `guide`                                                                         | Pane and overlay borders, split boundaries, and low-contrast hierarchy guides                                                                                                                                                                   |
| `surface`   | `canvas`, `chrome`, `strip`, `overlay`, `field`, `hover`, `pressed`, `selection`, `current`, `related`, `inactive` | Backgrounds for the terminal canvas, persistent chrome, the Tab strip, overlays, editable form fields, pointer feedback, pressed controls, provisional or range selection, persistent current context, associated ancestors, and inactive panes |
| `hint`      | `key`, `label`                                                                                                     | Key glyphs and their action labels in footers and help                                                                                                                                                                                          |

Theme rules:

- `frame-guide` is quieter than a region divider. `surface-related` identifies an ancestor of the
  provisionally selected item with a weaker fill than that item's own surface; a current ancestor
  keeps `surface-current`, which it shares with its current descendant. It never
  transfers focus or selection; direct interaction on that ancestor takes precedence. Unknown-host
  and reduced-color themes may use the base background, retaining hierarchy and independent markers.
- Every native Client ships a `terminal-native` theme, shown to people as the terminal's own
  colors. By default a Client paints the named theme of the active appearance only on a truecolor
  host whose appearance it knows, from an explicit Light or Dark setting or from the background the
  host reports. It uses `terminal-native` when host appearance or color level is unknown, when the
  host is below truecolor, and when the person chooses the terminal's own colors. Where named
  themes cannot paint, the theme choices stay visible as unavailable with the reason.
- Named themes come from the shared theme catalog and its role resolver; a Client does not
  hand-author a named theme's roles, except where a cross-client contract binds a role value.
- The `terminal-native` theme maps text and base surfaces (`canvas`, `chrome`, `strip`, and
  `overlay`) to the terminal default foreground and background (`Color::Reset`). It resolves current and interaction
  surfaces through the paired colors and modifier fallbacks below, and maps accent, frame, and the
  remaining status roles to named ANSI-16 colors. `status-working` uses the terminal default
  foreground, and `status-idle` that foreground with `DIM`, so both read on either polarity. It uses
  the `DIM` modifier, not a gray color, for `text-secondary`, so de-emphasis follows the host palette
  on both light and dark backgrounds. `text-muted` is that same `DIM` text: no quieter host-relative
  step stays readable on every host palette.
- `surface-current` uses a neutral fill to distinguish persistent context from an accented selection.
  In `terminal-native`, this is a paired ANSI foreground/background: host-relative body and metadata
  text, and the host-relative `status-working` and `status-idle` glyphs, adopt the paired foreground
  on this fill. Secondary text and `status-idle` drop DIM there; Working keeps its breathing
  intensity. Other status roles, `agent-*` roles, and explicit token colors retain their meaning.
  RGB themes keep their text hierarchy. Because ANSI 7 can sit within a few steps of a light host's
  default background, `terminal-native` current context also underlines its complete band, as
  Monochrome does, including leading blank cells, status glyphs and checked-value cells; an owner
  that documents `BOLD` identity, such as the active Tab, keeps it as well. When quantization
  collapses the fill onto chrome, current identity keeps a cue that configurable text emphasis
  cannot remove: a stable marker, or an underline across a row whose owner documents the fill as its
  only current cue.
- `agent-attention`, `agent-done`, and `agent-error` each have one fixed value shared by every
  theme, including `terminal-native`. Quantization maps them to the
  nearest color at each level, which is `Yellow`, `Green`, and `Red` at 16 colors, and to the host
  default in monochrome, where shape fallbacks keep outcomes distinct. The state's word, wherever it
  appears, keeps its `status-*` text role.
- `surface-hover`, `surface-pressed`, `surface-selection`, and `surface-current` remain ordered and
  distinguishable after quantization. When a terminal-native or reduced-color mapping cannot
  provide four distinct fills, hover uses the resolver's `REVERSED` band, pressed also bolds the
  action's primary label, provisional selection retains its `accent-selection` marker, and current
  context retains its fill where one remains, otherwise its current marker, `BOLD` or underline.
  Within that band, a `status-*` or `agent-*` glyph of a known color keeps its foreground on
  whichever of ANSI black or white contrasts more with it, rather than becoming a block of its own
  color; host-relative status glyphs reverse with the band's text. A row that mixes text roles in
  the band pairs it with primary text on chrome, and every other cell, including secondary text,
  resting accents, explicit token colors and blanks, takes that foreground without `DIM`. The row
  keeps one displayed background even where a host applies `DIM` before reverse video swaps the
  colors. Individual components apply the semantic state but never branch on the palette.
- Monochrome gives current context an underline, provisional selection a bold reverse band, and
  focused frames a bold reverse outline. `accent-focus`, the focus cue of an unframed region, its
  divider or a focused button's brackets, adds `BOLD` and `UNDERLINED`, so it never reads as part of
  an adjacent reverse band. Every cell of the provisional band that is not explicitly plain takes
  the band's `BOLD` without `DIM`: a host can brighten bold text or apply `DIM` before reverse
  video, which would otherwise show metadata on a background of its own. Independent current,
  navigation and checked markers remain visible; focus and value choices never rely on bold text
  alone. Renderers retain the complete role style, including its modifiers.
- `accent-destructive` identifies an explicitly destructive action intent, independently of action
  hierarchy, focus, current context, hover, and press. It has its own palette value and never borrows
  `status-error`. Disabled intent uses `text-disabled`. In `terminal-native`, resting destructive
  wording is the host palette's red directly on the host default background, without a backing
  fill: a destructive menu row reads as red text, and so does a destructive button, including its
  brackets and key hint. Every other theme keeps the destructive key hint in `text-secondary`. The
  theme resolver retains the destructive foreground on a contrast pair when a native reverse band
  or a known fill below 3:1 would obscure it; the surrounding
  interaction band and navigation/focus markers remain. Monochrome retains the explicit localized
  action wording and ordinary interaction cues, without introducing warning bold, underline, or
  blink. Named ANSI contrast is checked against the reference palette and actual host evidence; a
  customized host palette is not inferred from RGB tests.
- `accent-action` identifies the one primary action in the current decision context while it is at
  rest. It never identifies keyboard focus, selection, or a successful outcome; those meanings keep
  their own roles.
- An RGB theme is designed for one background polarity and declares it. It paints only when the
  host appearance is known through a probe or an explicit setting, and only at truecolor: a lower
  level cannot hold its canvas and ANSI palette, so a quantized copy would be a different theme.
- The theme owner quantizes theme values once at startup to the detected color level (truecolor,
  256-color, or 16-color) using the nearest perceptual match, then resolves any collapsed
  interaction roles to their documented modifier and marker fallbacks. Rendering code never
  inspects the color level and never branches on the active theme.
- A theme value change is reviewed against every surface that consumes the role. When runtime code
  adopts a new role, that implementation change updates every shipped theme atomically; a partially
  populated runtime role inventory is invalid.
- Paint a region's complete background, including padding and empty cells, through its surface
  role. RGB themes distinguish canvas, persistent chrome and overlays; a border alone is not the
  region's surface. Terminal-native and lower color levels retain their admitted host behavior.
- A named theme takes every surface, including `surface-strip`, `surface-related` and the
  interaction fills, from its palette. A Client never derives fills from the host background.
- `surface-field` is the band of an editable form field and stays distinct from `surface-overlay`.
  `terminal-native` and Monochrome paint no field fill, and the theme owner drops the fill wherever
  quantization collapses it onto `surface-overlay`. A field without its own fill draws its
  bracketed boundary, as described in [`components.md`](components.md#search-and-editors).

Forbidden in any theme: `DarkGray` (bright black) for text, because many profiles map it close to
the background; `White` or `Black` as text on an unpainted surface; `Yellow` or `Blue` as text on
`Color::Reset` in the `terminal-native` theme, because their contrast depends on polarity. These
restrictions govern text, including status words. `agent-*` glyphs are not text: their fixed values
may quantize to `Yellow` on a host-default surface, because the glyph is a one-cell indicator whose
state also has a status word and a shape fallback without color.

## Text hierarchy

Hierarchy describes information priority, not interaction state.

| Level     | Presentation            | Use when                                                    |
| --------- | ----------------------- | ----------------------------------------------------------- |
| Heading   | `text-primary` + `BOLD` | Overlay titles and section labels                           |
| Primary   | `text-primary`          | Values, labels, and content the person must read to decide  |
| Secondary | `text-secondary`        | Supporting copy, metadata, timestamps, and key hint labels  |
| Muted     | `text-muted`            | Labels that only group the rows under them                  |
| Disabled  | `text-disabled`         | Unavailable actions that remain visible for discoverability |

- Use at most three hierarchy levels in one region. Disabled is an availability treatment applied
  to affected content, not a hierarchy level.
- A theme derives `text-muted` as the quietest step from `text-secondary` toward its chrome that
  still clears 4.5:1 on chrome. Muted text keeps that contrast only at rest: on any interaction,
  current or related fill, it is `text-secondary`. Where a theme has no measurable step, as in `terminal-native` and Monochrome,
  it equals `text-secondary`. Additional
  structure comes from spacing, grouping, or a frame, not from more colors or `ITALIC`.
- `BOLD` marks a heading, a current item whose owner documents bold identity, such as the active
  Tab, or the label or glyph of a pressed enabled action; it never marks pointer hover or a warning.
  `UNDERLINED` marks only `accent-link`, `terminal-native` current context, current context where
  no current fill can show, and Monochrome `accent-focus`.
  `REVERSED` is available only to the theme resolver as a terminal-native or reduced-color fallback
  for an interactive band; component renderers never request it directly. `BLINK` is forbidden
  everywhere.
- Status roles color the status glyph or the status word, not the whole line.

## Interaction states

| State                 | Presentation                                                                                                                                                                                                                                                                                               |
| --------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Focused region        | `frame-focused` border; an unframed region uses `accent-focus` on its title or a dedicated marker                                                                                                                                                                                                          |
| Unfocused pane        | `frame-unfocused` border; `surface-inactive` may mask its content only when the owner documents it                                                                                                                                                                                                         |
| Pointer hover         | `surface-hover` across the effective hit rectangle, without `BOLD`; an enabled action label resting in `text-secondary` rises to `text-primary`; an icon action instead strengthens its glyph to `text-primary` without a fill; action menus may move the provisional candidate and use its selection fill |
| Pointer pressed       | `surface-pressed` plus `BOLD` on the enabled action label or glyph while the primary button remains held on the armed target                                                                                                                                                                               |
| Provisional selection | `surface-selection` across the complete row plus the shared `accent-selection` navigation marker when the component provides one                                                                                                                                                                           |
| Selected range        | `surface-selection` across the selected cells; endpoints or the active edge use `accent-selection` when they are drawn                                                                                                                                                                                     |
| Current context       | Neutral `surface-current` across the complete row; current identity adds `BOLD` or a stable marker independently of focus, or its owner documents the fill as the only cue and underlines the row where no fill can show; `terminal-native` always underlines the complete row                             |
| Checked value         | A stable glyph using `accent-selection` for the value at the control's declared boundary; navigation remains independent                                                                                                                                                                                   |
| Active tab            | Neutral `surface-current` plus `BOLD` or a stable current marker; `terminal-native` also underlines the Tab; keyboard focus and provisional navigation remain separate                                                                                                                                     |
| Pending               | Fixed-width `status-pending` glyph plus a localized verb; the control footprint stays fixed                                                                                                                                                                                                                |
| Disabled              | `text-disabled`; no hover or pressed treatment; the action stays in help with its reason                                                                                                                                                                                                                   |

Exactly one region shows focus at a time. Focus is visible in every layout tier. Project it once at
the keyboard-owner boundary: a framed collection uses its `frame-focused` border, while an unframed
focused row uses a dedicated `accent-focus` track. A capturing overlay owns input through exclusive
occlusion instead: it keeps a neutral `frame-unfocused` border, and its heading and terminal caret or
provisional marker identify the owner. A row inside the framed collection does not repeat the
owner's focus marker; its reversible navigation anchor remains independently visible.

Current context and keyboard focus are separate facts. The current context remains identifiable
while input belongs elsewhere; moving keyboard focus must not erase that indication. A surface
documents both presentations and their simultaneous appearance. Where the fill is the only current
cue, a navigation or pointer surface replaces it only on the row that holds that navigation or
pointer, and leaving the row restores it.

A checked value may represent a staged or parent-draft value as well as a confirmed domain value. It
does not by itself prove persistence. When it differs from the confirmed or effective basis, the
control retains a separate changed-draft cue defined by [`components.md`](components.md#value-controls).

Current, checked, provisional, focus, hover, pressed, availability, and pending are independent
facts. Within the background channel, precedence is pressed, provisional selection, hover, current,
then the component's base surface. Markers, glyphs, text hierarchy, and status remain visible when a
higher-priority background wins. [`components.md`](components.md#state-model) owns required
combinations and control behavior.

## Frames and borders

- Frames use single-line box-drawing characters. Rounded corners are a theme option applied
  uniformly; a surface never selects its own corner style.
- Adjacent panes share one boundary cell. Do not draw two borders between two panes.
- A frame title truncates from the end with an ellipsis for labels and from the middle for paths.
  It never grows the frame or wraps.
- Draw a frame only when the inner area is at least one cell in each dimension; below that, draw
  the boundary line alone or nothing. Never paint a partial frame.
- Dividers between chrome regions use `frame-divider`; they are one cell wide.

## Glyphs

- Chrome assumes a UTF-8 host. Box drawing, arrows, `…`, and `•` are permitted. When
  [`terminal-environment.md`](terminal-environment.md#capability-detection) reports a non-UTF-8
  locale or simplified glyphs are enabled, every non-ASCII glyph renders through its declared ASCII
  fallback.
- Private-use glyphs, Nerd Font icons, and emoji are forbidden in chrome: their width is
  host-dependent and their presence is not guaranteed.
- Every glyph set with alternating frames, such as a spinner, keeps a constant display width so
  neighbors do not shift.
- Provisional row navigation uses the shared one-cell `›` marker with `>` as its ASCII fallback.
  Feature renderers reserve its track but never choose a local substitute or reuse it for current,
  checked, or changed state.
- Disclosure uses `▾` while expanded and `▸` while collapsed, with `v` and `>` as ASCII fallbacks,
  so a collapsed disclosure never reads as the `+` or `-` of an ASCII status glyph.
- A checked option, such as a group's applied color in its menu, shows `✓` in its checked track,
  with `*` as its ASCII fallback.
- One glyph has one meaning across the Client. Document glyph meanings with the owning surface.
  ASCII fallbacks may share a character only when their tracks never coincide: the collapsed
  disclosure `>` follows an identity or label, while the navigation `>` occupies the leading
  navigation track.

## Display width and text safety

- Measure text by display width, never by `char` or byte count. Display width sums the cell width
  of each extended grapheme cluster, as the buffer writes the text and the terminal grid places
  it: East Asian wide characters occupy two cells, combining marks occupy zero, and a ligature
  across clusters, such as Lam-Alef, occupies one cell per cluster. Use the same width function
  for measuring, truncating, laying out, and placing text; never place chrome text by a
  whole-string measure that merges such ligatures.
- Truncation markers are measured with that width function and are budgeted before the text. `…`
  is East Asian ambiguous width; the ASCII fallback is `..`.
- Wrap prose at word boundaries by display width. Do not break a URL or file path inside a token
  unless the token alone exceeds the line width.
- Sanitize text that originates outside the Client before painting it into chrome: strip control
  characters, escape sequences, and line breaks from labels, titles, and paths. The chrome buffer
  never interprets ANSI from user or process data.
- Never shrink, drop, or abbreviate a required action or instruction to fit; use the tiers in
  [`layout.md`](layout.md#size-tiers) to choose a shorter documented form.

## Verification

| Change                             | Required check                                                                                                                        |
| ---------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------- |
| New or changed surface             | Rendered under `terminal-native` and one RGB theme at every required geometry in [`verification.md`](verification.md#render-evidence) |
| Runtime theme role or value change | Every consuming surface re-rendered; the 16-color quantization result contains no `Rgb` or `Indexed` cell                             |
| Text with external origin          | A test proves control characters and escape sequences do not reach the buffer                                                         |
| Truncation or wrapping             | A test includes wide characters and a token wider than the line                                                                       |
| Glyph addition                     | Its ASCII fallback is declared and rendered in the simplified-glyph test                                                              |
