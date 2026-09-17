# Native panel lifecycle

## Scope and ownership

The native Client admits at most one structural panel mutation — split, move, or close of the
selected leaf — per accepted operation, transports it over the native carrier, and settles its
typed result through the UI mailbox. This reference governs the panel-operation extension of the
[native Tab Session](native-tab-session.md). Maintain it with `terminal_session/state/panel.rs`,
`terminal_session/mailbox/panel.rs`, `terminal_session/ui/panel.rs`, the projection validators in
`terminal_session/workbench_projection.rs`, the native executor in
`terminal_session/connection/native/panel.rs`, and the local-client workbench panel modules.

The Client never merges a panel result it cannot correlate. Admission requires a stable Session
with a selected leaf, a free structural-mutation slot, and an unexpired forward deadline. A panel
operation binds its request identity, attached-leaf key, baseline projection revision, desired
attachment, and total deadline once, at admission; nothing may silently renumber them afterwards.

## State and transition contract

| Transition                    | Guard and result                                                                                                                                                                              |
| ----------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Sample panel intent           | A stable Session samples the user intent under a bounded admission window; sampling never mutates Session state.                                                                                |
| Refuse panel admission        | Lock contention past the deadline, a queued/in-flight/awaiting structural mutation, a pending refresh or switch fence, a saturated mailbox, identity exhaustion, or no desired leaf refuses admission and preserves all Session facts. Move with no neighbor is a definite no-op refusal. |
| Admit panel operation         | Admission allocates the next request identity and lease serial, freezes the baseline revision/panels and desired leaf into the operation record, and enters the queued stage before submit.       |
| Submit panel operation        | The queued operation is submitted at most once before its deadline; a submit that reaches the carrier is recorded exactly once.                                                                  |
| Receive panel outcome         | The carrier produces not-submitted, precommit, unchanged, tab-unavailable, close-accepted, or a committed fact with its post-commit authoritative snapshot.                                      |
| Retire on ambiguous outcome   | A submitted operation whose outcome cannot be classified retires the Session as unknown; an operation that may have produced an effect is never automatically replayed.                          |
| Read post-commit snapshot     | A committed split/move re-reads the authority snapshot at a fresh identity. The exact `r+1` candidate must match the committed graft; a later superseding revision is validated canonically only. |
| Deposit typed result          | The worker deposits the outcome bound to the operation request, serial and deadline; a deposit that does not match the current operation identity is never placed.                               |
| Consume typed result          | The UI consumes a placed result only while request, serial, baseline revision and desired leaf still match the pending operation.                                                               |
| Merge committed projection    | A committed result whose candidate still contains the operation's selected leaf merges it into the projection cache at the candidate revision; the leaf stays selected and attached.             |
| Repair superseding membership | A committed result whose later candidate removed the selected leaf while the Tab survives — including a peer's legal close — enters membership reconciliation: it must not retire as divergence. |
| Enter recovery on unavailable | A committed result with an unavailable candidate parks in panel recovery; the committed fact is retained until a later refresh applies or membership replaces it.                                |
| Diverge on invalid authority  | An invalid graft, a foreign unchanged move, a tab-unavailable contradiction, or a divergent candidate retires the Session as authority divergence.                                              |
| Stage membership cleanup      | Membership reconciliation first stages the release of the exact old attachment; cleanup failure or deadline expiry retires with the committed fact retained.                                    |
| Install repaired membership   | Only after the old attachment is released may the new membership install: Empty when the Tab is gone, otherwise select a repaired leaf; attachable provenance then re-attaches.                    |
| Admit bounded refresh         | With no structural flight pending, a bounded refresh re-reads the authority snapshot; during panel recovery it is the only path that can resolve the retained commit.                            |
| Apply refresh outcome         | A canonical refresh merges, repairs a missing selected leaf, or installs Empty membership; an unavailable read parks, a divergent read retires. Refresh results are fenced while membership is pending. |
| Retire and close Session      | UI retirement freezes the pending records and preserves the committed fact; Exit closes the Session.                                                                                            |

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
(`PendingEventuallySettlesOrRetires`).

## Source correspondence

| Production surface                                                                                          | Model correspondence                                                                                     |
| ----------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------- |
| `PanelMarker::barrier`, admission windows (`terminal_session/state/panel.rs`)                                  | `SamplePanel`, `RefusePanelAdmission`, `RefusePanelAfterLockExpiry`, `RefuseMoveNoNeighbor`                 |
| `try_enqueue_panel_for_using` admission and structural exclusion                                             | `AdmitPanel`, `StructuralExclusion`, `OtherStructuralExcludesPanel`                                         |
| Mailbox flight queue/in-flight stages (`terminal_session/mailbox/panel.rs`)                                    | `stage = "queued"/"in-flight"`, `SubmitPanel`, `ExpirePanelFlight`                                          |
| `run_panel` outcome classification (definite precommit/unchanged, close receipt, committed, unknown)          | `Receive*` actions, `ReceiveAmbiguousOutcome`, `NoReplayAfterAmbiguous`                                     |
| `place_panel_result` identity/serial/deadline correlation                                                     | `PlaceResult`, `OnlyCurrentResultPlaced`, `DepositCorrelatesCurrent`, `RejectStaleResultDeposit`            |
| `panel_reply_current` stale request/serial/target rejection                                                  | `ResultCurrent` gating of all `Consume*` actions                                                            |
| `prepare_panel`, `prepare_panel_candidate` exact vs superseding validation                                    | `ReadCommitSnapshot*`, `ConsumeCommittedMerge`, `ConsumeCommittedUnavailable`, `ConsumeCommittedDivergent`  |
| `validate_split_panel`, `validate_moved_panel` exact `r+1` graft vs relaxed superseding                        | `ReceiveCommitted` validity, `ConsumeInvalidGraftCommit`, `ReadCommitSnapshotExact`/`Superseding`           |
| `merge_projection` requiring the prior selected leaf                                                         | `ConsumeCommittedMerge` vs `ConsumeCommittedLeaflessRepair` partition                                       |
| `apply_prepared_panel`, `retain_prepared_panel_membership` commit retention                                  | `commitHeld`/`commitExpected`, `EnterMembership`, `RecoveryRetainsCommit`                                   |
| `stage_panel_membership_cleanup`, `prepare_panel_membership_install` release-before-install ordering          | `ReleaseOldAttachment`, `InstallMembership*`, `AttachRequiresReleasedOld`                                   |
| `retain_pending_panel`, `PanelProjectionUnavailable` recovery                                                | `ConsumeCommittedUnavailable`, `stage = "recovery"`, `AdmitRefresh`, `ApplyRefresh*`                        |
| `apply_panel_event` UI settlement including unattached and Empty install                                      | `InstallMembershipAttach`, `InstallMembershipSelect`, `InstallMembershipEmpty`, `CompleteAttach`            |
| Close `Accepted` receipt vs later publication/removal                                                         | `ReceiveCloseAccepted`, `ConsumeCloseAccepted`, `PeerCloseLeaf`, `CloseReceiptKeepsSelection`               |

The model abstracts away byte-level transport, exact OS/PTY behavior, rendering, worker joins,
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
membership resolution, attach resolution and refresh application must eventually fire when
continuously enabled; nothing forces progress while a stage legitimately waits for a bounded
deadline.

Run `node scripts/tla/run-models.mjs --models native-client-panel-lifecycle --jobs 1` with pinned
TLA+ tools 1.7.4 and one worker per configuration. The five configurations below complete safety
and final temporal checking with empty queues and positive generated coverage for all 68 required
actions across the family.

| Configuration      | Finite role                                                   | Generated / distinct / depth | Model/configuration SHA-256 |
| ------------------ | ------------------------------------------------------------- | ---------------------------- | --------------------------- |
| `split-attached`   | split, attached, singleton Tab                                | —                            | —                           |
| `move-attached`    | move, attached, two leaves (unchanged/tab-unavailable paths)  | —                            | —                           |
| `close-attached`   | close, attached, two leaves (receipt + publication)           | —                            | —                           |
| `split-unattached` | split, unattached origin (direct install path)                | —                            | —                           |
| `move-singleton`   | move, attached, singleton (no-neighbor refusal)               | —                            | —                           |

### Negative controls

Temporary mutants (kept outside the repository) confirm the checked properties are load-bearing:

- Removing result-correlation identity from placement/consume allows a stale request/serial
  deposit to be consumed — `DepositCorrelatesCurrent`/`OnlyCurrentResultPlaced` produce a
  counterexample.
- Replacing leafless superseding membership repair with authority divergence — the previously
  shipped behavior — violates `LeaflessSupersedingRepairs` with a counterexample where a peer
  legally closes the selected leaf at `r+2` while the Tab survives.
- Installing or attaching the repaired membership without first releasing the old attachment
  violates `AttachRequiresReleasedOld`.

These results establish only the finite panel-operation interface described above — not
membership-vector validation, numeric deadlines, real carrier behavior, or multi-operation
pipelining.
