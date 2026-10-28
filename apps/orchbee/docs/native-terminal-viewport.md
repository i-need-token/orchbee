# Native terminal history viewport

## Scope and ownership

This reference governs the native Client's local live-grid view and retained-history viewport: the per-pane scrollback
reading mode entered with `Ctrl+B PageUp`, primary-screen wheel-up or the scrollbar, its demand legs, the row-window read/acknowledge
handshake, navigation intent, recovery, and how the painted surface is produced and fenced.
Maintain it with `workbench_client/terminal_viewport.rs`, `workbench_client/mailbox.rs`,
`workbench_client/connection/native.rs`, `workbench_client/ui.rs`, `terminal_input/dispatcher.rs`,
`client_view.rs`, `client_view/bindings.rs`, `tui_pointer.rs`, `tui_render.rs` and its
`tui_render/viewport.rs` painter, `terminal_live_view.rs`, `tui_backend.rs`, `localization.rs`, and `formal/tla/TerminalViewportCoordination.tla`.
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

Shortcut Help describes the actual configured PageUp and PageDown bindings separately:
PageUp enters retained history with Help closed and pages backward with Help open; PageDown pages
forward with Help open. This is discovery text only. History eligibility, Esc return to live,
copy/export, and ordinary Help's nonmodal input behavior remain unchanged. The capturing
[Workbench navigator](native-context-selection.md#capturing-navigator) consumes these
keys for local row paging while it owns input.

## Smaller live viewers

The native Client's `terminal_live_view::LiveView` projects a physical grid into a smaller local
pane without changing the controller, terminal size, process or projection demand. It follows the
visible cursor on both axes; a hidden cursor follows the physical bottom. A clear that moves the
cursor to the top therefore reveals the new output rather than an empty bottom suffix. This is a
native cell-grid adaptation; GUI terminal rendering remains with its existing terminal presenter.

A clipped pane keeps a localized live-crop indicator above its cells, with the effective history
entry binding when it fits and a complete scroll-up instruction at narrow widths. Minimal hides the
global Actions pointer button, leaving the indicator's full row to the live-crop hint. Full and
Compact place the button in the separate top strip, outside the crop header. The last local
row remains available to the shell's status overlay, so read-only/control feedback cannot cover
the current prompt. These local viewing rows never resize the physical terminal. Selection and
link inspection use the same visible source row and column offsets; a wide glyph crossing either
horizontal edge paints blank. Raster graphics use the same offsets and original placement scale;
cropped panes use that fallback instead of untransformed host graphics. Exact-size panes retain
their complete ordinary live content area.

Primary-screen wheel-up or the history binding enters the existing manual-history core; a clipped
current screen is eligible even without older scrollback. History holds its committed reading
anchor while output continues, and Esc explicitly returns to cursor-following live content. The
local crop does not create a new shared viewport or claim that clipped alternate-screen history
is available. `terminal_live_view/tests`, `tui_render/terminal_cells/tests`,
`terminal_content/source/tests` and `ui/tests/viewport/chrome` cover crop bounds, clear/resize,
source coordinates and the prompt beneath composed read-only chrome. Host font acceptance remains
real-window evidence.

## Automatic Agent tail presentation

The common trim preference may borrow this same core for a live-input tail window.
Eligibility requires an exact current Agent observation, a primary screen without application mouse
tracking, an unclipped local live grid, available retained rows, and a proved empty suffix. Prompt markers, cursor rows,
styled backgrounds/underlines and graphics prevent trimming. A returned window must match the live
attachment, epoch, generation, revision, geometry and expected row start before painting.
Stale, pending or failed reads keep the exact live grid; unchanged failed targets do not retry.
Automatic entry keeps projection demand `Active`: in-place text, selection, color and animation
changes must continue delivering live frames while the retained-row lease exists. Every new live
frame invalidates an older crop until an exact current window has been acknowledged.

Automatic tail owns a retained-row lease but does not consume ordinary terminal keys or display a
manual-history badge. Explicit scrolling, copy mode or admitted selection transfers the core to
manual history. Active selection and export fence automatic navigation. Disabling the preference
or losing eligibility recovers through the existing demand owner. No PTY resize or history deletion
occurs. The core model covers demand/read/ack/recovery; target admission and cursor/copy coordinates
are implementation obligations covered by focused tail and painter tests.
An automatic lease transferred to manual history retains its active projection demand; the manual
surface and input owner still use the committed history window. The executor's `tests/viewport.rs`
checks entry demand selection and partial-entry reset; `ui/terminal_tail/tests.rs` checks fresh
text/colors before and after a replacement crop, both locales/themes, and stale-window exclusion.

## State machine

Each pane owns an independent viewport core (`TerminalViewport`, one per `PaneRef`; hidden or
background panes are unaffected by the focus pane's mode). Absent viewport state means the live grid with local cropping when needed; automatic tail can also hold a core while retaining live input;
while a core exists its `mode` is one of:

| Mode         | Meaning                                                                                              |
| ------------ | ---------------------------------------------------------------------------------------------------- |
| `Entering`   | History+projection demand legs admitted; waiting for `HistoryChanged` facts and the first row window |
| `Viewing`    | A committed window is installed; reads navigate the retained history                                 |
| `Recovering` | Recovery demands (`FollowingBottom`+`Active`) admitted or in flight; a valid new frame ends it       |

Manual `Live` is the absence of a manual-history core; automatic tail is distinguished by its target. A detached pane has no core; a pane whose viewer is lost is
parked by the worker's demand path (`settle_pane_viewer_lost`), not by the core.

## Entry availability

`Ctrl+B PageUp` asks the focus pane to enter history. The request is refused — with an honest
localized `HistoryUnavailable` notice and zero `Enter` commands on the wire — unless the pane's
last live frame satisfies all of: a known frame exists, retained rows or a clipped local live grid exist, the pane is not on
the alternate screen, and the frame carries at least one readable row. The screen's own rows are
never conflated with retained history: a history window's `total_rows` comes only from its served
route. A Live scrollbar observes the verified frame's `scrollback_rows + geometry.rows`, with
checked addition; it never substitutes that sum for an unknown history window.

A non-Shift wheel over readable untracked primary-screen content enters three rows
before the tail instead of the keyboard entry's one page. The exact pane under the pointer owns
this read-side operation, including an unfocused ReadOnly pane; keyboard focus and controller
ownership stay unchanged. WorkbenchClientUi rechecks the pane incarnation, current frame and retained rows
or a clipped current grid before admitting Enter. Unfocused application mouse reporting and
alternate-screen wheel input remain inert. Downward wheel at Live, absent retained rows without a clipped grid,
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
- `EnterTail` — the same entry and failure/reset contract, with `SetProjectionDemand(Active)`
  instead of `Paused`, so automatic cropping cannot freeze the live Agent screen.
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

## Reading continuity across navigation

On Windows, `ui/history_positions.rs` retains each canonical presentation's explicit Live or
manual-history choice across tab/viewer replacement and context handoff in the same Client.
Identity includes Session, context, tab, panel, presentation, Worker and terminal. Removed or rebound
members are pruned; retirement discards the bank. Nothing is persisted or shared with another
Client. Automatic tail and copy episodes do not become manual-history bookmarks.

A bookmark captures the committed start, Host terminal/epoch, generation and geometry, plus up to
eight already-retained rows at that window's top. Mutable live-screen rows never serve as evidence.
Witness storage is capped at 16 MiB; eviction retains an unavailable choice for visible fallback.
Esc/End or typing records Live as soon as recovery is decided, even before command admission.

Return uses a fresh Enter/HistoryChanged/read/RowsApplied handshake after checking membership and
the physical terminal fence. The new facts' total determines the tail distance to the saved start,
so appended output does not move it toward the tail. Before installation or acknowledgement,
the returned start and witness rows must match exactly, including glyphs, attributes and wide cells.
This is a content-validated bookmark, not a portable Host pin. Changed generation/geometry,
shrinking history, changed or recycled witness rows, missing evidence or an unrepresentable target
produce localized feedback and ordinary recovery to Live. Pending restoration consumes local
navigation; Esc and typing keep their recover-before-input behavior. No old attachment, sequence,
pending command, grant or selection is replayed.

The core's traces check fresh totals, content refusal before paint/ACK and the commit boundary.
`ui/history_positions/tests.rs` checks projection replacement, context-bank handoff, explicit Live,
canonical pruning and terminal/geometry refusal. These are implementation witnesses; the bounded
coordination model continues to own the existing demand/read/ACK protocol.

## Direct sidebar switching

Windows sidebar Agent activation is immediate navigation intent, shared by a valid primary release
and sidebar `Enter`. It keeps the Machines/Workspaces/Agents picker closed. A separate selection
generation owns verification and waiting; opening Search explicitly retains its capturing picker
contract and invalidates any unadmitted direct selection.

| Transition                 | Owner and result                                                                                                                                                                                                                                                                                                  |
| -------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Accept a visible Agent     | `machine_picker/sidebar.rs` revalidates the exact machine and physical target and creates a Sidebar selection generation; the input dispatcher captures the packet.                                                                                                                                               |
| Verify and wait            | `tui_shell/machines.rs` uses a cancellable read-only connection to verify current membership and physical Agent identity within the original 60-second selection budget. On the current machine it reuses the active Session. A busy Session retains only the latest waiting target; Busy precedes exact-current. |
| Select an exact member     | `ui/direct_navigation.rs` resolves Session, context, Tab, Panel, Presentation and binding again. Same-context selection reuses the switch transaction; cross-context selection carries an exact member through snapshot validation and cleanup, without visiting a default member.                                |
| Keep current or move focus | Both fast paths require a live exact pane incarnation whose attached Host epoch and terminal match the verified target. A missing or different physical identity takes the fenced switch path.                                                                                                                    |
| Attach and paint           | The selected member and physical Host fence survive membership cleanup. An unexpected attached Host is detached before polling, control acquisition or frame intake. Input and feedback remain unavailable until the existing first-frame contract admits them.                                                   |
| Cancel or fail             | Escape, a newer selection or explicit Search retires unadmitted work; stale verification cannot install or reopen a surface. Accepted Session work retains its own settlement. A direct failure reports a localized retry notice without opening a picker.                                                        |

`ui/retained_frames.rs` owns the render-only image bank: at most eight panes and 16 MiB. Reuse requires
exact Session/context/Tab/Panel/Presentation/binding and geometry. Agent selection additionally evicts
images from a different or unknown Host terminal or epoch. Retained images grant no cursor,
selection, graphics, input or frame acknowledgement authority; fresh validated frames take over
through the existing projection path. Cross-machine and explicit-picker selection keep the
[machine candidate handoff](machines.md#cross-machine-workspace-navigation).

During Tab or Workspace navigation, `ui/navigation_draw.rs` retains the last flushed shell while
the newly selected Tab is attaching without a live frame. The shell does not paint the temporary
automatic Tab name or connection placeholder; the first staged live frame replaces the old shell
in one draw. The previous hit map is invalidated and the existing first-frame input gate remains
in force. Non-frame poll replies continue settling during this hold, without acknowledging any
unpainted frame. Resize, capturing surfaces, notifications, stable unavailability and failures
render normally; the existing attachment deadline bounds the wait.
Once the first live frame is visible, its pending draw acknowledgement alone does not display a
connection status footer.

Automatic Tab titles can reuse their last observed title while the active Tab waits for a frame,
under the same canonical binding and attached Host epoch. A fresh live title, including a cleared
title, takes precedence; blocked or faulted projections do not regain an old title.

The controlled witnesses are `machine_picker/tests/sidebar.rs`,
`terminal_input/dispatcher/tests/action_menu/sidebar_prefix.rs`,
`tui_shell/machines/tests/direct_navigation.rs`, `ui/direct_navigation/tests.rs` and
`state/tests/context_selection.rs`; `connection/native/tests/presentations.rs` checks unexpected
Host cleanup before any poll or control. They cover packet ownership, no implicit picker, exact child
landing, latest intent, cancellation, failure, stale results and physical replacement. The viewport
witness uses the shared shell/pane geometry projection. These are implementation traces, not
exhaustive TLC assurance or physical paint timing. Real-host acceptance samples same-tab,
cross-tab and cross-workspace clicks frame by frame for transient navigation and wrong-target content.

## Rendering

A validated render window uses the same styled-cell painter as live output, bounded by the pane
content geometry. Colors use the window's captured defaults, never the TUI theme or a newer live
frame. The captured grid owns each cell's one- or two-column footprint, including regional
indicators that a Unicode width table measures differently. The painter preserves the complete
safe cell string and supplies that footprint to Ratatui's diff. The output backend explicitly
positions the next write after supplementary scalars and multi-scalar symbols instead of
assuming the host advanced by the table's width. Blank replacement of a wide cell clears its
captured footprint; ordinary ASCII runs remain contiguous. `tui_render/terminal_cells/tests` and
`tui_backend/tests` protect these boundaries; physical glyph readability and resize/takeover/clear
acceptance still require matched real-host evidence, without generalizing to every emoji.
Style attributes and wide/spacer cells retain live-painter behavior; controls are sanitized
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

## Scrollbar interaction

Each pane reserves one chrome column at the right of its terminal content, independently of
history availability. Paint, native terminal resizing and layout queries share that geometry;
split dividers remain separate targets. A one-column pane keeps its content and omits the gutter.
A track shorter than two rows has no thumb or pointer target.

`ScrollbarSnapshot` observes the exact attachment, epoch, generation, columns, viewport rows,
total rows and start row. Live requires a readable primary-screen frame; history requires a
current validated window. Loading, recovery, alternate screens, empty history and ranges beyond
the protocol's `u32` tail addressing show no thumb and admit no new press. Counts are never
guessed. A short served window uses its actual row count for the thumb while retaining physical
pane geometry; a read-limit reduction cancels the old grab. Integer mapping rounds to the nearest
representable row and keeps at least one thumb cell and one cell of travel, using focus/text
and frame theme roles, `┃`/`│` glyphs and `#`/`|` ASCII alternatives. The thumb follows the
validated displayed window; pending drag intent is not painted as an optimistic position.
Thumb emphasis follows the same shell input ownership as pane titles.

| Event / guard                                                                                                                | Transition and result                                                                                                                                                                                                               |
| ---------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Plain left press on thumb                                                                                                    | Locally focus the exact attached `PaneRef`; capture its geometry, range and grab row without jumping.                                                                                                                               |
| Plain left press on track                                                                                                    | Focus that pane; center the thumb on the pressed row, bounded to the known range; retain capture for dragging.                                                                                                                      |
| Motion / matching release                                                                                                    | Map the press's range and grab offset; clamp outside-track rows and coalesce identical offsets. Release applies its final position and ends capture.                                                                                |
| Positive offset from Live                                                                                                    | Enter history with that tail offset. Facts still gate the first read; the admitted press supplies a known range while the first window is pending.                                                                                  |
| Positive offset in history                                                                                                   | `ViewportNav::OffsetFromBottom` replaces pending absolute intent through the existing single-read/ACK gates. Installing an older request clears only its matching offset, preserving a newer destination.                           |
| Offset zero                                                                                                                  | Queue ordinary Recover; only completed recovery demands followed by a fresh frame remove the core and restore Live following.                                                                                                       |
| Total rows, generation, attachment or geometry changes                                                                       | Cancel further drag intent on fresh-source check or draw. Output growth requires a new press; the old grab is never remapped to a new displayed thumb. Already admitted work keeps its original pane and ordinary completion rules. |
| Resize, pane switch/close/remint, covered region, capture release, host focus loss, keyboard input or modified pointer event | Cancel capture; late motion/release cannot start another pane's gesture or reach application mouse reporting.                                                                                                                       |

The pointer owns capture only. WorkbenchClientUi rechecks exact focus, read readiness, selection admission,
copy-mode exclusion and source bounds before every intent. Pending windows can retain an existing
capture but cannot authorize a new one. Changed facts totals reject further requests before the
replacement window paints. Live output in history refreshes the committed anchor without issuing
a scrollbar jump. Capture cancellation does not undo previously accepted navigation; that intent
still completes through the core's validation and ACK. Selection/copy ownership and application
mouse reporting remain intact: chrome
events never reach the program. Keyboard equivalents remain `Ctrl+B PageUp`, PageUp/PageDown,
Home, End and Esc.

The scrollbar uses a right-side track, grab offset and bounded scrolling.
Its read/ACK authority and cancellation remain owned by Orchbee.

Evidence: `terminal_scrollbar/tests` covers integer bounds; `tui_pointer/tests/scrollbar` covers
gesture/cancellation and every mouse-tracking mode; `ui/tests/viewport/scrollbar` covers composed
theme/locale rendering, native admission, decoded focus-loss/keyboard traces, output growth and
recovery. The retained viewport core checks only the coordination properties in the verification section; scrollbar geometry and gestures remain implementation evidence.

## Invariants

- `OneReadFlight`: at most one admitted read in flight; host pending is never double-occupied.
- `SingleStagedStep`: at most one un-admitted step waits in the staged slot.
- `OrderedNonreusedReads`: minted read sequences are never reissued, and no read sequence still
  on the wire — staged, admitted, or produced — overtakes the live flight. Faulty window payloads
  use the alien-sequence symbol `sq = 0`, exempt from pairwise uniqueness; they still occupy an
  executed read's original result slot.
- `AckAfterInstall`: `RowsApplied` only for an installed window, matching sequence, exactly once.
- `CommittedOnlyAfterAck`: only an exact accepted acknowledgement promotes an anchor; `Committed`
  reads use that previously accepted anchor.
- `NoReadWhileAck`: no read while an acknowledgement is pending or in flight.
- `SurfaceCurrent`: painted rows always match the current generation and column count.
  Drawing evidence lives in the product tests.
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

`TerminalViewportCoordination.tla/.cfg` abstracts one pane, two entry/read attempts, two anonymous input commands and a two-slot FIFO. `Enter` (including `EnterTail`), `Read` and `Recover` correspond to the viewport's staged commands; `Admit` and `Execute` represent the mailbox/worker FIFO; `Consume` represents outcome intake; `Detach` invalidates the pane epoch. The entry abstraction does not distinguish active versus paused projection demand; executor and tail-render tests own that distinction. Fault injection may corrupt an existing window response but cannot manufacture an independent producer.

The checked claims are bounded queue ownership, one read flight, current-epoch presentation, admitted-command preservation during recovery, and entry/recovery settlement under weak fairness of admission, execution and consumption. This is a smaller protocol abstraction, not an equivalence proof of every viewport operation. Concrete row bounds, navigation, deadlines, scrollbar gestures and generation/sequence validation remain obligations of `workbench_client/terminal_viewport/tests.rs`, `window.rs` and the projection/connection tests.

## Verification

Run `node scripts/tla/run-models.mjs --models native-terminal-viewport` for the bounded protocol and the native Cargo tests for its implementation. `scripts/tla/model-mutations.test.mjs` restores an independent outcome producer and requires an `OutcomeProductionOwned` counterexample. The complete `pnpm test:tla` gate runs this negative control and all retained configurations without success-cache reuse.

The model does not prove actual clock latency, payload validation, row geometry or all UI schedules. A timeout, unfinished state queue or incomplete action coverage is a failed check.

The TUI describes a lost reading anchor as showing the latest output. This one-shot notice expires after four seconds and clears immediately when the focused pane completes its return to live. The status chip floats over terminal content without changing terminal geometry; the recovery protocol and retained operation facts are unchanged.
