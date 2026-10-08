# Native terminal history viewport

## Scope and ownership

This reference governs the native Client's retained-history viewport: the per-pane scrollback
reading mode entered with `Ctrl+B PageUp` or primary-screen wheel-up, its demand legs, the row-window read/acknowledge
handshake, navigation intent, recovery, and how the painted surface is produced and fenced.
Maintain it with `terminal_session/terminal_viewport.rs`, `terminal_session/mailbox.rs`,
`terminal_session/connection/native.rs`, `terminal_session/ui.rs`, `terminal_input/dispatcher.rs`,
`client_view.rs`, `client_view/bindings.rs`, `tui_pointer.rs`, `tui_render.rs` and its
`tui_render/viewport.rs` painter, `localization.rs`, and `formal/tla/TerminalClientViewport.tla`.
The protocol contract is owned by `crates/terminal-protocol` (`row_window.rs`, `history.rs`,
`row_access.rs`) and by `crates/client-server-protocol/src/viewer.rs` for the
`TerminalViewerAction` surface; this document consumes that contract and never extends it.

History preserves captured cell colors, attributes and wide-cell geometry. Selection and clipboard
integration follow [native terminal selection and effects](native-terminal-selection-effects.md).
Search and copy mode, graphics display, and persistence remain separate obligations.

The same app-local core also serves the Windows CLI control stream through
`cli/terminal_stream/history.rs`. Each attached control invocation owns one core, read watermark
and strict history-event sequence. CLI demand/row exchanges use the same protocol; complete
ANSI history output is flushed before RowsApplied. The adapter's NDJSON contract belongs to
[Windows terminal streams](windows-terminal-streams.md); TUI notice copy remains TUI-owned.

## State machine

Each pane owns an independent viewport core (`TerminalViewport`, one per `PaneRef`; hidden or
background panes are unaffected by the focus pane's mode). Absent viewport state means `Live`;
while a core exists its `mode` is one of:

| Mode         | Meaning                                                                                              |
| ------------ | ---------------------------------------------------------------------------------------------------- |
| `Entering`   | History+projection demand legs admitted; waiting for `HistoryChanged` facts and the first row window |
| `Viewing`    | A committed window is installed; reads navigate the retained history                                 |
| `Recovering` | Recovery demands (`FollowingBottom`+`Active`) admitted or in flight; a valid new frame ends it       |

`Live` is the absence of a core. A detached pane has no core; a pane whose viewer is lost is
parked by the worker's demand path (`settle_pane_viewer_lost`), not by the core.

## Entry availability

`Ctrl+B PageUp` asks the focus pane to enter history. The request is refused — with an honest
localized `HistoryUnavailable` notice and zero `Enter` commands on the wire — unless the pane's
last live frame satisfies all of: a known frame exists, `scrollback_rows > 0`, the pane is not on
the alternate screen, and the frame carries at least one readable row. The screen's own rows are
never conflated with retained history: `total_rows` comes only from served windows, not from
`scrollback_rows`.

A non-Shift wheel over focused untracked primary-screen content enters three rows
before the tail instead of the keyboard entry's one page. NativeUi rechecks exact focus, current
frame and retained rows before admitting Enter. Downward wheel at Live, missing/empty history,
missing frame or alternate screen does nothing and emits no unavailable notice. Further wheel
notches reuse the core's signed three-row navigation, including coalescing while entry is pending
and recovery on reaching the live tail. Entry refusal leaves the pane Live without replay.

## Commands and demand legs

`QueuedCommand::Viewport` carries one logical `ViewportStep` per pane, executed in order by
`run_viewport` with a fresh incarnation+attachment lease check before each leg:

- `Enter` — `SetHistoryDemand(ViewingHistory)` then `SetProjectionDemand(Paused)`. A rejected or
  unavailable first leg publishes `ViewportOutcome::Enter{applied:false}` without a second leg. A
  failed or unknown second leg runs one reset leg (`SetHistoryDemand(FollowingBottom)`); reset
  failure or an unavailable outcome faults the pane. A fully applied entry publishes
  `ViewportOutcome::Enter{applied:true}`.
- `Read{origin,row_count,revision,generation,request_sequence,maximum_bytes}` — a
  `ReadViewportRows` lease-bound action. The worker validates the complete kind-46 Response
  envelope (current protocol, required correlation, nonoptional row-window encoding, requested
  payload bound and matching envelope/body epoch), then strictly decodes render-detail OBRW,
  nested OBPB and OBKG. `terminal_viewport/window.rs` materializes one immutable styled window.
  Windows CLI streams retain its captured default colors; TUI default markers use
  [native theme roles](native-themes.md). Repeated borrowed string aliases share an `Arc<str>` by exact
  pointer/length. Unique copied string bytes also fit the requested payload budget, checked before
  allocation; local exhaustion becomes `ResourceExhausted` recovery to release the already-served
  Host pin, never a retry against that pending pin. Host-issued `LimitExceeded` retains its
  existing bounded halving. That budget plus fixed per-cell storage bounds retained allocation.
  It publishes a boxed typed window in `ViewportOutcome::Window`; raw transport bytes never
  reach the UI row decoder. Both interactive consumers request render detail; text responses
  are rejected rather than silently dropping styles. CLI history uses the same app-local decoder. Malformed/foreign responses fail
  before row installation; `RowsRejected{reason}` publishes
  `ViewportOutcome::ReadRejected` with the typed reason; an opaque host rejection publishes the
  host-shaped variant of the same outcome; `Unavailable` faults the pane.
- `Ack{request_sequence,generation,revision}` — `RowsApplied`. An accepted outcome publishes
  `ViewportOutcome::Ack{request_sequence,accepted:true}`; a rejection publishes `accepted:false`.
- `Recover` — `SetHistoryDemand(FollowingBottom)` then `SetProjectionDemand(Active)`. Any leg
  failure faults the pane. Success publishes `ViewportOutcome::Recovered`.

Every outcome travels back as a fire-and-forget `WorkerEvent::ViewportOutcome` notice:
`publish_notice_before` deposits it with no wait key and no expected reply, the UI's
`take_event_before` admits it alongside the pane detach/failure notices, and
`apply_viewport_outcome` resolves the exact `PaneRef` before dispatching into the core.

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
- Read request sequences belong to the attachment, not the episode. The host's
  `last_row_request_sequence` survives `Enter`/`Recover` cycles, so `KeyedProjection` owns the
  client-side watermark across viewport episodes: a re-entered history session resumes strictly
  above the last issued sequence — issued, admitted, or cancelled alike — and only a genuinely
  new attachment incarnation resets it. Kind-53 event sequences are an independent domain.
  Exhaustion is a terminal fault, never a wraparound.
- Mint and admission are separate steps. The core stages at most one un-admitted step in
  `pending_commands`; `next_read_sequence` is consumed at mint, so a staged step that recovery
  or resize later replaces leaves an honest gap — issued sequences are never reissued. The read
  flight and the acknowledgement lifecycle bind only when the step is admitted
  (`command_admitted`); admitted commands are irrevocable and execute in order, and recovery
  clears their flight state at its own admission rather than at the decision. Every executed
  command publishes exactly one typed outcome; at most one outcome waits published while a
  second computed result waits behind it — a published outcome never blocks the next command's
  execution.
- At most one `RowsApplied` acknowledgement is in flight — emitted only after a window passed all
  install gates. While a read is in flight, or an acknowledgement is pending or in flight, no new
  read is emitted; navigation intent coalesces instead.
- Window install gates, in order: valid decode; matching attachment and host epoch; matching the
  active read sequence; matching the current facts generation; matching the current pane column
  count. A sequence mismatch, or a window arriving with no flight, cannot prove which pending it
  settled and enters recovery. A discarded window does not automatically mean another event is
  coming: when the held facts advanced past the generation the read was issued under, that
  kind-53 transition is itself the pin release, and the next read proceeds under the fresh
  generation. Only when the facts did not move since issue does the core arm a bounded wait —
  generation values are compared for equality with the issue generation, never ordered
  numerically.
- Installing a window does not commit it. Only an exact accepted `RowsApplied` promotes the
  pending anchor to committed. Acknowledgement rejection loses the anchor and enters recovery
  with a localized notice — the position is never faked from stale absolute offsets.
  A previously committed anchor survives a resize independently of a newer window's pending or
  in-flight acknowledgement. Resize discards that newer window and its un-admitted acknowledgement;
  after fresh facts, the old committed anchor can be refreshed without accepting the discarded
  window. A late acknowledgement cannot match a discarded window.
- Local line intent is positive toward older rows. Entry adds that intent to `Tail{rows_before}`;
  committed navigation negates each intent chunk because the Host adds wire `delta` to the
  committed **start row**. In-flight and residual intent retain the local sign. `Committed{delta}`
  reads are bounded to ±4096 per step; larger movement splits into residual bounded steps emitted
  one at a time. `Tail{rows_before}` reads serve first entry and absolute jumps only.
- [Keyboard copy mode](native-keyboard-copy.md) adds an internal `Newest` absolute jump: `Tail{0}`
  remains Viewing after its accepted row acknowledgement. Its primary entry admits a readable
  zero-history frame at Tail{0}; the ordinary PageUp/wheel entry and tail-recovery rules remain
  unchanged. Copy uses the committed-only window accessor, and owns input ahead of ordinary
  history. Its alternate surface uses the live frame and does not request history demand.
  `PressJump` abstracts both absolute destinations in the viewport model; destination choice and
  copy cursor/selection gating are checked by core/native traces and the copy coordination model.

## Facts deadlines and recovery

A single bounded budget (3 seconds) covers each wait episode: entry facts, one stale-generation
or stale-revision retry, and post-resize facts. Expiry queues `Recover` and surfaces the
recovering notice. The stale-retry and `LimitExceeded` halving budgets are per navigation
intent: they re-arm when a nav call finds no pending and no in-flight intent, and on resize —
merging into an ongoing intent shares its budget.

Once recovery is decided — `Recovering` entered or a `Recover` staged — late facts, windows,
read outcomes, and ack outcomes are absorbed without installing a window or minting further
`Read`/`Ack`/`Recover` commands; a legal kind-53 event is still acknowledged exactly once
through its own lane. `Recovering` ends only when recovery demands were applied _and_ a valid
frame arrives afterwards — a frame that was already in flight before the recovery demands cannot
end it early. Loss of the committed anchor (`anchor_status = Evicted`, or none while committed)
enters recovery the same way.

## Input ordering

Ordinary keyboard input while the focus pane is non-Live queues `Recover` first; the byte is
forwarded as one `Input` command for the exact same `PaneRef` only after the recovery command was
admitted — never between the two demand legs, never replayed after a refusal. If the wire has
room for the Recover alone the byte drops with the recovery still admitted; with no room at all
the Recover stays staged and the byte drops — a dropped byte is never replayed. `Recovering` panes
forward the byte immediately (recovery is already admitted). Non-Live panes consume the history
navigation keys (`PageUp`, `PageDown`, `Home`, `End`, `Esc`) and mouse wheel locally; a Live pane
keeps application input and mouse tracking, except for untracked primary-screen wheel history
entry. `Ctrl+B PageUp` enters history while help keeps
`PageUp` precedence when open.

Every non-wheel pointer event that targets a non-Live pane is consumed by the Client — presses on
the focused pane, presses that move focus onto it, queued pending-focus replays, gesture
continuations, and drags that end after a mid-gesture Live→non-Live transition. An ordinary
primary-button gesture over the content follows [native terminal selection and effects](native-terminal-selection-effects.md)
against the committed history window; other events terminate locally on release. None encodes
application mouse bytes. The wheel keeps its own contract: inside the retained window it scrolls
locally, and anywhere else over the pane it is consumed without dismissing the overlay.

## Rendering

A validated render window uses the same styled-cell painter as live output, bounded by the pane
content geometry. Colors use the window's captured defaults, never the TUI theme or a newer live
frame. Style attributes and wide/spacer cells retain live-painter behavior; controls are sanitized
and a clipped wide cell paints blank. History never shows a cursor. Graphics resources are fully
validated at decode but not displayed; this does not complete graphics history. Old-generation or old-column data never paints: the surface accessor re-validates the
installed window's generation and columns against current facts on every draw. `Entering` keeps
the last verified live frame under a history badge; `Viewing` without a current window clears to
the badge; `Recovering` clears to a recovering indicator. The leaf's last row carries a
localized position readout (`start_row`/`total_rows` of the currently validated window — an
expired or stale window shows none) on the left and a persistent localized return-to-live hint
on the right; narrow panes degrade on grapheme boundaries with the hint keeping priority. `Esc`
returns to live output. The badge rectangle and the entire footer row reset underlying cells
before painting their semantic styles, so child colors, hidden/reversed attributes and history
text cannot leak into chrome. A wide glyph crossing the badge's left edge is blanked rather
than painting half a glyph across the label.

## Invariants

- `OneReadFlight`: at most one admitted read in flight; host pending is never double-occupied.
- `SingleStagedStep`: at most one un-admitted step waits in the staged slot.
- `OrderedNonreusedReads`: minted read sequences are never reissued, and no read sequence still
  on the wire — staged, admitted, or produced — overtakes the live flight. Host-fault window
  outcomes carry no issued sequence (`sq = 0`) and are exempt from pairwise uniqueness.
- `AckAfterInstall`: `RowsApplied` only for an installed window, matching sequence, exactly once.
- `CommittedOnlyAfterAck`: only an exact accepted acknowledgement promotes an anchor; `Committed`
  reads use that previously accepted anchor. The model's `CommittedNeedsAck` checks the false-to-true
  promotion edge against `AckAccepted`, rather than requiring the current window's acknowledgement
  to remain live after resize.
- `NoReadWhileAck`: no read while an acknowledgement is pending or in flight.
- `SurfaceCurrent`: painted rows always match the current generation and column count. The
  model bounds the drawn generation to none-or-current; it does not itself prove a positive
  draw — drawing evidence lives in the product tests.
- `RecoverBeforeInput`: every input newly admitted to the wire is already covered — the pane
  was in `Recovering`, or a `Recover` was admitted ahead of it. Asserted on the admission
  edge; an already-executed recovery needs no re-witness.
- `EveryEventAcked`: every valid `HistoryChanged` is acknowledged exactly once.
- `MonotoneEvents`: accepted event sequences strictly increase.
- `BoundedDelta`: every emitted `Committed` step is within ±4096.
- `HonestModes`: Live keeps ordinary application keys and tracking; eligible primary-screen
  wheel-up may enter history. Non-Live navigation stays local; ordinary input recovers first.
- `PaneIncarnationFence`: every wire action and applied event resolves the exact `PaneRef` and
  attachment lease; events and outcomes for a dead incarnation are dropped. The checked action
  property keeps `Lost` terminal for this incarnation; reattachment belongs to a new incarnation.

## Source correspondence

| Production surface                          | Model correspondence                                                                                                                                               |
| ------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `terminal_session/terminal_viewport.rs`     | `TerminalViewport` core: `Mode`, `facts`, `window`, `ack`, `read_flight`, `committed`, `pending_nav`, `pending_commands`, deadlines                                |
| `terminal_session/mailbox.rs`               | `QueuedCommand::Viewport`, `ViewportStep`, `WorkerEvent::ViewportOutcome`                                                                                          |
| `terminal_session/connection/native.rs`     | `run_viewport` executor: demand legs, read, ack, recovery with lease fencing                                                                                       |
| `terminal_session/ui.rs`                    | `enter_history` gates, `history_key`, `history_wheel`, kind-53 arm, `KeyedProjection.viewport`/`history_sequence`/`read_watermark`                                 |
| `terminal_input/dispatcher.rs`              | `history_key` port route before ordinary pending bytes; wheel hand-off                                                                                             |
| `client_view.rs`, `client_view/bindings.rs` | `ClientAction::EnterHistory`, `Ctrl+B PageUp` binding, help precedence                                                                                             |
| `tui_pointer.rs`                            | `PointerProbe.viewport_non_live`, non-Live wheel scroll and non-wheel event consumption                                                                            |
| `tui_render.rs`, `tui_render/viewport.rs`   | Shared styled-cell painter, indicators, position/hint bottom line, generation/column fencing                                                                       |
| `localization.rs`                           | History notices, indicators, position, and hint strings (en/zh)                                                                                                    |
| `formal/tla/TerminalClientViewport.tla`     | State-machine model covering blockage, watermark, staged admission, typed outcomes, events, silence, bounds, and recovery; registered in `scripts/tla/models.json` |

## Verification

The model imports `Integers` for signed navigation and represents every admitted command as a
three-tuple. Payload-free Enter, Recover, and Input use `<<kind, 0, 0>>`; Read and Ack retain their
sequence and generation. This bijective encoding preserves queue length, order, admission, and
execution while avoiding TLC equality checks between strings and tuples. It does not change the
production command representation. The registered configuration checks the complete reachable graph
with all declared invariants, temporal properties, fairness, and required action coverage; finite
bounds remain evidence limits.

### Observation projection

The model keeps all 35 physical state variables and projects away the last-action label `action`
and last-emitted-delta witness `emitted_delta`. Neither value participates in a transition guard.
The named `Next` actions and all 26 weak-fairness obligations are retained. Every fair action changes
a physical command/outcome queue, staged slot, mode, event-pending flag, or deadline; projecting these
two witnesses therefore preserves its enabledness and non-stuttering steps. Unfair actions that
change only a witness project to the already-admitted stutter. Under these reader and producer
conditions, every supported old behavior projects to the retained-state specification. This is an
inspected representation argument, not a kernel-checked universal theorem.

`EmitLeavesStagedRead`, `CommittedReadGuard`, `BoundedDelta`, and `InstallGates` now check their named
transition edges instead of retaining diagnostic state. A committed emit's signed increment is
`nav_delta - nav_delta'`, equal to its original `chunk`; Tail and Init produce zero, and no other
transition produces a delta. The configuration retains the same 19 assertion names as 11 state
invariants and eight temporal properties, with `PaneIncarnationFence` added separately for the
existing identity claim. Constants, action coverage, and fairness are unchanged; no state constraint
narrows exploration. Adding a witness reader or another
delta producer requires revisiting this mapping.

A replay of the existing 20-state counterexample checks the physical projection and the converted
assertions along that recorded prefix. Its driver advances the fixed record and does not establish
the original infinite fair specification. Five source-derived faults separately check illegal
commit promotion, a wrong staged sequence, a cleared committed anchor, an oversized committed
increment, and an unreleased installed flight. These focused checks complement the registered
complete-graph check and native core tests; they do not replace either or imply independent review.

`Lost` represents the absence of this incarnation's Client core. A late history event is drained
without changing Client facts, accepted event sequence, or staged commands; admission requires an
owned core. This follows `NativeUi::clear_pane_projection`, the exact-key check in
`apply_poll_result`, and the absent-core check in `apply_viewport_outcome`. Host state and already
admitted queues remain represented independently. A known reachable prefix followed by detach,
anchor eviction, event delivery, and attempted admission distinguishes a missing fence from the
supported transition; UI tests also deliver late facts and outcomes after projection removal.

The lost-core projection gives the 17 discarded core fields their reset values only after this
incarnation becomes `Lost`: facts, window/ack, flight, navigation, refresh, deadline, retry flags,
recovery completion, drawn generation, and staging. The 18 remaining fields retain mode, the issued
read watermark, admitted queues, outcomes, delivery, accepted event witnesses, and Host state.
Active states are unchanged. Lost-mode guards never read the discarded fields to produce a new
core or wire command; outcome and event drains preserve the retained state. A lost deadline can
only perform finitely many ticks in the reference, which project to stutters. All other fair steps
still change retained state or stay disabled; `EnteredResolves` depends only on mode. Under this
terminal-incarnation and reader closure, the normalization preserves the supported safety and
progress observations. This is an inspected representation argument, not a universal kernel proof.

A one-step boundary checker covers the 20 recorded states and their detached counterparts against
the reference `Next`, all 11 state assertions, and eight safety action properties. Its 357 distinct
states are local evidence, not the full fair specification. Three negative controls detect clearing
admitted wire commands, normalizing an active core, and losing the issued read watermark. The
registered complete-graph check retains all constants, action coverage, and 26 fairness obligations;
no exploration constraint is added.

| Surface               | Evidence                                                                                                                                                                                                                                       |
| --------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Core transitions      | `terminal_viewport/tests` — entry, navigation coalescing, ±4096 residual steps, install gates, ack promotion, typed rejections, recovery, deadlines                                                                                            |
| Adversarial sequences | Deterministic trace tests — stray/foreign/stale windows, typed stale retry, halving, ack rejection, bottom exit, absolute jump                                                                                                                 |
| Input boundary        | `client_view` tests — `Ctrl+B PageUp` → `EnterHistory`, help precedence; dispatcher consumption is exercised through the port default                                                                                                          |
| Rendering             | Surface fencing is owned by the core (`surface()` re-validates generation/columns); painter coverage rides the shell render paths                                                                                                              |
| Outcome notice seam   | `ui/viewport_tests` — `publish_notice_before` → `take_event_before` → `apply_viewport_outcome` regression keeps the protocol open; the state-neutral applied Enter leaves the Entering core intact                                             |
| Model                 | `formal/tla/TerminalClientViewport.tla` + `.cfg` registered in `scripts/tla/models.json`; merge checks run the complete bounded specification and required action coverage, including staged flight, input cover, and lost-incarnation fencing |
