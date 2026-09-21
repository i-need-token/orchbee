# Native terminal history viewport

## Scope and ownership

This reference governs the native Client's retained-history viewport: the per-pane scrollback
reading mode entered with `Ctrl+B PageUp`, its demand legs, the row-window read/acknowledge
handshake, navigation intent, recovery, and how the painted surface is produced and fenced.
Maintain it with `terminal_session/terminal_viewport.rs`, `terminal_session/mailbox.rs`,
`terminal_session/connection/native.rs`, `terminal_session/ui.rs`, `terminal_input/dispatcher.rs`,
`client_view.rs`, `client_view/bindings.rs`, `tui_pointer.rs`, `tui_render.rs` and its
`tui_render/viewport.rs` painter, `localization.rs`, and `formal/tla/TerminalClientViewport.tla`.
The protocol contract is owned by `crates/terminal-protocol` (`row_window.rs`, `history.rs`,
`row_access.rs`) and by `crates/client-server-protocol/src/viewer.rs` for the
`TerminalViewerAction` surface; this document consumes that contract and never extends it.

This slice deliberately does not implement copy/search/selection on history, styled rendering,
clipboard integration, or persistence. Those are separate accepted decisions.

## State machine

Each pane owns an independent viewport core (`TerminalViewport`, one per `PaneRef`; hidden or
background panes are unaffected by the focus pane's mode). Absent viewport state means `Live`;
while a core exists its `mode` is one of:

| Mode         | Meaning                                                                                              |
| ------------ | ---------------------------------------------------------------------------------------------------- |
| `Entering`   | History+projection demand legs admitted; waiting for `HistoryChanged` facts and the first row window |
| `Viewing`    | A committed window is installed; reads navigate the retained history                                  |
| `Recovering` | Recovery demands (`FollowingBottom`+`Active`) admitted or in flight; a valid new frame ends it        |

`Live` is the absence of a core. A detached pane has no core; a pane whose viewer is lost is
parked by the worker's demand path (`settle_pane_viewer_lost`), not by the core.

## Commands and demand legs

`QueuedCommand::Viewport` carries one logical `ViewportStep` per pane, executed in order by
`run_viewport` with a fresh incarnation+attachment lease check before each leg:

- `Enter` — `SetHistoryDemand(ViewingHistory)` then `SetProjectionDemand(Paused)`. A rejected or
  unavailable first leg publishes `ViewportOutcome::Enter{applied:false}` without a second leg. A
  failed or unknown second leg runs one reset leg (`SetHistoryDemand(FollowingBottom)`); reset
  failure or an unavailable outcome faults the pane. A fully applied entry publishes
  `ViewportOutcome::Enter{applied:true}`.
- `Read{origin,row_count,revision,generation,request_sequence,maximum_bytes}` — a
  `ReadViewportRows` lease-bound action. A `Record` payload publishes
  `ViewportOutcome::Window{request_sequence,record}`; `RowsRejected{reason}` publishes
  `ViewportOutcome::ReadRejected` with the typed reason; an opaque host rejection publishes the
  host-shaped variant of the same outcome; `Unavailable` faults the pane.
- `Ack{request_sequence,generation,revision}` — `RowsApplied`. An accepted outcome publishes
  `ViewportOutcome::Ack{request_sequence,accepted:true}`; a rejection publishes `accepted:false`.
- `Recover` — `SetHistoryDemand(FollowingBottom)` then `SetProjectionDemand(Active)`. Any leg
  failure faults the pane. Success publishes `ViewportOutcome::Recovered`.

Every step is fenced by `viewer_once_ready`/`viewer_for_ready_lease`: the pane must still be the
canonical record with the same incarnation and live attachment, and the outcome's attachment and
(lease) generation must match. Actions for a dead viewer never run.

## Facts intake (kind 53)

`HistoryChanged` events enter through one producer: `TerminalProjection::receive_for_geometry`
validates the complete envelope, role, epoch, encoding, and exact attachment identity and, on a
valid kind-53 record, returns the single `TerminalViewerAction::HistoryObserved` acknowledgement.
`ui.rs`'s poll-result arm consumes that returned action — it never mints its own — and settles it
once through the ordinary `CausalSettlement::Feedback` lane after the draw that consumed the
event, in Live and non-Live states alike. The same validated event is decoded once more for the
viewport core's facts intake, gated by strict per-incarnation sequence monotonicity; a repeat or
regression retires the carrier as a protocol fault. Kind-53 records never re-run frame hooks.

## Read and acknowledgement discipline

- At most one viewport read is in flight per pane. The Host holds a pending read pin: a second
  read while pending is refused, so the core never emits one.
- At most one `RowsApplied` acknowledgement is in flight — emitted only after a window passed all
  install gates. While a read is in flight, or an acknowledgement is pending or in flight, no new
  read is emitted; navigation intent coalesces instead.
- Window install gates, in order: valid decode; matching attachment and host epoch; matching the
  active read sequence; matching the current facts generation; matching the current pane column
  count. A sequence mismatch, or a window arriving with no flight, cannot prove which pending it
  settled and enters recovery. A generation or column mismatch proves the pending pin was cleared
  by that transition, so the flight is released and fresh facts are awaited.
- Installing a window does not commit it. Only an exact accepted `RowsApplied` promotes the
  pending anchor to committed. Acknowledgement rejection loses the anchor and enters recovery
  with a localized notice — the position is never faked from stale absolute offsets.
- `Committed{delta}` reads are bounded to ±4096 per step; larger movement splits into residual
  bounded steps emitted one at a time. `Tail{rows_before}` reads serve first entry and absolute
  jumps only.

## Facts deadlines and recovery

A single bounded budget (3 seconds) covers each wait episode: entry facts, one stale-generation
or stale-revision retry, and post-resize facts. Expiry queues `Recover` and surfaces the
recovering notice. `Recovering` ends only when recovery demands were applied _and_ a valid frame
arrives afterwards — a frame that was already in flight before the recovery demands cannot end it
early. Loss of the committed anchor (`anchor_status = Evicted`, or none while committed) enters
recovery the same way.

## Input ordering

Ordinary keyboard input while the focus pane is non-Live queues `Recover` first; the byte is
forwarded as one `Input` command for the exact same `PaneRef` only after the recovery command was
admitted — never between the two demand legs, never replayed after a refusal. `Recovering` panes
forward the byte immediately (recovery is already admitted). Non-Live panes consume the history
navigation keys (`PageUp`, `PageDown`, `Home`, `End`, `Esc`) and mouse wheel locally; a Live pane
keeps full application input and mouse tracking. `Ctrl+B PageUp` enters history while help keeps
`PageUp` precedence when open.

## Rendering

A valid text row window paints as plain text (no cell styling), bounded by the pane content
geometry. Old-generation or old-column data never paints: the surface accessor re-validates the
installed window's generation and columns against current facts on every draw. `Entering` keeps
the last verified live frame under a history badge; `Viewing` without a current window clears to
the badge; `Recovering` clears to a recovering indicator. Position reads `top/total` from the
installed window's `start_row`/`total_rows`.

## Invariants

- `OneReadFlight`: at most one read in flight; host pending is never double-occupied.
- `AckAfterInstall`: `RowsApplied` only for an installed window, matching sequence, exactly once.
- `CommittedOnlyAfterAck`: `Committed` reads require an accepted acknowledgement.
- `NoReadWhileAck`: no read while an acknowledgement is pending or in flight.
- `SurfaceCurrent`: painted rows always match the current generation and column count.
- `RecoverBeforeInput`: ordinary input is enqueued only after recovery demand was admitted.
- `EveryEventAcked`: every valid `HistoryChanged` is acknowledged exactly once.
- `MonotoneEvents`: accepted event sequences strictly increase.
- `BoundedDelta`: every emitted `Committed` step is within ±4096.
- `HonestModes`: `Live` mode never consumes history keys or wheel; non-Live never forwards
  application keys or tracking.
- `PaneIncarnationFence`: every wire action and applied event resolves the exact `PaneRef` and
  attachment lease; outcomes for a dead incarnation are dropped.

## Source correspondence

| Production surface                          | Model correspondence                                                                                                        |
| ------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------- |
| `terminal_session/terminal_viewport.rs`     | `TerminalViewport` core: `Mode`, `facts`, `window`, `ack`, `read_flight`, `committed`, `pending_nav`, `pending_commands`, deadlines |
| `terminal_session/mailbox.rs`               | `QueuedCommand::Viewport`, `ViewportStep`, `WorkerEvent::ViewportOutcome`                                                    |
| `terminal_session/connection/native.rs`     | `run_viewport` executor: demand legs, read, ack, recovery with lease fencing                                                  |
| `terminal_session/ui.rs`                    | `enter_history`, `history_key`, `history_wheel`, kind-53 arm, `KeyedProjection.viewport`/`history_sequence`                    |
| `terminal_input/dispatcher.rs`              | `history_key` port route before ordinary pending bytes; wheel hand-off                                                        |
| `client_view.rs`, `client_view/bindings.rs` | `ClientAction::EnterHistory`, `Ctrl+B PageUp` binding, help precedence                                                         |
| `tui_pointer.rs`                            | `PointerProbe.viewport_non_live`, non-Live wheel consumption (three rows per notch)                                            |
| `tui_render.rs`, `tui_render/viewport.rs`   | Plain-text window painter, history/recovering indicators, generation/column fencing                                            |
| `localization.rs`                           | History notices and indicator strings (en/zh)                                                                                  |
| `formal/tla/TerminalClientViewport.tla`     | TLC-checked state machine covering blockage, events, silence, bounds, and recovery                                            |

## Verification

| Surface                 | Evidence                                                                                                                                           |
| ----------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------- |
| Core transitions        | `terminal_viewport/tests` — entry, navigation coalescing, ±4096 residual steps, install gates, ack promotion, typed rejections, recovery, deadlines   |
| Adversarial sequences   | Deterministic trace tests — stray/foreign/stale windows, typed stale retry, halving, ack rejection, bottom exit, absolute jump                          |
| Input boundary          | `client_view` tests — `Ctrl+B PageUp` → `EnterHistory`, help precedence; dispatcher consumption is exercised through the port default                   |
| Rendering               | Surface fencing is owned by the core (`surface()` re-validates generation/columns); painter coverage rides the shell render paths                       |
| Model                   | `formal/tla/TerminalClientViewport.tla` + `.cfg` registered in `scripts/tla/models.json`; TLC run by the gate owner                                   |
