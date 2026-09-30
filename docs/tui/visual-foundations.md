# TUI Visual Foundations

## Authority and scope

This standard owns color roles, the theme contract, text hierarchy, frames, glyphs, and display-width
rules for Orchbee-authored terminal interfaces: every surface a Rust Client paints into a character
cell grid, including chrome around retained terminal panes. Read it before authoring or reviewing
rendering code in a native Client.

Region composition and size tiers follow [`layout.md`](layout.md); status, hint, and overlay
behavior follows [`overlays-and-status.md`](overlays-and-status.md); host capability detection
follows [`terminal-environment.md`](terminal-environment.md). The frontend token registry in
[`frontend/design-tokens.md`](../frontend/design-tokens.md) does not apply to the cell grid, but its
rule that components consume semantic roles and themes supply concrete values does.

The content of a terminal pane belongs to the program running inside it. These rules govern only
Orchbee-drawn cells.

## Selection rules

- Choose a color or modifier by meaning, never by resemblance to another element or because the
  current value looks right on one terminal.
- Rendering code references semantic roles only. `ratatui::style::Color::Rgb`,
  `Color::Indexed`, and named ANSI variants appear only inside the theme module that maps roles to
  values. Review enforces this rule; no automated check exists yet.
- Every visible state has one role. Do not compose a state from two roles, borrow a status color
  for emphasis, or express hierarchy through color instead of modifiers.
- Explicit user-configured token foreground/bold/dim overrides are presentation data. The theme
  boundary applies them over each token's semantic role; rendering code still does not construct
  colors. State tokens keep state roles, ordinary values keep text roles, and focus/ordinal
  markers retain their separate roles. User overrides do not change state or identity.
- If no role fits, stop and propose a role change through the theme owner. Do not add a local
  constant.

## Color roles and themes

A theme maps every role below to one concrete value. A theme with a missing role is invalid. Roles
are stable names; themes change values.

| Role family | Roles                                            | Use when                                                                                |
| ----------- | ------------------------------------------------ | --------------------------------------------------------------------------------------- |
| `text`      | `primary`, `secondary`, `disabled`, `inverse`    | Content and chrome text by information priority; `inverse` only on a filled surface     |
| `accent`    | `focus`, `selection`, `link`                     | The focused frame or control, the current selection, and activatable references         |
| `status`    | `success`, `warning`, `error`, `info`, `pending` | Outcome and state meaning only; never decoration or hierarchy                           |
| `frame`     | `focused`, `unfocused`, `divider`                | Pane and overlay borders and split boundaries                                           |
| `surface`   | `chrome`, `overlay`, `selection`, `inactive`     | Backgrounds for persistent chrome, overlays, selected rows, and the unfocused-pane mask |
| `hint`      | `key`, `label`                                   | Key glyphs and their action labels in footers and help                                  |

Theme rules:

- Every native Client ships a `terminal-native` theme and uses it whenever host appearance or color
  level is unknown. It is the default until an explicit user setting selects another theme.
- The `terminal-native` theme maps text and surface roles to the terminal default foreground and
  background (`Color::Reset`) and accent, status, and frame roles to named ANSI-16 colors. It uses
  the `DIM` modifier, not a gray color, for `text-secondary`, so de-emphasis follows the host
  palette on both light and dark backgrounds.
- An RGB theme is designed for one background polarity and may be selected only when the host
  appearance is known through a probe or an explicit setting. It declares its polarity.
- The theme owner quantizes theme values once at startup to the detected color level (truecolor,
  256-color, or 16-color) using the nearest perceptual match. Rendering code never inspects the
  color level and never branches on the active theme.
- A theme value change is reviewed against every surface that consumes the role; a role addition
  updates every shipped theme in the same change.

Forbidden in any theme: `DarkGray` (bright black) for text, because many profiles map it close to
the background; `White` or `Black` as text on an unpainted surface; `Yellow` or `Blue` as text on
`Color::Reset` in the `terminal-native` theme, because their contrast depends on polarity.

## Text hierarchy

Hierarchy describes information priority, not interaction state.

| Level     | Presentation            | Use when                                                    |
| --------- | ----------------------- | ----------------------------------------------------------- |
| Heading   | `text-primary` + `BOLD` | Overlay titles and section labels                           |
| Primary   | `text-primary`          | Values, labels, and content the person must read to decide  |
| Secondary | `text-secondary`        | Supporting copy, metadata, timestamps, and key hint labels  |
| Disabled  | `text-disabled`         | Unavailable actions that remain visible for discoverability |

- Use at most these three visible levels in one region. Additional structure comes from spacing,
  grouping, or a frame, not from more colors or `ITALIC`.
- `BOLD` marks a heading or the active item; it never marks a warning. `UNDERLINED` marks only
  `accent-link`. `REVERSED` and `BLINK` are forbidden in chrome; `BLINK` is forbidden everywhere.
- Status roles color the status glyph or the status word, not the whole line.

## Interaction states

| State          | Presentation                                                                                       |
| -------------- | -------------------------------------------------------------------------------------------------- |
| Focused region | `frame-focused` border; an unframed region marks focus with `accent-focus` on its title            |
| Unfocused pane | `frame-unfocused` border; `surface-inactive` may mask its content only when the owner documents it |
| Selected row   | `surface-selection` background across the full row width                                           |
| Active tab     | `accent-focus` + `BOLD`; inactive tabs use `text-secondary`                                        |
| Pointer hover  | No visual change; the cell grid has no reliable hover                                              |
| Pending        | `status-pending` glyph plus a localized verb; frames are fixed-width                               |
| Disabled       | `text-disabled`; the action stays in help with its reason                                          |

Exactly one region shows focus at a time. Focus is visible in every layout tier.

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
- One glyph has one meaning across the Client. Document glyph meanings with the owning surface.

## Display width and text safety

- Measure text by display width, never by `char` or byte count. East Asian wide characters occupy
  two cells; combining marks occupy zero. Use the same width function for measuring, truncating,
  and laying out.
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

| Change                     | Required check                                                                                                                        |
| -------------------------- | ------------------------------------------------------------------------------------------------------------------------------------- |
| New or changed surface     | Rendered under `terminal-native` and one RGB theme at every required geometry in [`verification.md`](verification.md#render-evidence) |
| Theme role or value change | Every consuming surface re-rendered; the 16-color quantization result contains no `Rgb` or `Indexed` cell                             |
| Text with external origin  | A test proves control characters and escape sequences do not reach the buffer                                                         |
| Truncation or wrapping     | A test includes wide characters and a token wider than the line                                                                       |
| Glyph addition             | Its ASCII fallback is declared and rendered in the simplified-glyph test                                                              |
