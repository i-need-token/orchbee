# Native mouse input

## Scope and ownership

This reference governs the native Client's pointer behavior: host capture lifecycle, the
last-drawn-frame hit map, the gesture vocabulary the painted regions accept, passthrough encoding
to pane programs, and the alternate-screen wheel fallback. Maintain it with `tui_pointer.rs`,
`terminal_input/mouse.rs`, `terminal_input/decoder.rs`, `terminal_input/dispatcher.rs`,
`terminal_lifecycle.rs`, `tui_render.rs` and its `tabs`/`contexts` painters, `tui_shell.rs`,
`client_view.rs` and `client_view/bindings.rs`, `terminal_session/ui.rs`, `invocation.rs`,
`terminal_link_activation.rs`, and `localization.rs`. The repository standard
[`docs/tui/mouse-input.md`](../../../docs/tui/mouse-input.md) owns the rules this mapping must
satisfy; [native terminal environment](native-terminal-environment.md) owns the mode-lease
machinery that arming and restoration ride on. [Native terminal selection and effects](native-terminal-selection-effects.md)
owns untracked-pane selection and its copy lifecycle.

Windows [divider resizing](native-divider-resize.md) owns reversible split previews and one release-only shared ratio submission.

Windows [action menus](native-action-menus.md) consume Client-owned right presses before
ordinary focus. Menu capture cannot replace an existing genuine gesture or wheel stream.

Scroll-indicator jumps, middle-press surfaces,
pixel-format reports, a selection-copy key binding, triple-press line selection, and Ctrl-hover
link decoration remain separate decisions.

## Sidebar navigation input boundary

On Windows, `Ctrl+B Shift+B` explicitly enters sidebar keyboard navigation and
reveals the sidebar when its effective geometry can present it. `Ctrl+B b` keeps
its visibility-toggle meaning. `Picker` owns the visible tree, expansion, focused
stable row and scroll window; the active terminal context remains independent.
`InputDispatcher` owns the input lease and packet fences. No server, wire or
persistent state is added.

The shell's viewport synchronization settles effective sidebar visibility before
dispatching an already-read packet and after layout-changing actions. An absent
viewport sample uses the last dimensions for this settlement. Hidden geometry
retires Picker navigation before dispatcher reconciliation captures pending
fragments. Actual compact-sidebar dismissal retires it in the same local
transition for every backdrop Press variant. Help retains its underlying sidebar
projection and returns to that surface instead.

Dispatch reconciles the final pointer report before returning for paint, even
when no following byte exists. A standalone backdrop wheel Press therefore
retires before PublishFrame, and the next fresh packet is released without an
extra redraw. The trace covers both this packet boundary and a retained tail.

The shell regression traces call this real synchronization boundary for resize
and backdrop Press followed by Enter or split CSI, asserting no Selection and no
terminal bytes until a successful new frame releases later input. This is the
production refinement of the model's RetireSidebar event; a test that manually
observes a hidden Picker before synchronization does not establish that ordering.

| Event / input                               | Guard and observable result                                                                                                                                                                                                                                     |
| ------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Enter navigation                            | Clear competing local modes, reveal the sidebar and ask Picker to enter; capture the opening packet tail.                                                                                                                                                       |
| Navigate / disclose                         | Picker's typed key decision changes only local focus, expansion and window; no Selection request or terminal input.                                                                                                                                             |
| F1 help                                     | Cover the tree with existing shortcut help without leaving navigation, including the compact overlay; F1, Escape or an outside press dismisses help back to the sidebar, PageUp/PageDown page its registry-derived rows, and covered tree keys do not navigate. |
| Explicit Context activation                 | Revalidate the focused Context, invoke the existing sidebar Selection path, leave navigation and capture the decision packet tail.                                                                                                                              |
| Open object menu                            | The exact live row must occur in the last successfully drawn map on the current machine and Site; wrap its menu target in that map's Site/revision proof. Opening leaves Picker focus and terminal context unchanged.                                           |
| Menu cancellation                           | Existing menu capture fences the decision packet and already-polled input until a new frame; Picker reconciles its retained focused identity.                                                                                                                   |
| Escape / hidden sidebar / competing capture | Leave navigation without activation; capture pending fragments and the retiring packet until a fresh map publishes.                                                                                                                                             |
| Unbound key or paste in navigation          | Consume without action; outside an explicit input owner, preserve the ordinary terminal byte path.                                                                                                                                                              |

Project-row and disclosure presses request the dispatcher to acquire its paste
lease before focusing and expanding the exact Project; a failed lease write leaves
Picker unchanged. They never activate a terminal. A row-menu button and right press share the keyboard
menu's identity validation; opening a menu does not first focus or activate its
row. Disclosure, row and menu hit rectangles are disjoint. Other machines cannot
open a menu by silently switching the current endpoint. Menu command availability
and full source-proof revalidation remain with the existing native menu owner.

The deterministic evidence belongs to the existing pointer and dispatcher suites:
same labels with different identities, removed or replaced rows, foreign machine
or Site, undrawn targets, keyboard versus pointer menu admission, capture priority,
mode exit and split decision packets. These traces assert Picker requests, menu
targets, terminal bytes and returned input ownership. Host images establish
presentation only; controlled input traces do not claim physical mouse coverage.

`formal/tla/SidebarInputOwnership.tla`, registered as `sidebar-input-ownership`,
checks the input boundary when a key or paste fragment, owner retirement and a
successful frame can arrive in different orders. `owner` abstracts the dispatch
priority; `menuReturn` represents retained Picker navigation under a menu, not a
second focus store. `packetOwned` and `paintPending` map to dispatcher packet and
retirement guards. `fragment` maps to decoder provenance; `bornOwned` and `leaked`
are specification observations, not runtime state.

| Abstract event                                       | Production refinement and deterministic evidence                                                                                                                                                     |
| ---------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| EnterSidebar / SidebarKey                            | Explicit prefix action and Picker key dispatch; the `sidebar_mode_owns_only_explicit_navigation_and_fences_exit_packets` trace contrasts captured input with unchanged terminal input.               |
| OpenMenu / CloseMenu                                 | Shared painted-target resolver and existing menu capture/settlement; keyboard and row-menu trace preserves Picker focus and returns the paste lease.                                                 |
| RetireSidebar / CoveringCapture / EndCoveringCapture | Picker leaves navigation, dispatcher reconciles competing owners and marks pending decoder fragments captured; the exit trace includes a split key, external hide, repaint and later terminal bytes. |
| PublishFrame / PacketBoundary                        | Successful hit-map publication clears retirement capture; synchronization clears packet capture only after that publication.                                                                         |
| BeginFragment / FinishFragment                       | Existing key/paste decoder carries packet provenance across reads; a fragment born under capture cannot become terminal input after retirement.                                                      |

The model bounds one symbolic fragment and four owners, allowing repeated
entry, menu dismissal, retirement and paint in either order. It checks no input
leak, not tree ordering, canonical membership, menu command effects or physical
host behavior. Those remain with Picker tests, NativeActionMenus and controlled
dispatcher/host evidence. No fairness or unconditional progress is assumed: the
host must supply a new read and a successful frame before retired input is
released. The compact F1/help-return trace separately checks the real sidebar
surface projection.

## Sidebar width input

The sidebar preference belongs to Picker; `SidebarSurface` carries that same
preferred width into layout, terminal geometry and paint. Geometry and limits
remain with [shell layout](tui-shell-layout.md). A `DrawnSidebarWidth` records
the viewport, full surface including preference, and derived sidebar. Only its
exposed right border in a successful frame admits a resize. Keyboard navigation
uses that same painted source without requiring mouse capture.

`PointerState` owns one reversible width drag. Motion changes only the preview;
plain left release revalidates the complete drawn source, drops the gesture and
compares the original preference before committing once. Escape, Shift, missing
map, host resize, hidden sidebar or competing capture cancels without changing
the preference. Preview shares the divider composer's graphics cover and clean
repaint path. A no-op preserves the preferred width even when the viewport clamps
its effective width. Focus, expansion and terminal selection remain unchanged.

The status restore control appears only when a Full or Compact layout can show
the currently hidden sidebar and no covering input owner is active. It invokes
the existing explicit sidebar-focus action. Minimal and TooSmall layouts do not
paint an unusable restore control.

The existing pointer/dispatcher traces cover exact painted-source admission,
absolute preview and release, cancellation, stale preference, released mouse
capture with keyboard resize, packet fragments and fresh-frame release. Shell
composition traces cover preview graphics and terminal-geometry agreement;
geometry vectors and both-locale width rendering remain with the layout owner.

`SidebarInputOwnership.ResizeSidebar` maps to the shared width-decision fence:
the current keyboard owner is retained, the decision packet and pending
fragments are captured, and successful paint releases subsequent reads.
The existing `DividerDrag` model also supplies the width gesture lifecycle:
its source token abstracts viewport, surface and preference; local release/CAS
refines Release followed immediately by SettleKnown, or ReleaseNoChange for a
no-op. No asynchronous pending owner is added. Source loss, capture, Escape,
Shift and duplicate release use the existing cancellation events. This is a
synchronous subset of that model; geometry, exact target equality and local CAS
are checked by the width core and pointer traces, not by the symbolic token.

The actual shell layout callback returns whether synchronization observed a
change. After a handled Press or Motion, the dispatcher immediately invalidates
the old map and gesture on that result, including same-tier width changes and
height-only changes inside one read. No cached viewport class substitutes for
that observation. Real shell traces drive this boundary before a same-packet
release, then require a fresh draw and new press to admit another drag.

Ending a width preview also retires its map even when Escape, Shift or an
unchanged release leaves the preference intact: its status hint covered the
canonical Menu control. The existing redraw fence captures the ending decision
and fragments until a successful canonical frame; same-packet presses cannot
activate those hidden controls. A preview-paint trace checks rejected Menu
presses before that draw and admission after it, without changing Picker focus.

## Capture policy

- Startup arms button-event tracking (`?1002h`) with SGR encoding (`?1006h`) through one
  `ModeLease` pair; `orchbee tui --mouse off` skips arming. Any-motion tracking (`1003`) is never
  armed. The pair is restored on every exit path — normal return, failure, owner-thread panic,
  and each termination signal — like every other leased mode.
- `Ctrl+B m` releases and re-arms capture at runtime. The release applies only after the mode
  write succeeds, so the persistent status indicator (`mouse released · Ctrl+B m restores` /
  `鼠标已释放 · Ctrl+B m 恢复`) never disagrees with the host's actual reporting state. While
  released every decoded pointer event is ignored before probing or hit resolution, and all
  in-flight gestures and the wheel stream are cancelled; re-arming does not resurrect them.
- `--mouse` is admitted on `tui` (`off` and `on` values). Windows `terminal attach` and
  `agent attach` also admit it under the separate [stream contract](windows-terminal-streams.md).
  Bare `orchbee` rejects the option and still defaults to capture on; other routes reject it;
  `tui` continues to reject `--format`. Both declared spellings are admitted — `--mouse off|on` and
  `--mouse=off|on` — while values outside the closed pair, dash-led values, and repeated
  occurrences all refuse.
- A `Shift`-modified report is never claimed: the host reserves `Shift` for native selection. A
  `Shift` report during pane-program passthrough is dropped while that gesture survives so an
  unmodified release still reaches the pane; one received during Client selection cancels the
  Client gesture without copying.

## Region map

The renderer records each interactive region it paints into a staged `HitMapBuilder`; the shell
publishes the map only after the frame's draw completes. Regions carry stable identities —
`WorkbenchTabId`, `NativeContextLocation`, `WorkbenchPanelId` — resolved to positions against a
fresh probe at click time, so a reorder between paint and click cannot misroute. Layers resolve
front to back and never overlap within one layer:

| Painted surface                          | Layer            | Target                             | Accepted gesture                                                                                                                     |
| ---------------------------------------- | ---------------- | ---------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------ |
| Navigator context row                    | `Overlay`        | `Context(location)`                | Press selects that context (`SelectContextAt`)                                                                                       |
| Navigator tab row                        | `Overlay`        | `Tab(tab_id)`                      | Press selects that Tab (`SelectTab`)                                                                                                 |
| Overlay body off every control           | `OverlaySurface` | `OverlaySurface`                   | Swallowed — never reaches the layers beneath                                                                                         |
| Outside a capturing overlay              | —                | backdrop                           | Press dismisses the overlay; no region beneath — including uncovered foreground chrome like the tab strip or a pane — sees the event |
| Compact tab-strip token                  | `Foreground`     | `Tab(tab_id)`                      | Press selects that Tab (`SelectTab`)                                                                                                 |
| Pane leaf viewport (content rect)        | `Foreground`     | `Pane{tab,panel}` + painted source | Exact `Ctrl`+primary click opens the painted link; other presses focus, then use program passthrough or Client selection             |
| Pane leaf title row outside the viewport | `Foreground`     | `Pane{tab,panel}`                  | Press focuses only; the pane's program sees nothing                                                                                  |
| Visible Windows pane divider             | `Foreground`     | `Divider(source)`                  | Plain left press starts preview; release applies once; Esc or Shift cancels                                                          |
| Everything else (status, gaps)           | —                | dead space                         | Ignored                                                                                                                              |

A press on a pane region resolves against the _tab that was painted_: a stale-map edge where the
painted Tab is no longer active degrades to a plain Tab selection rather than focusing an
unreachable panel.

## Gesture lifecycle

| State       | Meaning and exits                                                                                                                                                                                                                                             |
| ----------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `Idle`      | No pointer owner. A press on a hit region opens a gesture or applies navigation directly.                                                                                                                                                                     |
| `Divider`   | Windows: one exact painted split owns a reversible line preview. Release hands off one changed ratio; cancellation discards it. Submission belongs to the shared local resize owner, not the pointer gesture.                                                 |
| `Pane`      | A focused reporting pane owns the pointer until release. Motion and release encode pane-local against the press's painted viewport origin — a drag that leaves the region still belongs to it. A release is never dropped mid-gesture.                        |
| `Selection` | A focused untracked pane owns one Host selection gesture for its exact attachment and projection generation. Motion coalesces within the pane; release remains behind the latest motion and only a Host-accepted active end may request copy.                 |
| `Pending`   | A press on an unfocused pane fired `FocusPanel`; the press and later input queue (FIFO, bound 16, motion merges to the newest cell) until the switch settles. Overflow cancels before any new input is accepted and reports the bounded dropped-input notice. |
| `Link`      | An exact `Ctrl`+primary press owns one painted cell until its matching same-cell release. Movement or source invalidation cancels it; modifier changes never transfer it to a pane or selection owner.                                                        |

Readiness is adjudicated against a fresh probe, never a cached sequence or a generation delta.
The queued press waits for its exact recorded pane incarnation to remain attached, become the
desired focus, settle its mode, and have a usable current frame. Read readiness admits Client
selection on an untracked `ReadOnly` pane. Application input independently requires
`input_ready`, the exact leaf's `Controlling` mode, and a live viewport; a tracking pane in
history consumes the queue locally. A missing or replaced incarnation or a focus redirected
elsewhere cancels without replay. The original two-second deadline cancels with a dropped-input
notice. A completed click may be replaced by a new press on the same painted pane before Start
admission, but repeated presses without release cannot replace it, reset that deadline, or bypass
the queue bound. A same-epoch local focus that reuses the installed attachment admits identically.

Modal input capture (a draft or an armed confirmation) swallows pointer events without gestures;
the modal's own outside-click rules are unchanged — ordinary overlays dismiss on an outside
press, modal surfaces swallow it.

## Pane passthrough

- A pane program owns pointer events inside its content viewport while the current frame reports
  any tracking mode, plus the wheel-on-alternate-screen fallback below. Frame facts — tracking,
  format, cell geometry, alternate screen, DEC 1007, application-cursor mode — come from the trusted
  `TerminalInteraction` of the newest verified frame, probed fresh per event.
- An untracked pane never receives selection bytes. Its ordinary primary-button gesture follows
  [native terminal selection and effects](native-terminal-selection-effects.md); `Shift` continues
  to bypass Client ownership.
- Forwarded events are translated to pane-local coordinates against the painted content origin
  and re-encoded by one owner in the pane's requested tracking × format: X10 reports presses
  only; Normal reports everything but motion; Button reports while a button is held (a press
  counts itself as held); Any reports all; releases always report. Same-cell motion is deduplicated,
  out-of-viewport non-release events report only under motion tracking while held and clamp to the
  edge cell, legacy `Default` format refuses coordinates above 222 rather than wrapping, `Utf8`
  encodes coordinates as codepoints, SGR uses `M`/`m` finals, and URXVT uses its decimal form.
- Cell-only input is encoded as SGR cells even when the pane requests `SgrPixels` (`?1016`),
  matching Herdr's cell fallback. Tracking, modifiers, release identity, clamping and motion
  deduplication retain the ordinary SGR rules. This does not provide exact pixel input or invent
  a cell center; host capture remains cell-based and never enables `?1016`.
- Byte delivery reuses the existing `forward` path, including its `Controlling` check,
  read-only refusal, and acknowledgement semantics. An attached pane may receive local read
  focus regardless of input authority; a title-row press focuses without pane input. A content
  press whose mode is still unknown waits within the original pending deadline. Once a tracking
  pane settles as `ReadOnly`, its program-owned press is dropped with the input notice and no
  implicit acquire. Read focus never substitutes for application-input permission, and history
  never turns a program-owned tracking gesture into Client selection.

## Link activation

An exact `Ctrl`+primary press with neither `Shift` nor `Alt` claims a pane content cell before
program tracking, focus, or selection admission. The explicit modifier is a distinct gesture
owner: an ordinary gesture already owned by `Pane`, `Selection`, or `Pending` cannot be stolen by
a later Ctrl change, while a `Link` gesture consumes later modifier changes until its matching
release. It neither focuses the pane nor requests terminal control, so a read-only Client can open
a link. Movement, a different release cell, capture release, a replaced pane incarnation, a
changed viewport, or hit-map invalidation cancels without replaying any event to the pane.

The renderer records a link source from the same frame or retained-history window it paints. The
successful paint of a noncapturing notification card suppresses every terminal link source for
that published frame. If such a frame replaces an in-flight link source, the gesture becomes a
cancelled tombstone: its matching release is swallowed and produces no external effect. The
source combines the exact pane/member binding with local attachment and sequence provenance, the
host epoch, content generation and revision, columns, total rows, and the painted viewport's
absolute start. Live rows start at `scrollback_rows`; retained-history rows use the validated row
window's `start_row`. A release converts its painted cell to one absolute terminal row and column
once. A fresh read-only viewer may have another attachment and sequence, but its member binding,
host epoch, generation, revision, columns, and total rows must match before and after row reads and
again before effect handoff. A mismatch has no effect.

Each temporary viewer must acknowledge its one detach before another read or an external effect.
An uncertain detach is not retried: the lane drops that connection and refuses the click.

Resolution reads render detail for the clicked row first, preserving OSC 8 metadata and applying
the shared wide-tail-to-head rule. Without an explicit hyperlink it uses the same bounded
lowercase `http://`/`https://` resolver as `terminal link-resolve`, including verified soft-wrap
context outside the visible viewport. One private, joined lane admits at most one click; a busy
lane reports refusal without queueing or retrying. Ordered enabled Windows plugin link handlers
have priority. A matching handler is revalidated immediately before its single submission. A
started, failed, or unknown handler outcome is final for that click. Only a definite no-handler
result may open a lowercase HTTP(S) URL through the Windows shell, once. Shell failure is visible
and is never replayed. Shutdown interrupts owned waits and joins the lane; it does not claim to
cancel an accepted plugin action or a browser process owned by Windows. Ctrl-hover decoration is
not part of this button-event capture slice.

### Link workflow and correctness

`PointerState` owns the `Link` gesture until release; `terminal_link_activation::Driver` owns the
single-flight admission bit and notice; its joined lane owns the accepted job through one terminal
outcome. The Workbench Snapshot and terminal Host remain canonical for member binding and content,
and the Windows plugin registry remains canonical for handler order. Inputs are one exact painted
source and cell, fresh Snapshot/viewer records, registry entries, and plugin or shell outcomes.
Missing or malformed facts are refusal, never an inferred no-handler result. Outputs are `Opened`,
`Handled`, `NoLink`, `Busy`, `Failed`, or `Unknown`; only plugin submission and Windows shell launch
are external effects.

| State / event                                                     | Guard                                                                | Next state and command                                                                       |
| ----------------------------------------------------------------- | -------------------------------------------------------------------- | -------------------------------------------------------------------------------------------- |
| `Idle` + exact Ctrl-primary press                                 | Published pane/content; no prior owner                               | `Link`; retain a valid source/cell, otherwise a cancelled tombstone; no effect               |
| `Link` + movement, Shift/Alt, membership/source/map/capture drift | Press owner still consumes reports                                   | Cancelled `Link`; matching release is swallowed; no effect                                   |
| `Link` + matching same-cell release                               | Source and exact pane remain current                                 | Single-flight `Resolving`; no pane input, focus, or control request                          |
| `Resolving` + fresh reads                                         | Member and content fences match twice                                | Fixed plugin preflight, definite no-handler browser eligibility, or terminal no-link/failure |
| Selected handler + registry re-read                               | Same entry, handler, action, URL, and context                        | At most one plugin submission; its started/failed/unknown result is final                    |
| Definite no handler + lowercase HTTP(S)                           | Deadline live; full registered-catalog identity unchanged at handoff | At most one `ShellExecute`; success/failure is final                                         |
| Busy admission or shutdown                                        | Existing job owns the lane                                           | Refuse without queue/retry, or stop waits and join the owned lane                            |

[`NativeTerminalLinkActivation`](../formal/tla/NativeTerminalLinkActivation.tla), registered in
`scripts/tla/models.json`, checks bounded gesture ownership, stale/registry refusal, one effect per
click, plugin/browser eligibility, and joined-lane shutdown. `LinkGesture`, `PaintedLinkSource`,
`ReadSession`, plugin `Preflight`/`Invocation`, `EffectHandoff`, and `Driver` are the implementation
refinement points. TLC checks the declared finite state graph; it does not prove Win32 behavior,
plugin execution, terminal protocol implementations, or cancellation of effects accepted by other
owners. Deterministic Rust tests and controlled Windows integration provide those boundaries.

## Wheel fallback

Over an alternate-screen pane whose program reports no tracking and enables DEC 1007, wheel notches encode as cursor
arrow presses — three per notch, in the pane's application-cursor form (`ESC O A` vs `ESC [ A`).
A per-pane stream bounds the burst: a reversal clears the queued remainder and drops the notch,
each flush emits at most one viewport of arrows, and a tick that finds the queue empty closes the
stream so the next notch starts fresh. Each flush rechecks eligibility against the newest frame;
disabling DEC 1007, leaving the alternate screen, enabling tracking or entering history discards
the queued arrows. With DEC 1007 off, alternate-screen wheel events send no application input.
Over untracked primary-screen content, an upward notch enters retained history three rows before
the tail; subsequent notches move three rows locally. Downward scrolling at the live tail, empty
history and unavailable frames have no effect or notice. Wheel never changes focus or scrolls
from the title/border; Shift stays host-owned, including in history. Entry, coalescing and recovery
use the [retained viewport](native-terminal-viewport.md) owner.

## Invariants and temporal rules

- `LastDrawnOnly`: events resolve only against the map of the last complete successful draw;
  between a layout change and the next publish every pointer event drops rather than misroutes.
- `PublishAfterDraw`: a staged map becomes authoritative only when its frame's draw completes —
  a failed draw publishes nothing.
- `StableIdentity`: regions carry `WorkbenchTabId`/`NativeContextLocation`/`WorkbenchPanelId`
  re-resolved at click time; no cached index survives a reorder.
- `OneGestureOwner`: press, drag, and release resolve against the press's region; capture
  release, modal capture, or an external layout change cancels the whole gesture.
- `OneLinkEffect`: one matching link release admits at most one plugin submission or one browser
  launch; handler failure, uncertainty, or registry drift never falls through to another effect.
- `PaintedLinkFence`: link coordinates and content fences come from the same completed draw; a
  fresh viewer must prove the captured member and content version before an external effect.
- `ReleaseNeverCrossesPress`: pending-focus input flushes in FIFO order — a release can never be
  delivered ahead of its press, and cancellation drops the queue rather than reordering it.
- `FocusIsClientLocal`: the focus change a press causes applies before any byte reaches the pane.
- `NeverFabricate`: cell events remain cell-addressed through the SGR fallback. No exact pixel
  position is inferred from a cell or unknown host pixel geometry.
- `PendingKeysFirst`: the dispatcher flushes already-accepted pending key bytes before handling a
  pointer event, so a mouse report can never overtake earlier keyboard input; a malformed or
  aborted mouse-shaped sequence (`DecodedKind::Ignored`) is consumed atomically — it can never
  degrade into key presses and its raw bytes are never forwarded to a pane. Unrelated unknown
  CSI keeps the compatibility passthrough (`Nondecision`).
- `CaptureHonesty`: the released indicator appears only after the mode write succeeds and the
  `Ctrl+B m` path is the same lease the startup and restoration paths use.

## Source correspondence

| Production surface                          | Model correspondence                                                                                                                                                                                                   |
| ------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `tui_pointer.rs`                            | `PointerState` capture/map/gesture lifecycle, `FrameHitMap`/`HitMapBuilder` staged regions, `Gesture` (`Idle`/`Pending`/`Pane`/`Selection`/`Divider`/`Link`), `WheelStream`                                            |
| `terminal_input/mouse.rs`                   | `MouseEvent` decode (`decode_sgr`, `decode_x10`), `pane_owns_pointer`, `encode_pane_report`, `encode_cursor_arrows`                                                                                                    |
| `terminal_input/decoder.rs`                 | Typed `DecodedKind::Mouse`/`Ignored` items; X10 atomic three-byte body consumption, timeout, and abort; mouse-shaped malformed/aborted/over-bound runs decode `Ignored`                                                |
| `terminal_input/dispatcher.rs`              | Explicit `Mouse` and `Ignored` arms before the raw-passthrough wildcard — `Ignored` is consumed and dropped, never pending; pending-byte flush first; `RegionsChanged` map staleness; external-change `invalidate_map` |
| `terminal_lifecycle.rs`                     | `set_pointer_capture` ModeLease pair (`?1002`/`?1006`) armed at enter and restored on every exit path                                                                                                                  |
| `tui_shell.rs`                              | Startup arming, `HitMapBuilder` staged per draw, publish-on-success, `set_pointer_capture` callback wiring                                                                                                             |
| `tui_render.rs`, `tabs`, `contexts`         | Per-painted-rect region recording for tab tokens, navigator rows, overlay surface, and pane leaves                                                                                                                     |
| `client_view.rs`, `client_view/bindings.rs` | `Ctrl+B m` → `TogglePointerCapture`, `SelectContextAt`/`FocusPanel`/`SelectTab` dispositions, released indicator and pointer notices                                                                                   |
| `terminal_session/ui.rs`, `state/`          | `pointer_probe` fresh facts, `focus_panel`/`select_context_at` admission, stable-id accessors (`tab_id`, `panel_id`, `location`)                                                                                       |
| `terminal_link_activation.rs`               | Exact source fences, OSC 8/plain resolution, ordered handler handoff, one-effect browser fallback, bounded lane and joined shutdown                                                                                    |
| `terminal_session.rs` (test seam)           | Single-pane `Legacy` probe backed by the real projection frame and controller status — the connected-PTY witness backend                                                                                               |
| `terminal_input.rs`                         | `PointerProbe`/`PointerTab`/`PointerFrameFacts` port surface and `forward_or_report` delivery                                                                                                                          |
| `invocation.rs`, `cli/`                     | `tui --mouse off\|on` admission, help text, and three-shell completion                                                                                                                                                 |
| `crates/terminal-protocol`                  | `TerminalInteraction` trusted Rust API exposing tracking/format/geometry/cursor-mode facts of the verified frame                                                                                                       |

## Verification

The Unix PTY suites below track the shared contract; they are not Windows execution evidence.
Windows cell fallback is covered by the encoder, pointer and attach regressions and the packaged
control/attach byte witnesses described in [terminal streams](windows-terminal-streams.md).

| Surface                   | Evidence                                                                                                                                                                                                                                                                                                                                                                                                                            |
| ------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Decode                    | `terminal_input/mouse/tests` SGR/X10 tables — press/release/motion/wheel/modifiers, cross-chunk atomicity, malformed rejection without key fallthrough; `terminal_input/decoder/tests` — valid SGR/X10 items, X10 split feeds, timeout drain, ESC/C1 abort, malformed bodies, over-bound runs, and interleave with keys/paste — all malformed mouse-shaped bytes as `Ignored`                                                       |
| Encode contract           | `encode_pane_report` table rows for every tracking × format cell, release finals, dedup, clamping, legacy bound, and SGR cell fallback                                                                                                                                                                                                                                                                                              |
| Hit map                   | `tui_pointer/tests` layer order, edge/outside cells, overlay capture and backdrop dismiss — including an outside press on the uncovered tab strip and pane that never selects or forwards — stale-map drop                                                                                                                                                                                                                          |
| Gesture and pending focus | `tui_pointer/tests` press/release ordering, queue bound and overflow cancel, timeout, orphan, read-only, target-moved refusal, settled-without-frame wait, and fact-gated admission                                                                                                                                                                                                                                                 |
| Dispatcher port writes    | `terminal_input/dispatcher/tests` — malformed SGR/X10 and aborted/timed-out mouse runs produce zero forwarded bytes while ordinary keys, paste, and unrelated unknown CSI pass through in order                                                                                                                                                                                                                                     |
| Wheel stream              | `tui_pointer/tests` three-per-notch, per-pass viewport cap, reversal clearing, stream close on drain                                                                                                                                                                                                                                                                                                                                |
| Capture lifecycle         | `terminal_lifecycle/tests` lease pairing and restoration; dispatcher `Ctrl+B m` toggle test; status indicator                                                                                                                                                                                                                                                                                                                       |
| Link click                | Recording pointer ports cover exact modifiers, same-cell release, cancellation, prior gesture owners and busy admission; the shared OSC 8 wide-cell and wrapped HTTP(S) resolvers retain their CLI integration fixtures; route tests use a recording browser port for priority, accepted ambiguity, definite no-handler fallback and one effect; controlled Windows integration covers the fresh viewer and native plugin transport |
| CLI                       | `invocation/tests` `tui --mouse` value admission, bare/other-family refusal, `--format` refusal; `cli/completion` scripts                                                                                                                                                                                                                                                                                                           |
| Real PTY (Legacy seam)    | `tui_shell/connected_tests::real_tui_routes_pointer_reports_to_the_reporting_pane_and_restores_modes` — recorded pane-local SGR bytes, pane-local translation, read-only drop, SGR cell fallback, wheel→arrows, capture toggle, mode restoration                                                                                                                                                                                    |
| Real PTY (public Native)  | `tests/local_startup_process/native_pointer.rs` — the spawned `orchbee` binary on a real PTY: capture arming, X10 and SGR pane-local reports, unfocused-pane click focus and queued flush, read-only rejection, wheel→arrows, `Ctrl+B m`, `--mouse off`, exit restoration, and zero-pixel host geometry keeping cell-addressed reports while `?1016` uses SGR cell fallback                                                         |

## Shared right-button policy

The [Panel input policy](native-panel-input.md) gates fresh right presses before focus or passthrough. Default `orchbee` reserves them for [action menus](native-action-menus.md); `panel` admits only unmodified content presses in a tracking terminal under the existing controller/readiness checks. A pending right press rechecks policy before any bytes. An admitted gesture captures its button and exact Pane incarnation: other buttons cannot inject a press or release it, and generic legacy release maps to its owner. Changing policy affects the next gesture; capture/focus/incarnation cancellation still applies. Menu opening and cancellation preserve focus and capture their matching release.

## Presentation selector

When the session is Ready or selected without attachment, Panels with multiple Presentations show `[<] [position/count] [>]` in their existing header: the outer frame for single/zoomed Panels, or the leaf header for split Panels. The control reserves twelve cells for title/status; narrow headers and the Minimal tier keep the existing label/count and keyboard route. Each arrow has a three-cell hit region. The count and spaces are inert; split Pane hit regions exclude the entire selector strip, so controls never overlap terminal content or focus regions. No extra header row or host mouse mode is introduced.

Only an unmodified left press selects the exact painted destination Presentation. Buttons wrap in canonical member order, equivalent to `Ctrl+B (` / `Ctrl+B )`. A bound target contains the current `PaneRef` and destination Presentation ID. An empty target contains its structural Pane key and exact selected Presentation ID; the fresh probe must still identify that member as unbound. Empty Panel body presses only focus and never start a terminal gesture. The fresh probe must retain the exact incarnation or unbound selection under the current desired anchor; the UI then resolves Panel/member within the active Tab before submitting the existing Switch transaction. Removed/replaced members, old anchors, modal input, capturing overlays and released mouse capture cannot redirect the gesture. Read-only terminal mode does not prohibit Client-local navigation; selection never forwards terminal bytes or bypasses binding-only child attachment. A successful click invalidates the old hit map until redraw.

Evidence covers button edges, inactive count/spacing, disjoint split rectangles, single/split/zoom rendering, narrow and Minimal degradation, modified/non-left reports, capture/overlay/modal guards, stale incarnation/anchor, removed members and actual UI admission to the exact child Switch. Final real-host visual/multi-Client acceptance remains deferred with the full Windows gate.
