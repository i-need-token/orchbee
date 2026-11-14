# Native keyboard resizing

## Input and ownership

Native Clients resize the focused Panel with the prefix followed by Alt+arrow: `Ctrl+b Alt+Left`, `Ctrl+b Alt+Right`, `Ctrl+b Alt+Up` and `Ctrl+b Alt+Down`. Each press moves the nearest divider of the focused Panel by five percentage points. There is no resize mode: every step takes the prefix again, and the prefix layer keeps no repeat window. The keys work wherever the Panel focus keys `Ctrl+b` + arrow work. A bare Alt+arrow without the prefix still reaches the program in the Panel, and arrows and Alt+arrows inside overlays, copy mode and the Find bar keep their own meaning. Dragging a divider is the pointer route; see [divider resizing](native-divider-resize.md).

The four keys are the configurable actions `resize_panel_left`, `resize_panel_right`, `resize_panel_up` and `resize_panel_down` of the [key configuration](windows-cli-configuration.md). They match through the resolved binding registry like every other prefix action, so shortcut help shows the effective chords. An override may move them to safe direct or other prefix chords; direct resize bindings require a modifier or a function key. `ClientView` resolves the key to a typed `ResizePane` action, `InputDispatcher` owns decoded packet routing, and `ClientControl` owns the asynchronous adjustment and its single mutation lane. The action changes neither focus, the maximized state nor terminal control.

## Geometry and submission

A keyboard action reads only the selected native cache and current viewport. `ClientPanelLayout::resize_split` prefers the requested source edge, then the opposite edge on the same axis, and uses distance followed by canonical traversal order for ties. Hidden or unavailable geometry fails; no eligible split is a visible no-op. Left/up reduce the first slot's share; right/down increase it. `ClientResizeAmount::apply` clamps the nominal ratio to 10%–90%; reaching the limit is a visible no-op. These are the same resolver and signed ratio rules used by CLI resizing, including nested layouts and the unmaximized geometry under maximization.

The numeric codec preserves each admitted ratio's exact `f64` value through JSON encoding and decoding. Workbench protocol explicitly enables `serde_json`'s `float_roundtrip` in runtime dependency scope; test-only feature unification is insufficient evidence for shipment. Directional arithmetic such as `0.55 - 0.05` must retain the same value after transport. This is a codec refinement of canonical ratio identity, not an epsilon comparison or a different JSON/storage format.

The captured Session, context, Tab, split, ordered slots, previous ratio and Workbench revision form one exact `layout.set-split-ratio` CAS. There is no optimistic local ratio. [Shared split ratios](../../orchbee-server/docs/shared-split-ratios.md) owns canonical commit, publications and checkpoint storage. The existing mailbox allocates a fresh request identity, and the existing cancellable mutation worker submits it once. No new wire or persisted format is introduced.

| State/event                                  | Next state and observable result                                            |
| -------------------------------------------- | --------------------------------------------------------------------------- |
| Idle, valid changed key                      | Submit one mutation; retain original selection and request identity         |
| Submission pending, more resize keys         | Consume and report busy; never accumulate or replay keys                    |
| Rejection for an older revision only         | Resubmit the same move against the newer revision; see divider resizing     |
| Other Server rejection                       | Report failure; preserve current canonical state; no retry                  |
| Reply missing or ambiguous after submission  | Report unknown outcome; inspect layout before a new explicit action         |
| Known commit                                 | Await matching publication and a compatible successful draw                 |
| Published and drawn exact identity/ratio     | Complete locally; never emit a remote Client-view receipt                   |
| Selection change, interruption or expiry     | Report incomplete if submission may have committed; never claim rollback    |
| Local adjustment and remote control delivery | Reject the remote delivery as busy, retaining separate completion ownership |
| Shutdown                                     | Stop admission, cancel bounded exchanges and join the owned lane            |

While a submitted adjustment settles, the status line shows its pending indicator; a refusal of the next key replaces that indicator with its notice.

A committed adjustment waits while the publication owner retains pending refresh demand or an admitted refresh flight. A temporary full/busy refresh queue keeps that demand alive; it is not an adjustment failure. The existing publication drain admits the read later, under the original adjustment deadline, without resubmitting the mutation. This refines the model's delayed publication transition; it does not assume eventual admission.

A read returning an older canonical revision does not disprove the known commit. The adjustment continues waiting for the publication owner to reach its commit watermark, up to the original deadline; it neither schedules a second adjustment-owned read nor resubmits the mutation.

A local adjustment and a remote adjustment never share acknowledgement identity. Finishing or interrupting a UI wait cannot clear the worker's still-pending handoff. Late unmatched completions are drained without completing another request. A readonly Client may change shared layout but cannot acquire terminal input or resize authority. Successful drawing confirms Client geometry; Host physical resizing is independently asynchronous.

## Correctness and maintenance

The owning implementation tests cover the step and divider choice, one in-flight mutation without backlog, exact completion provenance, inactive-target purity and draw-before-local-success. A raw-byte dispatcher test sends `Ctrl+b` followed by the CSI form of Alt+Right (`ESC [ 1 ; 3 C`) and checks that it yields one resize action and forwards nothing of the chord, while a bare Alt+Right still reaches the Panel.

Maintain the concrete witnesses in `crates/workbench-client/src/workbench_client/ui/control_steps/tests/keyboard.rs`, `crates/workbench-client/src/terminal_input/dispatcher/tests/configured.rs` and the adjacent state, carrier, input and rendering suites. Use controlled completion/clock order and generated trace replay where the workflow has multiple outcomes.

Run `cargo test --locked -p orchbee-workbench-client --lib`. This workflow has no separate TLA model. The evidence covers named/generated implementation traces; it does not establish exhaustive interleavings, physical terminal appearance, which hosts deliver Alt+arrow after the prefix, or untested platforms. Shared identity, ownership and commit protocols retain their separately registered core models.
