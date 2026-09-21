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
machinery that arming and restoration ride on.

This slice deliberately does not implement divider drag-resize, history selection and clipboard,
multi-press word/line selection, scroll-indicator jumps, right/middle-press surfaces, pixel-format
reports, or primary-screen wheel history scrolling. Those are separate accepted decisions.

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
- `--mouse` is admitted only on `tui` (`off` and `on` values). Bare `orchbee` rejects the option
  and still defaults to capture on; every other command family rejects it; `tui` continues to
  reject `--format`. Both declared spellings are admitted — `--mouse off|on` and
  `--mouse=off|on` — while values outside the closed pair, dash-led values, and repeated
  occurrences all refuse.
- A `Shift`-modified report is never claimed: the host reserves `Shift` for native selection, and
  a `Shift` report mid-drag is dropped while the gesture itself survives so an unmodified release
  still reaches the pane.

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
| Pane leaf viewport (content rect)        | `Foreground`     | `Pane{tab,panel}`   | Press focuses, then the pane's reported contract decides passthrough                                                                 |
| Pane leaf title row outside the viewport | `Foreground`     | `Pane{tab,panel}`   | Press focuses only; the pane's program sees nothing                                                                                  |
| Everything else (status, dividers, gaps) | —                | dead space          | Ignored                                                                                                                              |

A press on a pane region resolves against the _tab that was painted_: a stale-map edge where the
painted Tab is no longer active degrades to a plain Tab selection rather than focusing an
unreachable panel.

## Gesture lifecycle

| State     | Meaning and exits                                                                                                                                                                                                                                             |
| --------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `Idle`    | No pointer owner. A press on a hit region opens a gesture or applies navigation directly.                                                                                                                                                                     |
| `Pane`    | A focused reporting pane owns the pointer until release. Motion and release encode pane-local against the press's painted viewport origin — a drag that leaves the region still belongs to it. A release is never dropped mid-gesture.                        |
| `Pending` | A press on an unfocused pane fired `FocusPanel`; the press and later input queue (FIFO, bound 16, motion merges to the newest cell) until the switch settles. Overflow cancels before any new input is accepted and reports the bounded dropped-input notice. |

Readiness is adjudicated against a fresh probe, never a cached sequence or a generation delta:
the queued gesture is released only when the recorded `tab`/`panel` identities still resolve and
are the selection anchor, no switch is in flight (`attached == desired`), input is admissible
(`input_ready` — the same Ready-phase-and-`Controlling` gate `forward` itself checks; an
installed attachment key alone is a converging switch, not a ready target), and the desired
attachment has a current frame. A settled state whose anchor moved elsewhere, vanished
identities, a settled `ReadOnly` verdict, or the two-second deadline each cancel the gesture —
`ReadOnly` and expiry report the notice; cancellation never replays queued input onto a
different pane and never chases a new focus target. A same-epoch local focus that reuses the
installed attachment admits identically: the attachment's identity follows the anchor.

Modal input capture (a draft or an armed confirmation) swallows pointer events without gestures;
the modal's own outside-click rules are unchanged — ordinary overlays dismiss on an outside
press, modal surfaces swallow it.

## Pane passthrough

- A pane program owns pointer events inside its content viewport while the current frame reports
  any tracking mode, plus the wheel-on-alternate-screen fallback below. Frame facts — tracking,
  format, cell geometry, alternate screen, application-cursor mode — come from the trusted
  `TerminalInteraction` of the newest verified frame, probed fresh per event.
- Forwarded events are translated to pane-local coordinates against the painted content origin
  and re-encoded by one owner in the pane's requested tracking × format: X10 reports presses
  only; Normal reports everything but motion; Button reports while a button is held (a press
  counts itself as held); Any reports all; releases always report. Same-cell motion is deduplicated,
  out-of-viewport non-release events report only under motion tracking while held and clamp to the
  edge cell, legacy `Default` format refuses coordinates above 222 rather than wrapping, `Utf8`
  encodes coordinates as codepoints, SGR uses `M`/`m` finals, and URXVT uses its decimal form.
- A `SgrPixels` (`?1016`) request is refused: the pane receives nothing, the Client reports one
  bounded localized notice per gesture, and no pixel coordinates are fabricated — including a
  fake cell center.
- Byte delivery reuses the existing `forward` path, so the `Controlling` check, read-only
  refusal, and acknowledgement semantics are unchanged. A `ReadOnly` attachment drops pointer
  input without an implicit acquire. A fresh unfocused press on a pane that cannot currently
  admit input for this Client — a settled `ReadOnly` verdict, or a still-unsettled mode that
  has not arrived between `Attached` and `SetupConfigured` — is adjudicated on the spot: a
  content press reports the bounded dropped-input notice, the title row stays Client
  territory and drops quietly, and no `FocusPanel` is issued and no gesture opens. The
  notice states a factual input drop only; an unsettled mode is not a permission verdict. A
  pane whose mode reports `Controlling` still takes the ordinary local-focus path, and a
  focus switch that was already accepted still settles through the pending gesture.

## Wheel fallback

Over an alternate-screen pane whose program reports no tracking, wheel notches encode as cursor
arrow presses — three per notch, in the pane's application-cursor form (`ESC O A` vs `ESC [ A`).
A per-pane stream bounds the burst: a reversal clears the queued remainder and drops the notch,
each flush emits at most one viewport of arrows, and a tick that finds the queue empty closes the
stream so the next notch starts fresh. Primary-screen wheel history scrolling is a later slice.

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
- `NeverFabricate`: pixel requests are refused with a notice; unknown cell geometry already gates
  pane passthrough instead of fabricating a center.
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
| `tui_pointer.rs`                            | `PointerState` capture/map/gesture lifecycle, `FrameHitMap`/`HitMapBuilder` staged regions, `Gesture` (`Idle`/`Pending`/`Pane`), `WheelStream`                                                                         |
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

| Surface                   | Evidence                                                                                                                                                                                                                                                                                                                                                                                       |
| ------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Decode                    | `terminal_input/mouse/tests` SGR/X10 tables — press/release/motion/wheel/modifiers, cross-chunk atomicity, malformed rejection without key fallthrough; `terminal_input/decoder/tests` — valid SGR/X10 items, X10 split feeds, timeout drain, ESC/C1 abort, malformed bodies, over-bound runs, and interleave with keys/paste — all malformed mouse-shaped bytes as `Ignored`                  |
| Encode contract           | `encode_pane_report` table rows for every tracking × format cell, release finals, dedup, clamping, legacy bound, and pixel refusal                                                                                                                                                                                                                                                             |
| Hit map                   | `tui_pointer/tests` layer order, edge/outside cells, overlay capture and backdrop dismiss — including an outside press on the uncovered tab strip and pane that never selects or forwards — stale-map drop                                                                                                                                                                                     |
| Gesture and pending focus | `tui_pointer/tests` press/release ordering, queue bound and overflow cancel, timeout, orphan, read-only, target-moved refusal, settled-without-frame wait, and fact-gated admission                                                                                                                                                                                                            |
| Dispatcher port writes    | `terminal_input/dispatcher/tests` — malformed SGR/X10 and aborted/timed-out mouse runs produce zero forwarded bytes while ordinary keys, paste, and unrelated unknown CSI pass through in order                                                                                                                                                                                                |
| Wheel stream              | `tui_pointer/tests` three-per-notch, per-pass viewport cap, reversal clearing, stream close on drain                                                                                                                                                                                                                                                                                           |
| Capture lifecycle         | `terminal_lifecycle/tests` lease pairing and restoration; dispatcher `Ctrl+B m` toggle test; status indicator                                                                                                                                                                                                                                                                                  |
| CLI                       | `invocation/tests` `tui --mouse` value admission, bare/other-family refusal, `--format` refusal; `cli/completion` scripts                                                                                                                                                                                                                                                                      |
| Real PTY (Legacy seam)    | `tui_shell/connected_tests::real_tui_routes_pointer_reports_to_the_reporting_pane_and_restores_modes` — recorded pane-local SGR bytes, pane-local translation, read-only drop, pixel refusal, wheel→arrows, capture toggle, mode restoration                                                                                                                                                   |
| Real PTY (public Native)  | `tests/local_startup_process/native_pointer.rs` — the spawned `orchbee` binary on a real PTY: capture arming, X10 and SGR pane-local reports, unfocused-pane click focus and queued flush, read-only rejection, wheel→arrows, `Ctrl+B m`, `--mouse off`, exit restoration, and zero-pixel host geometry keeping cell-addressed reports while `?1016` still gets the bounded unsupported notice |
