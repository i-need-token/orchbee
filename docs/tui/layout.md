# TUI Layout

## Authority and scope

This standard owns region composition, information priority, alignment, size tiers, degradation
order, overlay geometry, viewport anchoring, truncation, and resize behavior for
Orchbee-authored terminal interfaces. Colors, frames, and glyphs follow
[`visual-foundations.md`](visual-foundations.md); overlay content and status behavior follow
[`overlays-and-status.md`](overlays-and-status.md); reusable surface and control composition follows
[`components.md`](components.md). The owning application documents its concrete regions and their
thresholds; this standard defines the rules those decisions must satisfy.

These rules are required for new and materially changed TUI behavior; their presence does not claim
every current Client conforms. Runtime adoption requires source and the evidence in
[`verification.md`](verification.md#contract-review). Record any staged gap with its owner and
completion condition.

## Regions

A full-screen Client divides the host area into persistent chrome and content, with transient status overlays:

| Region        | Contract                                                                                                                                                               |
| ------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Top strip     | At most one row; holds navigation such as tabs and the current location                                                                                                |
| Status line   | One row over content when needed; never reserves terminal height; remains above other overlays                                                                         |
| Side panel    | Optional; docked at Full width, and at Compact width while content keeps the Compact minimum beside it; otherwise an overlay at Compact width; hidden at Minimal width |
| Content       | The remaining area; receives every row and column the chrome does not claim                                                                                            |
| Pane boundary | One cell between split panes, shared by both neighbors                                                                                                                 |

- Chrome row and column counts are fixed per tier and documented by the owner. Chrome never grows
  to fit its content; content adapts through tiers.
- Within one tier, item count, label length, status, and async activity never change docked chrome
  or primary-content geometry. Only host geometry, an explicit layout action, or a tier transition
  may do so.
- A side panel may reserve a fixed footer within its own columns. Its list scrolls independently;
  the footer never reserves rows in adjacent terminal content.
- Content is never smaller than one row by one column while the chrome fits. When it cannot fit,
  the chrome yields in the documented degradation order before content disappears.
- Every size is an integer number of cells. Percentages resolve to cells once and clamp to the
  region's documented minimum and maximum before any other computation.
- A split pane keeps at least the minimum inner size documented by the owner; a split that cannot
  satisfy both children's minimums is refused, not drawn clipped.

## Information priority and alignment

Allocate space in this order: the task target and required decision; primary commitment, return,
exit, or recovery controls; current context and position; current-task state; supporting metadata;
then decoration. A lower-priority item never displaces, clips, or visually dominates a higher one.

- Every composed screen devotes its dominant area and emphasis to work content. A non-split screen
  has one dominant work region; peer panes in a split retain their geometry while focus identifies
  the current input region. Persistent chrome explains location and navigation without repeating
  the same context and operation in several places. Help remains discoverability, not a duplicate
  primary control.
- Each surface defines only the alignment tracks it needs, such as disclosure, state, ordinal,
  label, metadata, and trailing action. Those tracks remain stable across that surface's rows;
  conditional children, status, selection, or actions do not shift an identity label between rows
  or frames. Distinct components need not share unused tracks.
- A trailing value or action receives a reserved track before the label is measured. When that
  track is absent for a row, the row keeps the same label boundary as its peers.
- Related rows use spacing and indentation from one declared hierarchy. Indentation is computed
  from depth, not from whether a row currently exposes a disclosure glyph or status marker.
- When Compact hides or overlays a state-bearing side panel, a summary may use spare existing
  chrome only from that panel owner's projection. Preserve unknown, loading, unavailable, and
  incomplete source coverage separately from known empty or no matches; incomplete coverage never
  implies absence. Localize the summary and keep it subordinate to active navigation and required
  controls. Elide its detail before an active navigation target, and do not expand hit regions or
  input admission to keep a summary current. The owning surface records its source and wording.

## Row geometry

- Resolve every visible row's height from its semantic content, wrapping policy, and current width
  before viewport placement and pointer hit testing. Code never assumes `index == y` when rows can
  wrap, expand, or reveal detail.
- Selection, current-context fill, and hit rectangles cover the row's complete resolved height,
  including deliberate internal padding but excluding the gap between groups.
- When an intent must reveal a semantic row, the viewport scrolls only enough to draw that row in
  full. A row taller than the viewport aligns to its start and exposes its own documented scroll or
  expansion behavior.
- Reflow updates row geometry as one layout result. Rendering and pointer dispatch consume the same
  result; neither reconstructs offsets independently.

## Size tiers

Tiers give hints, chrome, and overlays a shared vocabulary for degradation. Width and height are
classified independently; a surface uses the lower of the two.

| Tier    | Width         | Height     | Behavior                                                                                                                                                                         |
| ------- | ------------- | ---------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Full    | ≥ 100 columns | ≥ 24 rows  | Complete hints, docked side panel, full labels                                                                                                                                   |
| Compact | 60–99 columns | 12–23 rows | Shortened hints, side panel docked while content keeps 60 columns beside it and otherwise an overlay, truncated labels                                                           |
| Minimal | < 60 columns  | < 12 rows  | Hide optional top-strip actions to keep the first terminal row clear; preserve complete return/exit and primary actions while usable, otherwise suspend into the too-small state |

Below 20 columns or 5 rows the Client paints one localized too-small notice and the status line,
nothing else, and resumes normal rendering when the host grows. Any suspended surface remains in
its model; the status line preserves a currently valid return, exit, or recovery hint, and hidden
submit or confirmation actions are unavailable.

Degradation follows one documented order per surface. The default order is: remove decoration and
repeated supporting text, drop optional metadata, use documented short labels and hints, move
secondary actions into a discoverable overflow, undock the side panel, hide the side panel, then
truncate labels. A surface never skips a step to reach a smaller result faster.

An interactive task that remains visible reserves complete return or exit and primary-action
controls before optional content. A shortcut is one atomic key-and-label record: use its documented
short form, wrap the complete record, or remove it by priority. Never draw a chopped key or label.
If the required target and controls do not fit, refuse that surface or use the too-small state
instead of presenting an incomplete task.

## Overlays

- An overlay is positioned relative to the content area, centered horizontally and vertically.
  Centering rounds toward the top-left cell. An application-wide Settings surface instead centers
  within the full host viewport above the transient status row, independent of navigation chrome.
- A pointer-opened context menu instead anchors to the triggering cell in the full viewport above
  the status line, including navigation chrome. Clamp its complete frame to that viewport; never
  move a sidebar trigger into the pane area merely because general overlays use that area.
  Keyboard opening retains the owning surface's deterministic placement.
- Overlay width is `min(preferred, area.width − 4)` and height is `min(preferred, area.height − 2)`.
  Preferred sizes are cells or percentages resolved through the rules above.
- A newly requested overlay whose resolved inner area is smaller than 4 columns by 3 rows is not
  opened. The triggering action reports its refusal through the status line and performs no domain
  effect. An already-open overlay follows the suspension contract in
  [`overlays-and-status.md`](overlays-and-status.md#lifecycle-and-return).
  A compact context menu instead needs one complete action row, its frame and any reserved reason;
  it allocates no title or footer rows and never publishes controls that did not fit.
- The overlay clears the cells beneath it before drawing so pane content never shows through.
- Capturing overlays retain a visible, current mode indicator and exit hint; compact context menus
  use their visible menu frame and ordinary Esc/Left/outside policy without extra hints. A status overlay
  occupies content cells without resizing the child terminal; dismissal repaints the covered cells.
- Allocate the fixed shell slots from
  [`overlays-and-status.md`](overlays-and-status.md#overlay-composition) before the content viewport.
  Header, navigation, query, status, actions, and footer never scroll with content. If they leave
  less than the declared minimum content area, refuse the overlay rather than crop a fixed slot.
- Overlay content that exceeds the remaining inner height scrolls and shows its position, except
  compact context menus omit the position counter. Variable
  row heights participate in that viewport calculation.

## Truncation and wrapping

- Truncate by display width using the rules in
  [`visual-foundations.md`](visual-foundations.md#display-width-and-text-safety). Labels truncate
  at the end; paths truncate in the middle; identifiers such as numbers never truncate.
- A row that holds a label and a key hint keeps the hint intact and truncates the label.
- A key hint remains an intact key-and-label record. Multiple hints wrap only between records; when
  wrapping is unavailable, lower-priority records disappear before any record is clipped.
- A list wider than its area truncates each row independently; it never introduces horizontal
  scrolling.
- Prose wraps at word boundaries. A wrapped block never exceeds its declared maximum height; excess
  content scrolls or is summarized through a documented shorter form.
- An active tab stays fully visible: its strip windows around it before drawing neighbors. A
  selector reveals its provisional cursor for selector navigation or under its declared
  cursor-visible policy; ordinary viewport movement may leave that cursor offscreen.

## Scrolling regions

- A list or text region taller than its area shows its position with a one-column scrollbar or a
  `current/total` indicator; the owner documents which.
- Every scroll-capable surface distinguishes viewport movement from selector navigation. Wheel
  input is viewport movement by default; the owner documents any key whose meaning depends on the
  active control.
- Viewport movement preserves domain selection, committed value, and the logical content anchor; a
  manually positioned viewport does not snap back merely because a provisional cursor is offscreen.
  A selector may declare a cursor-visible policy that relocates only its provisional cursor to the
  nearest fully visible eligible row after viewport movement. That relocation never activates a
  row or changes domain state.
- Selector navigation moves its provisional cursor by semantic row and scrolls only enough to draw
  that row in full. Activation by the current input owner reveals the resulting current tab or
  item; background state changes never move the viewport.
- A viewport page movement scrolls one viewport height minus one row so context remains; half-page
  movement is optional.
- A region that follows live output pins to the bottom until the person scrolls up, then holds its
  position until an explicit return to bottom.
- A viewport stores a semantic anchor: stable item identity and, for wrapped content, a logical
  content position plus an intra-line display offset. Content insertion, removal, reflow,
  expansion, and resize preserve that anchor; an absolute display-row number is not an anchor. If
  the item disappears, use the next surviving item, then the previous one, then the region's
  empty-state origin.
- Bottom follow is an explicit viewport state. New output and reflow keep the bottom pinned only
  while it is active; preserving an ordinary anchor never silently re-enables follow.

## Resize

- Every frame computes layout from the current host geometry. Cached rectangles serve only pointer
  hit testing against the last drawn frame and are discarded when geometry changes.
- A resize triggers a full redraw. Zero or absent geometry is ignored, and the last valid geometry
  remains in effect until a valid one arrives.
- Before reflow, capture each scrolling region's semantic anchor. After row heights and regions are
  recomputed, restore it first. An active exclusive editor or required decision then receives the
  smallest adjustment needed to keep its focused control visible; a selector does the same only
  under its documented cursor-visible policy. Otherwise the reading anchor wins, even when a
  provisional cursor remains offscreen. No case changes domain selection or resets an ordinary
  viewport to the top or bottom.
- A child terminal pane receives a resize only when its inner geometry changed. The final geometry
  of a resize burst is always applied even when intermediate ones are coalesced.
- Overlays recompute their geometry on resize. When they can no longer fit, they enter the suspended
  too-small state in [`overlays-and-status.md`](overlays-and-status.md#lifecycle-and-return) rather
  than discarding a draft, required decision, accepted work, or recovery; normal rendering resumes
  when they fit again.

## Verification

| Change                         | Required check                                                                                                                                                                                                         |
| ------------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Region or tier change          | Rendered at every required geometry in [`verification.md`](verification.md#render-evidence), including the too-small notice                                                                                            |
| Alignment or row change        | Mixed-depth, mixed-status, absent-action, wrapped, and expanded rows retain their declared tracks; render and pointer hits use identical resolved heights                                                              |
| Degradation order change       | One test per step proves the previous step's content is present and the next step's content is absent; required controls and hints remain whole                                                                        |
| Overlay addition               | Tests cover the largest and clamped geometry, new-size refusal, open-size suspension and resumption; fixed slots remain fixed while content scrolls                                                                    |
| Truncation or windowing change | Tests include wide characters, complete hint records, variable-height rows, an offscreen provisional cursor, and an active tab beyond the visible strip                                                                |
| Content mutation or resize     | Tests preserve an ordinary semantic anchor and bottom-follow independently, apply focus-or-cursor visibility priority without changing domain selection, use the removal fallback, and reject stale pointer rectangles |
