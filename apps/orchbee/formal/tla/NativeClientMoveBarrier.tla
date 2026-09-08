------------------------- MODULE NativeClientMoveBarrier -------------------------
EXTENDS Integers, Naturals, Sequences, FiniteSets

CONSTANTS InitialSelectionPhase, Singleton, MaxQueue, MaxRequest, MaxTime, Budget
ASSUME /\ InitialSelectionPhase \in {"ready", "selected-no-attach", "retryable"}
       /\ Singleton \in BOOLEAN /\ MaxQueue = 2 /\ MaxRequest = 3
       /\ MaxTime = 8 /\ Budget = 2

A == "A"
B == "B"
C == "C"
Tabs == IF Singleton THEN {A} ELSE {A, B, C}
InitialOrders == IF Singleton THEN {<<A>>} ELSE {<<A,B,C>>, <<B,A,C>>, <<B,C,A>>}
Viewer == [generation |-> 0, tab |-> A]
ExpectedViewers == IF InitialSelectionPhase = "ready" THEN {Viewer} ELSE {}
Key(kind, id) == [kind |-> kind, id |-> id]
NoKey == Key("none", 0)
Index(order, tab) == CHOOSE i \in 1..Len(order): order[i] = tab
Members(order) == {order[i]: i \in 1..Len(order)}
MoveDirection(order, intent) ==
  IF intent = "previous"
  THEN IF Index(order,A) = 1 THEN "last" ELSE "previous"
  ELSE IF Index(order,A) = Len(order) THEN "first" ELSE "next"
Permute(order, direction) ==
  LET i == Index(order,A)
      target == CASE direction = "first" -> 1
                  [] direction = "last" -> Len(order)
                  [] direction = "previous" -> i - 1
                  [] OTHER -> i + 1
      without == SubSeq(order,1,i-1) \o SubSeq(order,i+1,Len(order))
  IN SubSeq(without,1,target-1) \o <<A>> \o SubSeq(without,target,Len(without))
InitialProvenance(order) ==
  [i \in 1..Len(order) |->
    IF order[i] = A THEN IF InitialSelectionPhase = "selected-no-attach"
                         THEN "unavailable" ELSE "started"
    ELSE IF order[i] = B THEN "binding-only" ELSE "unknown"]
Remap(order, provenance, candidate) ==
  [i \in 1..Len(candidate) |-> provenance[Index(order,candidate[i])]]

TransportPhases == {"queued", "command", "committed", "snapshot",
                    "recovery-queued", "recovery-snapshot", "deposit-ready"}
PendingPhases == TransportPhases \cup {"await-ui"}
Phases == PendingPhases \cup {"idle", "projection-unavailable", "retired", "closed"}
Kinds == {"none", "precommit", "noop", "available", "unavailable"}

Initial(order, queued, cursor, oldWork) == [
  phase |-> "idle", cache |-> order, provenance |-> InitialProvenance(order),
  active |-> A, activeIndex |-> Index(order,A), generation |-> 0,
  viewers |-> ExpectedViewers, controllers |-> ExpectedViewers,
  historicalReady |-> (InitialSelectionPhase = "ready"), frameFresh |-> TRUE,
  cursor |-> cursor, usedMove |-> FALSE, submitted |-> FALSE,
  queued |-> queued, oldWork |-> oldWork, oldDeadline |-> 1,
  controlQueued |-> FALSE, controlDelivered |-> FALSE,
  other |-> "none", lockFree |-> TRUE,
  now |-> 0, deadline |-> -1, sample |-> "none", sampleDeadline |-> -1,
  intent |-> "next", direction |-> "next",
  baseline |-> order, baselineRevision |-> 1, cacheRevision |-> 1,
  expected |-> order, serverOrder |-> order, serverRevision |-> 1,
  knownCommit |-> FALSE, committedRevision |-> 0,
  pendingKey |-> NoKey, typedKey |-> NoKey, typedKind |-> "none",
  candidate |-> order, candidateRevision |-> 1,
  snapshotIssued |-> FALSE, recovery |-> FALSE, recoveryGeneration |-> 0,
  unavailableCount |-> 0
]

VARIABLE s
vars == <<s>>

Init ==
  \E order \in InitialOrders, queued \in 0..MaxQueue,
     cursor \in {0, MaxRequest-1, MaxRequest},
     oldWork \in IF InitialSelectionPhase = "ready" THEN {"idle","poll"} ELSE {"idle"}:
    s = Initial(order, queued, cursor, oldWork)

Live == s.phase \notin {"retired","closed"}
BeforeDeadline == s.now < s.deadline
Retired(state) ==
  [state EXCEPT !.phase = "retired", !.viewers = {}, !.controllers = {},
    !.controlQueued = FALSE, !.other = "none", !.sample = "none",
    !.oldWork = "idle", !.queued = 0]
CanOther == Live /\ s.phase = "idle" /\ s.other = "none"
SnapshotValid ==
  /\ s.serverRevision >= s.committedRevision
  /\ Members(s.serverOrder) = Members(s.baseline)
  /\ (s.serverRevision = s.committedRevision => s.serverOrder = s.expected)

ClockAdvance ==
  /\ Live /\ s.now < MaxTime
  /\ s' = [s EXCEPT !.now = @ + 1]
HoldMailboxLock ==
  /\ Live /\ s.lockFree
  /\ s' = [s EXCEPT !.lockFree = FALSE]
ReleaseMailboxLock ==
  /\ Live /\ ~s.lockFree
  /\ s' = [s EXCEPT !.lockFree = TRUE]
InvalidateOrdinaryFrame ==
  /\ Live /\ s.historicalReady /\ s.frameFresh
  /\ s' = [s EXCEPT !.frameFresh = FALSE]

SampleMove(intent) ==
  /\ Live /\ s.phase = "idle" /\ ~s.usedMove /\ s.sample = "none"
  /\ intent \in {"previous","next"}
  /\ s' = [s EXCEPT !.sample = "move", !.intent = intent,
       !.sampleDeadline = IF s.now <= MaxTime-Budget THEN s.now+Budget ELSE -1]
RefuseMoveAdmission ==
  /\ s.sample = "move"
  /\ (s.sampleDeadline = -1 \/ s.now >= s.sampleDeadline
       \/ s.queued = MaxQueue \/ s.cursor = MaxRequest \/ s.other # "none")
  /\ s' = [s EXCEPT !.sample = "none"]
RefuseMoveAfterLockExpiry ==
  /\ ~s.lockFree /\ s.sample = "move" /\ s.sampleDeadline >= 0
  /\ s.now >= s.sampleDeadline
  /\ RefuseMoveAdmission
AdmitMove ==
  /\ s.sample = "move" /\ s.lockFree /\ s.now < s.sampleDeadline
  /\ s.phase = "idle" /\ s.other = "none" /\ ~s.usedMove
  /\ s.queued < MaxQueue /\ s.cursor < MaxRequest
  /\ LET direction == MoveDirection(s.cache,s.intent) IN
       s' = [s EXCEPT !.phase = "queued", !.sample = "none",
         !.deadline = s.sampleDeadline, !.usedMove = TRUE,
         !.cursor = @ + 1, !.pendingKey = Key("move",s.cursor+1),
         !.baseline = s.cache, !.baselineRevision = s.cacheRevision,
         !.direction = direction, !.expected = Permute(s.cache,direction)]
AdmitOther(kind) ==
  /\ kind \in {"create","switch","refresh"} /\ CanOther
  /\ (kind = "create" => s.historicalReady)
  /\ s' = [s EXCEPT !.other = kind]
SettleOther ==
  /\ s.other # "none" /\ s.phase = "idle"
  /\ s' = [s EXCEPT !.other = "none"]
RejectOtherDuringMove ==
  /\ Live /\ s.phase \in PendingPhases \cup {"projection-unavailable"}
  /\ UNCHANGED vars

DrainQueuedOld ==
  /\ Live /\ s.queued > 0 /\ s.oldWork = "idle"
  /\ (s.phase \in TransportPhases => BeforeDeadline)
  /\ s' = [s EXCEPT !.queued = @ - 1]
OldPollProducesFrame ==
  /\ Live /\ s.oldWork = "poll" /\ s.now < s.oldDeadline
  /\ s' = [s EXCEPT !.oldWork = "draw"]
DrawOldFrame ==
  /\ Live /\ s.oldWork = "draw" /\ s.now < s.oldDeadline
  /\ s' = [s EXCEPT !.oldWork = "feedback"]
AcceptOldFeedback ==
  /\ Live /\ s.oldWork = "feedback" /\ s.now < s.oldDeadline
  /\ s' = [s EXCEPT !.oldWork = "idle"]
ExpireEarlierFlight ==
  /\ Live /\ s.oldWork # "idle" /\ s.now >= s.oldDeadline
  /\ s' = Retired(s)
QueueLaterControl ==
  /\ Live /\ s.controllers # {} /\ ~s.controlQueued
  /\ s.phase \in PendingPhases
  /\ s' = [s EXCEPT !.controlQueued = TRUE]
DeliverLaterControl ==
  /\ Live /\ s.controlQueued /\ s.phase \in {"idle","projection-unavailable"}
  /\ s.other = "none"
  /\ s' = [s EXCEPT !.controlQueued = FALSE, !.controlDelivered = TRUE]

SubmitMove ==
  /\ s.phase = "queued" /\ BeforeDeadline
  /\ s.queued = 0 /\ s.oldWork = "idle" /\ ~s.submitted
  /\ s' = [s EXCEPT !.phase = "command", !.submitted = TRUE]
ReceivePrecommit ==
  /\ s.phase = "command" /\ BeforeDeadline
  /\ s' = [s EXCEPT !.phase = "deposit-ready", !.typedKind = "precommit",
                    !.typedKey = s.pendingKey]
ReceiveNoop ==
  /\ s.phase = "command" /\ BeforeDeadline /\ s.expected = s.baseline
  /\ s' = [s EXCEPT !.phase = "deposit-ready", !.typedKind = "noop",
                    !.typedKey = s.pendingKey]
ReceiveCommittedMove ==
  /\ s.phase = "command" /\ BeforeDeadline /\ s.expected # s.baseline
  /\ s.serverRevision = s.baselineRevision
  /\ s' = [s EXCEPT !.phase = "committed", !.knownCommit = TRUE,
       !.committedRevision = s.baselineRevision+1,
       !.serverOrder = s.expected, !.serverRevision = s.baselineRevision+1]
AllocateCommittedSnapshot ==
  /\ s.phase = "committed" /\ BeforeDeadline /\ s.cursor < MaxRequest
  /\ s' = [s EXCEPT !.phase = "snapshot", !.cursor = @ + 1, !.snapshotIssued = TRUE]
CommittedSnapshotIdentityExhausted ==
  /\ s.phase = "committed" /\ s.cursor = MaxRequest
  /\ s' = Retired(s)
ExternalCanonicalMove ==
  /\ Live /\ s.knownCommit /\ s.serverRevision = s.committedRevision
  /\ Len(s.serverOrder) > 1
  /\ s' = [s EXCEPT !.serverOrder = Tail(@) \o <<Head(@)>>,
                    !.serverRevision = @ + 1]

ReceiveAvailableSnapshot ==
  /\ s.phase \in {"snapshot","recovery-snapshot"} /\ BeforeDeadline
  /\ SnapshotValid
  /\ s' = [s EXCEPT !.phase = "deposit-ready", !.typedKind = "available",
       !.typedKey = s.pendingKey, !.candidate = s.serverOrder,
       !.candidateRevision = s.serverRevision]
ReceiveUnavailableSnapshot ==
  /\ s.phase \in {"snapshot","recovery-snapshot"} /\ BeforeDeadline
  /\ s' = [s EXCEPT !.phase = "deposit-ready", !.typedKind = "unavailable",
                    !.typedKey = s.pendingKey]
CorruptOrAmbiguousExchange ==
  /\ s.phase \in {"command","committed","snapshot","recovery-snapshot"}
  /\ s' = Retired(s)
DepositTypedResult ==
  /\ s.phase = "deposit-ready" /\ s.lockFree /\ BeforeDeadline
  /\ s.typedKey = s.pendingKey
  /\ s' = [s EXCEPT !.phase = "await-ui"]
ExpireUndepositedTransport ==
  /\ s.phase \in TransportPhases /\ s.now >= s.deadline
  /\ s' = Retired(s)
ConsumeTypedResult ==
  /\ s.phase = "await-ui" /\ s.typedKey = s.pendingKey
  /\ s' = [s EXCEPT
       !.phase = IF s.typedKind = "unavailable" THEN "projection-unavailable" ELSE "idle",
       !.cache = IF s.typedKind = "available" THEN s.candidate ELSE @,
       !.provenance = IF s.typedKind = "available"
                      THEN Remap(s.cache,s.provenance,s.candidate) ELSE @,
       !.activeIndex = IF s.typedKind = "available" THEN Index(s.candidate,A) ELSE @,
       !.cacheRevision = IF s.typedKind = "available" THEN s.candidateRevision ELSE @,
       !.unavailableCount = @ + (IF s.typedKind = "unavailable" THEN 1 ELSE 0),
       !.recovery = FALSE]
ConsumeAfterTransportDeadline ==
  /\ s.now >= s.deadline /\ ConsumeTypedResult
ConsumeMovedProjection ==
  /\ s.typedKind = "available" /\ s.candidate # s.cache /\ ConsumeTypedResult
ConsumeRecoveryUnavailable ==
  /\ s.recovery /\ s.typedKind = "unavailable" /\ ConsumeTypedResult

SampleRecovery ==
  /\ s.phase = "projection-unavailable" /\ s.sample = "none"
  /\ s' = [s EXCEPT !.sample = "recovery",
       !.sampleDeadline = IF s.now <= MaxTime-Budget THEN s.now+Budget ELSE -1]
RefuseRecoveryAdmission ==
  /\ s.sample = "recovery"
  /\ (s.sampleDeadline = -1 \/ s.now >= s.sampleDeadline
       \/ s.queued = MaxQueue \/ s.recoveryGeneration = 2)
  /\ s' = [s EXCEPT !.sample = "none"]
AdmitRecovery ==
  /\ s.sample = "recovery" /\ s.lockFree /\ s.now < s.sampleDeadline
  /\ s.phase = "projection-unavailable" /\ s.queued < MaxQueue
  /\ s.recoveryGeneration < 2
  /\ s' = [s EXCEPT !.phase = "recovery-queued", !.sample = "none",
       !.deadline = s.sampleDeadline, !.recovery = TRUE,
       !.recoveryGeneration = @ + 1,
       !.pendingKey = Key("refresh",s.recoveryGeneration+1)]
IssueRecoverySnapshot ==
  /\ s.phase = "recovery-queued" /\ BeforeDeadline
  /\ s.queued = 0 /\ s.oldWork = "idle" /\ s.cursor < MaxRequest
  /\ s' = [s EXCEPT !.phase = "recovery-snapshot", !.cursor = @ + 1]
RecoveryIdentityExhausted ==
  /\ s.phase = "recovery-queued" /\ s.cursor = MaxRequest
  /\ s' = Retired(s)
ConsumeRecoverySuccess ==
  /\ s.recovery /\ s.typedKind = "available" /\ ConsumeTypedResult
Exit ==
  /\ s.phase # "closed"
  /\ s' = [Retired(s) EXCEPT !.phase = "closed"]

Next ==
  \/ ClockAdvance \/ HoldMailboxLock \/ ReleaseMailboxLock \/ InvalidateOrdinaryFrame
  \/ \E intent \in {"previous","next"}: SampleMove(intent)
  \/ RefuseMoveAdmission \/ RefuseMoveAfterLockExpiry \/ AdmitMove
  \/ \E kind \in {"create","switch","refresh"}: AdmitOther(kind)
  \/ SettleOther \/ RejectOtherDuringMove
  \/ DrainQueuedOld \/ OldPollProducesFrame \/ DrawOldFrame \/ AcceptOldFeedback
  \/ ExpireEarlierFlight \/ QueueLaterControl \/ DeliverLaterControl
  \/ SubmitMove \/ ReceivePrecommit \/ ReceiveNoop \/ ReceiveCommittedMove
  \/ AllocateCommittedSnapshot \/ CommittedSnapshotIdentityExhausted
  \/ ExternalCanonicalMove \/ ReceiveAvailableSnapshot \/ ReceiveUnavailableSnapshot
  \/ CorruptOrAmbiguousExchange \/ DepositTypedResult \/ ExpireUndepositedTransport
  \/ ConsumeTypedResult \/ ConsumeAfterTransportDeadline \/ ConsumeMovedProjection
  \/ ConsumeRecoveryUnavailable \/ SampleRecovery \/ RefuseRecoveryAdmission
  \/ AdmitRecovery \/ IssueRecoverySnapshot \/ RecoveryIdentityExhausted
  \/ ConsumeRecoverySuccess \/ Exit

Spec == Init /\ [][Next]_vars
  /\ WF_vars(ClockAdvance)
  /\ WF_vars(ReleaseMailboxLock)
  /\ WF_vars(ExpireUndepositedTransport)
  /\ WF_vars(ConsumeTypedResult)

TypeOK ==
  /\ s.phase \in Phases /\ s.now \in 0..MaxTime /\ s.deadline \in -1..MaxTime
  /\ s.sampleDeadline \in -1..MaxTime /\ s.cursor \in 0..MaxRequest
  /\ s.queued \in 0..MaxQueue /\ s.recoveryGeneration \in 0..2
  /\ s.unavailableCount \in 0..2 /\ s.typedKind \in Kinds
  /\ s.cache \in Seq(Tabs) /\ s.serverOrder \in Seq(Tabs)
  /\ s.activeIndex \in 1..Len(s.cache)
  /\ s.cacheRevision \in 1..3 /\ s.serverRevision \in 1..3
  /\ s.committedRevision \in 0..2 /\ s.generation = 0
  /\ s.lockFree \in BOOLEAN /\ s.knownCommit \in BOOLEAN
  /\ s.usedMove \in BOOLEAN /\ s.submitted \in BOOLEAN
ActiveAndProvenanceFollowIdentity ==
  /\ s.active = A /\ s.cache[s.activeIndex] = A
  /\ Members(s.cache) = Tabs /\ Len(s.cache) = Cardinality(Tabs)
  /\ s.provenance = InitialProvenance(s.cache)
MovePreservesAttachment ==
  Live => /\ s.viewers = ExpectedViewers /\ s.controllers = ExpectedViewers
          /\ s.historicalReady = (InitialSelectionPhase = "ready")
StructuralFlightsAreExclusive ==
  s.phase # "idle" => s.other = "none"
PendingUiStillBlocksStructuralAndEffects ==
  s.phase = "await-ui" => /\ ~CanOther /\ ~ENABLED DeliverLaterControl
                          /\ ~ENABLED ExpireUndepositedTransport
ProjectionRecoveryBlocksStructural ==
  s.phase = "projection-unavailable" => ~CanOther
KnownCommitFacts ==
  s.knownCommit => /\ s.committedRevision = s.baselineRevision+1
                   /\ s.serverRevision >= s.committedRevision
                   /\ s.expected = Permute(s.baseline,s.direction)
NoopDoesNotReadSnapshot ==
  s.typedKind = "noop" => ~s.snapshotIssued
TypedResultNamesPendingFlight ==
  s.phase \in {"deposit-ready","await-ui"} => s.typedKey = s.pendingKey
KnownCommitIsNotPrecommit ==
  s.knownCommit => s.typedKind \notin {"precommit","noop"}
AdmissionRefusalPreservesCore == [][
  s.sample \in {"move","recovery"} /\ s'.sample = "none" /\ s'.phase = s.phase =>
    s' = [s EXCEPT !.sample = "none"]
]_vars
MoveAdmissionCapturesDeadlineAndIdentity == [][
  s.phase = "idle" /\ s'.phase = "queued" =>
    /\ s.now < s.sampleDeadline /\ s'.deadline = s.sampleDeadline
    /\ s'.cursor = s.cursor+1 /\ s'.pendingKey = Key("move",s.cursor+1)
    /\ s'.baseline = s.cache /\ s'.baselineRevision = s.cacheRevision
]_vars
RecoveryAdmissionCapturesFreshDeadline == [][
  s.phase = "projection-unavailable" /\ s'.phase = "recovery-queued" =>
    /\ s.now < s.sampleDeadline /\ s'.deadline = s.sampleDeadline
    /\ s'.cursor = s.cursor /\ s'.recoveryGeneration = s.recoveryGeneration+1
    /\ s'.pendingKey = Key("refresh",s.recoveryGeneration+1)
]_vars
AvailableApplicationHonorsCommit == [][
  s.phase = "await-ui" /\ s.typedKind = "available" /\ s'.phase = "idle" =>
    /\ s'.cache = s.candidate /\ s'.cacheRevision = s.candidateRevision
    /\ s'.cacheRevision >= s.committedRevision
    /\ (s'.cacheRevision = s.committedRevision => s'.cache = s.expected)
]_vars
OtherStructuralFlightFreezesCache == [][
  s.other # "none" => UNCHANGED <<s.cache, s.provenance, s.activeIndex>>
]_vars
MoveSubmissionRequiresTruthfulDrain == [][
  ~s.submitted /\ s'.submitted => /\ s.queued = 0 /\ s.oldWork = "idle"
                                  /\ s.now < s.deadline
]_vars
KnownCommitNeverRegresses == [][s.knownCommit => s'.knownCommit]_vars
RequestCursorNeverRewinds == [][s'.cursor >= s.cursor]_vars
DepositPrecedesDeadline == [][
  s.phase = "deposit-ready" /\ s'.phase = "await-ui" =>
    /\ s.now < s.deadline /\ s.typedKey = s.pendingKey
]_vars
NoTransportBudgetRenewal == [][
  s.phase \in PendingPhases /\ s'.phase \in PendingPhases => s'.deadline = s.deadline
]_vars
PendingEventuallySettlesOrRetires ==
  (s.phase \in PendingPhases) ~> (s.phase \notin PendingPhases)

=============================================================================
