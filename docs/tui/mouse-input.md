# TUI Mouse Input

## Authority and scope

This standard owns pointer capture policy, hit testing, the gesture vocabulary, passthrough to
programs inside terminal panes, wheel normalization, and selection for Orchbee-authored terminal
interfaces. Host mode negotiation follows [`terminal-environment.md`](terminal-environment.md);
keyboard ownership follows [`keyboard-input.md`](keyboard-input.md). The owning application
documents which regions accept which gestures; this standard defines the rules those mappings
must satisfy.

## Capture policy

- A full-screen Client enables pointer reporting at startup: button-event tracking (DEC mode
  `1002`) with SGR encoding (`1006`). Any-motion tracking (`1003`) is forbidden; it floods input and
  removes the host's native selection entirely.
- Pixel-position reporting (`1016`) is enabled only while a pane needs pixel coordinates for
  passthrough and is disabled when that need ends.
- The Client provides one binding and one command that release capture. While released, the status
  line shows a persistent indicator, the host terminal owns selection, links, and scrolling, and
  every Client gesture is unavailable. The same binding restores capture.
- The Client never binds a `Shift`-modified gesture. Hosts reserve `Shift` to bypass reporting for
  native selection, and that bypass stays available.
- Capture is released before any external program takes over the host terminal and restored when
  the Client resumes.

## Hit testing

- The renderer records the rectangle and type of every interactive region it paints in a frame.
  Pointer events resolve against the regions of the last drawn frame, never against a predicted
  layout.
- Region types are finite and documented by the owner: navigation entry, pane viewport, pane
  boundary, list row, action, scroll indicator, status segment, and overlay control.
- Regions never overlap within one layer. An overlay layer is tested before the layers beneath it,
  and a capturing overlay swallows every gesture outside itself.
- Recorded regions are discarded on resize and after any frame that changes layout. Between the
  change and the next draw, pointer events are dropped, not misrouted.
- A press and its release resolve against the region of the press. A drag that leaves its region
  continues to belong to it until release.

## Gesture vocabulary

Each gesture on a region resolves to at most one semantic intent. The intents below are the shared
vocabulary; a region may implement a subset.

| Gesture           | Region           | Intent                                                                                 |
| ----------------- | ---------------- | -------------------------------------------------------------------------------------- |
| Left press        | Navigation entry | Select that entry                                                                      |
| Left press        | Pane viewport    | Focus that pane; then forward the press when the pane's program reports the mouse      |
| Left press        | List row         | Move the selection to that row; a second press on the selected row activates it        |
| Left press        | Action           | Invoke the action if available; otherwise show its reason                              |
| Left drag         | Pane boundary    | Preview the resize during the drag; commit one resize on release                       |
| Left drag         | Pane viewport    | Select text when the pane's program does not report the mouse                          |
| Double left press | Pane viewport    | Select the word under the pointer                                                      |
| Triple left press | Pane viewport    | Select the line under the pointer                                                      |
| Wheel             | List or overlay  | Scroll the region                                                                      |
| Wheel             | Pane viewport    | Enter or move within retained history, or forward as arrow keys in an alternate screen |
| Left press        | Scroll indicator | Jump the viewport to the pressed position                                              |
| Right press       | Any              | Reserved; a context surface is a separate accepted decision                            |
| Middle press      | Any              | Reserved; paste through the pointer is a separate accepted decision                    |

- Multi-press detection uses a bounded interval and the same cell. A press in a different cell
  starts a new sequence.
- A gesture never mutates shared workbench state until release. Scroll stays Client-local; a
  retained terminal's selection belongs to its Terminal Host attachment and is projected by the
  Client.
- Every gesture intent has a keyboard equivalent. The pointer is an additional path, never the only
  one.

## Passthrough

- A pane's program owns the pointer inside that pane's viewport while it has enabled any pointer
  reporting mode, or while it uses the alternate screen with DEC 1007 enabled and the gesture is a wheel movement.
- Forwarded events are translated to pane-local coordinates and re-encoded in the mode the program
  requested. Events outside the viewport are never forwarded.
- Wheel movement over an alternate-screen program that does not report the mouse is forwarded as
  arrow-key presses only while DEC 1007 is enabled, three per tick by default. Disabling the mode
  ends queued arrow work and returns subsequent wheel movement to the Client.
- Focus changes caused by a press are Client-local and happen before forwarding.
- A pane boundary and every chrome region belong to the Client regardless of the program's
  reporting state.

## Wheel normalization

- Wheel reports carry direction only. Treat consecutive reports separated by less than a short gap
  as one stream, and convert a stream to line deltas with a documented lines-per-tick default of
  three.
- Coalesce scroll application to the redraw cadence; never draw once per report.
- A single flush scrolls at most one viewport height. Excess remains in the stream and applies on
  later flushes.
- Trackpad-like streams may accumulate fractional lines; a direction reversal clears the
  accumulator.
- Direction inversion and speed are user settings owned by the application, applied after
  normalization.

## Selection and clipboard

- The Terminal Host owns pane-selection endpoints and exported text. The Client projects that
  selection over live output or retained history with `surface-selection` from
  [`visual-foundations.md`](visual-foundations.md#color-roles-and-themes). It clears local gesture
  work on `Esc`, a press outside it, or an invalid generation; Host endpoint eviction clears the
  selection with a notice.
- An application declares one clipboard sink for each copy. It never broadcasts one copy to
  multiple sinks and never treats an unacknowledged OSC 52 write as proof that a platform fallback
  is safe. Clipboard work never blocks rendering, and failure produces a status notice.
- Exported text is complete Host-produced plain text with the pane's own logical line breaks;
  chrome cells and pane boundaries are never included. A partial, malformed, over-limit, or
  invalid UTF-8 export writes nothing.
- A selection whose rows are evicted from retained history is cleared with a notice, not silently
  moved.

## Verification

| Change                     | Required check                                                                                                                                          |
| -------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Capture policy change      | Tests prove the exact enable and release sequences and the released-state indicator                                                                     |
| Region addition            | Tests resolve a press inside, on each edge, and outside the region against the last drawn frame                                                         |
| Gesture mapping change     | A table-driven test maps each gesture to its single intent and proves the keyboard equivalent exists                                                    |
| Passthrough change         | Tests cover reporting on, reporting off with alternate screen, reporting off with primary screen, and boundary presses                                  |
| Wheel normalization change | Tests cover a wheel stream, a trackpad stream, a reversal, and the per-flush cap                                                                        |
| Selection change           | Tests cover gesture order, live/history projection, revision movement, generation cancellation, multi-line Unicode export, eviction, and `Esc` clearing |
