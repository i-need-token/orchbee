# TUI Layout

## Authority and scope

This standard owns region composition, size tiers, degradation order, overlay geometry, truncation,
and resize behavior for Orchbee-authored terminal interfaces. Colors, frames, and glyphs follow
[`visual-foundations.md`](visual-foundations.md); overlay content and status behavior follow
[`overlays-and-status.md`](overlays-and-status.md). The owning application documents its concrete
regions and their thresholds; this standard defines the rules those decisions must satisfy.

## Regions

A full-screen Client divides the host area into persistent chrome and content, with transient status overlays:

| Region        | Contract                                                                                       |
| ------------- | ---------------------------------------------------------------------------------------------- |
| Top strip     | At most one row; holds navigation such as tabs and the current location                        |
| Status line   | One row over content when needed; never reserves terminal height; remains above other overlays |
| Side panel    | Optional; docked at Full width, an overlay at Compact width, hidden at Minimal width           |
| Content       | The remaining area; receives every row and column the chrome does not claim                    |
| Pane boundary | One cell between split panes, shared by both neighbors                                         |

- Chrome row and column counts are fixed per tier and documented by the owner. Chrome never grows
  to fit its content; content adapts through tiers.
- A side panel may reserve a fixed footer within its own columns. Its list scrolls independently;
  the footer never reserves rows in adjacent terminal content.
- Content is never smaller than one row by one column while the chrome fits. When it cannot fit,
  the chrome yields in the documented degradation order before content disappears.
- Every size is an integer number of cells. Percentages resolve to cells once and clamp to the
  region's documented minimum and maximum before any other computation.
- A split pane keeps at least the minimum inner size documented by the owner; a split that cannot
  satisfy both children's minimums is refused, not drawn clipped.

## Size tiers

Tiers give hints, chrome, and overlays a shared vocabulary for degradation. Width and height are
classified independently; a surface uses the lower of the two.

| Tier    | Width         | Height     | Behavior                                                                |
| ------- | ------------- | ---------- | ----------------------------------------------------------------------- |
| Full    | ≥ 100 columns | ≥ 24 rows  | Complete hints, docked side panel, full labels                          |
| Compact | 60–99 columns | 12–23 rows | Shortened hints, side panel as overlay, truncated labels                |
| Minimal | < 60 columns  | < 12 rows  | Mode indicator and exit hint only; overlays that cannot fit are refused |

Below 20 columns or 5 rows the Client paints one localized too-small notice and the status line,
nothing else, and resumes normal rendering when the host grows.

Degradation follows one documented order per surface. The default order is: drop optional context
text, shorten hint labels, drop hint labels while keeping the mode indicator, undock the side panel,
hide the side panel, truncate labels. A surface never skips a step to reach a smaller result faster.

## Overlays

- An overlay is positioned relative to the content area, centered horizontally and vertically.
  Centering rounds toward the top-left cell.
- A pointer-opened context menu instead anchors to the triggering cell in the full viewport above
  the status line, including navigation chrome. Clamp its complete frame to that viewport; never
  move a sidebar trigger into the pane area merely because general overlays use that area.
  Keyboard opening retains the owning surface's deterministic placement.
- Overlay width is `min(preferred, area.width − 4)` and height is `min(preferred, area.height − 2)`.
  Preferred sizes are cells or percentages resolved through the rules above.
- An overlay whose resolved inner area is smaller than 4 columns by 3 rows is not drawn. The
  triggering action reports its result through the status line instead.
- The overlay clears the cells beneath it before drawing so pane content never shows through.
- Capturing overlays retain a visible, current mode indicator and exit hint. A status overlay
  occupies content cells without resizing the child terminal; dismissal repaints the covered cells.
- Overlay content that exceeds the inner height scrolls and shows its position; the frame and the
  footer never scroll.

## Truncation and wrapping

- Truncate by display width using the rules in
  [`visual-foundations.md`](visual-foundations.md#display-width-and-text-safety). Labels truncate
  at the end; paths truncate in the middle; identifiers such as numbers never truncate.
- A row that holds a label and a key hint keeps the hint intact and truncates the label.
- A list wider than its area truncates each row independently; it never introduces horizontal
  scrolling.
- Prose wraps at word boundaries. A wrapped block never exceeds its declared maximum height; excess
  content scrolls or is summarized through a documented shorter form.
- The active tab or selected row stays visible: a strip or list windows around the active item so it
  is fully drawn before its neighbors are.

## Scrolling regions

- A list or text region taller than its area shows its position with a one-column scrollbar or a
  `current/total` indicator; the owner documents which.
- Scrolling moves the viewport, never the selection, unless the selection would leave the viewport.
- Page movement scrolls one viewport height minus one row so context remains; half-page movement is
  optional.
- A region that follows live output pins to the bottom until the person scrolls up, then holds its
  position until an explicit return to bottom.

## Resize

- Every frame computes layout from the current host geometry. Cached rectangles serve only pointer
  hit testing against the last drawn frame and are discarded when geometry changes.
- A resize triggers a full redraw. Zero or absent geometry is ignored, and the last valid geometry
  remains in effect until a valid one arrives.
- A child terminal pane receives a resize only when its inner geometry changed. The final geometry
  of a resize burst is always applied even when intermediate ones are coalesced.
- Overlays recompute their geometry on resize and close themselves when they can no longer fit,
  reporting through the status line.

## Verification

| Change                         | Required check                                                                                                              |
| ------------------------------ | --------------------------------------------------------------------------------------------------------------------------- |
| Region or tier change          | Rendered at every required geometry in [`verification.md`](verification.md#render-evidence), including the too-small notice |
| Degradation order change       | One test per step proves the previous step's content is present and the next step's content is absent                       |
| Overlay addition               | Tests cover the largest, the clamped, and the refused geometry                                                              |
| Truncation or windowing change | Tests include wide characters and an active item beyond the visible window                                                  |
| Resize handling                | A test proves a stale cached rectangle is not used after the geometry changes                                               |
