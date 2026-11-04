# Native panel lifecycle

## Bound and unbound members

Canonical Panels retain their primary and ordered children even when some members have no terminal
binding. Server runtime membership contains only bound members and records which one is primary;
a bound child never substitutes for an unbound primary. Registration and structural operations
preserve the unbound members. Legacy single-member readers keep their stricter contract.

Attachment uses exact existing Workspace membership and never launches implicitly. An unbound layout remains occupied. Explicit Tab Create, Split or member creation can establish a binding after the Workspace directory and Worker pass admission.

Close determines its scope from canonical topology, never from the number of bound terminals.
Unbound siblings survive Panel Close. An entirely unbound Panel or Tab uses the retained Close
publication path without Worker disposal; it is removed only at canonical publication. Scope
conflicts, revision credit, replay, publication recovery and relocation fences still apply.

## Creating a Panel terminal

The private `panel.create-terminal` action addresses one canonical Context/Tab/Panel. Its closed
`target` is `child { label }` to append one bound child, or `unbound { presentationId }` to start that
exact existing primary/child while preserving its ID, label and override. Missing or bound targets
fail before launch, without primary fallback; child capacity is 16. It creates no Panel or split.

Server shares terminal creation with split through typed placement. Split allocates five identities, child creation one, and existing-member binding none. Navigation and creation grants, revision, launch/profile/options, Workspace directory/Worker, pending Close and ledger checks precede prepared canonical/runtime commit and dispatch. Pending/final receipts carry real identities; prebuilt bounded results and settlement seals preserve replay and unknown outcomes. Creation neither selects a Client member nor sends input. Layout application and member controls consume this private creation owner.

## Scope and ownership

The native Client admits at most one structural panel mutation — split, move, or close of the
selected leaf — per accepted operation, transports it over the native carrier, and settles its
typed result through the UI mailbox. This reference governs the panel-operation extension of the
[native Tab Session](workbench-client-runtime.md) and the pane-membership collection that holds one
attachment incarnation per visible leaf. Maintain it with `workbench_state/panel.rs`,
`workbench_client/mailbox/panel.rs`, `workbench_client/ui/panel.rs`, the pane record and reconcile
machinery in `workbench_state.rs` and `workbench_state/membership.rs`, the
projection validators in
`workbench_projection.rs`, the native executor in
`workbench_client/connection/native/panel.rs`, and the local-client workbench panel modules.

The Client never merges a panel result it cannot correlate. Admission requires a stable Session
with a selected leaf, a free structural-mutation slot, and an unexpired forward deadline. A panel
operation binds its request identity, attached-leaf key, baseline projection revision, desired
attachment, and total deadline once, at admission; nothing may silently renumber them afterwards.

## State and transition contract

| Transition                    | Guard and result                                                                                                                                                                                                                                                                          |
| ----------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Sample panel intent           | A stable Session samples the user intent under a bounded admission window; sampling never mutates Session state.                                                                                                                                                                          |
| Refuse panel admission        | Lock contention past the deadline, a queued/in-flight/awaiting structural mutation, a pending refresh or switch fence, a saturated mailbox, identity exhaustion, or no desired leaf refuses admission and preserves all Session facts. Move with no neighbor is a definite no-op refusal. |
| Admit panel operation         | Admission allocates the next request identity and lease serial, freezes the baseline revision/panels and desired leaf into the operation record, and enters the queued stage before submit.                                                                                               |
| Submit panel operation        | The queued operation is submitted at most once before its deadline; a submit that reaches the carrier is recorded exactly once.                                                                                                                                                           |
| Receive panel outcome         | The carrier produces not-submitted, precommit, unchanged, tab-unavailable, close-accepted, or a committed fact with its post-commit authoritative snapshot.                                                                                                                               |
| Retire on ambiguous outcome   | A submitted operation whose outcome cannot be classified retires the Session as unknown; an operation that may have produced an effect is never automatically replayed.                                                                                                                   |
| Read post-commit snapshot     | A committed split/move re-reads the authority snapshot at a fresh identity. The exact `r+1` candidate must match the committed graft; a later superseding revision is validated canonically only.                                                                                         |
| Read tab-unavailable snapshot | A tab-unavailable outcome re-reads the authority: an older revision or a same-revision contradiction is divergent, a missing Tab installs Empty membership, and a newer canonical Tab candidate is accepted through the normal candidate path.                                            |
| Deposit typed result          | The worker deposits the outcome bound to the operation request, serial and deadline; a deposit that does not match the current operation identity is never placed.                                                                                                                        |
| Reject stale input            | A deposit or membership/refresh reply carrying a mismatched request, serial, or generation is rejected in place; it preserves the stage, correlation identity, commit fact, attachment, selection, and cache revision.                                                                    |
| Consume typed result          | The UI consumes a placed result only while request, serial, baseline revision and desired leaf still match the pending operation.                                                                                                                                                         |
| Merge committed projection    | A committed result whose candidate still contains the operation's selected leaf merges it into the projection cache at the candidate revision; the leaf stays selected and attached.                                                                                                      |
| Repair superseding membership | A committed result whose later candidate removed the selected leaf while the Tab survives — including a peer's legal close — enters membership reconciliation: it must not retire as divergence.                                                                                          |
| Enter recovery on unavailable | A committed result with an unavailable candidate parks in panel recovery; the committed fact is retained until a later refresh applies or membership replaces it.                                                                                                                         |
| Diverge on invalid authority  | An invalid graft, a foreign unchanged move, a tab-unavailable contradiction, or a divergent candidate retires the Session as authority divergence.                                                                                                                                        |
| Stage membership cleanup      | Membership reconciliation first stages the release of the exact old attachment; cleanup failure or deadline expiry retires with the committed fact retained.                                                                                                                              |
| Install repaired membership   | Only after the old attachment is released may the new membership install: Empty when the Tab is gone, otherwise select a repaired leaf; attachable provenance then re-attaches.                                                                                                           |
| Offer attach reply            | A pending install attach receives a carrier reply — attached, rejected, unavailable, or a protocol violation — before the membership deadline.                                                                                                                                            |
| Park retryable attach         | A rejected or unavailable attach reply parks the Session in a stable retryable stage: no viewer attaches, the retained commit stays valid, and the operation is never replayed nor re-attached on its own. Protocol replies and deadline expiry still retire.                             |
| Admit bounded refresh         | With no structural flight pending, a bounded refresh re-reads the authority snapshot; during panel recovery it is the only path that can resolve the retained commit.                                                                                                                     |
| Apply refresh outcome         | A canonical refresh merges, repairs a missing selected leaf, or installs Empty membership; an unavailable read parks, a divergent read retires. Refresh results are fenced while membership is pending.                                                                                   |
| Retire and close Session      | UI retirement freezes the pending records and preserves the committed fact; Exit closes the Session.                                                                                                                                                                                      |

Close `Accepted` is only a receipt: it never moves selection or attachment by itself. Selection
changes only when a later canonical publication removes the leaf — through merge, membership
repair, or Empty install — and the attachment may change only after the old one is released.

## Invariants and temporal rules

The single-operation implementation contract requires one selected attachment per operation; exact request/serial/baseline correlation; structural exclusion while a panel flight freezes its baseline; and retention of every known committed fact. Invalid grafts must not apply. A legal superseding leaf removal repairs membership instead of fabricating divergence.

Release the old attachment before installing a replacement. A rejected or unavailable attach parks without automatic replay or re-attach; an invalid protocol reply or elapsed deadline retains the documented retirement boundary. Close acceptance alone does not change selection before publication. Expiry and controlled completion settle or retire pending work. Stale inputs preserve the stage, correlation, commit, attachment, selection and cached revision. The state, mailbox, connection and projection suites exercise these requirements against production code.

These operation-specific traces do not claim exhaustive TLC progress. The retained pane collection core below checks the shared identity and cleanup protocol separately.

## Pane membership collection

Within one selected Tab the Session holds a pane record per visible leaf, not one attachment. A
logical pane is `PaneKey {anchor, WorkbenchPanelId}`; its physical attachment is
`PaneRef {pane, incarnation}`, minted each time a record's attach target is issued.
`sync_pane_set` reconciles the observed leaf set under one absolute deadline: missing leaves mint
records, re-observed current-anchor departing records heal in place, and vanished leaves depart —
a departing record clears projection and frame bookkeeping while waiting for a disposal proof that
only its own `PaneRef` settles (`settle_pane_detached`). Anchor-level teardown
(`drop_anchor_panes`) replaces a whole epoch atomically through `PreparedDesiredInstall`, while a
same-Tab focus move is local and never departs or re-mints a record.

Mailbox admission partitions by command scope: input admits only while its pane is the logical
focus at enqueue time; resize and polls admit every exact non-departing incarnation of the
desired set. The state owner requests sizes only for attached controlling panes in the visible
set: all unzoomed leaves, or the focused leaf under zoom. Read-only and hidden panes do not resize. An admitted command keeps the `PaneRef` it captured at enqueue — dispatch fences on
that exact reference and does not re-check focus, so accepted input still lands on its original
pane after a focus move and never migrates to the newly focused pane. A requested Workspace or
Context navigation does not move focus until its own refresh lands, so terminal input is refused
with a visible notice from the request until the landing — whether the request waits in the Shell
selection queue or already owns the refresh — and never reaches the pane the user is leaving.
Commands or worker events
that arrive tagged with a retired incarnation — queued before a rebind or settled late — are
dropped or refused at the exact `PaneRef` and never replay onto the re-bound attachment. A poll
result waits as a pending draw until a completed draw on the exact live record stages it;
`FrameApplied` commits only onto the record that staged the frame — a pending draw whose pane
departed settles without feedback rather than fabricating an acknowledgement — and readiness
follows the focus pane alone while background panes attach and render independently. The
shared HostViewers capacity of 8 applies across all visible panes, and read-only panes never
acquire control or resize.

A typed viewer `TerminalRuntimeUnavailable` response for an exact live pane disposes that viewer
locally. So does a viewport exit leg the Host refuses, once `Detach` proves its Server slot
released ([native-terminal-viewport.md](native-terminal-viewport.md#commands-and-demand-legs)).
For the focused pane this is allowed in `Ready` beside every owner that can itself begin
from a parked focus and keeps its premises when one parks: close work (a Tab, Panel or Workspace
close confirmation, a retained Tab close, or unresolved Workspace close evidence), a rename or
Workspace setup draft, a publication or explicit refresh, and a submitted Move, Panel or Rename
with its projection recovery. A takeover, whose completion needs `Ready`, a setup or Workspace
close in flight, a recovery refresh or an uncertain Rename keeps the existing retirement boundary.
The existing `ViewerLost` fault parks the exact selection. Pane reconcile never attaches the focused pane, so a reconcile planned before the loss
settled cannot replace that fault. A parked focus, whether its viewer was lost, its attach failed
or it never attaches, leaves its siblings working as under `Ready`: reconcile attaches them, and
they keep polling and resizing.
While that exact focused fault remains, each pushed publication hint drains through the existing
debt, including after an early unchanged or refused read; idle time issues no read. Canonical
removal of a parked Panel then uses the existing sibling cleanup and membership installation to
select a survivor or enter `Empty`. No terminal request is replayed or reported successful on loss; other refusal
reasons and transport/protocol failures retain their existing handling.

A lost viewer does not prove that its terminal ended: a Worker may close a viewer whose terminal
lives on. The Client therefore re-attaches the parked focused pane on its own through the same-Tab
switch a reselection admits, with the same panel focused, after a 0.3-second pause; later losses
of that pane wait 1 and then 3 seconds, and one pane gets at most three automatic attempts a
minute. Close work, an armed close confirmation, a pending Refresh, or any other structural owner
defers the attempt without spending it, and the deferral shows no busy notice. Like any switch
from the parked pane, the re-attach first detaches the live siblings under the old anchor, which
then attach again under the new one; a refusal before that cleanup restores their exact
projections and leaves the pane parked, or, when it cannot restore them, detaches them and parks
the pane again while they attach again through reconcile. A terminal that ended settles through
the switch's own snapshot and attach: when that snapshot no longer holds the Panel, the switch
lands on the Tab's surviving leaf, and a failed attach parks as an ordinary attach fault and is not
retried automatically. A reselection still retries at once.

Zoom is a Client-local per-Tab flag, and it belongs to the leaf it draws: the focus in the selected
Tab, the remembered Panel in a background Tab. Focus moving inside the Tab carries the zoom along.
When the drawn Panel leaves the Tab by any route, such as the close action, its shell exiting or
another Client's close, the install drops the flag and the surviving leaves return to their normal
layout; the leaf that takes the focus does not inherit it. A hidden leaf leaving keeps the zoom, and
a plugin overlay still restores the zoom of its return Panel.

## Source correspondence

| Runtime owner                                   | Evidence obligation                                                                                                      |
| ----------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------ |
| `workbench_state/panel.rs` and membership state | Admission exclusion, exact result correlation, retained commits, stale-input preservation, legal superseding-leaf repair |
| `workbench_client/mailbox/panel.rs`             | FIFO handoff, at most one submission and honest unknown outcomes                                                         |
| `workbench_client/connection/native/panel.rs`   | Concrete result validation, typed attach failures and bounded cleanup                                                    |
| `workbench_projection`                          | Exact-next and later-publication validation, selection and Empty installation                                            |
| `workbench_state/tests.rs` and generated tests  | Pane incarnation fencing, replacement and wrong-pane negative controls                                                   |

`PaneIncarnationFencing` abstracts only the final identity/cleanup boundary. Concrete geometry, payloads, operation receipts and complete panel lifecycle composition stay in these implementation suites.

## Finite model evidence

`PaneIncarnationFencing.tla/.cfg` checks two logical panes, at most two incarnations per pane, two selection anchors, one queued command and one execution flight. `Mint`/`Depart`/`Cleanup` map to membership replacement and retirement; `Admit`/`Start`/`Complete` preserve the captured pane, incarnation and anchor; `Draw` consumes only a current frame; `Intend`/`Land`/`Abandon` map to a requested navigation that moves focus only when it lands. Cleanup, start, completion and landing use weak fairness. The model checks current frames/effects, successor preservation under late cleanup, input admission only after a pending navigation settles, and eventual settlement of departing membership and pending navigation.

`workbench_state/tests/generated.rs` exercises `ReincarnationFence` against production reducers. `reincarnated_pane_fences_late_work_from_the_retired_attachment_incarnation` and `same_anchor_panes_fence_cross_pane_and_fabricated_incarnation_refs` in `state/tests.rs` cover wrong-incarnation and wrong-pane effects. Mailbox, connection and workbench-projection tests own panel-operation results, receipt correlation, membership repair, draw acknowledgements and failure handling. Those detailed compositions are implementation-test claims, not exhaustive TLC claims.

The maintained mutation controls remove the completion identity guard, which must fail `FramesAreCurrent` or `EffectsUseCurrentIncarnation`, and the pending-navigation input guard, which must fail `InputFollowsSettledNavigation`. Run `node scripts/tla/run-models.mjs --models native-client-pane-membership` and the owning Cargo tests.

## Terminal settlement

Host `TerminalExited` and `TerminalFailed` records reach only viewers for that exact physical
terminal. Server checks Host epoch, terminal and creation identities; Client checks its projection's
epoch and terminal. Settlement is separate from frame validity: it preserves final pixels, history
and canonical membership, displays the localized exit code when known, and removes input and resize
authority. A controller refusal or typed control-runtime unavailability downgrades only control;
viewer polling remains authoritative for whether readable output is still available.
CLI frame readers consume settlement without invalidating pixels or emitting a projection-blocked
stream reset; their existing output schemas stay unchanged.

Settlement describes every ended terminal that keeps its Panel. An interactive Shell that exits
with code 0 does not keep it: Server closes that Panel, or its Tab when the Panel is the sole leaf,
through the [canonical Close](../../orchbee-server/docs/terminal-tab-close.md#normal-shell-exit),
including when a later viewer attachment first observes the exit. Clients remove the member
through the existing peer-Close path; the last Tab leaves the Workspace's ordinary empty state.
Until the member is removed, the Client holds the pane's closing presentation for at most one
second (`workbench_client/normal_exit_hold.rs`): exit code 0 of a Panel's only Presentation keeps
its final pixels and label, and the status line raises no automatic exit, view-only, connection or
viewer-loss cue. The hold changes no attachment, input, selection or membership and ends when the
pane leaves membership. A pane still present at the bound, such as a non-interactive program that
Server keeps, shows its ordinary settlement; explicitly invoked status and notices still appear.
Tests cover the hold's eligibility, bound, release on departure, and the redraw a release owes.

An exit arriving during setup or an admitted Create also downgrades the retained selection and
clears its controller geometry. Precommit refusal therefore cannot restore a controlling selection
after its Shell ended. No failed input is replayed. Navigation, child-to-primary selection, Create
and Close continue through the existing owners, including with only one ended Tab.

New viewer attachments sample retained Host inventory for settlement, so detaching and starting a
Client again preserves ended output. Host rejects controller attachment to a settled terminal; the
Client keeps the read-only viewer. A definite initial viewer refusal instead parks a stable
navigation view and starts the ordinary command loop. It never creates a replacement terminal or
deletes history. Transport, protocol, unknown-outcome and cleanup failures retain their existing
boundaries. Projection/state tests cover exact-target settlement and selection restoration;
real-window review covers exit 0, nonzero exit, ordinary input, child return, reentry and narrow
status presentation.
