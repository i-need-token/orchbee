# Native divider resizing

## Contract and owners

A Windows Client can resize a painted pane split by pressing its divider with the plain left button, dragging a preview line, then releasing once to apply. Preview leaves the shared layout and program dimensions unchanged. Ratios remain 10%–90%, shared across Clients and saved by the existing checkpoint owner. Keyboard resizing remains available through [native keyboard resizing](native-keyboard-resize.md). This capability follows [mouse input](../../../docs/tui/mouse-input.md); it does not add a protocol, storage format, input takeover or live process recovery.

`PointerState` owns one reversible `DividerDrag`. `tui_layout` owns the deterministic geometry and its immutable `DrawnDivider`: Session, terminal context, Tab, focused panel, Workbench revision, split, ordered slots, previous ratio, orientation, split area, painted divider and viewport. The renderer records only visible dividers in the last successful frame. Zoomed, collapsed and unframed splits expose no resize target. A fresh native check at press, motion, idle and release must agree with that entire source. A different focus, revision, geometry, capture or missing hit map cancels the preview; it never retargets to a new divider.

Decoded `MouseEvent` coordinates are untrusted cell positions. Only plain left press admits a drag; left-button motion and left or generic release continue it even outside the original region. Ctrl or Alt cannot replace an admitted owner. Other buttons and wheels cannot replace it. Shift cancels the preview and remains Host-owned. The pure `DividerDrag::update` uses displacement along the split axis divided by distributable cells, added to the original ratio and clamped. Perpendicular motion does nothing. Zero displacement preserves the exact original floating-point value. Release projects its own final coordinate, independently of the last motion event. The preview uses the existing minimum-pane-size projection; it may stop moving before the nominal ratio reaches its limit in a small viewport.

The input port returns admission or a visible busy/failure notice and accepts a changed release ratio once. `NativeUi` rechecks the selected cache and geometry. `ClientControl` owns the single local asynchronous acknowledgement used by keyboard and mouse: original selection, request identity and deadline. Its lane atomically distinguishes transient backlog from a stopped or failed owner; an idle readiness refusal remains visible. It submits the existing exact split-ratio CAS once, waits for canonical publication and a compatible successful draw, and emits no remote receipt. [Shared split ratios](../../orchbee-server/docs/shared-split-ratios.md) owns Server mutation and persistence. The first frame and a newly switched surface initialize their control owner before publication; every draw observes the current viewport without another control tick. Rendering is pure; preview geometry never becomes authoritative pane geometry. At rest, the preview replaces only the glyph and divider foreground, preserving the visible underlay background and modifiers, including the recovered lead-cell style when it crosses a wide glyph. Hovered and pressed previews apply the reviewed shared interaction surfaces. Graphics are covered while a preview is actually displayed so images cannot obscure the line. If the successful base frame no longer exposes the captured source, the shell restores the canonical frame and graphics before that same flush. The localized status explains release and Esc. A painted preview owns the status tail: the closed machine chooser is hidden until the preview ends, while an opened machine picker keeps its normal capture priority. Hiding the chooser retires its painted pointer regions in the same frame, so a drag or release crossing that former area keeps its divider owner.

## Transitions and decisions

| State/event                                                    | Guard and outcome                                                                       |
| -------------------------------------------------------------- | --------------------------------------------------------------------------------------- |
| Idle, plain left press                                         | Exact visible source and free mutation lane: retain source and press cell; show preview |
| Idle, modified/other press                                     | No drag, no pane bytes or mutation                                                      |
| Preview, motion                                                | Same source: replace reversible preview; never resize programs                          |
| Preview, unchanged release                                     | End locally; no mutation, revision or request allocation                                |
| Preview, changed release                                       | Drop gesture before one exact CAS handoff; wait in existing local acknowledgement       |
| Preview, Esc/Shift/map loss/capture/navigation/viewport change | Drop preview; no mutation or delayed replay                                             |
| Submission pending, further divider press                      | Consume with busy notice; retain no new drag                                            |
| Definite refusal / ambiguous result                            | Visible failure / unknown; no automatic retry                                           |
| Known commit, publication and compatible draw                  | Complete locally; programs resize only through their physical controller                |

Ordinary pane mouse tracking, selection, wheel normalization and Host capture leases retain their existing owners. A release without an admitted drag is inert. Bindings and overlays can cancel a preview; they do not commit it. A ReadOnly Client may change shared ratios but receives no input or terminal-resize authority from the gesture.

## Correctness and maintenance

The owning implementation tests cover hit geometry, captured target fencing, latest-intent coalescing, interruption, one submission, CAS conflicts and publication/draw ordering.

Maintain the concrete witnesses in `src/tui_layout/divider/tests.rs`, `src/tui_pointer/tests/divider.rs`, `src/terminal_session/client_control/tests/divider.rs` and the adjacent state, carrier, input and rendering suites. Use controlled completion/clock order and generated trace replay where the workflow has multiple outcomes.

Run `cargo test --locked -p orchbee --all-targets --all-features`. This workflow has no separate TLA model. The evidence covers named/generated implementation traces; it does not establish exhaustive interleavings, physical terminal appearance or untested platforms. Shared identity, ownership and commit protocols retain their separately registered core models.
