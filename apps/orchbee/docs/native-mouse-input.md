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

Windows [Tab arrangement](native-tab-groups.md#pointer-arrangement-and-menu-navigation) owns
press/drag/release, selected membership, group boundaries and static feedback. InputDispatcher
captures the release packet's keys, paste and pending fragments for both commit and cancellation.

Windows [action menus](native-action-menus.md) consume Client-owned right presses before
ordinary focus. Menu capture cannot replace an existing genuine gesture or wheel stream.

Middle-press surfaces,
pixel-format reports, a selection-copy key binding, triple-press line selection, and Ctrl-hover
link decoration remain separate decisions.

## Mouse controls across surfaces

Mouse and keyboard enter the same native state owners. Visible bracketed controls and complete rows have exact hit rectangles; disabled, clipped or covered cells cannot activate an underlying target. Local navigation may act on press. Native form, menu, settings, copy, resize and Agent-board buttons wait for a matching plain-left release with the same painted token and rectangle. Keyboard input, focus loss, capture release and changed drawing authority revoke that press. A decision owns its packet tail and late release, then fresh input returns to the retained terminal focus.

Confirmation, rename/setup/worktree forms, Settings, the Machine picker and plugin popups
register the exact painted X target for outside-backdrop dismissal. A plain-left press and matching
release must remain outside the same completely painted surface; entering its body, a new token,
map invalidation or focus loss cancels the intent. Wheel and modified reports are inert there.
Closing follows the existing X owner, preserving accepted submissions and their recovery; no
underlying pane, control or shell receives the gesture or its decision-packet tail. Existing menu,
Help, navigator and Agent board backdrop routes retain their local return policy.

`tui_composition::project_capture` is the one authority for exclusive paint, input, and notification
admission. Machine capture covers native exclusive surfaces; their existing order covers Copy,
which covers Resize. Claims describe retained capture intent rather than proof that a surface was
painted. A missing or stale context for the selected owner consumes input instead of exposing a
lower owner. Copy exit retains its physical packet fence even when Resize becomes the next owner.

Client shortcut help remains noncapturing. `client_chrome_overlay_allowed` admits its painting and
mouse route over retained Copy or Resize while rejecting it beneath Machine or a native exclusive
capture. Help mouse controls run before the retained mode; ordinary key and paste routing keep
that mode's existing contract. Closing help restores the parent without releasing its input lease.

When a separately admitted higher capture covers a chrome gesture, the dispatcher immediately
retires its Divider, Sidebar-width, Tab-drag, or Tab-create arm into the existing one-shot release
cleanup. Selection and modal interaction keep their own owners. The frame painter independently
suppresses those previews whenever any exclusive owner is present, including a capture installed
between input reconciliation and drawing. A pending plugin popup still admits its exact painted
close target; its body, key, and paste readiness remain with the plugin owner.

The central generic owner and the existing divider, sidebar-width, Tab-create, Tab-drag, and
painted-modal owners retain only typed primary-release cleanup after revocation. That cleanup runs
before capture, host-focus, and hit-map guards, including for an already queued report after capture
release. The exact late release is consumed once and fences its whole physical packet without
re-hit or effect; intervening pointer reports cannot re-hit through the debt. A fresh non-wheel
press retires it, while an orphan release does not create one. Pane passthrough, links, selection,
and their cleanup owners are unchanged by this contract.

For painted modal controls, the cancelled painted-press record and the dispatcher's matching
button-release debts, including an action menu's paired packet debt, settle in the same pre-owner
step. A newer Settings or Machine surface cannot hide that cleanup or make a later release fence
twice. An active release-committed modal control or copy-surface drag bypasses packet cleanup until
its own owner has processed the exact release; cancelled cleanup never re-hits that target or runs
its effect.

| Surface                         | Mouse flow                                                                                                                                              |
| ------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Machine/Workspace/Agent search  | Bracketed categories switch views; complete rows select exact identities. Agent filters, search, Clear/Done and X remain local to the picker.           |
| Workspace navigator             | X closes; each column has its own scroll area. Scrolling changes local focus without activating a context or Tab.                                       |
| Shortcut help                   | X and previous/next page controls; wheel inside the document changes one page. Closing restores any underlying copy, resize or sidebar mode.            |
| Native confirmations and rename | X, Cancel and Confirm/Submit share keyboard owners. Destructive confirmation remains unavailable unless the full warning and action row were drawn.     |
| Workspace/Worktree forms        | Clickable fields, exact choice rows, Cancel/Submit and stage-specific status/hide controls; accepted operations retain their existing completion owner. |
| Settings                        | Clickable rows, choice arrows, child Confirm/Cancel, page Back and header X; only Save writes.                                                          |
| Agent board                     | Full-row selection, Open, Refresh and X; Open addresses the selected stable Agent identity.                                                             |
| Copy and resize modes           | Pane action menus enter them; mode toolbars, copy drag/scroll and resize directions use their existing native owners.                                   |

Wheel belongs only to the painted list under the pointer; headings, buttons, frame padding and the backdrop are inert. Lists normalize three rows/choices per notch, bound accumulated work and settle at most one painted viewport per pass; Worktree capacity counts its two-line choices. Reversal drops old-direction work while retaining the first opposite notch. A changed owner/source/geometry cancels backlog. Pending list movement settles before a following keyboard decision, and any resulting native draft token still requires its own successful draw. No scroll path executes a row or changes a setting value.

Released capture deliberately returns mouse input to the host. The existing keyboard capture toggle remains the restoration path; local controls cannot reclaim mouse events the host does not send.

When a complete help page cannot fit, the resize notice retains a painted X button. Dismissal still captures the decision packet and restores the underlying mode; widening the window instead restores the retained help page. A refused page cannot leave an invisible mouse owner with only a keyboard exit.

An already-read packet resolves pending input frames before idle decoder timeouts. Rendering or native-state work taking longer than the 50 ms wait must not turn a split mouse report into a lone Escape plus printable coordinates, either in the terminal or in a native form. Only a poll without input expires these waits; an actual idle lone Escape still cancels or forwards through its existing owner. Shell-loop and modal/dispatcher traces cover ready SGR/X10 fragments across a busy frame and retain the idle-Escape witnesses.

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
transition for ordinary backdrop button presses. Backdrop wheel is inert. Help retains its underlying sidebar
projection and returns to that surface instead.

Dispatch reconciles the final pointer report before returning for paint, even
when no following byte exists. An ordinary backdrop button press retires before
PublishFrame; an outside wheel leaves navigation active. The trace covers the
decision packet boundary, retained tails and fresh input after publication.

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
| Open object menu                            | The exact live row must occur in the last successfully drawn map on the current machine and Session; wrap its menu target in that map's Session/revision proof. Opening leaves Picker focus and terminal context unchanged.                                     |
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
or Session, undrawn targets, keyboard versus pointer menu admission, capture priority,
mode exit and split decision packets. These traces assert Picker requests, menu
targets, terminal bytes and returned input ownership. Host images establish
presentation only; controlled input traces do not claim physical mouse coverage.

| Abstract event                                       | Production refinement and deterministic evidence                                                                                                                                                     |
| ---------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| EnterSidebar / SidebarKey                            | Explicit prefix action and Picker key dispatch; the `sidebar_mode_owns_only_explicit_navigation_and_fences_exit_packets` trace contrasts captured input with unchanged terminal input.               |
| OpenMenu / CloseMenu                                 | Shared painted-target resolver and existing menu capture/settlement; keyboard and row-menu trace preserves Picker focus and returns the paste lease.                                                 |
| RetireSidebar / CoveringCapture / EndCoveringCapture | Picker leaves navigation, dispatcher reconciles competing owners and marks pending decoder fragments captured; the exit trace includes a split key, external hide, repaint and later terminal bytes. |
| PublishFrame / PacketBoundary                        | Successful hit-map publication clears retirement capture; synchronization clears packet capture only after that publication.                                                                         |
| BeginFragment / FinishFragment                       | Existing key/paste decoder carries packet provenance across reads; a fragment born under capture cannot become terminal input after retirement.                                                      |

## Sidebar width input

The sidebar preference belongs to Picker; `SidebarSurface` carries that same
preferred width into layout, terminal geometry and paint. Geometry and limits
remain with [shell layout](tui-shell-layout.md). A `DrawnSidebarWidth` records
the viewport, full surface including preference, and derived sidebar. Only its
exposed right border in a successful frame admits a resize. Keyboard navigation
uses that same painted source without requiring mouse capture.

`PointerState` owns one reversible width drag. Motion projects the actual sidebar
width, row layout, border hits and docked terminal geometry before the next frame;
the preference remains unchanged while held. A Compact overlay changes its own
width without shrinking the terminal underneath. The immutable canonical source
and last successful draft-frame proof are distinct, so a second held move survives
its own first frame while external stale geometry still cancels. A
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

The actual shell layout callback returns whether synchronization observed an
external host or sidebar-owner change. Owned draft projections also synchronize
terminal sizes, but do not report themselves as external changes. After a handled
Press or Motion, the dispatcher immediately invalidates
the old map and gesture on that result, including same-tier width changes and
height-only changes inside one read. No cached viewport class substitutes for
that observation. Real shell traces drive this boundary before a same-packet
release, then require a fresh draw and new press to admit another drag.

Ending a width draft also retires its map even when Escape, Shift or an
unchanged release leaves the preference intact: its status hint covered the
canonical Menu control. The existing redraw fence captures the ending decision
and fragments until a successful canonical frame; same-packet presses cannot
activate those hidden controls. A preview-paint trace checks rejected Menu
presses before that draw and admission after it, without changing Picker focus.

## Capture policy

- Startup arms button-event tracking (`?1002h`) with SGR encoding (`?1006h`) through one
  `ModeLease`; `orchbee tui --mouse off` skips arming. After this baseline completes, the dispatcher
  issues one `1003` DECRQM query with a DA1 barrier for the retained outer endpoint. Only a reset
  report (`2`) followed by that barrier admits global any-motion tracking. Unsupported or permanently
  fixed reports retain button-only tracking; a contradictory set report, missing reply, or the
  300 ms deadline settles unknown. None retries on menu open or capture re-arm. The same lease
  disables `1003`, `1006` and `1002` on every exit path — normal return, failure, owner-thread panic,
  and each termination signal — like every other leased mode.
- `Ctrl+B m` releases and re-arms capture at runtime. The release applies only after the mode
  write succeeds, so the persistent status indicator (`mouse released · Ctrl+b m restores` /
  `鼠标已释放 · Ctrl+b m 恢复`) never disagrees with the host's actual reporting state. While
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
front to back; within a layer, the first matching region wins. Specific controls are recorded
before any containing surface region:

The central interaction projection is now available as an adoption foundation. A feature may
record an `InteractionSpec` only after its renderer admits and paints exact geometry. The record
combines a closed `InteractionTarget` carrying the owner's complete semantic witness, hit layer,
exact hit rectangle, complete painted rectangle, explicit primary-button contract
(`ActivateOnRelease`, `SelectOnPress`, or `Specialized`), and availability. The fingerprint also
canonicalizes the frontmost semantic winners within that painted rectangle. Adding, removing, or
replacing a child therefore changes its containing row's effective exposure even when the press
cell itself remains uncovered; a target outside that rectangle does not. A smaller child hit
rectangle may refer to a complete painted rectangle. Neither target identity nor gesture policy is
inferred from text, color, position, or a broad row target; child actions must be recorded before a
containing row.

`PointerState` projects only `Rest`, `Hovered`, or `Pressed` into the next candidate frame. It does
not write keyboard focus, current or checked state, availability, pending state, or any domain model.
The action-menu pointer adapter additionally reports a real enabled-row movement to its owner,
which updates only the [provisional menu candidate](native-action-menus.md#interaction).
Other collections retain their provisional selection. `Hovered` requires a real button-free movement
report after global `AnyMotion` admission or a completed scoped `MenuMotion` request. Button-only
observability, capture release, host-focus loss, and observability downgrade clear retained position,
so restoration cannot synthesize hover. The dispatcher publishes global `AnyMotion` only after both
capability admission and the typed mode transition complete successfully; scoped menu observability
requires its own completed transition. An uncertain partial transition exits through the restoring boundary instead
of publishing a fallback state. Replies received while capture is released settle admission only;
the next capture uses that result without another query. Late and duplicate replies cannot reopen
settled admission. Action menus retain a scoped request with button fallback while global admission
is absent; their exit restores the global level, and released capture is never implicitly re-armed.
Mode support admission and semantic delivery evidence remain distinct; the
[terminal environment reference](native-terminal-environment.md) owns the probe and host witness.

An enabled `ActivateOnRelease` target arms on a plain left press and activates only when release
still resolves to the same typed target, complete painted rectangle, contract, and availability.
Its layer, exact hit extent, and effective exposure must also match. Leaving that target,
replacement, occlusion, disablement, structural invalidation, or ownership loss permanently
cancels the arm; a visual-only repaint of the same fingerprint does not. A cancelled arm consumes
its late release and cannot retarget, while an already observed stationary position can reconcile
hover against the next successful map. `SelectOnPress` exists only for a feature that explicitly
owns provisional navigation, while `Specialized` leaves press handling to the feature's dedicated
drag or selection state machine.

The shell snapshots the projection before a candidate draw and publishes its candidate map only
after that frame flushes successfully. Publication revalidates a stationary observed position. If
that changes the projection, the shell schedules exactly one later-turn repaint; the follow-up map
carries the new revision and does not reschedule itself. A failed or partial draw publishes no
candidate and invalidates the previous pointer authority before the fatal loop exit. A packet made
entirely of button-free motion can skip repaint only when every decoded item leaves the projection
unchanged and no decoder, timer, layout, or owner transition changed anything else.

Settings, action menus, sidebars, Agent board, Help, Navigator, Machine Picker, Presentation
selectors, Tab creation, and the global Menu and Sidebar Restore buttons use this central
protocol. Generic controls activate on matching release. Sidebar Menu/Create retain their
specialized menu-opening owner, and Agent-board rows use `SelectOnPress` only for provisional
navigation. Windows Tab/group arrangement, dividers, scrollbars, Copy selection, and pane input
retain their specialized gesture owners. Disabled targets stay visible and consuming without
falling through to a containing row or pane.

Help controls and scrolling carry the exact opening episode and successfully presented page;
complete refusal admits only its close control. Help remains noncapturing for keyboard input and
paste. Navigator rows and close carry their opening and catalog source. Machine Picker uses an
exact opening, mode, ordered catalog, and actual filter/query basis; its renderer stages targets
in the central map and owns no local rectangle cache. Async replacement, reopen, or source drift
cannot activate an older candidate. Global chrome actions have stable action identity and recheck
live admission on release; Sidebar Restore also requires an available, currently hidden sidebar.

Copy, Resize, and native forms publish explicit specialized modal-control fingerprints with
availability and exact geometry. A raw modal-shaped region does not acquire implicit Enabled
authority. Shared controls preserve focus, current, checked, provisional navigation, status, and
operation cues while applying hover or pressed backgrounds. A cancelled Tab click also retires
its pressed presentation; returning does not rearm it, while a later threshold-crossing drag
retains the specialized arrangement contract.

| Painted surface                          | Layer            | Target                                                     | Accepted gesture                                                                                                              |
| ---------------------------------------- | ---------------- | ---------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------- |
| Navigator context row                    | `Overlay`        | Windows `NavigatorRow(Row)`; otherwise `Context(location)` | Matching primary release selects the exact painted context                                                                    |
| Navigator tab row                        | `Overlay`        | Windows `NavigatorRow(Row)`; otherwise `Tab(tab_id)`       | Matching primary release selects the exact painted Tab                                                                        |
| Overlay body off every control           | `OverlaySurface` | `OverlaySurface`                                           | Swallowed — never reaches the layers beneath                                                                                  |
| Outside a capturing overlay              | —                | backdrop                                                   | Ordinary button press dismisses; wheel is inert. No region beneath, including uncovered tab chrome or a pane, sees the event. |
| Compact tab-strip token                  | `Foreground`     | `Tab(tab_id)` with explicit interaction metadata           | Windows press arms click/drag and release activates or commits once; other platforms use matching-release selection           |
| Pane leaf viewport (content rect)        | `Foreground`     | `Pane{tab,panel}` + painted source                         | Exact `Ctrl`+primary click opens the painted link; other presses focus, then use program passthrough or Client selection      |
| Pane leaf title row outside the viewport | `Foreground`     | `Pane{tab,panel}`                                          | Press focuses only; the pane's program sees nothing                                                                           |
| Visible Windows pane divider             | `Foreground`     | `Divider(source)`                                          | Plain left press starts preview; release applies once; Esc or Shift cancels                                                   |
| Everything else (status, gaps)           | —                | dead space                                                 | Ignored                                                                                                                       |

A press on a pane region resolves against the _tab that was painted_: a stale-map edge where the
painted Tab is no longer active degrades to a plain Tab selection rather than focusing an
unreachable panel.

The Windows navigator uses `NavigatorRow(episode, source, target)` proofs for every completely
painted row and an exact `DrawSource` for close. Only a matching plain primary release on that
exact row can request activation; keyboard Enter additionally requires
the painted focus. The [navigator contract](native-context-selection.md#windows-capturing-navigator)
owns admission and cancellation. Backdrop presses dismiss without click-through, except wheel
and the existing Shift host bypass. Wheel scrolls only its painted column without activation; modified/right row presses do not activate. A consumed row/backdrop decision owns its matching release after a successful fresh
frame; release, host focus loss or a new qualifying non-Shift press clears that ownership.
Paint retirement alone cannot clear it. Entry cancels the existing gesture through its cleanup
owner, without copying/exporting selection or flushing queued pane mouse input.

## Gesture lifecycle

The pane's dedicated right chrome column records `Scrollbar{pane,content,source}` on the
Foreground layer, disjoint from pane content and split dividers. Plain left press jumps on the
track or grabs the thumb; motion and release stay Client-owned even over another pane. The
`Scrollbar` gesture retains exact pane identity and painted geometry, and cancels on changed
history range, generation, geometry, focus, capture, keyboard input or host focus loss.
Unknown/loading windows expose no new target. The [viewport contract](native-terminal-viewport.md#scrollbar-interaction)
owns bounded navigation, live-output cancellation and recovery. Selection/copy-mode and program
mouse reporting keep their existing precedence; no scrollbar event encodes terminal input.

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

Modal input capture gives its painted controls exclusive pointer ownership; no gesture reaches the pane underneath. Ordinary overlays dismiss on an outside button press; modal surfaces swallow it. Wheel outside a painted list is inert.

A modal control that uses the current painted-release arm is tied to its exact painted target and
rectangle. Moving outside that rectangle permanently cancels the arm; moving back inside or
repainting cannot re-arm it. A visual-only repaint of the same target and rectangle preserves an
arm that never left. Activation then requires the matching release, and a cancelled interaction
requires a fresh press. This is the current modal-control invariant; it does not claim that every
native chrome target has already migrated from press activation.

## Pane passthrough

A fresh event over pane content resolves against the exact painted `PaneRef`, viewport, and the
newest verified frame. `Shift` stays host-owned. Program routing uses the same tracking predicate
as the encoder:

| Tracking | Program-reportable classes                                                            |
| -------- | ------------------------------------------------------------------------------------- |
| None     | None. Client selection/history or the separate DEC 1007 fallback may apply.           |
| X10      | Left, middle, or right press only.                                                    |
| Normal   | Encodable presses and a captured release; no motion.                                  |
| Button   | Encodable presses, held-button motion, and a captured release; no button-free motion. |
| Any      | The Button classes plus button-free motion.                                           |

Fresh admission is narrower than wire reportability. A reportable non-wheel button press may start
a `Pane` gesture; a reportable wheel press is one-shot. A release or held-button motion without that
press-time owner remains an inert Client-side event and never retargets into the pane under the
pointer. Button-free `Any` motion is the one stateless program event: it reaches only the exact
already-focused, live, controlling pane inside its painted content rectangle, starts no gesture,
and cannot focus or migrate to a sibling or replacement incarnation. Current startup capture
remains button-event `1002`; this routing does not itself enable global any-motion capture.

Once admitted, a `Pane` gesture retains its exact pane incarnation, painted viewport origin, and
button until matching release. Accepted motion and release remain with that owner even if a newer
frame changes tracking. The current tracking gate remains authoritative for bytes: an event class
the new mode does not report is consumed silently, while its matching release still clears the
gesture. This separates gesture cleanup from fresh Client routing and prevents an orphan tail from
becoming selection, history, or input for another pane.

With no tracking, only a vertical wheel press on an alternate-screen pane with DEC 1007 enters the
cursor-arrow fallback below. The fallback is unavailable whenever tracking is active. An active
mode receives only the classes it reports. For example, X10 wheel remains Client-owned: on the
primary screen it can drive retained history without emitting an illegal program report. Selection
bytes never reach an untracked pane. Its ordinary primary-button gesture follows
[native terminal selection and effects](native-terminal-selection-effects.md).

Forwarded events translate to pane-local coordinates against the painted content origin and use one
tracking × format encoder. Same-cell captured motion is deduplicated. Out-of-viewport non-release
events report only under motion tracking while held and clamp to the edge cell. Legacy `Default`
refuses coordinates above 222 rather than wrapping; `Utf8` encodes coordinates as codepoints; SGR
uses `M`/`m` finals; URXVT uses its decimal form. Cell-only input uses SGR cells when the pane
requests `SgrPixels` (`?1016`), matching Herdr's cell fallback without inventing a pixel
position or enabling `?1016` in host capture.

Byte delivery reuses the existing `forward` path, including its `Controlling` check, read-only
refusal, and acknowledgement semantics. An attached pane may receive local read focus without input
authority; a title-row press focuses without pane input. A content press whose mode is still unknown
waits within the original pending deadline. Once a reporting pane settles as `ReadOnly`, its
program-owned press is dropped with the input notice and no implicit acquire. Read focus never
substitutes for application-input permission, and history never converts a captured program gesture
into Client selection.

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

## Wheel fallback

Over an alternate-screen pane whose program reports no tracking and enables DEC 1007, wheel notches encode as cursor
arrow presses — three per notch, in the pane's application-cursor form (`ESC O A` vs `ESC [ A`).
A per-pane stream bounds the burst: a reversal clears the old direction's queued remainder and
admits the first new notch, including when both directions arrive in one input packet;
each flush emits at most one viewport of arrows, and a tick that finds the queue empty closes the
stream so the next notch starts fresh. Each flush rechecks eligibility against the newest frame;
disabling DEC 1007, leaving the alternate screen, enabling tracking or entering history discards
the queued arrows. With DEC 1007 off, alternate-screen wheel events send no application input.
Regression witnesses cover immediate reversal, reversal with a multi-viewport backlog, the new
direction's application-cursor encoding, bounded draining and no resumption of the retired tail.
Over untracked primary-screen content, an upward notch enters retained history three rows before
the tail; subsequent notches move three rows locally. Downward scrolling at the live tail, empty
history and unavailable frames have no effect or notice. Wheel never changes focus or scrolls
from the title/border; Shift stays host-owned, including in history. Entry, coalescing and recovery
use the [retained viewport](native-terminal-viewport.md) owner.

## Invariants and temporal rules

- `LastDrawnOnly`: events resolve only against the map of the last complete successful draw;
  between a layout change and the next publish every pointer event drops rather than misroutes.
- `PublishAfterDraw`: a staged map becomes authoritative only when its frame's draw completes —
  a failed draw publishes nothing and retires the previous map because the host may contain a
  partial candidate.
- `ExplicitInteraction`: generic hover and activation require painted `InteractionSpec` metadata;
  labels, colors, region variants, and coordinates never invent a contract.
- `ReleaseFingerprint`: generic activation requires the same target, painted rectangle, contract,
  availability, and authoritative hit. Frame sequence alone neither cancels nor re-arms it.
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

| Production surface                                                                                       | Model correspondence                                                                                                                                                                                                              |
| -------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `tui_pointer.rs`, `tui_pointer/interaction.rs`                                                           | `PointerState` capture/map/gesture lifecycle, typed interaction metadata and projection, stable release fingerprint, `FrameHitMap`/`HitMapBuilder` staged regions, specialized `Gesture` owners, `WheelStream`                    |
| `terminal_input/mouse.rs`                                                                                | `MouseEvent` decode (`decode_sgr`, `decode_x10`), `route_pane_pointer`, `encode_pane_report`, `encode_cursor_arrows`                                                                                                              |
| `terminal_input/decoder.rs`                                                                              | Typed `DecodedKind::Mouse`/`Ignored` items; X10 atomic three-byte body consumption, timeout, and abort; mouse-shaped malformed/aborted/over-bound runs decode `Ignored`                                                           |
| `terminal_input/dispatcher.rs`                                                                           | Explicit `Mouse` and `Ignored` arms before raw forwarding; generic pointer outcomes and whole-packet unchanged-motion redraw decision; pending bytes and decoder deadlines retain ordering; structural changes invalidate the map |
| `terminal_lifecycle.rs`                                                                                  | Baseline Button/SGR capture, one-shot typed `1003` admission, settled motion observability, journaled restoration on every exit                                                                                                   |
| `tui_shell.rs`                                                                                           | Startup arming, immutable pointer projection per candidate, publish on success only, one later-turn reconciliation repaint, failed-draw authority invalidation, capture callback wiring                                           |
| `tui_render/settings.rs`, `action_menu.rs`, `agent_board.rs`, `agent_sidebar.rs`, `workspace_sidebar.rs` | Exact typed interactions for Settings, open action menus, Agent board and visible sidebars                                                                                                                                        |
| `tui_render.rs`, `help`, `navigator`, `tabs`, `contexts`, `presentation_selector`, `machine_picker`      | Exact authored chrome interactions and source witnesses; pane leaf regions keep their dedicated input contract                                                                                                                    |
| `client_view.rs`, `client_view/bindings.rs`                                                              | `Ctrl+B m` → `TogglePointerCapture`, `SelectContextAt`/`FocusPanel`/`SelectTab` dispositions, released indicator and pointer notices                                                                                              |
| `terminal_session/ui.rs`, `state/`                                                                       | `pointer_probe` fresh facts, `focus_panel`/`select_context_at` admission, stable-id accessors (`tab_id`, `panel_id`, `location`)                                                                                                  |
| `terminal_link_activation.rs`                                                                            | Exact source fences, OSC 8/plain resolution, ordered handler handoff, one-effect browser fallback, bounded lane and joined shutdown                                                                                               |
| `terminal_session.rs` (test seam)                                                                        | Single-pane `Legacy` probe backed by the real projection frame and controller status — the connected-PTY witness backend                                                                                                          |
| `terminal_input.rs`                                                                                      | `PointerProbe`/`PointerTab`/`PointerFrameFacts` port surface and `forward_or_report` delivery                                                                                                                                     |
| `invocation.rs`, `cli/`                                                                                  | `tui --mouse off\|on` admission, help text, and three-shell completion                                                                                                                                                            |
| `crates/terminal-protocol`                                                                               | `TerminalInteraction` trusted Rust API exposing tracking/format/geometry/cursor-mode facts of the verified frame                                                                                                                  |

## Verification

The Unix PTY suites below track the shared contract; they are not Windows execution evidence.
Windows cell fallback is covered by the encoder, pointer and attach regressions and the packaged
control/attach byte witnesses described in [terminal streams](windows-terminal-streams.md).

| Surface                   | Evidence                                                                                                                                                                                                                                                                                                                                                                                                                            |
| ------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Decode                    | `terminal_input/mouse/tests` SGR/X10 tables — press/release/motion/wheel/modifiers, cross-chunk atomicity, malformed rejection without key fallthrough; `terminal_input/decoder/tests` — valid SGR/X10 items, X10 split feeds, timeout drain, ESC/C1 abort, malformed bodies, over-bound runs, and interleave with keys/paste — all malformed mouse-shaped bytes as `Ignored`                                                       |
| Encode and ownership      | `route_pane_pointer` fresh-event matrix plus `encode_pane_report` rows for every tracking × format cell, captured cleanup, release finals, dedup, clamping, legacy bound, and SGR cell fallback                                                                                                                                                                                                                                     |
| Hit map                   | `tui_pointer/tests` layer order, edge/outside cells, overlay capture and backdrop dismiss — including an outside press on the uncovered tab strip and pane that never selects or forwards — stale-map drop                                                                                                                                                                                                                          |
| Interaction projection    | `tui_pointer/tests/interaction` covers hover enter/repeat/leave, disabled and specialized targets, press/release, permanent leave cancellation, repaint continuity, changed fingerprint/occlusion, transport loss, child precedence, and one bounded stationary-position follow-up                                                                                                                                                  |
| Generic adopters          | Renderer/dispatcher tests cover Settings, menus, sidebars, Board, Help, Navigator, Machine Picker, Presentation, Tab creation and global chrome: exact sources/geometry, disabled consumption, stale refusal, hover isolation from committed selection, menu candidate movement, release cancellation and no retarget                                                                                                               |
| Gesture and pending focus | `tui_pointer/tests` press/release ordering, queue bound and overflow cancel, timeout, orphan, read-only, target-moved refusal, settled-without-frame wait, and fact-gated admission                                                                                                                                                                                                                                                 |
| Dispatcher port writes    | `terminal_input/dispatcher/tests` — malformed SGR/X10 and aborted/timed-out mouse runs produce zero forwarded bytes while ordinary keys, paste, and unrelated unknown CSI pass through in order; continuous button-free motion cannot starve an expired Escape and only an all-unchanged-motion packet skips repaint                                                                                                                |
| Wheel stream              | `tui_pointer/tests` three-per-notch, per-pass viewport cap, reversal clearing, stream close on drain                                                                                                                                                                                                                                                                                                                                |
| Capture lifecycle         | `terminal_lifecycle/tests` lease pairing and restoration; dispatcher `Ctrl+B m` toggle test; status indicator                                                                                                                                                                                                                                                                                                                       |
| Link click                | Recording pointer ports cover exact modifiers, same-cell release, cancellation, prior gesture owners and busy admission; the shared OSC 8 wide-cell and wrapped HTTP(S) resolvers retain their CLI integration fixtures; route tests use a recording browser port for priority, accepted ambiguity, definite no-handler fallback and one effect; controlled Windows integration covers the fresh viewer and native plugin transport |
| CLI                       | `invocation/tests` `tui --mouse` value admission, bare/other-family refusal, `--format` refusal; `cli/completion` scripts                                                                                                                                                                                                                                                                                                           |
| Controlled Windows Host   | `apps/terminal-host` `test-conpty-orchbee-pointer` launches the Orchbee child through pinned ConPTY/Ghostty and exercises the actual inherited Reader, reply decoder, one-shot mode admission, menu motion without a capability reply, return to button policy, semantic hover distinction and mode restoration; it does not establish physical pointer/font/palette behavior                                                       |
| Real PTY (Legacy seam)    | `tui_shell/tests/connected::real_tui_routes_pointer_reports_to_the_reporting_pane_and_restores_modes` — recorded pane-local SGR bytes, pane-local translation, read-only drop, SGR cell fallback, wheel→arrows, capture toggle, mode restoration                                                                                                                                                                                    |
| Real PTY (public Native)  | `tests/local_startup_process/native_pointer.rs` — the spawned `orchbee` binary on a real PTY: capture arming, X10 and SGR pane-local reports, unfocused-pane click focus and queued flush, read-only rejection, wheel→arrows, `Ctrl+B m`, `--mouse off`, exit restoration, and zero-pixel host geometry keeping cell-addressed reports while `?1016` uses SGR cell fallback                                                         |

## Shared right-button policy

The [Panel input policy](native-panel-input.md) gates fresh right presses before focus or passthrough. Default `orchbee` reserves them for [action menus](native-action-menus.md); `panel` admits only unmodified content presses in a tracking terminal under the existing controller/readiness checks. A pending right press rechecks policy before any bytes. An admitted gesture captures its button and exact Pane incarnation: other buttons cannot inject a press or release it, and generic legacy release maps to its owner. Changing policy affects the next gesture; capture/focus/incarnation cancellation still applies. Menu opening and cancellation preserve focus and capture their matching release.

## Presentation selector

When the session is Ready or selected without attachment, Panels with multiple Presentations show `[<] [position/count] [>]` in their existing header: the borderless title band for single/zoomed Panels, or the leaf header for split Panels. The control reserves twelve cells for title/status; narrow headers and the Minimal tier keep the existing label/count and keyboard route. Each arrow has a three-cell hit region. The count and spaces are inert; split Pane hit regions exclude the entire selector strip, so controls never overlap terminal content or focus regions. No extra header row or host mouse mode is introduced.

An unmodified left press arms the exact painted destination Presentation; only its matching release selects it. Buttons wrap in canonical member order, equivalent to the effective Previous/Next Presentation bindings (defaults `Ctrl+B (` / `Ctrl+B )`). A bound target contains the current `PaneRef` and destination Presentation ID. An empty target contains its structural Pane key and exact selected Presentation ID; the fresh probe must still identify that member as unbound. Empty Panel body presses only focus and never start a terminal gesture. The fresh probe must retain the exact incarnation or unbound selection under the current desired anchor; the UI then resolves Panel/member within the active Tab before submitting the existing Switch transaction. Removed/replaced members, old anchors, modal input, capturing overlays and released mouse capture cannot redirect the gesture. Read-only terminal mode does not prohibit Client-local navigation; selection never forwards terminal bytes or bypasses binding-only child attachment. A successful click invalidates the old hit map until redraw.

Evidence covers button edges, inactive count/spacing, disjoint split rectangles, single/split/zoom rendering, narrow and Minimal degradation, modified/non-left reports, capture/overlay/modal guards, stale incarnation/anchor, removed members and actual UI admission to the exact child Switch. Composed TestBackend captures establish cell composition; physical host palette, font, pointer hardware, latency, and multi-Client visual behavior require their own native evidence.
