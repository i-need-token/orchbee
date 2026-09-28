# Native panel lifecycle

## Bound and unbound members

Canonical Panels retain their primary and ordered children even when some members have no terminal
binding. Server runtime membership contains only bound members and records which one is primary;
a bound child never substitutes for an unbound primary. Registration and structural operations
preserve the unbound members. Legacy single-member readers keep their stricter contract.

Open uses existing bound membership. A layout containing only unbound members is occupied and
returns terminal-runtime-unavailable without a launch or authority retirement. Explicit Tab Create
or Split can create the first bound terminal and install the standalone reference normally.

Close determines its scope from canonical topology, never from the number of bound terminals.
Unbound siblings survive Panel Close. An entirely unbound Panel or Tab uses the retained Close
publication path without Worker disposal; it is removed only at canonical publication. Scope
conflicts, revision credit, replay, publication recovery and relocation fences still apply.

## Creating a Panel terminal

The private `panel.create-terminal` action addresses one canonical Context/Tab/Panel. Its closed
`target` is `child { label }` to append one bound child, or `unbound { presentationId }` to start that
exact existing primary/child while preserving its ID, label and override. Missing or bound targets
fail before launch, without primary fallback; child capacity is 16. It creates no Panel or split.

The Server shares terminal creation with split through typed placement. Split allocates five
identities, child creation one, existing-member binding none. Both navigation and creation grants
are required. Revision, profile/launch/options, Workspace directory, pending Close and request-ledger
checks precede prepared canonical/runtime commit and dispatch. Pending/final receipts carry only
real identities; prebuilt bounded results and settlement seals preserve replay and unknown outcomes.
The first standalone terminal can use the active Worker without inventing a source binding.
Creation does not select a Client member or send input. Existing layout export/apply and member
controls are the intended consumers; private carrier support alone is not user-facing completion.

## Scope and ownership

The native Client admits at most one structural panel mutation — split, move, or close of the
selected leaf — per accepted operation, transports it over the native carrier, and settles its
typed result through the UI mailbox. This reference governs the panel-operation extension of the
[native Tab Session](native-tab-session.md) and the pane-membership collection that holds one
attachment incarnation per visible leaf. Maintain it with `terminal_session/state/panel.rs`,
`terminal_session/mailbox/panel.rs`, `terminal_session/ui/panel.rs`, the pane record and reconcile
machinery in `terminal_session/state.rs` and `terminal_session/state/membership.rs`, the
projection validators in
`terminal_session/workbench_projection.rs`, the native executor in
`terminal_session/connection/native/panel.rs`, and the local-client workbench panel modules.

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

`NativeClientPanelLifecycle` enforces: at most one attachment which always equals the selected
leaf (`SingleAttachment`, `AttachedIsSelected`); selection is a known cached leaf iff any exists
(`SelectedIsKnownLeaf`, `DesiredIsSelection`); a pending panel flight excludes every other
structural mutation and refresh, and freezes the baseline projection (`StructuralExclusion`,
`OtherStructuralExcludesPanel`, `PanelFlightFreezesCache`); only the current operation's result
may be placed or consumed (`OnlyCurrentResultPlaced`, `DepositCorrelatesCurrent`); a committed
revision is always the baseline successor (`CommittedRevisionIsSuccessor`) and a retained commit
is never dropped (`CommittedFactIsRetained`, `RecoveryRetainsCommit`); an invalid graft never
applies (`InvalidGraftNeverApplies`); a leafless superseding candidate always repairs through
membership, never divergence (`LeaflessRepairNotDivergence`, `LeaflessSupersedingRepairs`);
attaching a new leaf requires the old one already released (`AttachRequiresReleasedOld`,
`PendingMembershipOwnsOld`, `InstalledMembershipReleasedOld`); membership reconciliation names
the pending panel request (`MembershipCorrelationNamesRequest`); a close receipt keeps selection
until its publication installs (`CloseReceiptKeepsSelection`, `CloseAcceptKeepsSelection`);
submission is at most once and never replays after an ambiguous outcome (`SubmissionIsAtMostOnce`,
`NoReplayAfterAmbiguous`); request identity never rewinds (`RequestCursorNeverRewinds`); pending
stages settle or retire under weak fairness of expiry and consumption
(`PendingEventuallySettlesOrRetires`). A parked attach holds no attachment and retains a valid
commit (`ParkedHasNoAttachment`, `ParkRetainsValidCommit`); a pending rejected/unavailable reply
can never retire (`AttachUnavailableNeverRetires`, `RejectedAttachNeverRetires`) and is always
consumed (`AttachReplyEventuallyConsumed`); a parked Session leaves only through a fresh
membership install or retirement — never an automatic re-attach or replay
(`ParkedSettlesViaMembership`); and a rejected stale input preserves stage, request/serial
correlation, commit fact, attachment, selection, and cache revision
(`StaleRejectionPreservesState`). Those claims remain the single-pane specialization: the panel
flight they govern still acts on one leaf, while the collection of live pane attachments around it
is governed below.

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
pane after a focus move and never migrates to the newly focused pane. Commands or worker events
that arrive tagged with a retired incarnation — queued before a rebind or settled late — are
dropped or refused at the exact `PaneRef` and never replay onto the re-bound attachment. A poll
result waits as a pending draw until a completed draw on the exact live record stages it;
`FrameApplied` commits only onto the record that staged the frame — a pending draw whose pane
departed settles without feedback rather than fabricating an acknowledgement — and readiness
follows the focus pane alone while background panes attach and render independently. The
shared HostViewers capacity of 8 applies across all visible panes, and read-only panes never
acquire control or resize.

`NativeClientPaneMembership` is the bounded collection witness. Over two panes, two anchors and
two incarnations per pane it enforces that live records always belong to the current anchor
(`LiveRecordsShareCurrentAnchor`), departing records carry no pending draw, staged or accepted
frame work (`DepartingCarriesNoFrameWork`), a received-but-undrawn frame exists only on a live
attached projected record (`PendingFrameIsLiveAttachedProjected`), an accepted frame is always
the frame that record itself completed drawing (`AcceptedFrameIsOwnStaged`) and only lands on a
live attached projected record (`AcceptedImpliesAttachedLiveProjected`), a host viewer always
equals its record's exact incarnation (`ViewerIsExactLiveIncarnation`) within the shared budget
(`ViewerBudget`), and delivered inputs always reached the exact attachment incarnation they were
tagged for by a command enqueued while that pane held focus
(`InputsDeliverOnlyToTaggedLiveFocus`). Under weak fairness of disposal and reappearance a
departed pane settles (`DepartedEventuallyDisposesOrReappears`).

## Source correspondence

| Production surface                                                                                                                                                                | Model correspondence                                                                                                                                                             |
| --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `PanelMarker::barrier`, admission windows (`terminal_session/state/panel.rs`)                                                                                                     | `SamplePanel`, `RefusePanelAdmission`, `RefusePanelAfterLockExpiry`, `RefuseMoveNoNeighbor`                                                                                      |
| `try_enqueue_panel_for_using` admission and structural exclusion                                                                                                                  | `AdmitPanel`, `StructuralExclusion`, `OtherStructuralExcludesPanel`                                                                                                              |
| Mailbox flight queue/in-flight stages (`terminal_session/mailbox/panel.rs`)                                                                                                       | `stage = "queued"/"in-flight"`, `SubmitPanel`, `ExpirePanelFlight`                                                                                                               |
| `run_panel` outcome classification (definite precommit/unchanged, close receipt, committed, unknown)                                                                              | `Receive*` actions, `ReceiveAmbiguousOutcome`, `NoReplayAfterAmbiguous`                                                                                                          |
| `place_panel_result` identity/serial/deadline correlation                                                                                                                         | `PlaceResult`, `OnlyCurrentResultPlaced`, `DepositCorrelatesCurrent`, `RejectStaleResultDeposit`                                                                                 |
| `panel_reply_current` stale request/serial/target rejection                                                                                                                       | `ResultCurrent` gating of all `Consume*` actions, `RejectStaleResultDeposit`, `RejectStaleMembershipReply`, `RejectStaleRefreshConsume`, `StaleRejectionPreservesState`          |
| `prepare_panel`, `prepare_panel_candidate` exact vs superseding validation                                                                                                        | `ReadCommitSnapshot*`, `ConsumeCommittedMerge`, `ConsumeCommittedUnavailable`, `ConsumeCommittedDivergent`                                                                       |
| `prepare_panel` tab-unavailable snapshot classification (older/divergent, Tab-gone, newer canonical Tab)                                                                          | `ReadTabUnavailableSnapshot`, `ConsumeTabUnavailableMembership`/`Divergent`/`NotApplied`/`Refused`                                                                               |
| `validate_split_panel`, `validate_moved_panel` exact `r+1` graft vs relaxed superseding                                                                                           | `ReceiveCommitted` validity, `ConsumeInvalidGraftCommit`, `ReadCommitSnapshotExact`/`Superseding`                                                                                |
| `merge_projection` requiring the prior selected leaf                                                                                                                              | `ConsumeCommittedMerge` vs `ConsumeCommittedLeaflessRepair` partition                                                                                                            |
| `apply_prepared_panel`, `retain_prepared_panel_membership` commit retention                                                                                                       | `commitHeld`/`commitExpected`, `EnterMembership`, `RecoveryRetainsCommit`                                                                                                        |
| `stage_panel_membership_cleanup`, `prepare_panel_membership_install` release-before-install ordering                                                                              | `ReleaseOldAttachment`, `InstallMembership*`, `AttachRequiresReleasedOld`                                                                                                        |
| `retain_pending_panel`, `PanelProjectionUnavailable` recovery                                                                                                                     | `ConsumeCommittedUnavailable`, `stage = "recovery"`, `AdmitRefresh`, `ApplyRefresh*`                                                                                             |
| `apply_panel_event` UI settlement including unattached and Empty install                                                                                                          | `InstallMembershipAttach`, `InstallMembershipSelect`, `InstallMembershipEmpty`, `CompleteAttach`                                                                                 |
| `setup_target` attach outcome classification and `settle_without_attachment` (`SessionPhaseKind::RetryableAttachUnavailable`)                                                     | `OfferAttachReply`, `CompleteAttach`, `ParkAttachUnavailable`, `FailAttach`, `ExpireAttach`, `ParkedHasNoAttachment`, `RejectedAttachNeverRetires`, `ParkedSettlesViaMembership` |
| Close `Accepted` receipt vs later publication/removal                                                                                                                             | `ReceiveCloseAccepted`, `ConsumeCloseAccepted`, `PeerCloseLeaf`, `CloseReceiptKeepsSelection`                                                                                    |
| `PaneKey`/`PaneRef`/`PaneRecord`, `sync_pane_set`, `depart_all_panes`, `drop_anchor_panes`, `settle_pane_detached` (`terminal_session/state.rs`, `state/membership.rs`)           | `rec[p]`, `MintPane`, `VanishLeaf`, `ReappearLeaf`, `SettleDetachProof`, `AdvanceAnchorEpoch`, `LiveRecordsShareCurrentAnchor`, `DepartingCarriesNoFrameWork`                    |
| Mailbox admission split — input binds logical focus; resize/polls bind exact desired-set incarnations (`terminal_session/mailbox.rs` `admits`)                                    | `Admitted`, `EnqueueFocusControl`, `EnqueuePaneResize`, `EnqueuePoll`, `EnqueueStaleCommand`, `DequeueAdmit`, `DropStaleQueueCommand`                                            |
| Executor exact-incarnation fence in `run_input`/`run_resize`/`run_poll` (`terminal_session/connection/native.rs`) — dispatch keeps the frozen `PaneRef` and never re-checks focus | `ExecuteControl` delivery gate, `ExecutePoll` pending deposit, `ExpireInflight`                                                                                                  |
| Poll result causal settlement pending the draw pass (`publish_poll_result_before`/`take_causal_before`, `CausalSettlement::NoFeedback` on depart)                                 | `rec[p].pending` deposit, `PendingFrameIsLiveAttachedProjected`, depart clearing `pending`                                                                                       |
| Per-record `stage_drawn_frame` / `mark_feedback_accepted` / `invalidate_projection_frame`                                                                                         | `DrawPendingFrame`, `SettleFrameAck`, `AcceptedFrameIsOwnStaged`, `AcceptedImpliesAttachedLiveProjected`                                                                         |
| `settle_pane_fault` known-refusal parking; `settle_attached_event` / `record_setup_configuration` exact-`PaneRef` settlement                                                      | `SettlePaneFault`, `RejectStalePaneFault`, `RejectStaleDetachProof`, `RejectStaleFrameAck`                                                                                       |
| `plan_panel_focus_local` same-Tab focus move                                                                                                                                      | `MoveFocusLocal`                                                                                                                                                                 |
| Shared HostViewers capacity admission (`host_viewers` limit 8)                                                                                                                    | `ViewerBudget`, `AttachPane` capacity gate                                                                                                                                       |

The models abstract away byte-level transport, exact OS/PTY behavior, rendering, worker joins,
allocation of real terminal identities, and move direction/geometry. Existing real tests cover the
concrete seams: native-executor scripted-carrier runs in
`terminal_session/connection/native/tests/panel.rs`, local-client workbench adapters in
`local_client/tests/workbench/panel_split.rs`, `panel_move.rs` and `panel_close.rs`, projection
validators in `terminal_session/workbench_projection/tests/panel_expectation.rs`, `panel_move.rs`
and `panel_close.rs`, and end-to-end CLI mutation flows in `tests/cli_mutation.rs`.

## Finite model evidence

`NativeClientPanelLifecycle` is a bounded design witness, not a proof of the unbounded system.
It covers one Tab, three leaves `{p, q, n}`, one panel operation per run, `MaxQueue = 2`,
`MaxRequest = 4`, `MaxRevision = 4`, `MaxTime = 5`, `Budget = 2`, `MaxPeer = 2`, and
`MaxRefresh = 2`. Peer leaf removal is unrestricted while the Tab survives and is not assumed to
preserve the operation's source leaf — the superseding-leaf counterexample is reachable by
construction. Fairness is weak only: deadline expiry, result deposit, result consumption,
membership resolution, attach reply offer and resolution, and refresh application must eventually
fire when continuously enabled; nothing forces progress while a stage legitimately waits for a
bounded deadline.

Run `node scripts/tla/run-models.mjs --models native-client-panel-lifecycle --jobs 1` with pinned
TLA+ tools 1.7.4 and one worker per configuration. The five configurations below complete safety
and final temporal checking with empty queues and positive generated coverage for all 71 required
actions across the family.

| Configuration      | Finite role                                                  | Generated / distinct / depth | Model/configuration SHA-256 |
| ------------------ | ------------------------------------------------------------ | ---------------------------- | --------------------------- |
| `split-attached`   | split, attached, singleton Tab                               | —                            | —                           |
| `move-attached`    | move, attached, two leaves (unchanged/tab-unavailable paths) | —                            | —                           |
| `close-attached`   | close, attached, two leaves (receipt + publication)          | —                            | —                           |
| `split-unattached` | split, unattached origin (direct install path)               | —                            | —                           |
| `move-singleton`   | move, attached, singleton (no-neighbor refusal)              | —                            | —                           |

`NativeClientPaneMembership` is a bounded design witness for the collection, not a proof of the
unbounded system. It covers two panes `{p0, p1}`, two selection anchors (one epoch switch), two
attachment incarnations per pane, `MaxQueue = 2`, `MaxFrame = 2`, and
`MaxLog = 3`. Adversarial enqueued commands carry strictly retired or never-held incarnation
tags — incarnation numbers only grow, so a forged tag can never collide with a not-yet-minted
attachment — keeping rejection paths reachable witnesses rather than assumed guards. Run
`node scripts/tla/run-models.mjs --models native-client-pane-membership --jobs 1` with pinned
TLA+ tools 1.7.4; the registered entry uses eight TLC workers and a 1800000ms per-configuration
timeout, and each run's actual resources (heap, host) are recorded in that run's own
evidence rather than fixed here — an execution-resource setting only, not a change to any
variable, guard, bound, fairness clause, or checked property, and not evidence of completion. Both
configurations must complete with empty queues and
positive generated coverage for all 23 registered actions across the family. The
`viewer-contention` configuration shrinks the shared viewer budget below the pane count so
`ViewerBudget` is exercised as a real constraint rather than a vacuous one. Server-busy timing,
per-step lease yield, deadline arithmetic, and non-`Detached` outcome classification stay
unmodeled — deterministic reducer and scripted-carrier tests own those claims.

| Configuration       | Finite role                                                       | Generated / distinct / depth | Model/configuration SHA-256 |
| ------------------- | ----------------------------------------------------------------- | ---------------------------- | --------------------------- |
| `default`           | two panes, two anchors, two incarnations, `MaxViewers = 2`        | —                            | —                           |
| `viewer-contention` | same bounds with `MaxViewers = 1` — real shared-budget contention | —                            | —                           |

The delivered-input witness is kept as a summary rather than a sequence. The model
once carried `appliedInputs`, a ghost log of every delivered control input whose only
readers were its `MaxLog` length bound, the `Len < MaxLog` append guard inside
`ExecuteControl`, and the per-entry `tag = delivered` safety check — entry pane fields,
order, and duplicates had no consumers. The model therefore stores the abstraction
`alpha(X, L) = (X, Len(L), forall l in L: l.tag = l.delivered)` as
`(appliedCount, allTaggedValid)`: `Init` maps the empty log to `(0, TRUE)`; the 22
actions that left the log unchanged commute trivially with `alpha`; and
`ExecuteControl`, the only writer, keeps its guard on `appliedCount < MaxLog` and folds
each delivery into the assigned conjunction with the actual
`inflight.incarnation = viewer[p]` comparison — written
`allTaggedValid' = (allTaggedValid /\ (...))` with explicit parentheses, never a
constant `TRUE`. The parentheses are load-bearing, not stylistic: equality binds
tighter than `/\`, so the unparenthesized form parses as
`(allTaggedValid' = allTaggedValid) /\ comparison` and turns the comparison into an
extra enablement guard — a FALSE comparison then makes the whole action produce no
successor (it does not fall through to the `ELSE` branch), so a mismatched delivery
could never be recorded. The saturated path (`appliedCount = MaxLog`) still consumes
the command without recording, exactly as the sequence version skipped the append, so
the post-saturation blind spot is identical in both representations. Fairness is
unchanged: every weak-fairness guard reads only preserved state, and any step that
changed the log already changed the summary, so enabled-ness and non-stuttering
survive the quotient. `InputsDeliverOnlyToTaggedLiveFocus` is now exactly
`allTaggedValid`; the other invariants and the liveness property never read the log.
The correspondence is an exact bisimulation on reachable states, not a one-way safety
over-approximation: every summary state has a concrete lift, and each quotient step is
implementable from any lift because guards read only `X` and the count.

The control-command kind is likewise a quotient, not a payload model. Everywhere the
model reads a kind it only asks membership (`in ControlKinds`), the `"poll"` equality,
or the `Idle` sentinel — nothing ever distinguishes `input` from `resize`, so the
model carries a single `control` label and `CommandKinds = {"control", "poll"}`. The
mapping `beta(input) = beta(resize) = control`, `beta(poll) = poll`,
`beta(idle) = idle` preserves every variable field (pane, incarnation, `focusAdmitted`), every
action's enablement, all seven weak-fairness guards and the non-stuttering relation —
no action only flips the sub-label — and every quotient step lifts back to a same-named
concrete step at either `input` or `resize`. The real Rust `Input` and `Resize`
commands stay fully distinct; this model claims nothing about their respective
payloads, geometry, or byte encodings.

The three `RejectStale*` actions keep their names and their stale existential guards but no
longer update state: each is `IF (\E d : G(d)) THEN UNCHANGED X ELSE FALSE`, a guarded
stuttering successor that still registers in generated coverage. The model once carried a
`rejections` counter bounded by `MaxReject`; its only readers were the three `RejectStale*`
guards and the failure branch of `ExecutePoll` — a verification-only tally of rejected stale
witnesses, not a bound the product enforces. The projection `alpha(X, r) = X` removes it. In
the lift direction, a new trace maps onto an old trace by firing the at-most-`MaxReject` ghost
rejections right after `Init` until the counter saturates; from then on each new `RejectStale*`
stutter corresponds to an old stuttering step — the `UNCHANGED vars` self-loop the old
`[][Next]_vars` spec always permitted, including in states where the saturated counter had
disabled the old `RejectStale*` actions themselves. A new failed poll
corresponds to an old `ExpireInflight` step (the X-update — clear `inflight`, leave
`frameCursor` and `rec` untouched — is identical), and successful polls plus every other X
action are unchanged. Fairness is not strengthened: `ExpireInflight` is weak-fair, so a
continuously occupied `inflight` is still eventually cleared, and the `Idle` slot breaks the
continuous-enabledness chains weak fairness reasons about. The nine business invariants and the
liveness property read only `X` and are unchanged; `TypeOK` keeps every conjunct except the
dropped ghost-counter clause; the model no longer claims a
rejection-count bound the product lacks. This is a two-direction fair-trace projection on
reachable states, not an arbitrary lift or a one-step strong bisimulation.

The generated Rust state model (`terminal_session/state/tests/generated.rs`) exercises the same
contract against the real reducers: its `ReincarnationFence` adversarial class mints a second
incarnation of one logical pane and verifies that work tagged with the retired attachment is
refused at the exact `PaneRef`, while
`reincarnated_pane_fences_late_work_from_the_retired_attachment_incarnation` and
`same_anchor_panes_fence_cross_pane_and_fabricated_incarnation_refs` in `state/tests.rs` pin the
wrong-incarnation and wrong-pane negative controls directly against production settlement APIs.

### Negative controls

Temporary mutants (kept outside the repository) confirm the checked properties are load-bearing:

- Removing result-correlation identity from placement/consume allows a stale request/serial
  deposit to be consumed — `DepositCorrelatesCurrent`/`OnlyCurrentResultPlaced` produce a
  counterexample.
- Deleting the stale-input guard so a mismatched deposit or reply mutates protected state
  violates `StaleRejectionPreservesState`.
- Replacing leafless superseding membership repair with authority divergence — the previously
  shipped behavior — violates `LeaflessSupersedingRepairs` with a counterexample where a peer
  legally closes the selected leaf at `r+2` while the Tab survives.
- Installing or attaching the repaired membership without first releasing the old attachment
  violates `AttachRequiresReleasedOld`.
- Converting the retryable park into retirement violates `RejectedAttachNeverRetires`; replaying
  or auto re-attaching from the parked stage violates `ParkedSettlesViaMembership`.
- Acknowledging a received-but-undrawn poll frame directly — removing the completed-draw gate so
  `SettleFrameAck` commits `accepted` from `pending` — violates `AcceptedFrameIsOwnStaged`: the
  acknowledged frame was never staged by a draw on that record.
- Letting `ExecuteControl` record a delivery attributed to the sibling pane's viewer — a
  wrong-delivery mutant — is the intended control for `InputsDeliverOnlyToTaggedLiveFocus`: the
  first legal delivery to `p0` then folds `inflight.incarnation = viewer[p1]` (`1 = 0`) into
  `allTaggedValid` as `FALSE` through the parenthesized conjunction while the count still
  increments and the real admission gate is preserved. With the earlier unparenthesized
  expression this mutant could not surface a violation: the FALSE comparison acted as an extra
  guard and produced no successor rather than a FALSE flag.

These results establish only the finite panel-operation interface described above. Known
boundaries, disclosed rather than silently claimed: `commitValid = FALSE` and noncanonical
candidates are faulty-authority defensive inputs, not normal honest peer traces; `SomeLeaf` uses
a nondeterministic `CHOOSE` and over-approximates the selection clamp; a snapshot may be re-read
repeatedly through the request cursor where production performs a single read; a close receipt's
effects are delayed until a later publication or peer step; `transport` has no producer in the
abstract model. Membership-vector validation, numeric deadline precision, real carrier behavior,
and multi-operation pipelining remain outside this bounded design witness — it is evidence for
the modeled interface, not a proof of the unbounded production system.

Two modeled inputs are safe over-approximations, not normal wire histories. Reachability is
three-tiered: an abstract `Receive*`/`Consume*` action appearing in the aggregate witness proves
only that the modeled interface can consume that shape; the native adapter accepts a strict
per-intent subset of result kinds; and an honest Server emits a still smaller subset.
`ReceiveTabUnavailable` is a reducer/authority-fault input: an honest authority emits
`Rejected{TabUnavailable}` only at `revision == expected == baseline` with the Tab already absent
from that baseline. Such a request names a Tab absent from its own baseline, so well-formed
Client planning cannot produce it; if a peer removes the Tab after that baseline, the revision
advances and the Server returns `RevisionMismatch` before checking the Tab. The modeled action
fires at any `serverRev` regardless of `serverTab`. `ReceiveUnchangedForeign` is adapter-unreachable — the
real adapter validates `Unchanged.revision == expected` and surfaces a mismatch as an
unclassifiable outcome, never as a result — while the model still feeds it to the reducer's
divergence arm. Both stay in the model deliberately: they exercise the defensive verdicts the
production arms encode, and the 71-action aggregate coverage attests only that each abstract
action has a witness, not that each is reachable from an honest server. The normal committed
paths — including superseding leaf loss through a legal peer close — remain honest histories, and
the bounded safety properties stand on this conservative envelope.
