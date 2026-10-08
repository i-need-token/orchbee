# TUI shell layout

## Scope and ownership

The native Client shell divides the host area into a top strip, a content region, an optional side
panel, a pane area with shared boundaries, and a one-row status line. This reference documents the
concrete regions, thresholds, and degradation order that `docs/tui/layout.md` requires the owning
application to declare. It owns the layout projection only: colors, glyphs, and frame style belong
to the theme surface, overlay contents belong to their surface renderers, and terminal cell drawing
belongs to `terminal_cells`.

The projection is pure. `tui_layout::project_shell_areas` computes every region from the current
frame rectangle and the sidebar surface the view asks for; `tui_shell` computes the terminal
geometry from the same two functions (`project_shell_areas` then `terminal_area`), so the drawn
content area and the wire geometry can never diverge. No render code mutates business state, clocks,
or I/O, and no layout value is cached across frames.

## Viewport tiers

Width and height are classified independently and the lower tier wins.

| Tier     | Width tier (columns) | Height tier (rows) |
| -------- | -------------------- | ------------------ |
| Full     | >= 100               | >= 24              |
| Compact  | 60..=99              | 12..=23            |
| Minimal  | 20..=59              | 5..=11             |
| TooSmall | < 20                 | < 5                |

`ViewportTier::from_size(columns, rows)` in `tui_layout` is the single classifier. Chrome row and
column counts per tier:

| Chrome      | Full              | Compact                  | Minimal   | TooSmall    |
| ----------- | ----------------- | ------------------------ | --------- | ----------- |
| Top strip   | 1 row (tabs)      | 1 row (tabs)             | none      | none        |
| Status      | 1 row             | 1 row                    | 1 row     | 1 row       |
| Side panel  | docked 30 columns | overlay 30 columns, left | hidden    | hidden      |
| Panel frame | drawn             | drawn                    | not drawn | not drawn   |
| Content     | remainder         | remainder                | remainder | notice only |

At TooSmall the Client paints one localized too-small notice and the status line, nothing else, and
resumes normal rendering when the host grows. The sidebar surface the view requests is an intent;
`project_shell_areas` translates it per tier (`Docked` and `Overlay` both mean visible: docked at
Full, overlay at Compact, hidden below), so the rendered chrome and the wire geometry agree without
the caller knowing the tier.

## Status line

Exactly one bottom row, always drawn, never covered by an overlay. Segments left to right follow
`docs/tui/overlays-and-status.md`: input indicator (the armed prefix badge, empty in the ordinary
layer), context or notice, then right-aligned hints. A segment is drawn whole or dropped; the
context segment drops first, hint text shortens by tier (full hint at Full, `Ctrl+B ?` at Compact,
the exit hint at Minimal and TooSmall). The status text area is reported back to the caller only
when it was drawn completely, preserving the existing draw gate that refuses a confirmation receipt
on a partial draw.

On Windows the ordinary status layout reserves distinct cells for the Menu button and, when the
owner's complete hint still fits, the Machines indicator. The optional indicator uses the label's
display width, capped at 28 columns; it yields to notices, prefix/resize modes and capturing
surfaces. Each frame passes its allocation to the Machines painter, so the final paint and pointer
region cannot cover another status segment. The armed prefix replaces the Menu button with its
badge and an `Esc` cancellation hint, shortening to the key when needed, then to the badge alone. Menus and Settings
replace the complete status row, including its background, before painting their owner's hint.

## Content area and overlays

The content area overlays position against is `ShellAreas.panel`: the workbench region below the
top strip and beside a docked side panel. `tui_layout` exposes two pure functions for surface
renderers:

- `overlay_area(&ShellAreas) -> Rect` — the content region every centered overlay uses.
- `place_overlay(area, preferred_width, preferred_height) -> Option<Rect>` — resolves to
  `min(preferred, area - 4)` by `min(preferred, area - 2)`, centered with the extra cell rounding
  toward the top-left, and returns `None` when the resolved inner area would be below 4x3.

A refused overlay is not drawn; the caller reports through the status line instead. Because the
status row is drawn after every overlay and modal, the never-covered invariant holds even while
older surface renderers still compute their own geometry.

## Split panes

`divide_panes(area, orientation)` returns `PaneDivision { first, divider, second }` where `divider`
is exactly one shared cell column or row between the children, or `None` when either child would be
smaller than the documented 4x3 minimum. The old 50/50 intent is preserved: the divider cell is
taken out of the second child's share, so `first` keeps the width a 50% split used to give it.

A leaf draws no edge borders: edges on the region boundary are served by the region frame and edges
facing a divider are served by the divider cell. Each leaf owns a one-row title line inside its
rectangle; the active leaf's title carries focus emphasis while the pane owns input, and the leaf
below the title receives
the terminal cells. An inactive leaf paints a localized empty state naming the retained terminal
and the focus binding — it never paints fabricated live content. Divider cells are collected during
the walk and drawn once with junction glyphs (`├ ┤ ┬ ┴ ┼`) resolved from neighboring divider cells.

Under a Client-local zoom (`Ctrl+B z`; the session contract lives in `native-tab-session.md`),
the drawn set narrows to the focused leaf: it renders across the whole content area and records
the only pane hit region, while undrawn leaves still report their canonical leaf rectangles so
their viewers stay alive. The focused leaf reports the whole content area to the wire so its
terminal converges to the zoomed size; divider cells are not drawn while zoomed. This split
between the drawn/hit set and the observed set is the only zoom geometry effect — the canonical
projection itself is unchanged.

Capturing surfaces replace the pane's focus emphasis with the unfocused frame and secondary title
styles. Reference overlays preserve pane focus and use an unfocused frame. Sidebar section headings
use primary bold text rather than focus color.
The Site status colors only its glyph; its text uses the primary text role.
Ready uses the existing success role, while other phases retain their warning
role. Live and retained terminal cells share the theme-default color projection:
default-marked foreground and background follow the theme, and explicit program
colors remain unchanged.

## Degradation order

Per `docs/tui/layout.md`: drop optional context text, shorten hint labels, drop hint labels while
keeping the mode indicator, undock the side panel, hide the side panel, truncate labels. The tier
table above binds those steps to concrete rows and columns so no surface skips a step.

## Resize

Every frame recomputes the projection from the live host rectangle. A resize to a degenerate
terminal area still yields a nonzero inner projection while the host is at least 4 columns by 3
rows, so the session loop survives small geometry and the too-small screen remains exitable;
keeping the last valid geometry below that floor is the environment slice's concern.

## Correctness evidence

`cargo test -p orchbee` covers: the tier boundary matrix including the width/height lower-of-two
rule, the one-row status and strip, sidebar presentation per tier, overlay clamping, centering, and
refusal, pane division geometry and the shared divider, plus TestBackend renders at 120x40, 80x24,
60x12, 50x10, 15x4 and mixed-axis geometries in `en-US` and `zh-CN` proving the status row is never
covered, the too-small notice is localized, and active/long-label tab tokens stay visible through
display-width truncation.

## Tab metadata annotations

The native compact strip and navigator keep one token/row per canonical Tab. Pinned Tabs use the
localized `Pinned` / `固定` annotation; grouped Tabs show `group label / Tab label`. The full text
passes the existing sanitizer and cell-width budget, with active/selected styles supplied by the
ordinary Tab theme roles. Drawn widths alone define the stable Tab-ID pointer regions. Annotations
never become raw names, automatic ordinals, extra focus stops or synthetic terminal rows.
