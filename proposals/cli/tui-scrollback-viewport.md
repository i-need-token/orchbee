# TUI Scrollback Viewport

| Field              | Value                                                                                                                                                                                                                                                                                                                                                     |
| ------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Status             | Proposal under review; the wire semantics below are verified against `crates/terminal-protocol`, `schemas/terminal/references/protocol.md`, and `apps/terminal-host`; the Client-local viewport model adapts the proven `packages/terminal` projected-terminal behavior for the TUI; key bindings, clipboard transport, and presentation stay provisional |
| Owner              | TUI viewport experience and the maintainers of the Rust `orchbee` Client, the OBCS/OBCN relay vocabularies, and `packages/terminal`                                                                                                                                                                                                                       |
| Current authority  | `schemas/terminal` and `apps/terminal-host/docs/runtime-model.md` own wire and Host semantics; `apps/orchbee/README.md` owns TUI invariants                                                                                                                                                                                                               |
| Parent design      | [`native-command-surface.md`](native-command-surface.md) owns the TUI-as-peer-Client contract; [`native-runtime-topology.md`](../platform/native-runtime-topology.md) owns the Client/Server/Worker/Host relay topology                                                                                                                                   |
| Decision authority | The product owner decides key bindings, clipboard transport, and presentation details through `m3-input-design`; this proposal decides only the viewport model and its use of existing wire semantics                                                                                                                                                     |
| Exit condition     | A TUI user can scroll upward through retained terminal history and copy historical text with no wire-protocol changes and no shared-state mutation; this proposal is deleted once the implemented surface is documented in the owning package guides                                                                                                      |

Simplified Chinese review translation:
[tui-scrollback-viewport.zh-CN.md](tui-scrollback-viewport.zh-CN.md).
English remains canonical.

## Purpose and scope

The TUI today renders only the live terminal frame: `TerminalProjection` stores the latest
`TerminalTextFrame`, and `tui_render/terminal_cells.rs` paints it. A user cannot scroll upward
through retained output or copy historical text. This proposal defines the Client-local scrollback
viewport that closes that gap using only existing wire semantics.

In scope:

- The data plane: how the TUI obtains history rows through the existing attachment `read`
  capability, `HistoryDemand`, `ReadRows`, `RowsApplied`, selection, and selection export.
- The Client-local viewport state machine, its offset and anchoring semantics, its behavior on new
  frames, automatic return to bottom, and invalidation on session switch or retirement.
- Provisional interaction bindings, selection and copy, limits and failure behavior, and the shared
  semantic boundary with the parallel `m3-terminal-read` implementation.
- The internal relay additions required to carry existing wire semantics to the TUI.

Out of scope:

- No wire-protocol additions or changes; every message used here already exists.
- No implementation code.
- No full scrollback search UI design; search stays a follow-up slice (see
  [Reserved decisions and gaps](#reserved-decisions-and-gaps)).
- No changes to `apps/`, `crates/`, repository `docs/`, other proposals, or `.oxlintrc.json`.

## Verified wire semantics

All message kinds, roles, and limits in this section are source-verified. OBTP is the Client to
Terminal Host wire; nothing in it changes.

### Attachment capability and projection demand

- A viewer attaches with `AttachTerminal` requesting `AttachmentCapabilityState { read: true,
input: false, resize: false }` and no resize lease; the Host answers
  `AttachmentCapabilities` with the granted immutable capabilities. The existing TUI viewer attach
  already requests exactly this grant, so scrollback needs no new capability.
- `SetProjectionDemand { demand: Active | Paused }` controls whether the Host pushes frame events;
  `ProjectionDemandAccepted` echoes the accepted demand, and `ProjectionBlocked` reports projection
  capture failure. Pausing demand while viewing history is the mechanism that stops live-frame churn.

### History demand, events, and observation

- `SetHistoryDemand { demand: FollowingBottom | ViewingHistory }` selects the attachment's history
  mode; `HistoryDemandAccepted` echoes it. Switching to `ViewingHistory` arms history tracking and
  marks the attachment dirty so the next `HistoryChanged` publishes current state; switching back to
  `FollowingBottom` releases the pending and committed anchors.
- `HistoryChanged` is an event carrying `attachment_id`, positive `generation`, `revision`, positive
  attachment-local `sequence`, `total_rows`, and `anchor_status` in `{ Valid, None, Evicted }`.
  `Valid` means the committed anchor survived; `None` means no committed anchor exists; `Evicted`
  means retained-row eviction, screen replacement, or pin loss destroyed it.
- `HistoryObserved` acknowledges one event by echoing its `sequence`, `revision`, and `generation`.
  Events coalesce while one awaits acknowledgement; stale or duplicate acknowledgements are
  idempotent no-ops and never clear a newer flight.
- Mutation, generation change (resize), screen change, and eviction each mark a viewing attachment
  dirty and republish `HistoryChanged` with the resulting anchor status.

### Row windows and anchors

`ReadRows` is scoped by attachment, `detail` (`Render` or `Text`), `purpose` (`Viewport`, `Search`,
or `Buffer`), positive `request_sequence`, exact `generation`, `revision`, `revision_policy`
(`Exact` or `CurrentView`), `row_count`, `maximum_bytes`, and one origin:

- `Absolute { row }` names a retained absolute row.
- `Oldest { offset }` names an offset from the oldest retained row.
- `Tail { rows_before }` names how many rows remain below the requested viewport.
- `Committed { delta }` applies a bounded signed delta (`±4096`) to the attachment's last applied
  anchor.

Policy rules, verified in `row_access.rs` and the Host runtime model:

- `Exact` requires the requested revision to equal the current Host revision and is valid for every
  purpose and origin.
- `CurrentView` is valid only for a `Viewport` request with `Tail` or `Committed` origin. Its
  revision is an observation cursor, not an admission fence: under the terminal mutex the Host
  resolves the origin against the then-current view and returns the captured current revision,
  generation, and total-row count. Continuous PTY output therefore cannot starve interactive reads.
- Both policies require the exact current generation, so a resize or screen-generation transition
  cannot reinterpret an old request.
- At most one request is in flight per attachment and purpose lane; `Viewport`, `Search`, and
  `Buffer` lanes are independent.
- The Host resolves the origin once and returns one contiguous `RowsSnapshot` row-window record
  (`PayloadEncoding::RowWindow`). A response is only a pending anchor; the matching `RowsApplied`
  request promotes it to the committed anchor that `Committed` origins and selection gestures
  resolve against.
- Resize abandons generation-pinned pending captures; a committed anchor follows a valid Ghostty
  reflow pin. Screen replacement, detach, or eviction invalidates anchors and republishes
  `HistoryChanged` accordingly.

The `RowsSnapshot` header reports the fields a viewport needs: detail, columns, host epoch,
attachment, request sequence, captured revision, captured generation, total retained rows, absolute
first row, contiguous row count, and body layout. `Render` detail concatenates one OBPB projection
snapshot plus OBKG graphics; `Text` detail stores plain-text rows with optional logical mappings and
no styles.

### Selection and export

- `SelectionGesture` actions (`Start`, `Update`, `End`, `Cancel`) carry the attachment's exact
  generation and viewport-relative row/column endpoints but no revision. Under the terminal mutex
  the Host resolves each point at processing time against the committed tracked anchor while the
  attachment is viewing history, or against the active viewport otherwise. The client therefore
  supplies surface coordinates only; it never translates history rows itself.
- `SelectionCommand` (`Clear`, `SelectAll`, `SetRange`, `SelectOutput`) carries exact
  generation and revision. Published `SelectionChanged` endpoints are absolute row/column pairs;
  state is `Active`, `None`, or `Evicted`, where `Evicted` means the retained rows are gone.
- `SelectionExport` produces plain text only: `SelectionExportStarted`, then at most one
  `SelectionExportChunk` event in flight (up to 1 MiB plus a 32-byte header), paced by
  `SelectionExportChunkApplied`, ending in `SelectionExportFinished` or a `SelectionExportCancel`
  with reason `ClipboardFailed`, `Disposed`, `Replaced`, or `User`. Total export is capped at
  16 MiB. The wire performs no platform clipboard effect; writing the clipboard is the Client's
  local responsibility.

## Relay surface and required additions

The wire is complete, but today the internal relay cannot carry it to the TUI. Verified facts:

- OBCS `TerminalViewerAction` and OBCN `TerminalViewerAction` currently carry only `Attach`,
  `AttachBinding` (OBCS), `Poll`, `FrameApplied`, `NeedSnapshot`, and `Detach`. No action reaches
  the history, row-access, selection, or export controls.
- The Worker's `terminal_viewer_operation.rs` maps each viewer action to a correlated OBTP
  `exchange()`; `request_id` doubles as the OBTP `CorrelationId`. The same pattern can carry every
  control this proposal needs.
- The Worker's `HostViewers` routes only frame records (kinds 41/42) and `ProjectionBlocked`
  (kind 49) into one record slot per viewer. A second undelivered record retires the viewer.
  `HistoryChanged` (kind 53), `SelectionChanged` (kind 62), `SelectionExportChunk` (kind 65), and
  `SelectionExportFinished` (kind 67 event) would currently be rejected as unexpected events.
- `TerminalViewerOutcome::Record(Vec<u8>)` already returns opaque record bytes on both hops, so
  poll-delivered event records and request-correlated response records need no new outcome shape.
- `MAXIMUM_VIEWER_RECORD_BYTES` is 64 bytes + 36 MiB on both hops; a row window may legally reach
  44 MiB, so clients must bound `maximum_bytes` below the relay record cap.

Required additions (internal relay vocabulary, additive only — OBTP itself is untouched):

| Layer                                | Addition                                                                                                                                                                                                                                                                                        |
| ------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| OBCS and OBCN `TerminalViewerAction` | New typed variants mirroring the OBTP controls: `SetHistoryDemand`, `HistoryObserved`, `ReadRows`, `RowsApplied`, `SelectionGesture`, `SelectionCommand`, `SelectionExport`, `SelectionExportChunkApplied`, `SelectionExportCancel`                                                             |
| Worker viewer routing                | A bounded per-viewer FIFO (recommended depth at least 8) for non-frame event records — `HistoryChanged`, `SelectionChanged`, `SelectionExportChunk`, `SelectionExportFinished` — kept separate from the single frame slot; overflow retires the viewer, matching the existing saturation policy |
| Worker exchange mapping              | Each new action performs one correlated OBTP `exchange()` and returns the response record (or `FeedbackAccepted`-style acknowledgement) through the existing outcome vocabulary                                                                                                                 |
| Server local-terminal service        | Transparent passthrough of the new actions and outcomes, consistent with existing viewer relay                                                                                                                                                                                                  |
| TUI client                           | `LocalClient::submit_terminal_viewer` plumbing for the new actions plus a bounded Rust OBRW decoder for `Render`-detail windows (OBRW header plus the inner OBPB snapshot mapped onto the cell types the renderer already draws; OBKG placements may be skipped in the first slice)             |

## Shared semantic boundary with `terminal read`

`m3-terminal-read` implements the one-shot `orchbee terminal read` CLI over the same wire
primitives. The boundary is the purpose lane and the lifecycle, not a different protocol.

| Concern          | Shared wire/transport layer (both consumers)                                                           | TUI scrollback viewport (this proposal)                                                            | `terminal read` CLI (sibling task)                                                           |
| ---------------- | ------------------------------------------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------- |
| Attachment       | `read` capability attach; `AttachTerminal`/`DetachTerminal`; capability/epoch validation               | Reuses the existing long-lived viewer attachment                                                   | May hold a short-lived read attachment and detach after output                               |
| Demand           | `SetHistoryDemand`/`HistoryDemandAccepted` semantics                                                   | `ViewingHistory` for the session's viewing duration                                                | `ViewingHistory` only for the read, then restore                                             |
| Row access       | `ReadRows`/`RowsSnapshot`/`RowsApplied`; generation/revision/request-sequence identity; lane isolation | `purpose: Viewport`, `CurrentView` policy, `Tail`/`Committed` origins, `Render` detail, one flight | `purpose: Buffer`, `Exact` policy, `Absolute`/`Oldest` origins, `Text` detail, paged flights |
| Events           | `HistoryChanged`/`HistoryObserved` exact acknowledgement                                               | Acknowledge every event; anchor status drives refresh or recover                                   | Acknowledge events received during the read                                                  |
| Selection/export | Gesture, command, `SelectionChanged`, chunked plain-text export                                        | Drives copy through the committed anchor                                                           | May use export or `Text` reads for output                                                    |
| Identity rules   | Attachment, host epoch, generation, revision, request sequence, event sequence                         | Enforced per flight; stale windows discarded                                                       | Enforced per page; stale pages retried or reported                                           |
| Limits           | Negotiated `rowWindowBytes`, relay record cap, protocol hard bounds                                    | Viewport-sized windows; overscan cache                                                             | Byte-budgeted pages with adaptive shrink                                                     |

Nothing TUI-specific leaks into the CLI: viewport phase, scroll offset, overscan cache, key and
mouse intent, selection gestures, and clipboard transport are Client-local. Conversely, the CLI's
one-shot snapshot does not drive the TUI viewport. Neither consumer adds a separate read protocol;
both extend the same relay vocabulary described above.

## Data plane

### Entering the scrollback viewport

Entry is non-modal: the first scroll-up intent moves the viewport into history without seizing
keyboard input. Preconditions: primary screen (`TerminalTextFrame.alternate_screen == false`),
`total_rows` exceeds the visible row count, no active `disposed` state.

```text
User scroll-up intent
  → Client: SetHistoryDemand { demand: ViewingHistory }
  → Client: SetProjectionDemand { demand: Paused }
  → Host:   HistoryDemandAccepted          (echo)
  → Host:   ProjectionDemandAccepted       (echo)
  → Host:   HistoryChanged { anchor_status: None|Valid, generation, revision, sequence, total_rows }
  → Client: HistoryObserved { sequence, revision, generation }
  → Client: ReadRows { purpose: Viewport, detail: Render, revision_policy: CurrentView,
                       origin: Tail { rows_before: n }, row_count: visible + overscan,
                       generation, revision, request_sequence, maximum_bytes }
  → Host:   RowsSnapshot record            (pending anchor)
  → Client: install window as the rendered view
  → Client: RowsApplied { request_sequence, generation, revision }
  → Host:   RowsApplied echo               (pending anchor promoted to committed anchor)
```

The `anchor_status: None` on entry is expected, not an error: no committed anchor exists until the
first `RowsApplied`. The viewport entry request uses `Tail { rows_before }` because the client knows
only how far above the bottom it wants the window — absolute row numbering is not yet known.

### Scrolling while viewing

- Line and page deltas resolve locally to a new `rows_before` value, then issue one
  `ReadRows { origin: Committed { delta }, revision_policy: CurrentView }` flight on the `Viewport`
  lane. `CurrentView` lets the Host resolve under the mutex against the committed anchor, so PTY
  output advancing `revision` never makes the read stale.
- Jumps (`Home`, drag-to-position) resolve a target `rows_before` and issue
  `ReadRows { origin: Tail { rows_before }, revision_policy: CurrentView }`.
- Requests arriving while a flight is pending accumulate into a single pending delta and dispatch
  when the lane frees — the wire allows at most one flight per lane.
- Each accepted `RowsSnapshot` replaces the cached window, updates `start_row`, `rows_before`,
  `revision`, `generation`, and `total_rows`, and is acknowledged with `RowsApplied` only after the
  client installs it for rendering.
- While viewing, every `HistoryChanged` is acknowledged with `HistoryObserved`; `Valid` anchor
  status triggers a `Committed { delta: 0 }` refresh flight, and any other status triggers
  recovery (see below).

### Selection and copy

```text
User drag / keyboard selection intent
  → Client: SelectionGesture { action: Start|Update|End|Cancel, generation,
                               viewport-relative point }
  → Host:   SelectionChanged event { state: Active { start, end, rectangle, preview } }
  → Client: (copy intent) SelectionExport { format: Plain, sequence }
  → Host:   SelectionExportStarted
  → Host:   SelectionExportChunk event { sequence, chunk_sequence, final, bytes }   (≤ 1 MiB each)
  → Client: SelectionExportChunkApplied { sequence, chunk_sequence }                (paces next chunk)
  → Host:   SelectionExportFinished event
  → Client: writes assembled plain text to the local clipboard transport
```

Because the Host resolves gesture points against the committed anchor, selection over historical
rows needs no client-side row translation. `SelectionCommand` (`Clear`, `SelectAll`, `SetRange`,
`SelectOutput`) covers programmatic selection and cancellation.

### Returning to live output

Any of — scrolling to `rows_before == 0`, `End`, terminal input, an evicted anchor, detach, or a
terminal switch — exits the viewport:

```text
Client: SetHistoryDemand { demand: FollowingBottom }
Client: SetProjectionDemand { demand: Active }
  → Host:   both accepted echoes
  → Host:   next live snapshot frame (fresh base; deltas cannot follow a pause)
  → Client: discard history window and cache; render the live frame
```

The TUI keeps the last live frame visible underneath while entering, and restores live rendering on
the first frame after reactivation — the same `recovering → live on next snapshot` rule the web
client uses.

## Client-local viewport state

### States

| State        | Meaning                                                                                                      |
| ------------ | ------------------------------------------------------------------------------------------------------------ |
| `live`       | Following bottom: live frame projection is rendered; no history window is installed                          |
| `entering`   | First `Viewport`-lane `ReadRows` flight outstanding; the frozen last live frame stays on screen              |
| `viewing`    | A committed history window is rendered; zero or one `Viewport`-lane refresh/scroll flight may be outstanding |
| `recovering` | Return-to-bottom requested; waiting for the next live snapshot frame before discarding history state         |
| `disposed`   | Attachment, terminal, or session gone; all viewport state is dropped                                         |

### Offset and anchoring model

- Primary representation: `start_row`, the absolute retained row index of the first visible row,
  taken from the applied `RowsSnapshot` header. Absolute rows are stable across revisions; retained
  row 0 is the oldest retained row, and a gap between 0 and the oldest retained index marks evicted
  history.
- Navigation representation: `rows_before = total_rows − (start_row + visible_rows)`, the number of
  retained rows below the visible window. Scroll intents edit `rows_before`; `Tail` origins consume
  it directly.
- Stability authority: the Host-owned committed tracked anchor. `RowsApplied` pins the applied
  window; `Committed { delta }` origins and selection gestures resolve against it under the terminal
  mutex. The client never re-derives anchoring from frame state.
- Identity carried per viewport session: `attachment_id`, `host_epoch`, `generation`, `revision`,
  `total_rows`, `start_row`, `rows_before`, `request_sequence` counter, and the last observed
  `HistoryChanged` sequence.

### State transition table

| State                  | Event                                                                             | Next         | Action                                                                                                                              |
| ---------------------- | --------------------------------------------------------------------------------- | ------------ | ----------------------------------------------------------------------------------------------------------------------------------- |
| `live`                 | First live frame / normal frame                                                   | `live`       | Apply projection as today                                                                                                           |
| `live`                 | Scroll-up intent; alternate screen, no scrollback, or `total_rows` ≤ visible rows | `live`       | Show a localized "no scrollback" status hint                                                                                        |
| `live`                 | Scroll-up intent; primary screen with scrollback                                  | `entering`   | Send `SetHistoryDemand(ViewingHistory)`, `SetProjectionDemand(Paused)`, and a `Tail` `ReadRows` flight                              |
| `entering`             | `HistoryDemandAccepted` / `ProjectionDemandAccepted`                              | `entering`   | Record acceptance; still waiting on the row flight                                                                                  |
| `entering`             | `HistoryChanged`                                                                  | `entering`   | `HistoryObserved`; `anchor_status` is informational until the first window lands                                                    |
| `entering`             | `RowsSnapshot` matching the flight                                                | `viewing`    | Install window, render it, send `RowsApplied`                                                                                       |
| `entering`             | Typed failure, stale or malformed window, send failure                            | `recovering` | Fail closed: restore `FollowingBottom` + `Active` and surface a status hint                                                         |
| `viewing`              | Line/page scroll intent                                                           | `viewing`    | Accumulate or dispatch `Committed { delta }` `CurrentView` flight                                                                   |
| `viewing`              | `Home` / jump intent                                                              | `viewing`    | `Tail { rows_before }` `CurrentView` flight toward target                                                                           |
| `viewing`              | `End` or scroll reaching `rows_before == 0`                                       | `recovering` | `SetHistoryDemand(FollowingBottom)` + `SetProjectionDemand(Active)`                                                                 |
| `viewing`              | `RowsSnapshot` matching the flight                                                | `viewing`    | Replace window; `RowsApplied` after install                                                                                         |
| `viewing`              | `RowsSnapshot` stale (generation/revision/request sequence mismatch)              | `viewing`    | Discard; issue a `Committed { delta: 0 }` refresh if none pending                                                                   |
| `viewing`              | `HistoryChanged` with `anchor_status: Valid`                                      | `viewing`    | `HistoryObserved`; refresh flight `Committed { delta: 0 }`                                                                          |
| `viewing`              | `HistoryChanged` with `anchor_status: Evicted` or `None`                          | `recovering` | `HistoryObserved`; anchor identity is gone, return to bottom                                                                        |
| `viewing`              | Live frame record arrives while paused                                            | `recovering` | Unexpected under paused demand; treat as desynchronization, recover                                                                 |
| `viewing`              | `SelectionChanged` / export events                                                | `viewing`    | Update or retire selection lane; orthogonal to viewport phase                                                                       |
| `viewing`              | Terminal input intent                                                             | `recovering` | Snap to bottom first; the input proceeds independently through the controller                                                       |
| `viewing`              | Viewport resize                                                                   | `viewing`    | Drop the rendered window, show a re-anchoring placeholder; the resulting `HistoryChanged` decides refresh (`Valid`) or `recovering` |
| `viewing`              | Detach, attach failure, transport loss                                            | `disposed`   | Anchor and window die with the attachment                                                                                           |
| `entering` / `viewing` | Terminal retirement or session/attachment switch                                  | `disposed`   | Discard all viewport state with the old attachment; the new attachment starts `live`                                                |
| `recovering`           | Next live snapshot frame                                                          | `live`       | Discard history store and counters; resume projection                                                                               |
| `recovering`           | Scroll-up intent before the snapshot arrives                                      | `recovering` | Ignore (or buffer at most one intent) until `live`                                                                                  |
| `recovering`           | `HistoryChanged`, detach, or transport loss                                       | `disposed`   | Acknowledge what can be acknowledged; the attachment ends the session anyway                                                        |
| `disposed`             | Any event                                                                         | `disposed`   | Terminal state; nothing re-enters without a fresh `live` attach                                                                     |

### Stability while viewing

- Paused projection demand means no live frames arrive while viewing; historical content is pinned
  by the committed anchor and does not move when new output lands.
- `HistoryChanged` with `Valid` carries the new `revision`, `generation`, and `total_rows`; the
  client updates metadata, acknowledges, and refreshes content via `Committed { delta: 0 }` — the
  viewport stays on the same logical rows.
- A `CurrentView` window that returns `revision` at or beyond (`>=`) the last seen event already reflects
  the change and supersedes a pending refresh or recover transition.
- New output never auto-scrolls an open history viewport; only an invalid anchor or the user's own
  intent moves it.

### Automatic return to bottom

- Reaching `rows_before == 0` (scrolling past the last retained row), `End`, or any terminal input
  transitions to `recovering` and restores `FollowingBottom` demand plus `Active` projection.
- On the first live snapshot frame the client discards the history window, cache, selection lane,
  and counters, and resumes normal projection.
- Return-to-bottom never follows output while the user is intentionally viewing history.

### Invalidation

- Retirement, detach, attachment replacement, terminal or session switch: `disposed`; every
  viewport counter is keyed to the attachment and dies with it.
- Generation change (resize): pending generation-pinned captures are abandoned by the Host; a
  committed anchor survives only when its Ghostty reflow pin does, reported through
  `anchor_status`. The client waits for the deciding `HistoryChanged` rather than preemptively
  recovering.
- Eviction (`scrollback_bytes` shrink or budget pressure), screen replacement, or pin loss:
  `anchor_status: Evicted` → `recovering`. `UpdateTerminalConfiguration` responses additionally
  carry `history_rows_evicted` for the shrinking caller, which the read-only viewer never sees;
  `HistoryChanged` is the viewer's eviction signal.
- Stale generation/revision/request sequence on a `RowsSnapshot`: discard the record, keep the
  viewport, refresh via `Committed { delta: 0 }`; never render a window whose identity is
  unverified.
- Attach failure or transport loss while `entering`/`viewing`: the viewer attachment is gone —
  viewport is `disposed`; the session layer decides re-attach or surface failure.

## Interaction (provisional)

All bindings below are **provisional**: `m3-input-design` owns the TUI key vocabulary, and the
product owner signs off. The model stance is non-modal scrolling — typing is never captured by
scrollback; terminal input snaps the viewport back to the bottom.

| Intent                   | Provisional binding                                                              | Notes                                                                                           |
| ------------------------ | -------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------- |
| Enter scrollback         | Mouse wheel up, or `Ctrl+B PageUp` / `Ctrl+B Up`                                 | Non-modal; works without a mode key when the terminal application does not claim the pointer    |
| One line up/down         | Mouse wheel, or `Ctrl+B Up` / `Ctrl+B Down`                                      | Maps to a `Committed { delta }` flight of ±1                                                    |
| Page up/down             | `Ctrl+B PageUp` / `Ctrl+B PageDown`                                              | `visible_rows − 1` per page, matching terminal convention                                       |
| Jump to top/bottom       | `Ctrl+B Home` / `Ctrl+B End`                                                     | Top = oldest retained row; End returns to `live`                                                |
| Exit scrollback          | `Ctrl+B End`, scrolling to bottom, or typing terminal input                      | Snap-to-bottom; no separate quit key needed in the non-modal model                              |
| Selection start/move/end | Mouse drag over the viewport                                                     | `SelectionGesture` `Start`/`Update`/`End`; keyboard-driven selection is a follow-up modal slice |
| Cancel selection         | `Ctrl+B Escape` or starting a new selection                                      | `SelectionGesture` `Cancel` or `SelectionCommand` `Clear`                                       |
| Copy selection           | `Ctrl+B y`                                                                       | `SelectionExport` plain text → local clipboard (transport reserved below)                       |
| Status indicator         | Panel-chrome badge such as `History 1,240/5,000` plus an evicted-ellipsis marker | Presentation detail; reserved                                                                   |

Optional follow-ups this proposal deliberately does not design: a modal copy-mode cursor, scrollback
search UI, prompt-navigation jumps, and a scrollbar affordance. The `purpose: Search` row lane
already exists for a later search slice.

## Selection and copy

Two clipboard routes exist:

| Route                                             | Direction                                                                | Fit                                                                                                                          |
| ------------------------------------------------- | ------------------------------------------------------------------------ | ---------------------------------------------------------------------------------------------------------------------------- |
| OSC 52 (`TerminalClipboardWriteRequest`, kind 56) | Terminal _application_ asks the Client to write the clipboard            | Wrong direction for user-driven copy; it carries application intent only and grants no platform effect                       |
| Local clipboard write by the TUI process          | Client assembles `SelectionExport` bytes and writes the system clipboard | Correct direction; keeps the copy semantics (`SelectionChanged` → export → clipboard) on the Client where the wire puts them |

Recommended path: user selection lives on the wire (`SelectionGesture`/`SelectionCommand` →
`SelectionChanged`); copy is a `SelectionExport` drained chunk-by-chunk into a bounded local buffer
(≤ 16 MiB) and written to the system clipboard by the TUI. The remaining reserved choice is the
clipboard transport itself — emitting OSC 52 to the host terminal (works over SSH, no dependencies)
versus a native clipboard integration — decided by the product owner, not here.

Copy must never mutate shared Workbench state: it touches only the attachment-scoped selection and a
Client-local clipboard write.

## Limits and failure behavior

| Bound                                       | Value                                              | Viewport rule                                                                               |
| ------------------------------------------- | -------------------------------------------------- | ------------------------------------------------------------------------------------------- |
| `scrollback_bytes` default / zero / maximum | 10,000,000 / disabled / 1,000,000,000              | Zero means `total_rows` equals the screen; entry shows "no scrollback"                      |
| Row window                                  | ≤ 44 MiB and ≤ 4096 rows, 112-byte OBRW header     | Viewport requests are far smaller; the request itself carries a tighter `maximum_bytes`     |
| Negotiated `rowWindowBytes`                 | Worker already negotiates the protocol maximum     | Still cap per-request `maximum_bytes` (recommended ≤ 4 MiB) below the relay record cap      |
| Relay record                                | 64 B + 36 MiB                                      | A legal 44 MiB window cannot traverse the relay; viewport and page sizes stay well under it |
| `Committed` delta                           | ±4096 rows                                         | Larger accumulated deltas split into multiple flights or resolve through `Tail`             |
| Frame cells / rows / columns                | 262,144 / 4096 / 4096                              | Viewport cache mirrors frame-bounded geometry                                               |
| Selection preview / export / chunk          | 4 KiB / 16 MiB / 1 MiB                             | Copy drains chunks and cancels on overflow or dispose                                       |
| Viewport cache                              | Recommended ≤ 8 MiB decoded cells, 16-row overscan | Windows exceeding the cache budget are rejected and the viewport recovers                   |
| History event acknowledgement               | Exactly one in-flight `HistoryChanged`             | Always `HistoryObserved`; stale acknowledgements are no-ops                                 |

Failure matrix — fail closed, keep the live view wherever possible:

| Condition                                                                                       | Behavior                                                                                                                                                            |
| ----------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| No retained scrollback (`total_rows` ≤ visible rows, alternate screen, or `scrollback_bytes` 0) | Stay `live`; localized status hint; no demand change is sent                                                                                                        |
| Oldest rows evicted while viewing                                                               | `HistoryChanged` `Evicted` → `recovering`; the top-of-history marker (`start_row` > oldest index) renders as an ellipsis row only while the window is still trusted |
| Stale generation/revision/sequence window                                                       | Discard without rendering; refresh through `Committed { delta: 0 }`; repeated staleness ends in `recovering`                                                        |
| Malformed or mismatched window                                                                  | Same discard-and-recover rule; a window whose identity cannot be verified is never painted                                                                          |
| `TypedFailure` on any history control                                                           | `recovering`; surface the typed failure through the existing notice path                                                                                            |
| Attach failure or transport loss                                                                | `disposed`; session re-attach policy applies as for live projection loss                                                                                            |
| Row window too large for the negotiated/relay budget                                            | Shrink `maximum_bytes` and `row_count`; if even a single-screen window fails, `recovering`                                                                          |
| Terminal retirement or session switch                                                           | `disposed`; the replacement attachment starts `live` with empty history state                                                                                       |
| Resize while viewing                                                                            | Drop the rendered window (its column geometry is stale); the next `HistoryChanged` resolves to refresh or `recovering`                                              |

## Shared-document consistency

The accepted TUI Client contract already assigns each Client its own "active location, active Tab,
focus, layout projection, viewport, scroll, search, and selection presentation". Scrollback fits
that boundary exactly:

- Canonical content lives in the Terminal Host: VT state, retained rows, eviction, anchors,
  revision/generation identity.
- The scrollback viewport is a read-only projection of that content through the attachment's `read`
  capability — it cannot mutate the terminal, the Workbench, or another Client.
- `start_row`, `rows_before`, the cache, pending intent, gestures, and clipboard state are
  Client-local presentation state; two Clients legitimately view different rows of the same
  terminal.
- Shared Workbench documents must never carry viewport position, scroll offset, selection, local
  copy state, search cursor, or scrollbar presentation — they would leak one Client's transient view
  into every other Client.

## Implementation surface

For reviewers planning the follow-up slices, the non-wire work items are:

1. Relay vocabulary: the OBCS/OBCN `TerminalViewerAction` and Worker routing additions listed in
   [Relay surface and required additions](#relay-surface-and-required-additions), shared with
   `m3-terminal-read`.
2. A bounded Rust OBRW `Render`-detail decoder producing the renderer's existing cell rows.
3. TUI-local viewport module beside `TerminalProjection`: state machine, window cache, flight
   bookkeeping, and the input/dispatch surface for the provisional intents.
4. Local clipboard transport for exported selection bytes.

## Reserved decisions and gaps

Reserved for their owners rather than decided here:

- Clipboard transport to the system clipboard (OSC 52 emission vs native integration).
- Whether scrollback search joins this slice; recommendation: defer — `purpose: Search` exists on
  the wire, but the UI needs `m3-input-design` vocabulary first.
- Presentation details: status badge wording, history/live divider, evicted-row marker, scrollbar.
- Final OBCS/OBCN action-variant shape and any versioning cost, owned by the relay-protocol
  maintainers.
- No OBTP wire gap was found: every semantic this viewport needs — history demand, events, row
  windows, anchors, selection, and export — is already on the wire.

## Verification

```sh
pnpm docs:confirm-pairing -- proposals/cli/tui-scrollback-viewport.md
```
