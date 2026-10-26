# Native keyboard resizing

## Input and ownership

Native Clients enter resize mode with `Ctrl+B P`. Plain arrows move a nearby divider by five percentage points; Shift+arrow moves it by one point. `Esc` leaves the mode. The mode is Client-local and never changes focus, zoom or terminal control. Its status indicator remains visible; `?` opens mode help; plain PageUp/PageDown page it. Unknown keys, prefix chords, releases and atomic paste are consumed while the mode owns input. Exclusive dialogs and popups take priority. Opening another capture or reloading bindings leaves the mode. After exit, ordinary input again belongs to the pane.

`ClientView` owns the mode and a pure event-to-action registry. `InputDispatcher` owns decoded packet routing; `ClientControl` owns the asynchronous adjustment and its single mutation lane. Configuration can bind `resize_mode` and `resize_pane_left`, `resize_pane_right`, `resize_pane_up`, `resize_pane_down` to safe direct or prefix chords. Direct resizing uses five points without entering the mode. Existing profiles and ordinary arrows retain their meaning outside the mode.

The pane action menu opens the same resize mode. Its toolbar exposes four directions, a 5%/1% step toggle, help and Done. It replaces the Tab strip, or covers the first content row in Minimal layout without changing Host geometry. Direction controls carry the painted Session/context/Tab/focus/revision and viewport/sidebar; release revalidates all of them before using the existing keyboard adjustment lane. No resize submits on press. A pending submission disables further direction controls. Keyboard input, focus loss, capture or geometry changes cancel a pending click; Done exits without cancelling an already submitted adjustment.

## Geometry and submission

A keyboard action reads only the selected native cache and current viewport. `ClientPanelLayout::resize_split` prefers the requested source edge, then the opposite edge on the same axis, and uses distance followed by canonical traversal order for ties. Hidden or unavailable geometry fails; no eligible split is a visible no-op. Left/up reduce the first slot's share; right/down increase it. `ClientResizeAmount::apply` clamps the nominal ratio to 10%–90%; reaching the limit is a visible no-op. These are the same resolver and signed ratio rules used by CLI resizing, including nested layouts and zoom's unzoomed geometry.

The numeric codec preserves each admitted ratio's exact `f64` value through JSON encoding and decoding. Workbench protocol explicitly enables `serde_json`'s `float_roundtrip` in runtime dependency scope; test-only feature unification is insufficient evidence for shipment. Directional arithmetic such as `0.54 - 0.05` must retain the same value after transport. This is a codec refinement of canonical ratio identity, not an epsilon comparison or a different JSON/storage format.

The captured Session, context, Tab, split, ordered slots, previous ratio and Workbench revision form one exact `layout.set-split-ratio` CAS. There is no optimistic local ratio. [Shared split ratios](../../orchbee-server/docs/shared-split-ratios.md) owns canonical commit, publications and checkpoint storage. The existing mailbox allocates a fresh request identity, and the existing cancellable mutation worker submits it once. No new wire or persisted format is introduced.

| State/event                                  | Next state and observable result                                            |
| -------------------------------------------- | --------------------------------------------------------------------------- |
| Idle, valid changed key                      | Submit one mutation; retain original selection and request identity         |
| Submission pending, more resize keys         | Consume and report busy; never accumulate or replay keys                    |
| Server rejection                             | Report failure; preserve current canonical state; no retry                  |
| Reply missing or ambiguous after submission  | Report unknown outcome; inspect layout before a new explicit action         |
| Known commit                                 | Await matching publication and a compatible successful draw                 |
| Published and drawn exact identity/ratio     | Complete locally; never emit a remote Client-view receipt                   |
| Selection change, interruption or expiry     | Report incomplete if submission may have committed; never claim rollback    |
| Esc while submitted                          | Leave input mode; the submitted adjustment still settles                    |
| Local adjustment and remote control delivery | Reject the remote delivery as busy, retaining separate completion ownership |
| Shutdown                                     | Stop admission, cancel bounded exchanges and join the owned lane            |

A committed adjustment waits while the publication owner retains pending refresh demand or an admitted refresh flight. A temporary full/busy refresh queue keeps that demand alive; it is not an adjustment failure. The existing publication drain admits the read later, under the original adjustment deadline, without resubmitting the mutation. This refines the model's delayed publication transition; it does not assume eventual admission.

A read returning an older canonical revision does not disprove the known commit. The adjustment continues waiting for the publication owner to reach its commit watermark, up to the original deadline; it neither schedules a second adjustment-owned read nor resubmits the mutation.

A local adjustment and a remote adjustment never share acknowledgement identity. Finishing or interrupting a UI wait cannot clear the worker's still-pending handoff. Late unmatched completions are drained without completing another request. A readonly Client may change shared layout but cannot acquire terminal input or resize authority. Successful drawing confirms Client geometry; Host physical resizing is independently asynchronous.

## Correctness and maintenance

The owning implementation tests cover one in-flight mutation without backlog, exact completion provenance, mode exit/replacement, inactive-target purity and draw-before-local-success.

Maintain the concrete witnesses in `src/workbench_client/client_control/tests.rs`, `src/workbench_client/ui.rs` and the adjacent state, carrier, input and rendering suites. Use controlled completion/clock order and generated trace replay where the workflow has multiple outcomes.

Run `cargo test --locked -p orchbee --all-targets --all-features`. This workflow has no separate TLA model. The evidence covers named/generated implementation traces; it does not establish exhaustive interleavings, physical terminal appearance or untested platforms. Shared identity, ownership and commit protocols retain their separately registered core models.
