# Windows Client view control

This contract owns native CLI discovery and navigation of a live Windows TUI Client. The TUI owns its selection and zoom; Server routes bounded requests and retains observations. Canonical Workbench snapshots, revisions and terminal control ownership are unchanged. A routed request does not activate an operating-system window.

## Identity and publication

An authenticated Client Session may publish one interactive view. Its Client identity is the admitted nonzero Session generation encoded as 32 lowercase hexadecimal characters, scoped by Site. Server assigns it; generations never repeat during one Server lifetime. Ordinary CLI connections do not publish views. Synchronization has no caller-supplied Client identity and cannot acknowledge another Session's work.

The view reports its observed Workbench revision, phase (`starting`, `ready`, `busy`, `unavailable`) and a required nullable selection. Selection carries TerminalContext identity, required nullable Tab/Panel identities, an explicit zoom flag and the active Tab panel count (zero without a Tab, positive with a Tab). A Panel requires a Tab; zoom requires a Panel. These are Client observations, not additional canonical Workbench fields.

Each UI sample has nonzero canonical u64-text heartbeat and view revision. Heartbeat advances with UI sampling; view revision advances when the reported view changes. Coalescing may skip numbers. An equal revision requires equal view content; older revisions or heartbeats conflict. Repeated network synchronization without a new UI heartbeat does not renew freshness. A view is current for five seconds after a new UI sample reaches Server, then stale until another sample arrives.

## Control and acknowledgement

A control command contains operation identity, exact Client identity, expected view revision, minimum observed Workbench revision and one intent:

- Context: select the exact TerminalContext (the CLI resolves a Workspace to its context).
- Tab: select the exact Tab in its context.
- Panel: select the exact Panel in its Tab/context.
- Zoom: set the exact selected Panel's Tab zoom to an explicit boolean.
- ToggleZoom: toggle that Tab once from the state observed by its UI owner after target activation.
- Layout: read exact target geometry without selecting or resizing; the [layout owner](windows-client-layout.md) defines its observation and budget.

Toggle resolution belongs to the TUI and is retained for the delivered operation. Equal operation identity and complete command returns its retained observation, including after the Client disconnects. Changed command data conflicts. After receipt expiry, absence does not establish that an earlier command had no effect.

| Transition                 | Guard and observation                                                                                                                                                                                                     |
| -------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Submit                     | Current registered Client, exact view revision, available capacity and no active command for that Client; retain `queued`                                                                                                 |
| Synchronize                | Only the connection owner publishes its sample; exact queued precondition permits one delivery and atomically records `delivered`                                                                                         |
| Queued timeout             | Five seconds without delivery produces `rejected: expired`; nothing was delivered                                                                                                                                         |
| Client acknowledgement     | Exact delivered operation and owner; retain `applied`, `rejected` before any action, `zoom-toggled` with its prior state, `zoom-unchanged` for a single-Panel zoom-on/toggle, or `incomplete` after possible partial work |
| Disconnect before delivery | Remove live view; queued work becomes rejected without execution                                                                                                                                                          |
| Disconnect after delivery  | Remove live view; unacknowledged work becomes incomplete with no claimed final view                                                                                                                                       |
| Caller timeout or Ctrl+C   | Stop only CLI waiting; query the retained operation identity                                                                                                                                                              |
| Completed retention        | Keep completed observations for 60 seconds; only expired completed records may be reclaimed                                                                                                                               |

There are at most 32 live views within the existing Client Session admission bound, one active command per Client and 256 retained control records. Capacity refusal never evicts pending or unexpired evidence. Delivered work remains owned until acknowledgement or disconnect. Server has no terminal-input, Git or global-focus side effect for a Client command.

## TUI execution

One Windows control lane owns a separate admitted connection and a bounded latest-sample handoff. It never blocks the terminal carrier or performs UI mutations from its thread. The UI consumes semantic commands after local input and before drawing, checks the published view precondition, refreshes canonical membership to the command's minimum revision, and uses the existing context, Tab and Panel selection owners. Async attachment and refresh keep their existing owners and deadlines.

Local navigation or modal intent interrupts further remote steps. Already accepted native work is not described as undone; interruption after such a step is incomplete. Missing targets, stale views, unsupported states and modal/busy admission have explicit outcomes. Geometry changes re-observe viewport sizes and invalidate pointer hit maps before drawing. Applied acknowledgement requires the requested selection/zoom to be observed and a successful draw. A failed draw or lost connection cannot fabricate applied success. A viewport too small to present content is busy. Zoom-on or toggle for a single-Panel Tab returns `zoom-unchanged` after the requested Panel is selected and drawn; it never invents a zoom flag. This outcome requires an observed single-Panel target, `zoomed: false` and the requested minimum revision. Completed draw observations are pinned until acknowledgement, so later local navigation cannot rewrite the receipt.

The lane stops, wakes blocked I/O and joins on TUI exit. Server Session cleanup removes publication and settles retained requests; managed Site stop joins those handlers. Client view/control state is volatile and does not survive Server restart.

## CLI routing and wire boundary

An explicit Client selector chooses that exact live Client. Without it, validated caller Terminal/Worker context selects a unique view focused on that binding; outside such a context, exactly one current view is required. Missing, invalid, stale or ambiguous context is an explicit failure; there is no most-recent or global-focus guess. Target resolution continues to use canonical Workbench identities.

`workbench.client-view` has Synchronize, List, Get, Submit and Read actions, with `workbench.client-view.result` responses. Requests carry protocol version, exchange request identity and Site identity. Synchronize carries required nullable completion; its result carries required nullable next control. Operation identity is separate from exchange identity. Frame kinds 25/26 retain OBWP version 1; request bodies are limited to 16 KiB and results to 64 KiB before allocation. Rust and TypeScript independently validate closed variants, required nullable fields, identity/sequence bounds and cross-field relationships.

The boundary is additive within exact cohort admission. Old cohorts do not silently reinterpret new messages. Rolling back the cohort discards only volatile view/control observations; it does not change canonical Workspace or terminal state. New Mac/Linux control runtimes and Desktop CLI exposure are outside this Windows implementation.

## Model and evidence

The registered `ClientViewControl` TLA model abstracts owner routing, publication fences, one delivery, UI effects, drawing, interruption and receipt retirement. Exact duration, capacity and serialization bounds belong to maintained executable tests. The Server registry is a deterministic state owner over admitted generation, Client samples, monotonic time, queued/delivered commands and completed observations. The authenticated carrier is an adapter. The TUI controller owns ordered semantic steps; the connection lane owns transport and joining; CLI owns selection, one submission and bounded waiting. No owner writes another's private state.

Evidence must cover owner isolation, two Clients, stale/frozen UI, revision changes, no delivery replay, duplicate/conflicting identities, capacity/expiry, partial execution, human-input interference, disconnect and shutdown. Generated stateful traces retain reduced counterexamples. Shared Rust/TypeScript/schema fixtures and frame limits establish bounded contract agreement. Controlled Windows ConPTY traces must show CLI selection reaching the intended Client and successful drawing, with the other Client unchanged. Herdr `e0507237ad8d41f60aece10d00ef29ce2339ca34` focus/zoom handlers are the selected behavioral reference; the separate Client routing is Orchbee's architecture. TLA execution is deferred by the user; model assets and unexecuted checks must be reported honestly. Same-author checks do not imply independent review.

### Targeted zoom toggle

`ToggleZoom` is an exact target intent. The UI captures the target Tab's prior zoom once after activating that Tab, then preserves that decision across Panel focus and subsequent ticks. `ZoomToggled` carries `previouslyZoomed` and a successfully drawn observation with the inverse zoom value. It cannot acknowledge a plain `Zoom` intent. Single-panel targets report `ZoomUnchanged`; they do not claim a toggle occurred. Interrupted navigation remains incomplete, and status reads never execute another toggle.

Layout reads use the same one-delivery lane but complete with `layout-read` without a draw acknowledgement. Dedicated frame kinds 27/28 carry their larger bounded payloads. `ClientViewControl` also abstracts read purity and pending/result memory reservations; exact payload accounting is exercised by registry tests.

### Directional Panel focus

After entering a context or Tab, exact Panel navigation may find the destination viewer still attaching or completing setup. It waits for that existing owner within the original operation deadline, without reattaching or renewing the deadline. Missing membership, a definitely unavailable binding or a faulted non-live viewer fails explicitly. Local interruption still stops further steps and preserves partial-effect truth.

`panel focus [source] --direction left|right|up|down` resolves a neighbor from the exact Client's unzoomed cell geometry before any navigation. It uses the [Panel layout](windows-client-layout.md) Snapshot, membership, visibility, revision and ordering rules. The source may be in an inactive or unvisited Tab. An unavailable or hidden source fails explicitly; it cannot establish that no neighbor exists.

No neighbor completes as `direction-unchanged`, carrying the source `layoutRevision` and unchanged Client view, without selecting the source context or Tab, changing focus or zoom, attaching, resizing or requesting a draw. The source revision must meet the command minimum; an unrelated currently selected context need not advance to that revision. When the source Tab is selected, its layout revision equals the sampled view revision.

A found neighbor is captured once as an exact destination Panel. Existing ordered selection and draw acknowledgement execute that fixed destination, preserving the original directional intent in the receipt. Further ticks, viewport changes, status reads and replay never perform another directional step. `direction-focused` carries the destination Panel and observed view; that destination differs from the source and matches the requested context, Tab and minimum revision. A found neighbor already focused still produces this outcome after a draw. Zoom is preserved. Local interruption and membership loss use existing rejected/incomplete semantics; no OS window activation is performed.

Both new completion variants use ordinary bounded frames 25/26. Rust, TypeScript and JSON Schema reject unknown variants and fields; semantic validators reject a directional result for a different intent, a source-as-destination, a mismatched view and a stale source revision. The Server retains the original operation and never resolves geometry itself. The TLA abstraction adds no-neighbor completion without effects or drawing; execution remains deferred. Maintained controller and registry witnesses cover no-neighbor purity, fixed destination, draw ordering, interruption and receipt replay. Windows ConPTY evidence covers exact Client isolation and inactive-source behavior. Pinned Herdr focus-direction handling supplies the no-neighbor behavior; Orchbee retains its own cell chrome and authenticated Client routing.
