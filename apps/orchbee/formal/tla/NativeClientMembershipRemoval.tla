---------------------- MODULE NativeClientMembershipRemoval ----------------------
EXTENDS Integers, Sequences, FiniteSets

CONSTANTS Origin, InitialMode, MaxTime, Budget, CleanupReserve
ASSUME /\ Origin \in {"refresh", "create", "move", "switch", "own-close"}
       /\ InitialMode \in {"ready-control", "ready-readonly", "selected", "retryable", "empty"}
       /\ MaxTime = 8 /\ Budget = 2 /\ CleanupReserve = 1

Tabs == {"A", "B", "C"}
NoTab == "none"
Members(order) == {order[i] : i \in 1..Len(order)}
Orders == {order \in UNION {[1..n -> Tabs] : n \in 0..3} :
             Cardinality(Members(order)) = Len(order)}
Index(order, tab) == IF tab \in Members(order)
                    THEN CHOOSE i \in 1..Len(order): order[i] = tab ELSE 0
Remove(order, tab) == SelectSeq(order, LAMBDA value: value # tab)
Neighbor(order, ordinal) == IF order = <<>> THEN NoTab
                           ELSE order[IF ordinal <= Len(order) /\ ordinal > 0
                                      THEN ordinal ELSE Len(order)]
Key(tab, generation) == [tab |-> tab, generation |-> generation]
NoKey == Key(NoTab, 0)
Keys == {NoKey} \cup [tab : Tabs, generation : 0..2]
\* A is the initial active role, B the Switch desired role, and C the
\* Create result. These declared representatives cover ordinal boundaries;
\* they are not a checked symmetry reduction or an all-permutation theorem.
ActiveOrders == {<<"A">>, <<"A", "B">>, <<"B", "A">>,
                 <<"A", "B", "C">>, <<"B", "A", "C">>, <<"B", "C", "A">>}
InitialOrders == CASE Origin = "create" -> {<<"A", "B">>, <<"B", "A">>}
                     [] Origin = "switch" ->
                        {o \in Orders : {"A", "B"} \subseteq Members(o)}
                     [] Origin = "refresh" -> ActiveOrders \cup {<<>>}
                     [] OTHER -> ActiveOrders
Modes == {"ready-control", "ready-readonly", "selected", "retryable", "empty"}
HasViewer(mode) == mode \in {"ready-control", "ready-readonly"}
InitialProvenance(order, active, mode) ==
  [i \in 1..Len(order) |-> IF order[i] = active /\ mode = "selected"
                          THEN "non-started" ELSE "binding-only"]
TransportPhases == {"queued", "command-wait", "snapshot-wait", "deposit-ready"}
ForwardPhases == {"cleanup-ready", "cleanup-wait", "install", "attach-ready",
                  "attach-wait", "first-frame", "feedback-wait"}
ReservePhases == {"reserve-cleanup-ready", "reserve-cleanup-wait"}
PendingPhases == TransportPhases \cup ForwardPhases \cup ReservePhases
                 \cup {"await-ui", "settlement"}
Phases == PendingPhases \cup {"idle", "stable", "recovery", "retired", "closed"}

VARIABLE s
vars == <<s>>

Initial(order, active, target, mode) == [
  phase |-> "idle", mode |-> mode, used |-> FALSE,
  cache |-> order, cacheRevision |-> 1,
  provenance |-> InitialProvenance(order, active, mode),
  baseline |-> order, target |-> target,
  ordinal |-> IF Origin = "create" THEN Len(order) + 1 ELSE Index(order, target),
  active |-> active, desired |-> target,
  viewer |-> IF HasViewer(mode) THEN Key(active, 0) ELSE NoKey,
  controller |-> (mode = "ready-control"), generation |-> IF Origin = "switch" THEN 1 ELSE 0,
  historicalReady |-> HasViewer(mode), freshProof |-> FALSE,
  serverOrder |-> order, serverRevision |-> 1,
  removed |-> NoTab, removalRevision |-> 0, peerCreated |-> FALSE,
  closingObserved |-> FALSE, knownCommit |-> FALSE, createStarted |-> FALSE,
  knownClosedRevision |-> 0, uiClosedRevision |-> 0, handoffClosedRevision |-> 0,
  knownClosedAttempt |-> 0, handoffClosedAttempt |-> 0,
  candidate |-> order, candidateRevision |-> 1, candidateFault |-> "none",
  resultKind |-> "none", pendingKey |-> 0, typedKey |-> 0,
  deposited |-> FALSE, uiFence |-> FALSE,
  oldFlight |-> HasViewer(mode), cleanupKey |-> NoKey,
  cleanupSettled |-> FALSE, planned |-> NoTab, installed |-> FALSE,
  reconciliation |-> FALSE, freshAttempted |-> FALSE, recoveryUsed |-> FALSE,
  now |-> 0, transportDeadline |-> -1, forwardDeadline |-> -1,
  totalCleanupDeadline |-> -1,
  originalForward |-> -1, originalTotal |-> -1
]

Init == \E order \in InitialOrders:
  \E active \in IF order = <<>> THEN {NoTab} ELSE {"A"}:
    \E target \in CASE Origin = "create" -> {"C"}
                       [] Origin = "switch" -> {"B"}
                       [] OTHER -> {active}:
      \E mode \in ({InitialMode} \cap
                   IF order = <<>> THEN {"empty"}
                   ELSE IF Origin = "create" THEN {"ready-control", "ready-readonly"}
                   ELSE Modes \ {"empty"}):
        \/ s = Initial(order, active, target, mode)
        \/ /\ Origin = "switch" /\ HasViewer(mode)
           /\ s = [Initial(order, active, target, mode) EXCEPT
                !.viewer = Key(target, 1), !.historicalReady = FALSE]

Live == s.phase \notin {"retired", "closed"}
InitialReserve == IF Origin \in {"create", "switch"} THEN CleanupReserve ELSE 0
Expected == IF Origin = "create" THEN s.baseline \o <<s.target>>
            ELSE Remove(s.baseline, s.target) \o <<s.target>>
ExactCommitExpected == Origin \in {"create", "move"} /\ s.knownCommit
CandidateValid ==
  /\ s.candidate \in Orders
  /\ s.candidateRevision >= s.cacheRevision
  /\ s.candidateFault = "none"
  /\ (s.candidateRevision = s.cacheRevision => s.candidate = s.cache)
  /\ (ExactCommitExpected =>
       /\ s.candidateRevision >= 2
       /\ (s.candidateRevision = 2 => s.candidate = Expected))
  /\ (s.knownClosedRevision > 0 =>
       /\ s.candidateRevision >= s.knownClosedRevision
       /\ s.target \notin Members(s.candidate))
Planned == IF s.target \in Members(s.candidate) THEN s.target
           ELSE IF s.target = NoTab /\ s.candidate # <<>> THEN Head(s.candidate)
           ELSE Neighbor(s.candidate, s.ordinal)
NeedsReconciliation == Planned # s.active \/ s.desired # s.active
                       \/ (s.viewer # NoKey /\ s.viewer.tab # Planned)
Remap == [i \in 1..Len(s.candidate) |->
  IF Origin = "create" /\ s.knownCommit /\ s.candidate[i] = s.target
  THEN IF s.createStarted THEN "started" ELSE "non-started"
  ELSE IF s.candidate[i] \in Members(s.cache)
  THEN s.provenance[Index(s.cache, s.candidate[i])]
  ELSE "binding-only"]
CanAttach == s.active # NoTab
  /\ s.provenance[Index(s.cache, s.active)] # "non-started"
\* Create reserves generation 1 at commit. Surviving C keeps that reservation;
\* a replacement consumes the checked successor before selection publication.
SelectedGeneration == IF s.planned = NoTab THEN s.generation
  ELSE IF Origin = "create" /\ s.knownCommit /\ s.planned = s.target THEN 1
  ELSE s.generation + 1
Retired(state) == [state EXCEPT !.phase = "retired", !.viewer = NoKey,
  !.controller = FALSE, !.uiFence = FALSE]
Visible(state) == <<state.cache, state.cacheRevision, state.active, state.desired,
  state.viewer, state.controller, state.provenance, state.uiFence,
  state.knownCommit, state.knownClosedRevision, state.uiClosedRevision>>

AdvanceTime == /\ Live /\ s.now < MaxTime
               /\ s' = [s EXCEPT !.now = @ + 1]
BeginStructuralFlight ==
  /\ s.phase = "idle" /\ ~s.used /\ s.now <= MaxTime-Budget-InitialReserve
  /\ s' = [s EXCEPT !.phase = "queued", !.used = TRUE,
       !.pendingKey = 1, !.uiFence = TRUE, !.transportDeadline = s.now+Budget,
       !.originalForward = IF Origin \in {"create", "switch"} THEN s.now+Budget ELSE -1,
       !.originalTotal = IF Origin \in {"create", "switch"}
                         THEN s.now+Budget+CleanupReserve ELSE -1]
RefuseExhaustedAdmission ==
  /\ s.phase = "idle" /\ s.now > MaxTime-Budget-InitialReserve
  /\ UNCHANGED vars
DrainOldFlight ==
  /\ s.phase = "queued" /\ s.oldFlight /\ s.now < s.transportDeadline
  /\ s' = [s EXCEPT !.oldFlight = FALSE]
DispatchStructuralRead ==
  /\ s.phase = "queued" /\ ~s.oldFlight /\ s.now < s.transportDeadline
  /\ s' = [s EXCEPT !.phase = IF Origin \in {"create", "move"}
                             THEN "command-wait" ELSE "snapshot-wait"]
RetainCreateCommit(started) ==
  /\ Origin = "create" /\ started \in BOOLEAN
  /\ s.phase = "command-wait" /\ s.serverRevision = 1
  /\ s.now < s.transportDeadline
  /\ s' = [s EXCEPT !.knownCommit = TRUE, !.createStarted = started,
       !.generation = 1,
       !.serverOrder = Expected, !.serverRevision = 2, !.phase = "snapshot-wait"]
RetainMoveCommit ==
  /\ Origin = "move" /\ s.phase = "command-wait" /\ s.serverRevision = 1
  /\ Expected # s.baseline /\ s.now < s.transportDeadline
  /\ s' = [s EXCEPT !.knownCommit = TRUE, !.serverOrder = Expected,
       !.serverRevision = 2, !.phase = "snapshot-wait"]
ReceivePrecommitOrNoop ==
  /\ s.phase = "command-wait" /\ s.now < s.transportDeadline
  /\ s' = [s EXCEPT !.phase = "deposit-ready", !.resultKind = "precommit",
       !.typedKey = s.pendingKey]
ReceiveMoveTabUnavailable ==
  /\ Origin = "move" /\ s.phase = "command-wait"
  /\ s.target \notin Members(s.serverOrder) /\ s.now < s.transportDeadline
  /\ s' = [s EXCEPT !.phase = "snapshot-wait"]
ObserveClosingWithoutRemoval ==
  /\ Live /\ ~s.closingObserved /\ s.target \in Members(s.serverOrder)
  /\ s' = [s EXCEPT !.closingObserved = TRUE]
PublishPeerRemoval(tab) ==
  /\ Live /\ s.removed = NoTab /\ tab \in Members(s.serverOrder)
  /\ (Origin = "own-close" => tab = s.target)
  /\ s' = [s EXCEPT !.serverOrder = Remove(@, tab), !.serverRevision = @ + 1,
       !.removed = tab, !.removalRevision = s.serverRevision+1]
ObserveOwnClosed ==
  /\ Origin = "own-close" /\ Live /\ s.used
  /\ s.removed = s.target /\ s.knownClosedRevision = 0
  /\ s' = [s EXCEPT !.knownClosedRevision = s.removalRevision,
       !.knownClosedAttempt = 1]
ObserveEmptyPeerCreate ==
  /\ Origin = "refresh" /\ Live /\ s.baseline = <<>>
  /\ ~s.peerCreated /\ s.serverOrder = <<>> /\ s.serverRevision = 1
  /\ s' = [s EXCEPT !.serverOrder = <<"A">>, !.serverRevision = 2,
       !.peerCreated = TRUE]
ReadAvailableSnapshot ==
  /\ s.phase = "snapshot-wait" /\ s.now < s.transportDeadline
  /\ s' = [s EXCEPT !.phase = "deposit-ready", !.resultKind = "available",
       !.candidate = s.serverOrder, !.candidateRevision = s.serverRevision,
       !.candidateFault = "none", !.typedKey = s.pendingKey]
ReadInvalidSnapshot(fault) ==
  /\ fault \in {"older", "same-revision", "immutable-survivor", "exact-commit", "closed-present"}
  /\ s.phase = "snapshot-wait" /\ s.now < s.transportDeadline
  /\ (fault = "immutable-survivor" => Members(s.serverOrder) \cap Members(s.cache) # {})
  /\ (fault = "exact-commit" => ExactCommitExpected)
  /\ (fault = "closed-present" => s.knownClosedRevision > 0)
  /\ s' = [s EXCEPT !.phase = "deposit-ready", !.resultKind = "available",
       !.candidate = CASE fault = "same-revision" -> IF s.cache = <<>> THEN <<"A">> ELSE <<>>
                         [] fault = "exact-commit" -> Remove(Expected, s.target)
                         [] fault = "closed-present" -> s.baseline
                         [] OTHER -> s.serverOrder,
       !.candidateRevision = CASE fault = "older" -> s.cacheRevision-1
                                 [] fault = "same-revision" -> s.cacheRevision
                                 [] fault = "exact-commit" -> 2
                                 [] OTHER -> s.serverRevision,
       !.candidateFault = IF fault = "immutable-survivor" THEN fault ELSE "none",
       !.typedKey = s.pendingKey]
\* Zero denotes the untrusted relative predecessor of the baseline, not an
\* admitted wire revision. Canonical snapshots remain in revisions 1..4.
ReadOlderSnapshot ==
  /\ s.cacheRevision > 0
  /\ ReadInvalidSnapshot("older")
ReadUnavailableSnapshot ==
  /\ s.phase = "snapshot-wait" /\ s.now < s.transportDeadline
  /\ s' = [s EXCEPT !.phase = "deposit-ready", !.resultKind = "unavailable",
       !.typedKey = s.pendingKey]
DepositExactResult ==
  /\ s.phase = "deposit-ready" /\ s.now < s.transportDeadline
  /\ s.typedKey = s.pendingKey
  /\ s' = [s EXCEPT !.phase = "await-ui", !.deposited = TRUE]
InjectStaleResult ==
  /\ Live /\ s.uiFence /\ UNCHANGED vars
\* This family projects defensive stale observations to stuttering, including
\* repetition. NativeClientRefreshFence and concrete mailbox/byte traces own
\* actual event storage and filtering; the property is a refinement boundary.
IgnoreStaleResult ==
  /\ Live /\ s.uiFence /\ UNCHANGED vars
RejectSnapshotDivergence ==
  /\ s.phase = "await-ui" /\ s.resultKind = "available" /\ ~CandidateValid
  /\ s' = Retired(s)
ConsumeUnavailableOrPrecommit ==
  /\ s.phase = "await-ui" /\ s.resultKind \in {"unavailable", "precommit"}
  /\ s' = [s EXCEPT !.phase = IF s.resultKind = "unavailable"
                     /\ (s.knownCommit \/ s.knownClosedRevision > 0)
                     THEN "recovery" ELSE "settlement",
       !.uiFence = IF s.resultKind = "unavailable"
                      /\ (s.knownCommit \/ s.knownClosedRevision > 0) THEN FALSE ELSE @,
       !.uiClosedRevision = s.knownClosedRevision]
ConsumeInactiveRemoval ==
  /\ s.phase = "await-ui" /\ s.resultKind = "available" /\ CandidateValid
  /\ ~NeedsReconciliation
  /\ s' = [s EXCEPT !.cache = s.candidate, !.cacheRevision = s.candidateRevision,
       !.provenance = Remap, !.phase = "settlement", !.installed = TRUE,
       !.planned = Planned, !.uiClosedRevision = s.knownClosedRevision]
StageActiveRemoval ==
  /\ s.phase = "await-ui" /\ s.resultKind = "available" /\ CandidateValid
  /\ NeedsReconciliation
  /\ ((Origin \in {"create", "switch"} /\ ~s.recoveryUsed)
       \/ s.now <= MaxTime-Budget-CleanupReserve)
  /\ s' = [s EXCEPT !.phase = IF s.viewer = NoKey THEN "install" ELSE "cleanup-ready",
       !.planned = Planned, !.cleanupKey = s.viewer,
       !.cleanupSettled = (s.viewer = NoKey), !.reconciliation = TRUE,
       !.forwardDeadline = IF Origin \in {"create", "switch"} /\ ~s.recoveryUsed
                            THEN s.originalForward ELSE s.now+Budget,
       !.totalCleanupDeadline = IF Origin \in {"create", "switch"} /\ ~s.recoveryUsed
                                THEN s.originalTotal ELSE s.now+Budget+CleanupReserve,
       !.uiClosedRevision = s.knownClosedRevision]
RejectSetupBudgetOverflow ==
  /\ s.phase = "await-ui" /\ s.resultKind = "available" /\ CandidateValid
  /\ NeedsReconciliation /\ (Origin \notin {"create", "switch"} \/ s.recoveryUsed)
  /\ s.now > MaxTime-Budget-CleanupReserve /\ s' = Retired(s)
RequestExactOldCleanup ==
  /\ s.phase = "cleanup-ready" /\ s.viewer = s.cleanupKey /\ s.viewer # NoKey
  /\ s.now < s.forwardDeadline /\ s' = [s EXCEPT !.phase = "cleanup-wait"]
CompleteExactOldCleanup ==
  /\ s.phase = "cleanup-wait" /\ s.viewer = s.cleanupKey
  /\ s.now < s.forwardDeadline
  /\ s' = [s EXCEPT !.viewer = NoKey, !.controller = FALSE,
       !.cleanupSettled = TRUE, !.phase = "install"]
CompleteAlreadyGoneCleanup ==
  /\ s.phase \in {"cleanup-ready", "cleanup-wait"}
  /\ s.viewer = s.cleanupKey /\ s.cleanupKey.tab = s.removed
  /\ s.removalRevision > 0 /\ s.now < s.forwardDeadline
  /\ s' = [s EXCEPT !.viewer = NoKey, !.controller = FALSE,
       !.cleanupSettled = TRUE, !.phase = "install"]
RetireAmbiguousCleanup ==
  /\ s.phase \in {"cleanup-wait", "reserve-cleanup-wait"}
  /\ s' = Retired(s)
ApplyAfterCleanup ==
  /\ s.phase = "install" /\ s.viewer = NoKey /\ s.cleanupSettled
  /\ s.now < s.forwardDeadline /\ SelectedGeneration <= 2
  /\ s' = [s EXCEPT !.cache = s.candidate, !.cacheRevision = s.candidateRevision,
       !.provenance = Remap, !.active = s.planned, !.desired = s.planned,
       !.generation = SelectedGeneration,
       !.historicalReady = FALSE, !.freshProof = FALSE, !.installed = TRUE,
       !.phase = "attach-ready"]
ApplyHistoricalCreatedTargetRemoval ==
  /\ Origin = "create" /\ s.knownCommit /\ s.target \notin Members(s.candidate)
  /\ ApplyAfterCleanup
ApplyHistoricalMovedTargetRemoval ==
  /\ Origin = "move" /\ s.knownCommit /\ s.target \notin Members(s.candidate)
  /\ ApplyAfterCleanup
ChooseFreshAttachment ==
  /\ s.phase = "attach-ready" /\ s.now < s.forwardDeadline
  /\ CanAttach /\ s.generation \in 1..2 /\ s.viewer = NoKey
  /\ s' = [s EXCEPT !.phase = "attach-wait", !.freshAttempted = TRUE]
SettleWithoutAttachment ==
  /\ s.phase = "attach-ready" /\ ~CanAttach
  /\ s' = [s EXCEPT !.phase = "settlement",
       !.mode = IF s.active = NoTab THEN "empty" ELSE "selected"]
FreshAttachUnavailable ==
  /\ s.phase = "attach-wait" /\ s.now < s.forwardDeadline
  /\ s' = [s EXCEPT !.phase = "settlement", !.mode = "retryable"]
FreshAttachCompletes(controlling) ==
  /\ controlling \in BOOLEAN /\ s.phase = "attach-wait"
  /\ s.now < s.forwardDeadline
  /\ s' = [s EXCEPT !.viewer = Key(s.active, s.generation),
       !.controller = controlling, !.phase = "first-frame"]
DrawFreshFrame ==
  /\ s.phase = "first-frame" /\ s.now < s.forwardDeadline
  /\ s' = [s EXCEPT !.phase = "feedback-wait"]
AcceptFreshReadyProof ==
  /\ s.phase = "feedback-wait" /\ s.now < s.forwardDeadline
  /\ s' = [s EXCEPT !.phase = "settlement", !.freshProof = TRUE,
       !.mode = IF s.controller THEN "ready-control" ELSE "ready-readonly"]
SettleExactUiResult ==
  /\ s.phase = "settlement" /\ s.typedKey = s.pendingKey /\ s.deposited
  /\ s' = [s EXCEPT !.phase = "stable", !.uiFence = FALSE]
BeginExplicitRecovery ==
  /\ s.phase = "recovery" /\ ~s.recoveryUsed
  /\ s.now <= MaxTime-Budget
  /\ s' = [s EXCEPT !.phase = "snapshot-wait", !.pendingKey = 2,
       !.typedKey = 0, !.deposited = FALSE, !.uiFence = TRUE,
       !.recoveryUsed = TRUE, !.transportDeadline = s.now+Budget]
ExpirePendingTransport ==
  /\ s.phase \in TransportPhases /\ ~s.deposited
  /\ s.now >= s.transportDeadline /\ s' = Retired(s)
ExpireForwardSetup ==
  /\ s.phase \in ForwardPhases /\ s.now >= s.forwardDeadline
  /\ s' = IF s.phase = "attach-wait" THEN Retired(s)
           ELSE IF s.viewer # NoKey
           THEN [s EXCEPT !.phase = IF s.phase = "cleanup-wait"
                                  THEN "reserve-cleanup-wait" ELSE "reserve-cleanup-ready",
                 !.cleanupKey = s.viewer]
           ELSE IF ~s.installed THEN Retired(s)
           ELSE [s EXCEPT !.phase = "settlement",
                 !.mode = IF s.active = NoTab THEN "empty" ELSE "retryable"]
RequestReserveCleanup ==
  /\ s.phase = "reserve-cleanup-ready" /\ s.now < s.totalCleanupDeadline
  /\ s' = [s EXCEPT !.phase = "reserve-cleanup-wait"]
CompleteReserveCleanup ==
  /\ s.phase = "reserve-cleanup-wait" /\ s.viewer = s.cleanupKey
  /\ s.now < s.totalCleanupDeadline
  /\ s' = [Retired(s) EXCEPT !.cleanupSettled = TRUE]
ExpireCleanupReserve ==
  /\ s.phase \in ReservePhases /\ s.now >= s.totalCleanupDeadline
  /\ s' = Retired(s)
RetainKnownResultOnWorkerExit ==
  /\ s.knownClosedRevision > 0 /\ s.handoffClosedRevision = 0
  /\ s' = [Retired(s) EXCEPT !.handoffClosedRevision = s.knownClosedRevision,
       !.handoffClosedAttempt = s.knownClosedAttempt]
Retire == /\ Live /\ s' = Retired(s)
Exit == /\ s.phase # "closed"
        /\ s' = [Retired(s) EXCEPT !.phase = "closed",
             !.handoffClosedRevision = s.knownClosedRevision,
             !.handoffClosedAttempt = s.knownClosedAttempt]

ConsumeUi == RejectSnapshotDivergence \/ ConsumeUnavailableOrPrecommit
             \/ ConsumeInactiveRemoval \/ StageActiveRemoval \/ RejectSetupBudgetOverflow
             \/ SettleExactUiResult
Expiry == ExpirePendingTransport \/ ExpireForwardSetup \/ ExpireCleanupReserve
Next ==
  \/ AdvanceTime \/ BeginStructuralFlight \/ RefuseExhaustedAdmission
  \/ DrainOldFlight \/ DispatchStructuralRead
  \/ \E started \in BOOLEAN: RetainCreateCommit(started)
  \/ RetainMoveCommit \/ ReceivePrecommitOrNoop \/ ReceiveMoveTabUnavailable
  \/ ObserveClosingWithoutRemoval \/ \E tab \in Tabs: PublishPeerRemoval(tab)
  \/ ObserveOwnClosed \/ ObserveEmptyPeerCreate
  \/ ReadAvailableSnapshot \/ ReadUnavailableSnapshot
  \/ ReadOlderSnapshot
  \/ \E fault \in {"same-revision", "immutable-survivor", "exact-commit", "closed-present"}:
       ReadInvalidSnapshot(fault)
  \/ DepositExactResult \/ InjectStaleResult \/ IgnoreStaleResult
  \/ RejectSnapshotDivergence \/ ConsumeUnavailableOrPrecommit \/ ConsumeInactiveRemoval
  \/ StageActiveRemoval \/ RejectSetupBudgetOverflow
  \/ RequestExactOldCleanup \/ CompleteExactOldCleanup \/ CompleteAlreadyGoneCleanup
  \/ RetireAmbiguousCleanup \/ ApplyAfterCleanup
  \/ ApplyHistoricalCreatedTargetRemoval \/ ApplyHistoricalMovedTargetRemoval
  \/ ChooseFreshAttachment \/ SettleWithoutAttachment \/ FreshAttachUnavailable
  \/ \E controlling \in BOOLEAN: FreshAttachCompletes(controlling)
  \/ DrawFreshFrame \/ AcceptFreshReadyProof \/ SettleExactUiResult
  \/ BeginExplicitRecovery \/ ExpirePendingTransport \/ ExpireForwardSetup
  \/ RequestReserveCleanup \/ CompleteReserveCleanup \/ ExpireCleanupReserve
  \/ RetainKnownResultOnWorkerExit \/ Retire \/ Exit

Spec == Init /\ [][Next]_vars /\ WF_vars(AdvanceTime) /\ WF_vars(Expiry)
        /\ WF_vars(ConsumeUi)

TypeOK ==
  /\ s.phase \in Phases /\ s.mode \in Modes
  /\ s.cache \in Orders /\ s.baseline \in Orders /\ s.serverOrder \in Orders
  /\ s.candidate \in Orders /\ s.provenance \in Seq({"started", "non-started", "binding-only"})
  /\ Len(s.provenance) = Len(s.cache)
  /\ s.cacheRevision \in 1..4 /\ s.serverRevision \in 1..4 /\ s.candidateRevision \in 0..4
  /\ s.candidateFault \in {"none", "immutable-survivor"}
  /\ s.resultKind \in {"none", "available", "unavailable", "precommit"}
  /\ s.ordinal \in 0..3
  /\ s.target \in Tabs \cup {NoTab} /\ s.active \in Tabs \cup {NoTab}
  /\ s.desired \in Tabs \cup {NoTab} /\ s.planned \in Tabs \cup {NoTab}
  /\ s.viewer \in Keys /\ s.cleanupKey \in Keys /\ s.generation \in 0..2
  /\ s.removed \in Tabs \cup {NoTab} /\ s.removalRevision \in 0..4
  /\ s.knownClosedRevision \in 0..4 /\ s.uiClosedRevision \in 0..4
  /\ s.handoffClosedRevision \in 0..4
  /\ s.knownClosedAttempt \in 0..1 /\ s.handoffClosedAttempt \in 0..1
  /\ s.pendingKey \in 0..2 /\ s.typedKey \in 0..2 /\ s.now \in 0..MaxTime
  /\ s.transportDeadline \in -1..MaxTime /\ s.forwardDeadline \in -1..MaxTime
  /\ s.totalCleanupDeadline \in -1..MaxTime
  /\ \A flag \in {s.used, s.controller, s.historicalReady, s.freshProof, s.peerCreated,
       s.closingObserved, s.knownCommit, s.createStarted, s.deposited, s.uiFence,
       s.oldFlight, s.cleanupSettled, s.installed,
       s.reconciliation, s.freshAttempted, s.recoveryUsed}: flag \in BOOLEAN
AtMostOneViewerAndController == s.controller => s.viewer # NoKey
NoRemovedIdResurrection == s.active # NoTab => s.active \in Members(s.cache)
EmptyHasNoAttachmentOrReady ==
  Live /\ s.phase = "stable" /\ s.cache = <<>> =>
    /\ s.active = NoTab /\ s.desired = NoTab /\ s.viewer = NoKey
    /\ ~s.controller /\ ~s.historicalReady /\ ~s.freshProof
CandidateWaitsForExactCleanup ==
  Live /\ s.reconciliation /\ ~s.cleanupSettled /\ ~s.installed => s.cache = s.baseline
NoReplacementSelectionBeforeCleanup ==
  s.reconciliation /\ s.installed => s.cleanupSettled
NeighborUsesHistoricalOrdinal ==
  s.reconciliation /\ s.installed =>
    s.active = IF s.target \in Members(s.cache) THEN s.target
               ELSE IF s.target = NoTab /\ s.cache # <<>> THEN Head(s.cache)
               ELSE Neighbor(s.cache, s.ordinal)
FreshSelectionUsesFreshKey ==
  /\ (Origin = "create" /\ s.knownCommit => s.generation >= 1)
  /\ (s.reconciliation /\ s.installed /\ s.active # NoTab =>
       /\ IF Origin = "create" /\ s.knownCommit /\ s.active = s.target
          THEN s.generation = 1
          ELSE s.generation > (IF Origin = "switch" \/ (Origin = "create" /\ s.knownCommit)
                               THEN 1 ELSE 0)
       /\ (s.viewer # NoKey => s.viewer = Key(s.active, s.generation)))
FreshReadyRequiresProof ==
  Live /\ s.phase = "stable" /\ s.reconciliation /\ HasViewer(s.mode) =>
    /\ s.freshProof /\ s.viewer = Key(s.active, s.generation)
PendingResultFencesExecutor == s.phase \in PendingPhases => s.uiFence
WorkerExitPreservesKnownClosed == s.handoffClosedRevision > 0 =>
  /\ s.handoffClosedRevision = s.knownClosedRevision
  /\ s.handoffClosedAttempt = s.knownClosedAttempt
ExistingSetupBudgetNeverRenews ==
  Origin \in {"create", "switch"} /\ s.reconciliation /\ ~s.recoveryUsed =>
    /\ s.forwardDeadline = s.originalForward /\ s.totalCleanupDeadline = s.originalTotal

SameRevisionIsExact == [][s.cache' # s.cache => s.cacheRevision' > s.cacheRevision]_vars
SurvivingFactsAreImmutable == [][s.cache' # s.cache =>
  /\ s.candidateFault = "none"
  /\ \A tab \in Members(s.cache) \cap Members(s.cache'):
       s.provenance'[Index(s.cache', tab)] = s.provenance[Index(s.cache, tab)]]_vars
KnownClosedAbsentAtCommitOrLater == [][s.cache' # s.cache /\ s.knownClosedRevision > 0 =>
  s.cacheRevision' >= s.knownClosedRevision /\ s.target \notin Members(s.cache')]_vars
HistoricalCommitNeverForgotten == [][s.knownCommit => s.knownCommit']_vars
KnownClosedNeverForgotten == [][s.knownClosedRevision > 0 =>
  /\ s.knownClosedRevision' = s.knownClosedRevision
  /\ s.knownClosedAttempt' = s.knownClosedAttempt]_vars
InactiveRemovalPreservesExactAttachment == [][ConsumeInactiveRemoval =>
  UNCHANGED <<s.viewer, s.controller, s.generation, s.historicalReady>>]_vars
StaleResultCannotSettleCurrentFlight == [][IgnoreStaleResult =>
  Visible(s') = Visible(s)]_vars
NoAutomaticCreateOrStart == [][s.serverOrder' # s.serverOrder =>
  PublishPeerRemoval(s.removed') \/ ObserveEmptyPeerCreate
  \/ (Origin = "create" /\ \E started \in BOOLEAN: RetainCreateCommit(started))
  \/ RetainMoveCommit]_vars
ReconciliationEventuallySettles ==
  (s.phase \in PendingPhases) ~> (s.phase \in {"stable", "recovery", "retired", "closed"})
=============================================================================
