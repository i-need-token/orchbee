# Native mouse input

## Scope and ownership

This reference governs the native Client's pointer behavior: host capture lifecycle, the
last-drawn-frame hit map, the gesture vocabulary the painted regions accept, passthrough encoding
to pane programs, and the alternate-screen wheel fallback. Maintain it with `tui_pointer.rs`,
`terminal_input/mouse.rs`, `terminal_input/decoder.rs`, `terminal_input/dispatcher.rs`,
`terminal_lifecycle.rs`, `tui_render.rs` and its `tabs`/`contexts` painters, `tui_shell.rs`,
`client_view.rs` and `client_view/bindings.rs`, `terminal_session/ui.rs`, `invocation.rs`, and
`localization.rs`. The repository standard
[`docs/tui/mouse-input.md`](../../../docs/tui/mouse-input.md) owns the rules this mapping must
satisfy; [native terminal environment](native-terminal-environment.md) owns the mode-lease
machinery that arming and restoration ride on. [Native terminal selection and effects](native-terminal-selection-effects.md)
owns untracked-pane selection and its copy lifecycle.

Windows [divider resizing](native-divider-resize.md) owns reversible split previews and one release-only shared ratio submission.

Scroll-indicator jumps, right-button context menus, middle-press surfaces,
pixel-format reports, a selection-copy key binding, and triple-press line selection remain separate
decisions.

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

| Painted surface                          | Layer            | Target              | Accepted gesture                                                                                                                     |
| ---------------------------------------- | ---------------- | ------------------- | ------------------------------------------------------------------------------------------------------------------------------------ |
| Navigator context row                    | `Overlay`        | `Context(location)` | Press selects that context (`SelectContextAt`)                                                                                       |
| Navigator tab row                        | `Overlay`        | `Tab(tab_id)`       | Press selects that Tab (`SelectTab`)                                                                                                 |
| Overlay body off every control           | `OverlaySurface` | `OverlaySurface`    | Swallowed — never reaches the layers beneath                                                                                         |
| Outside a capturing overlay              | —                | backdrop            | Press dismisses the overlay; no region beneath — including uncovered foreground chrome like the tab strip or a pane — sees the event |
| Compact tab-strip token                  | `Foreground`     | `Tab(tab_id)`       | Press selects that Tab (`SelectTab`)                                                                                                 |
| Pane leaf viewport (content rect)        | `Foreground`     | `Pane{tab,panel}`   | Press focuses, then the pane's reported contract decides program passthrough or Client selection                                     |
| Pane leaf title row outside the viewport | `Foreground`     | `Pane{tab,panel}`   | Press focuses only; the pane's program sees nothing                                                                                  |
| Visible Windows pane divider             | `Foreground`     | `Divider(source)`   | Plain left press starts preview; release applies once; Esc or Shift cancels                                                          |
| Everything else (status, gaps)           | —                | dead space          | Ignored                                                                                                                              |

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
| `tui_pointer.rs`                            | `PointerState` capture/map/gesture lifecycle, `FrameHitMap`/`HitMapBuilder` staged regions, `Gesture` (`Idle`/`Pending`/`Pane`/`Selection`/`Divider`), `WheelStream`                                                   |
| `terminal_input/mouse.rs`                   | `MouseEvent` decode (`decode_sgr`, `decode_x10`), `pane_owns_pointer`, `encode_pane_report`, `encode_cursor_arrows`                                                                                                    |
| `terminal_input/decoder.rs`                 | Typed `DecodedKind::Mouse`/`Ignored` items; X10 atomic three-byte body consumption, timeout, and abort; mouse-shaped malformed/aborted/over-bound runs decode `Ignored`                                                |
| `terminal_input/dispatcher.rs`              | Explicit `Mouse` and `Ignored` arms before the raw-passthrough wildcard — `Ignored` is consumed and dropped, never pending; pending-byte flush first; `RegionsChanged` map staleness; external-change `invalidate_map` |
| `terminal_lifecycle.rs`                     | `set_pointer_capture` ModeLease pair (`?1002`/`?1006`) armed at enter and restored on every exit path                                                                                                                  |
| `tui_shell.rs`                              | Startup arming, `HitMapBuilder` staged per draw, publish-on-success, `set_pointer_capture` callback wiring                                                                                                             |
| `tui_render.rs`, `tabs`, `contexts`         | Per-painted-rect region recording for tab tokens, navigator rows, overlay surface, and pane leaves                                                                                                                     |
| `client_view.rs`, `client_view/bindings.rs` | `Ctrl+B m` → `TogglePointerCapture`, `SelectContextAt`/`FocusPanel`/`SelectTab` dispositions, released indicator and pointer notices                                                                                   |
| `terminal_session/ui.rs`, `state/`          | `pointer_probe` fresh facts, `focus_panel`/`select_context_at` admission, stable-id accessors (`tab_id`, `panel_id`, `location`)                                                                                       |
| `terminal_session.rs` (test seam)           | Single-pane `Legacy` probe backed by the real projection frame and controller status — the connected-PTY witness backend                                                                                               |
| `terminal_input.rs`                         | `PointerProbe`/`PointerTab`/`PointerFrameFacts` port surface and `forward_or_report` delivery                                                                                                                          |
| `invocation.rs`, `cli/`                     | `tui --mouse off\|on` admission, help text, and three-shell completion                                                                                                                                                 |
| `crates/terminal-protocol`                  | `TerminalInteraction` trusted Rust API exposing tracking/format/geometry/cursor-mode facts of the verified frame                                                                                                       |

## Verification

The Unix PTY suites below track the shared contract; they are not Windows execution evidence.
Windows cell fallback is covered by the encoder, pointer and attach regressions and the packaged
control/attach byte witnesses described in [terminal streams](windows-terminal-streams.md).

| Surface                   | Evidence                                                                                                                                                                                                                                                                                                                                                                      |
| ------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Decode                    | `terminal_input/mouse/tests` SGR/X10 tables — press/release/motion/wheel/modifiers, cross-chunk atomicity, malformed rejection without key fallthrough; `terminal_input/decoder/tests` — valid SGR/X10 items, X10 split feeds, timeout drain, ESC/C1 abort, malformed bodies, over-bound runs, and interleave with keys/paste — all malformed mouse-shaped bytes as `Ignored` |
| Encode contract           | `encode_pane_report` table rows for every tracking × format cell, release finals, dedup, clamping, legacy bound, and SGR cell fallback                                                                                                                                                                                                                                        |
| Hit map                   | `tui_pointer/tests` layer order, edge/outside cells, overlay capture and backdrop dismiss — including an outside press on the uncovered tab strip and pane that never selects or forwards — stale-map drop                                                                                                                                                                    |
| Gesture and pending focus | `tui_pointer/tests` press/release ordering, queue bound and overflow cancel, timeout, orphan, read-only, target-moved refusal, settled-without-frame wait, and fact-gated admission                                                                                                                                                                                           |
| Dispatcher port writes    | `terminal_input/dispatcher/tests` — malformed SGR/X10 and aborted/timed-out mouse runs produce zero forwarded bytes while ordinary keys, paste, and unrelated unknown CSI pass through in order                                                                                                                                                                               |
| Wheel stream              | `tui_pointer/tests` three-per-notch, per-pass viewport cap, reversal clearing, stream close on drain                                                                                                                                                                                                                                                                          |
| Capture lifecycle         | `terminal_lifecycle/tests` lease pairing and restoration; dispatcher `Ctrl+B m` toggle test; status indicator                                                                                                                                                                                                                                                                 |
| CLI                       | `invocation/tests` `tui --mouse` value admission, bare/other-family refusal, `--format` refusal; `cli/completion` scripts                                                                                                                                                                                                                                                     |
| Real PTY (Legacy seam)    | `tui_shell/connected_tests::real_tui_routes_pointer_reports_to_the_reporting_pane_and_restores_modes` — recorded pane-local SGR bytes, pane-local translation, read-only drop, SGR cell fallback, wheel→arrows, capture toggle, mode restoration                                                                                                                              |
| Real PTY (public Native)  | `tests/local_startup_process/native_pointer.rs` — the spawned `orchbee` binary on a real PTY: capture arming, X10 and SGR pane-local reports, unfocused-pane click focus and queued flush, read-only rejection, wheel→arrows, `Ctrl+B m`, `--mouse off`, exit restoration, and zero-pixel host geometry keeping cell-addressed reports while `?1016` uses SGR cell fallback   |

## Shared right-button policy

The [Panel input policy](native-panel-input.md) gates fresh right presses before focus or passthrough. Default `orchbee` reserves them; `panel` admits only unmodified content presses in a tracking terminal under the existing controller/readiness checks. A pending right press rechecks policy before any bytes. An admitted gesture captures its button and exact Pane incarnation: other buttons cannot inject a press or release it, and generic legacy release maps to its owner. Changing policy affects the next gesture; capture/focus/incarnation cancellation still applies. This does not add a context menu.

## Presentation selector

When the session is Ready or selected without attachment, Panels with multiple Presentations show `[<] [position/count] [>]` in their existing header: the outer frame for single/zoomed Panels, or the leaf header for split Panels. The control reserves twelve cells for title/status; narrow headers and the Minimal tier keep the existing label/count and keyboard route. Each arrow has a three-cell hit region. The count and spaces are inert; split Pane hit regions exclude the entire selector strip, so controls never overlap terminal content or focus regions. No extra header row or host mouse mode is introduced.

Only an unmodified left press selects the exact painted destination Presentation. Buttons wrap in canonical member order, equivalent to `Ctrl+B (` / `Ctrl+B )`. A bound target contains the current `PaneRef` and destination Presentation ID. An empty target contains its structural Pane key and exact selected Presentation ID; the fresh probe must still identify that member as unbound. Empty Panel body presses only focus and never start a terminal gesture. The fresh probe must retain the exact incarnation or unbound selection under the current desired anchor; the UI then resolves Panel/member within the active Tab before submitting the existing Switch transaction. Removed/replaced members, old anchors, modal input, capturing overlays and released mouse capture cannot redirect the gesture. Read-only terminal mode does not prohibit Client-local navigation; selection never forwards terminal bytes or bypasses binding-only child attachment. A successful click invalidates the old hit map until redraw.

Evidence covers button edges, inactive count/spacing, disjoint split rectangles, single/split/zoom rendering, narrow and Minimal degradation, modified/non-left reports, capture/overlay/modal guards, stale incarnation/anchor, removed members and actual UI admission to the exact child Switch. Final real-host visual/multi-Client acceptance remains deferred with the full Windows gate.
