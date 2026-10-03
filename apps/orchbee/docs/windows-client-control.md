# Windows Client view control

This contract owns native CLI discovery and navigation of a live Windows TUI Client. The TUI owns its selection and zoom; Server routes bounded requests and retains observations. Canonical Workbench snapshots, revisions and terminal control ownership are unchanged. A routed request does not activate an operating-system window.

Ordinary Workspace/Tab focus shares [Client selection memory](native-context-selection.md): return restores the last surviving Tab/Panel, while explicit Panel or Agent navigation overrides it. History is private to the selected Client and is discarded on restart.

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

## Client viewport observations

Windows Panel list/get with an exact Client reads that Client's installed primary-terminal
viewports through `viewports`, an ordered batch of 1–16 unique Panel IDs. `viewport-read`
returns the canonical revision and each Panel's nullable primary terminal identity plus an
observation: unavailable (`not-attached` or `frame-unavailable`), pending (`entering`,
`awaiting-window`, `recovering`), or observed (`live` or `history`). Inactive or unbound Panels
never acquire viewers for a query. Only the current primary incarnation contributes.

Observed sources preserve Host epoch, attachment, generation, revision, columns, screen,
total rows and starting row. Scroll counters are canonical decimal uint64 strings;
`viewportRows` is the Client's cell height, not a possibly shortened history response.
`maxOffsetFromBottom = max(totalRows - viewportRows, 0)` and
`offsetFromBottom = maxOffsetFromBottom - min(startRow, maxOffsetFromBottom)`.
Live primary screens start at scrollback; alternate screens have zero scrollback.
History uses the same generation/geometry-fenced installed window the painter can read.
Pending recovery or missing data never fabricates a bottom position.

This read may execute while the Client is Busy, including history browsing, without relaxing
navigation or mutation admission. Starting/unavailable views cannot produce viewport success.
Exact view revision, one pending operation and canonical membership checks remain required.
The UI takes no attachment, input, resize, navigation, history-read, acknowledgement or paint
effect. A batch retains at most 12 KiB; the Server reserves that amount before admission.
Existing frame kinds and 16 KiB request / 64 KiB result limits remain unchanged. Rust,
TypeScript and schema/corpus bindings agree; the additive contract requires matched cohorts.
State is volatile; rollback has no durable migration. Focused evidence covers bounds, wrong
membership/route, pending/recovery states, Busy read versus mutation admission, and real
Windows two-Client history observations without focus or viewport changes.

## Model and evidence

The registered `ClientViewControl` TLA model abstracts owner routing, publication fences, one delivery, UI effects, drawing, interruption and receipt retirement. Exact duration, capacity and serialization bounds belong to maintained executable tests. The Server registry is a deterministic state owner over admitted generation, Client samples, monotonic time, queued/delivered commands and completed observations. The authenticated carrier is an adapter. The TUI controller owns ordered semantic steps; the connection lane owns transport and joining; CLI owns selection, one submission and bounded waiting. No owner writes another's private state.

The model normalizes an operation's `expected` revision to zero after it leaves `queued` or
`delivered`. Its projection retains every other state field and retains `expected` while Pending.
Every guard reading that revision requires a Pending status, Submit stamps it before use, and the
bounded model never returns a submitted operation to `absent`. Completion and Disconnect clear only
the affected expectations. Init projects identically, and each original action projects to the same
named action or admitted stutter. All 11 invariants, two Clients, five operations, revision bound,
shared layout budget, action coverage, and the safety-only `Spec` remain unchanged. This removes
unused model history; runtime command envelopes, retained receipts, and conflict comparisons still
retain their complete original data.

Typed local action-boundary checks instantiate both actual modules and check initial, type, and
single-step projection. Negative controls clear a Pending expectation or retain a completed one and
must violate that mapping. Those checks do not establish full reachable-state coverage or a
kernel-checked universal theorem; the registered complete-graph check remains required. Adding a
completed-expectation reader, changing replay semantics in the model, or permitting operation reuse
requires revisiting the projection.

Evidence must cover owner isolation, two Clients, stale/frozen UI, revision changes, no delivery replay, duplicate/conflicting identities, capacity/expiry, partial execution, human-input interference, disconnect and shutdown. Generated stateful traces retain reduced counterexamples. Shared Rust/TypeScript/schema fixtures and frame limits establish bounded contract agreement. Controlled Windows ConPTY traces must show CLI selection reaching the intended Client and successful drawing, with the other Client unchanged. Herdr `e0507237ad8d41f60aece10d00ef29ce2339ca34` focus/zoom handlers are the selected behavioral reference; the separate Client routing is Orchbee's architecture. Registered TLA execution is required for model or abstracted-behavior changes; unexecuted checks must be reported honestly. Same-author checks do not imply independent review.

### Targeted zoom toggle

`ToggleZoom` is an exact target intent. The UI captures the target Tab's prior zoom once after activating that Tab, then preserves that decision across Panel focus and subsequent ticks. `ZoomToggled` carries `previouslyZoomed` and a successfully drawn observation with the inverse zoom value. It cannot acknowledge a plain `Zoom` intent. Single-panel targets report `ZoomUnchanged`; they do not claim a toggle occurred. Interrupted navigation remains incomplete, and status reads never execute another toggle.

Layout reads use the same one-delivery lane but complete with `layout-read` without a draw acknowledgement. Dedicated frame kinds 27/28 carry their larger bounded payloads. `ClientViewControl` also abstracts read purity and pending/result memory reservations; exact payload accounting is exercised by registry tests.

### Directional Panel focus

After entering a context or Tab, exact Panel navigation may find the destination viewer still attaching or completing setup. It waits for that existing owner within the original operation deadline, without reattaching or renewing the deadline. Missing membership, a definitely unavailable binding or a faulted non-live viewer fails explicitly. Local interruption still stops further steps and preserves partial-effect truth.

`panel focus [source] --direction left|right|up|down` resolves a neighbor from the exact Client's unzoomed cell geometry before any navigation. It uses the [Panel layout](windows-client-layout.md) Snapshot, membership, visibility, revision and ordering rules. The source may be in an inactive or unvisited Tab. An unavailable or hidden source fails explicitly; it cannot establish that no neighbor exists.

No neighbor completes as `direction-unchanged`, carrying the source `layoutRevision` and unchanged Client view, without selecting the source context or Tab, changing focus or zoom, attaching, resizing or requesting a draw. The source revision must meet the command minimum; an unrelated currently selected context need not advance to that revision. When the source Tab is selected, its layout revision equals the sampled view revision.

A found neighbor is captured once as an exact destination Panel. Existing ordered selection and draw acknowledgement execute that fixed destination, preserving the original directional intent in the receipt. Further ticks, viewport changes, status reads and replay never perform another directional step. `direction-focused` carries the destination Panel and observed view; that destination differs from the source and matches the requested context, Tab and minimum revision. A found neighbor already focused still produces this outcome after a draw. Zoom is preserved. Local interruption and membership loss use existing rejected/incomplete semantics; no OS window activation is performed.

Both new completion variants use ordinary bounded frames 25/26. Rust, TypeScript and JSON Schema reject unknown variants and fields; semantic validators reject a directional result for a different intent, a source-as-destination, a mismatched view and a stale source revision. The Server retains the original operation and never resolves geometry itself. The TLA abstraction adds no-neighbor completion without effects or drawing; execution remains deferred. Maintained controller and registry witnesses cover no-neighbor purity, fixed destination, draw ordering, interruption and receipt replay. Windows ConPTY evidence covers exact Client isolation and inactive-source behavior. Pinned Herdr focus-direction handling supplies the no-neighbor behavior; Orchbee retains its own cell chrome and authenticated Client routing.

`panel resize [panel] --direction left|right|up|down [--amount <number>]` adjusts one eligible split border, preserving focus and zoom. `layout set-split-ratio [--tab <tab> | --panel <panel> | --current] --path root|0.1 --ratio <number>` sets an exact binary split path. Both route to one Windows TUI and retain a `layout-adjusted` receipt. Selected changes wait for a draw; inactive changes store without navigation. Finite ratios clamp to 0.1–0.9, and finite resize amounts normalize to an absolute magnitude at most 0.5 (default 0.05). Ratios belong to that TUI session and are not restored after restart. See the [layout contract](windows-client-layout.md).

## Agent navigation

Windows `agent focus <target> [--client <id>]` resolves a canonical Session or terminal-backed Agent to one navigable primary Panel. One `agent-focus` control retains the exact Agent and Presentation identity through Client navigation and completion. The Client rechecks its observation; a physical target also requires its matching Host frame. Successful explicit focus marks current known versions in the selected Tab seen, including an already-focused target and outer focus loss. Ordinary paint and read-only `agent attention` keep their existing rules. `--request-id`, `--detach`, the 30000 ms default total deadline and retained status recovery remain unchanged. Starting or Unavailable Clients refuse before Agent reads; Busy can enter the one-shot mode-exit preflight without discarding submitted operations. History waits for recovery and a fresh live frame; loss/replacement yields rejection or incomplete navigation, without retargeting. `orchbee.client.control` v5 carries this operation; no OS window activation occurs. The [Agent navigation contract](native-agent-navigation.md) owns identity, acknowledgement and final acceptance.

## Client-relative Workbench queries

Windows `workspace list/get`, `tab list/get` and `panel list/get` accept optional
`--client <id>`. Without it, queries keep their canonical headless projection.
With it, one exact current Ready Client supplies global `focused` flags; Panel reads also
admit Busy for the read-only viewport contract above. Workspace
rows also carry `activeTabId`, including the remembered selection in inactive
Workspaces. Null means an empty context. Focus means selected in that TUI, not
operating-system window focus. Missing, inadmissibly busy, stale or ambiguous facts fail; no
other Client is substituted.

The read-only `Selections` intent carries 1–128 unique TerminalContext IDs. The
TUI resolves current selection or its private navigation memory against one fresh
canonical snapshot: keep a surviving remembered Tab, otherwise use the same first
Tab fallback as normal context reentry. It neither visits the context nor updates
memory. `NavigationRead` preserves requested order, canonical revision and the
Client observation. A payload is bounded to 16 KiB and uses the existing retained
control operation, 64 KiB result envelope and Server capacity accounting.

The CLI resolves query targets first, reads batches on one admitted connection,
checks exact request/context identities and membership, and verifies an unchanged
canonical revision and Client view revision before emitting one result. One total
five-second deadline and signal owner cover every phase. No partial success,
automatic replay, navigation, acknowledgement, terminal attachment or input effect
occurs. Selected results include Client/operation/view provenance; unselected
results never invent focused or active fields. Cancellation closes the connection
and exits130; unavailable or changed observations retain typed failures.

Tests cover bounds and duplicate IDs, reply correlation, invalid membership and
revisions, empty/inactive/reordered/removed selections, independent Clients,
headless compatibility, grammar, schemas and bilingual output. Real Windows
acceptance must compare both Clients and canonical state before and after reads.
Agent summaries, Panel process/title and Client scroll use their separate observation owners;
Workspace worktree and reported metadata remain dependent joins.

## Exact Panel Presentation observation

The scalar passive `presentation` intent reads one Panel's current native member without
changing focus or input ownership. `presentation-read` carries the canonical revision and
Panel ID plus a required nullable member: Presentation ID, exact binding, existing local
attachment generation and pane incarnation, and the installed viewport observation. The
payload is bounded to 4 KiB; Server reserves this capacity before admission and validates the
Panel, minimum revision and Ready/Busy completion view before retaining a receipt. No local
member yields null; a stale binding cannot borrow another member's frame. This operation
cannot complete as applied navigation.

The [current Panel contract](native-current-panel.md#exact-current-presentation-for-process-information)
owns Windows process-info/current composition and their before/after member fences. Client control v6
admits this intent; v1–v5 remain closed. Existing primary `viewports` reads retain their meaning.

Unbound native members extend the passive selection as a strict alternative containing only
`presentationId` and nonzero `selectionGeneration`. They have no attachment, binding or viewport;
null remains absent local membership. Native stable-state and canonical identity checks precede
publication. Control v7 exposes this alternative, while existing bound/null receipts remain v6 and
legacy versions reject the unbound shape. Neither variant acknowledges attention or changes focus.

## Explicit focus acknowledgement

A successful Workspace, Tab, direct Panel, directional Panel or Agent focus acknowledges the
current known attention versions of every Presentation in the selected Tab, including hidden
siblings and an already selected target while the outer terminal is unfocused. The existing
Client attention owner performs this once after the matching Ready draw. An empty Context has
nothing to acknowledge. No acknowledgement is queued for later observations or another Tab.

The controller uses the resolved directional destination; no-neighbor returns without navigation,
draw or acknowledgement. Read-only queries, zoom and resize do not acquire this explicit effect.
A decoded human input intent or deadline expiry before the completion draw prevents acknowledgement;
retained rejected/incomplete outcomes still describe any navigation already performed. Agent focus
keeps its exact subject/member/physical-frame validation. No OS activation or terminal input occurs.

This follows pinned Herdr `e0507237` Pane focus and Workspace/Tab switch acknowledgement. Workspace/Tab navigation while a modal surface is open remains a separate obligation. Targeted controller and
attention tests cover outer focus loss, same-target navigation, current-version scope, directional
resolution/no-neighbor, human input, deadline and replay; full Windows acceptance remains final.

## Panel focus mode exit

Direct Panel focus and a resolved directional neighbor may enter a Busy Client for one cancellable
mode-exit preflight. Before cancelling any draft or shell surface, the control lane reads one fresh
canonical Snapshot and the Client validates Site, minimum revision and exact Context/Tab/Panel
membership. A missing, denied, stale or unavailable target leaves modes untouched. Directional
geometry resolves once before mode exit; no neighbor returns unchanged without a draw or acknowledgement.

Panel and Agent focus share the existing Client-owned exit sequence: cancel unsent dialogs and
shell modes once, request history recovery through its viewport owner, wait for recovery and a
fresh live frame, then navigate and acknowledge after the matching Ready draw. Stable unbound
selection is admissible without creating an attachment. Submitted mutations, pending refresh,
close settlement, retirement and an undersized viewport refuse before cancellation. A new mode,
human input or expiry interrupts the request; retained incomplete outcomes report any earlier mode
or navigation effect. History buffers and submitted operations are never discarded to manufacture readiness.

Workspace/Tab focus does not acquire Panel mode dismissal. No wire fields, public schemas, OS
activation, terminal input or persistent state are added. Focused controller, canonical-preflight,
Server admission and Windows lane shutdown tests cover these boundaries; full Windows acceptance
remains part of final regression.

## Plugin overlay handoff

The matched native cohort accepts a private `overlay` intent with exact context/Tab/Panel/Presentation, expected binding, required nullable return binding and prior zoom. The Client reserves one of 32 transient records before changing focus, and acknowledges only after the normal observed/drawn focus and zoom path. Canonical removal restores the exact surviving return member and prior zoom only while the overlay still owns focus; navigation, manual zoom and rebinds remain fenced. The isolated cancellable lifecycle lane observes full Host identity without input/resize or exclusive Close authority, retains uncertain Close identities and joins on shutdown. [Windows plugins](windows-plugins.md#overlay-lifecycle-model) owns the lifecycle contract. Known public Overlay answers use v8; v1–v7 remain closed to this intent. This adds no Desktop or OS-window activation.

## Scrollback editor popup model

The private `editor-open` Client intent contains only `ScrollbackEditorSpec.source`: the exact current Context, Tab, Panel and bound native Presentation member. It uses the existing retained control operation and `popup-opened` completion; this completion requires the same source, minimum canonical revision, operation ID and Busy view as plugin popup opening. Its public control receipt is v10; v1-v9 remain closed to `editor-open`. Generic `popup-close` closes the current independent popup of either kind.

The Client owns one independent popup lane shared by typed plugin and editor variants. Editor geometry fills the admitted content surface with the existing frame; plugin dimensions and defaults are unchanged. Admission requires a Ready, sufficiently large Client, no competing modal/popup and a fresh exact member. It neither changes canonical Pane topology/selection nor controls or resizes the source terminal. Opening hides the source cursor and captures input; input is admitted only after a matching editor frame has been installed and drawn. The existing popup keyboard, paste, mouse, resize and close path owns editor interaction and restores normal source input after settlement.

The editor lane reconnects once, attaches a read-only viewer to the source binding, decodes and acknowledges a full source frame, then submits one private editor Open with that attachment and exact generation/revision on the same connection. Source capture has one twenty-second budget and rejects blocked/invalid projection; it never submits an executable, environment, file path or transcript. A known Open answer precedes source Detach and destination attachment/controller acquisition. Lost or malformed post-delivery answers are unknown, never replayed. Source viewers stay owned until confirmed detach or connection EOF; Server/Host own uncertain creation and document cleanup.

Transitions are Opening -> Active -> Closing -> Closed. A known refusal permits a fresh explicit intent; transport uncertainty is Failed and closes the lane connection for Server-owned recovery. Close observed before editor submission detaches the source and closes without creating an editor. Close during submitted Open waits for the one answer, then retires that exact popup. Shutdown interrupts in-flight I/O, joins the actor and relies on the existing EOF cleanup owner, never reports disposal from elapsed time. Active completion requires an installed matching destination frame, not merely process creation. Natural exit uses the same disposal acknowledgement path.

Evidence covers closed Rust/TypeScript/JSON intents and receipts, exact source admission, same-connection source frame acknowledgement before Open, no source control/resize, one submitted intent, projection/input/resize/close, refusal, malformed/lost replies and joined shutdown using native Windows pipes. These focused fixtures do not establish actual GUI/editor availability or deployed TUI acceptance; those remain final Windows workflow obligations. TLA execution is deferred by the active user instruction.
