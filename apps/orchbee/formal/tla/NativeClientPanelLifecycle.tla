--------------------- MODULE NativeClientPanelLifecycle ---------------------
EXTENDS Integers, Naturals, FiniteSets

CONSTANTS OpKind, Attached, Pair, MaxQueue, MaxRequest, MaxRevision, MaxTime,
          Budget, MaxPeer, MaxRefresh
ASSUME /\ OpKind \in {"split", "move", "close"}
       /\ Attached \in BOOLEAN /\ Pair \in BOOLEAN
       /\ MaxQueue = 2 /\ MaxRequest = 4 /\ MaxRevision = 4
       /\ MaxTime = 5 /\ Budget = 2 /\ MaxPeer = 2 /\ MaxRefresh = 2

P == "p"
Q == "q"
N == "n"
NoPanel == "none"
Panels == {P, Q, N}
StartPanels == IF Pair THEN {P, Q} ELSE {P}
ExpectedAfter(base) == IF OpKind = "split" THEN base \cup {N} ELSE base

Stages == {"none", "queued", "in-flight", "await-ui", "cleanup", "installing",
           "attach-pending", "recovery"}
WireKinds == {"none", "not-submitted", "precommit", "unchanged-same",
              "unchanged-foreign", "close-accepted", "committed",
              "tab-unavailable"}
Candidates == {"none", "exact", "superseding-kept", "superseding-leafless",
               "tab-gone", "unavailable", "divergent", "refreshed", "leafless"}
MembershipStages == {"none", "staged", "cleaned"}
MembershipOrigins == {"none", "panel", "refresh"}
Debt == {"clean", "pending", "in-flight"}
Reasons == {"none", "protocol", "divergence", "unknown", "transport",
            "deadline", "cleanup", "exit"}
Reads == {"none", "available", "unavailable", "divergent"}

VARIABLE s
vars == <<s>>

Init ==
  s = [
    now |-> 0, retired |-> FALSE, closed |-> FALSE, retireReason |-> "none",
    lockFree |-> TRUE, queued |-> 0, otherStructural |-> FALSE,
    sample |-> "none", sampleDeadline |-> -1,
    opUsed |-> FALSE, cursor |-> 0, serial |-> 0,
    stage |-> "none", opReq |-> 0, opSerial |-> 0, opBaselineRev |-> 0,
    opBaselinePanels |-> {}, opTarget |-> NoPanel, opMoveTarget |-> NoPanel,
    opDeadline |-> -1,
    submitted |-> FALSE, submitCount |-> 0,
    commitHeld |-> FALSE, commitRev |-> 0, commitExpected |-> {},
    commitValid |-> TRUE,
    wire |-> "none", result |-> "none", resultReq |-> 0, resultSerial |-> 0,
    candidate |-> "none", candidatePanels |-> {}, candidateRev |-> 0,
    tabUnavailRev |-> 0,
    membership |-> "none", membershipReq |-> 0, membershipOld |-> NoPanel,
    membershipOrigin |-> "none", membershipDeadline |-> -1,
    attached |-> IF Attached THEN {P} ELSE {},
    selected |-> P, desired |-> P,
    cachePanels |-> StartPanels, cacheRev |-> 1,
    serverTab |-> TRUE, serverPanels |-> StartPanels, serverRev |-> 1,
    peerSteps |-> 0,
    refreshGen |-> 0, refreshCount |-> 0, refreshRead |-> "none",
    publicationDebt |-> "clean", publicationLate |-> FALSE,
    applied |-> FALSE, leafless |-> FALSE, staleRejected |-> FALSE,
    closeConsumed |-> FALSE, publicationInstalled |-> FALSE
  ]

Live == ~s.retired /\ ~s.closed
PendingStages == {"queued", "in-flight", "await-ui", "cleanup", "installing",
                  "attach-pending"}
BeforeDeadline == s.now < s.opDeadline
SomeLeaf(ps) == CHOOSE l \in ps: TRUE

Retire(reason, state) ==
  [state EXCEPT !.retired = TRUE, !.retireReason = reason, !.sample = "none",
    !.wire = "none", !.otherStructural = FALSE, !.lockFree = TRUE]

EnterMembership(origin, state) ==
  IF state.attached # {}
  THEN [state EXCEPT !.stage = "cleanup", !.membership = "staged",
         !.membershipOld = SomeLeaf(state.attached),
         !.membershipReq = IF origin = "panel" THEN state.opReq
                           ELSE state.refreshGen,
         !.membershipOrigin = origin,
         !.membershipDeadline = state.now + Budget]
  ELSE [state EXCEPT !.stage = "installing", !.membership = "none",
         !.membershipReq = IF origin = "panel" THEN state.opReq
                           ELSE state.refreshGen,
         !.membershipOrigin = origin,
         !.membershipDeadline = state.now + Budget]

ResolvePublicationDebt(state) ==
  IF state.publicationDebt = "in-flight"
  THEN IF state.publicationLate THEN "pending" ELSE "clean"
  ELSE state.publicationDebt

ResultCurrent ==
  /\ s.stage = "await-ui" /\ s.resultReq = s.opReq
  /\ s.resultSerial = s.opSerial /\ s.cacheRev = s.opBaselineRev
  /\ s.desired = s.opTarget

ClearResult(state) ==
  [state EXCEPT !.stage = "none", !.result = "none", !.resultReq = 0,
    !.resultSerial = 0, !.candidate = "none", !.candidatePanels = {},
    !.candidateRev = 0, !.wire = "none"]

KeepCandidate(state) ==
  [state EXCEPT !.stage = "none", !.result = "none", !.resultReq = 0,
    !.resultSerial = 0, !.wire = "none"]

InstalledRecord(state) ==
  [state EXCEPT !.cachePanels = state.candidatePanels,
    !.cacheRev = state.candidateRev,
    !.selected = IF state.candidatePanels = {} THEN NoPanel
                 ELSE SomeLeaf(state.candidatePanels),
    !.desired = IF state.candidatePanels = {} THEN NoPanel
                ELSE SomeLeaf(state.candidatePanels),
    !.applied = state.applied \/ (state.membershipOrigin = "panel"),
    !.membership = "none", !.membershipReq = 0, !.membershipOld = NoPanel,
    !.membershipOrigin = "none", !.refreshGen = 0, !.refreshRead = "none",
    !.candidate = "none", !.candidatePanels = {}, !.candidateRev = 0,
    !.leafless = FALSE,
    !.publicationDebt = ResolvePublicationDebt(state),
    !.publicationLate = FALSE,
    !.publicationInstalled = state.publicationInstalled
        \/ (state.closeConsumed /\ state.opTarget \notin state.candidatePanels)]

TimedStage ==
  s.stage \in {"queued", "in-flight", "cleanup", "installing",
                "attach-pending"}
ActiveFlow ==
  s.sample # "none" \/ s.stage # "none" \/ s.refreshGen # 0
    \/ s.otherStructural

ClockAdvance ==
  /\ Live /\ s.now < MaxTime
  /\ (s.sample # "none" \/ TimedStage)
  /\ s' = [s EXCEPT !.now = @ + 1]
HoldMailboxLock ==
  /\ Live /\ s.lockFree /\ ActiveFlow
  /\ s' = [s EXCEPT !.lockFree = FALSE]
ReleaseMailboxLock ==
  /\ Live /\ ~s.lockFree
  /\ s' = [s EXCEPT !.lockFree = TRUE]
QueueOrdinaryWork ==
  /\ Live /\ s.queued < MaxQueue /\ ActiveFlow
  /\ s' = [s EXCEPT !.queued = @ + 1]
DrainQueuedWork ==
  /\ Live /\ s.queued > 0 /\ ActiveFlow
  /\ s' = [s EXCEPT !.queued = @ - 1]
AdmitOtherStructural ==
  /\ Live /\ ~s.otherStructural /\ s.stage = "none" /\ s.refreshGen = 0
  /\ s.sample # "none"
  /\ s' = [s EXCEPT !.otherStructural = TRUE]
SettleOtherStructural ==
  /\ Live /\ s.otherStructural
  /\ s' = [s EXCEPT !.otherStructural = FALSE]

PeerCloseLeaf(leaf) ==
  /\ Live /\ s.serverTab /\ leaf \in s.serverPanels
  /\ s.peerSteps < MaxPeer /\ s.serverRev < MaxRevision
  /\ (ActiveFlow \/ s.commitHeld \/ s.closeConsumed)
  /\ LET rest == s.serverPanels \ {leaf} IN
       s' = [s EXCEPT !.serverPanels = rest, !.serverTab = rest # {},
         !.serverRev = @ + 1, !.peerSteps = @ + 1,
         !.publicationDebt = IF s.publicationDebt = "in-flight" THEN "in-flight"
                             ELSE "pending",
         !.publicationLate = s.publicationLate
                             \/ s.publicationDebt = "in-flight"]

SamplePanel ==
  /\ Live /\ s.stage = "none" /\ s.sample = "none" /\ ~s.opUsed
  /\ s' = [s EXCEPT !.sample = "panel",
       !.sampleDeadline = IF s.now <= MaxTime - Budget THEN s.now + Budget
                          ELSE -1]
RefusePanelAdmission ==
  /\ s.sample = "panel"
  /\ (s.sampleDeadline = -1 \/ s.now >= s.sampleDeadline \/ ~s.lockFree
       \/ s.otherStructural \/ s.refreshGen # 0 \/ s.stage # "none"
       \/ s.queued = MaxQueue \/ s.cursor = MaxRequest
       \/ s.selected = NoPanel)
  /\ s' = [s EXCEPT !.sample = "none"]
RefusePanelAfterLockExpiry ==
  /\ ~s.lockFree /\ s.sample = "panel" /\ s.sampleDeadline >= 0
  /\ s.now >= s.sampleDeadline
  /\ RefusePanelAdmission
RefuseMoveNoNeighbor ==
  /\ s.sample = "panel" /\ OpKind = "move"
  /\ Cardinality(s.cachePanels) < 2
  /\ s' = [s EXCEPT !.sample = "none", !.opUsed = TRUE]
AdmitPanel ==
  /\ s.sample = "panel" /\ s.lockFree /\ s.now < s.sampleDeadline
  /\ s.stage = "none" /\ ~s.otherStructural /\ s.refreshGen = 0
  /\ s.queued < MaxQueue /\ s.cursor < MaxRequest /\ ~s.opUsed
  /\ s.selected # NoPanel
  /\ (OpKind = "move" => Cardinality(s.cachePanels) >= 2)
  /\ s' = [s EXCEPT !.stage = "queued", !.sample = "none", !.opUsed = TRUE,
       !.cursor = @ + 1, !.serial = @ + 1, !.opReq = s.cursor + 1,
       !.opSerial = s.serial + 1, !.opBaselineRev = s.cacheRev,
       !.opBaselinePanels = s.cachePanels, !.opTarget = s.selected,
       !.opMoveTarget = IF OpKind = "move"
                        THEN SomeLeaf(s.cachePanels \ {s.selected})
                        ELSE NoPanel,
       !.opDeadline = s.sampleDeadline]

SubmitPanel ==
  /\ Live /\ s.stage = "queued" /\ BeforeDeadline /\ ~s.submitted
  /\ s' = [s EXCEPT !.stage = "in-flight", !.submitted = TRUE,
       !.submitCount = @ + 1]

ReceiveNotSubmitted ==
  /\ Live /\ s.stage = "in-flight" /\ BeforeDeadline /\ s.wire = "none"
  /\ s' = [s EXCEPT !.wire = "not-submitted"]
ReceivePrecommit ==
  /\ Live /\ s.stage = "in-flight" /\ BeforeDeadline /\ s.wire = "none"
  /\ s' = [s EXCEPT !.wire = "precommit"]
ReceiveUnchangedSame ==
  /\ Live /\ s.stage = "in-flight" /\ BeforeDeadline /\ s.wire = "none"
  /\ OpKind = "move" /\ s.serverRev = s.opBaselineRev
  /\ s' = [s EXCEPT !.wire = "unchanged-same"]
ReceiveUnchangedForeign ==
  /\ Live /\ s.stage = "in-flight" /\ BeforeDeadline /\ s.wire = "none"
  /\ OpKind = "move" /\ s.serverRev # s.opBaselineRev
  /\ s' = [s EXCEPT !.wire = "unchanged-foreign"]
ReceiveTabUnavailable ==
  /\ Live /\ s.stage = "in-flight" /\ BeforeDeadline /\ s.wire = "none"
  /\ OpKind = "move"
  /\ s' = [s EXCEPT !.wire = "tab-unavailable",
       !.tabUnavailRev = s.serverRev]
ReceiveCloseAccepted ==
  /\ Live /\ s.stage = "in-flight" /\ BeforeDeadline /\ s.wire = "none"
  /\ OpKind = "close"
  /\ s' = [s EXCEPT !.wire = "close-accepted"]
ReceiveCommitted(valid) ==
  /\ Live /\ s.stage = "in-flight" /\ BeforeDeadline /\ s.wire = "none"
  /\ OpKind \in {"split", "move"} /\ valid \in BOOLEAN
  /\ s.serverRev = s.opBaselineRev
  /\ s' = [s EXCEPT !.wire = "committed", !.commitHeld = TRUE,
       !.commitRev = s.opBaselineRev + 1,
       !.commitExpected = ExpectedAfter(s.opBaselinePanels),
       !.commitValid = valid,
       !.serverPanels = ExpectedAfter(s.opBaselinePanels),
       !.serverRev = @ + 1]
ReceiveAmbiguousOutcome ==
  /\ Live /\ s.stage = "in-flight" /\ s.submitted
  /\ s' = Retire("unknown", s)

ReadCommitSnapshotExact ==
  /\ Live /\ s.wire = "committed" /\ BeforeDeadline /\ s.cursor < MaxRequest
  /\ s.serverRev = s.commitRev
  /\ s' = [s EXCEPT !.cursor = @ + 1, !.candidate = "exact",
       !.candidatePanels = s.commitExpected, !.candidateRev = s.commitRev]
ReadCommitSnapshotSuperseding ==
  /\ Live /\ s.wire = "committed" /\ BeforeDeadline /\ s.cursor < MaxRequest
  /\ s.serverRev > s.commitRev
  /\ s' = [s EXCEPT !.cursor = @ + 1,
       !.candidate = IF ~s.serverTab THEN "tab-gone"
                     ELSE IF s.opTarget \in s.serverPanels
                     THEN "superseding-kept" ELSE "superseding-leafless",
       !.candidatePanels = s.serverPanels, !.candidateRev = s.serverRev]
ReadCommitSnapshotUnavailable ==
  /\ Live /\ s.wire = "committed" /\ BeforeDeadline /\ s.cursor < MaxRequest
  /\ s' = [s EXCEPT !.cursor = @ + 1, !.candidate = "unavailable"]
ReadCommitSnapshotDivergent ==
  /\ Live /\ s.wire = "committed" /\ BeforeDeadline /\ s.cursor < MaxRequest
  /\ s' = [s EXCEPT !.cursor = @ + 1, !.candidate = "divergent"]
SnapshotIdentityExhausted ==
  /\ Live /\ s.wire = "committed" /\ s.cursor = MaxRequest
  /\ s' = Retire("protocol", s)
ReadTabUnavailableSnapshot ==
  /\ Live /\ s.wire = "tab-unavailable" /\ BeforeDeadline
       /\ s.cursor < MaxRequest
  /\ \/ /\ ~s.serverTab /\ s.serverRev >= s.tabUnavailRev
        /\ s' = [s EXCEPT !.cursor = @ + 1, !.candidate = "tab-gone",
            !.candidatePanels = {}, !.candidateRev = s.serverRev]
     \/ /\ s.serverTab
        /\ s' = [s EXCEPT !.cursor = @ + 1, !.candidate = "divergent"]
     \/ s' = [s EXCEPT !.cursor = @ + 1, !.candidate = "unavailable"]

PlaceResult ==
  /\ Live /\ s.stage = "in-flight" /\ s.wire # "none" /\ BeforeDeadline
  /\ (s.wire \in {"committed", "tab-unavailable"} => s.candidate # "none")
  /\ s' = [s EXCEPT !.stage = "await-ui", !.result = s.wire,
       !.resultReq = s.opReq, !.resultSerial = s.opSerial]
RejectStaleResultDeposit ==
  /\ Live /\ s.stage \in {"in-flight", "await-ui"}
  /\ s' = [s EXCEPT !.staleRejected = TRUE]
ExpirePanelFlight ==
  /\ Live /\ s.stage \in {"queued", "in-flight"} /\ ~BeforeDeadline
  /\ s' = Retire("deadline", s)

ConsumeNotSubmitted ==
  /\ Live /\ ResultCurrent /\ s.result = "not-submitted"
  /\ s' = ClearResult(s)
ConsumePrecommit ==
  /\ Live /\ ResultCurrent /\ s.result = "precommit"
  /\ s' = ClearResult(s)
ConsumeUnchangedSame ==
  /\ Live /\ ResultCurrent /\ s.result = "unchanged-same"
  /\ s' = ClearResult(s)
ConsumeUnchangedForeign ==
  /\ Live /\ ResultCurrent /\ s.result = "unchanged-foreign"
  /\ s' = Retire("divergence", ClearResult(s))
ConsumeCloseAccepted ==
  /\ Live /\ ResultCurrent /\ s.result = "close-accepted"
  /\ s' = [ClearResult(s) EXCEPT !.closeConsumed = TRUE,
       !.publicationDebt = IF s.publicationDebt = "in-flight" THEN "in-flight"
                           ELSE "pending"]
ConsumeCommittedMerge ==
  /\ Live /\ ResultCurrent /\ s.result = "committed" /\ s.commitValid
  /\ s.candidate \in {"exact", "superseding-kept"}
  /\ s' = [ClearResult(s) EXCEPT !.cachePanels = s.candidatePanels,
       !.cacheRev = s.candidateRev, !.applied = TRUE]
ConsumeCommittedLeaflessRepair ==
  /\ Live /\ ResultCurrent /\ s.result = "committed" /\ s.commitValid
  /\ s.candidate = "superseding-leafless"
  /\ s' = [EnterMembership("panel", KeepCandidate(s)) EXCEPT
       !.leafless = TRUE]
ConsumeCommittedTabGone ==
  /\ Live /\ ResultCurrent /\ s.result = "committed" /\ s.commitValid
  /\ s.candidate = "tab-gone"
  /\ s' = EnterMembership("panel", KeepCandidate(s))
ConsumeCommittedUnavailable ==
  /\ Live /\ ResultCurrent /\ s.result = "committed" /\ s.commitValid
  /\ s.candidate = "unavailable"
  /\ s' = [ClearResult(s) EXCEPT !.stage = "recovery"]
ConsumeCommittedDivergent ==
  /\ Live /\ ResultCurrent /\ s.result = "committed" /\ s.commitValid
  /\ s.candidate = "divergent"
  /\ s' = Retire("divergence", ClearResult(s))
ConsumeInvalidGraftCommit ==
  /\ Live /\ ResultCurrent /\ s.result = "committed" /\ ~s.commitValid
  /\ s' = [Retire("divergence", ClearResult(s)) EXCEPT !.commitHeld = FALSE]
ConsumeTabUnavailableMembership ==
  /\ Live /\ ResultCurrent /\ s.result = "tab-unavailable"
  /\ s.candidate = "tab-gone"
  /\ s' = EnterMembership("panel", KeepCandidate(s))
ConsumeTabUnavailableDivergent ==
  /\ Live /\ ResultCurrent /\ s.result = "tab-unavailable"
  /\ s.candidate = "divergent"
  /\ s' = Retire("divergence", ClearResult(s))
ConsumeTabUnavailableNotApplied ==
  /\ Live /\ ResultCurrent /\ s.result = "tab-unavailable"
  /\ s.candidate = "unavailable"
  /\ s' = ClearResult(s)

ReleaseOldAttachment ==
  /\ Live /\ s.stage = "cleanup" /\ s.membership = "staged"
  /\ s' = [s EXCEPT !.attached = {}, !.membership = "cleaned",
       !.stage = "installing"]
FailMembershipCleanup ==
  /\ Live /\ s.stage = "cleanup"
  /\ s' = Retire("cleanup", s)
FailMembershipInstall ==
  /\ Live /\ s.stage = "installing"
  /\ s' = Retire("protocol", s)
ExpireMembership ==
  /\ Live /\ s.stage \in {"cleanup", "installing"}
  /\ s.membershipDeadline >= 0 /\ s.now >= s.membershipDeadline
  /\ s' = Retire("deadline", s)
RejectStaleMembershipReply ==
  /\ Live /\ s.membershipOrigin # "none"
  /\ s' = [s EXCEPT !.staleRejected = TRUE]

InstallMembershipAttach ==
  /\ Live /\ s.stage = "installing"
  /\ s.membership \in {"none", "cleaned"} /\ s.attached = {}
  /\ s.candidatePanels # {}
  /\ s' = [InstalledRecord(s) EXCEPT !.stage = "attach-pending"]
InstallMembershipSelect ==
  /\ Live /\ s.stage = "installing"
  /\ s.membership \in {"none", "cleaned"} /\ s.attached = {}
  /\ s.candidatePanels # {}
  /\ s' = [InstalledRecord(s) EXCEPT !.stage = "none"]
InstallMembershipEmpty ==
  /\ Live /\ s.stage = "installing"
  /\ s.membership \in {"none", "cleaned"} /\ s.attached = {}
  /\ s.candidatePanels = {}
  /\ s' = [InstalledRecord(s) EXCEPT !.stage = "none"]

CompleteAttach ==
  /\ Live /\ s.stage = "attach-pending"
  /\ s' = [s EXCEPT !.attached = {s.selected}, !.stage = "none"]
FailAttach ==
  /\ Live /\ s.stage = "attach-pending"
  /\ s' = Retire("protocol", s)
ExpireAttach ==
  /\ Live /\ s.stage = "attach-pending" /\ s.membershipDeadline >= 0
  /\ s.now >= s.membershipDeadline
  /\ s' = Retire("deadline", s)

AdmitRefresh ==
  /\ Live /\ s.stage \in {"none", "recovery"} /\ s.refreshGen = 0
  /\ ~s.otherStructural /\ s.refreshCount < MaxRefresh
  /\ s.cursor < MaxRequest
  /\ s' = [s EXCEPT !.refreshGen = s.refreshCount + 1,
       !.refreshCount = @ + 1, !.cursor = @ + 1,
       !.publicationDebt = IF s.publicationDebt = "pending" THEN "in-flight"
                           ELSE s.publicationDebt]
ReadRefreshAvailable ==
  /\ Live /\ s.refreshGen > 0 /\ s.refreshRead = "none"
  /\ s' = [s EXCEPT !.refreshRead = "available",
       !.candidate = IF ~s.serverTab THEN "tab-gone"
                     ELSE IF s.selected \in s.serverPanels
                     THEN "refreshed" ELSE "leafless",
       !.candidatePanels = s.serverPanels, !.candidateRev = s.serverRev]
ReadRefreshUnavailable ==
  /\ Live /\ s.refreshGen > 0 /\ s.refreshRead = "none"
  /\ s' = [s EXCEPT !.refreshRead = "unavailable"]
ReadRefreshDivergent ==
  /\ Live /\ s.refreshGen > 0 /\ s.refreshRead = "none"
  /\ s' = [s EXCEPT !.refreshRead = "divergent"]

ApplyRefreshMerge ==
  /\ Live /\ s.stage \in {"none", "recovery"}
  /\ s.refreshRead = "available" /\ s.candidate = "refreshed"
  /\ s' = [s EXCEPT !.refreshRead = "none", !.refreshGen = 0,
       !.cachePanels = s.candidatePanels, !.cacheRev = s.candidateRev,
       !.stage = IF s.stage = "recovery" THEN "none" ELSE s.stage,
       !.applied = s.applied \/ s.stage = "recovery",
       !.candidate = "none", !.candidatePanels = {}, !.candidateRev = 0,
       !.publicationDebt = ResolvePublicationDebt(s),
       !.publicationLate = FALSE,
       !.publicationInstalled = s.publicationInstalled
           \/ (s.closeConsumed /\ s.opTarget \notin s.candidatePanels)]
ApplyRefreshRepairLeafless ==
  /\ Live /\ s.stage \in {"none", "recovery"}
  /\ s.refreshRead = "available" /\ s.candidate = "leafless"
  /\ s' = [EnterMembership("refresh", s) EXCEPT
       !.leafless = TRUE, !.refreshRead = "none", !.candidate = "none"]
ApplyRefreshMembershipTabGone ==
  /\ Live /\ s.stage \in {"none", "recovery"}
  /\ s.refreshRead = "available" /\ s.candidate = "tab-gone"
  /\ s' = [EnterMembership("refresh", s) EXCEPT
       !.refreshRead = "none", !.candidate = "none"]
ApplyRefreshUnavailable ==
  /\ Live /\ s.stage \in {"none", "recovery"}
  /\ s.refreshRead = "unavailable"
  /\ s' = [s EXCEPT !.refreshRead = "none", !.refreshGen = 0,
       !.publicationDebt = IF s.publicationDebt = "in-flight" THEN "pending"
                           ELSE s.publicationDebt,
       !.publicationLate = FALSE]
ApplyRefreshDivergent ==
  /\ Live /\ s.stage \in {"none", "recovery"}
  /\ s.refreshRead = "divergent"
  /\ s' = [Retire("divergence", s) EXCEPT !.refreshRead = "none",
       !.refreshGen = 0]
RejectStaleRefreshConsume ==
  /\ Live /\ s.refreshGen > 0
  /\ s' = [s EXCEPT !.staleRejected = TRUE]

RequestSessionRetirement ==
  /\ Live /\ ~s.retired
  /\ (ActiveFlow \/ s.wire # "none" \/ s.membershipOrigin # "none")
  /\ s' = Retire("exit", s)
Exit ==
  /\ s.retired /\ ~s.closed
  /\ s' = [s EXCEPT !.closed = TRUE]

Next ==
  \/ ClockAdvance \/ HoldMailboxLock \/ ReleaseMailboxLock
  \/ QueueOrdinaryWork \/ DrainQueuedWork
  \/ AdmitOtherStructural \/ SettleOtherStructural
  \/ \E leaf \in Panels: PeerCloseLeaf(leaf)
  \/ SamplePanel \/ RefusePanelAdmission \/ RefusePanelAfterLockExpiry
  \/ RefuseMoveNoNeighbor \/ AdmitPanel
  \/ SubmitPanel
  \/ ReceiveNotSubmitted \/ ReceivePrecommit
  \/ ReceiveUnchangedSame \/ ReceiveUnchangedForeign
  \/ ReceiveTabUnavailable \/ ReceiveCloseAccepted
  \/ \E valid \in BOOLEAN: ReceiveCommitted(valid)
  \/ ReceiveAmbiguousOutcome
  \/ ReadCommitSnapshotExact \/ ReadCommitSnapshotSuperseding
  \/ ReadCommitSnapshotUnavailable \/ ReadCommitSnapshotDivergent
  \/ SnapshotIdentityExhausted \/ ReadTabUnavailableSnapshot
  \/ PlaceResult \/ RejectStaleResultDeposit \/ ExpirePanelFlight
  \/ ConsumeNotSubmitted \/ ConsumePrecommit
  \/ ConsumeUnchangedSame \/ ConsumeUnchangedForeign
  \/ ConsumeCloseAccepted
  \/ ConsumeCommittedMerge \/ ConsumeCommittedLeaflessRepair
  \/ ConsumeCommittedTabGone \/ ConsumeCommittedUnavailable
  \/ ConsumeCommittedDivergent \/ ConsumeInvalidGraftCommit
  \/ ConsumeTabUnavailableMembership \/ ConsumeTabUnavailableDivergent
  \/ ConsumeTabUnavailableNotApplied
  \/ ReleaseOldAttachment \/ FailMembershipCleanup \/ FailMembershipInstall
  \/ ExpireMembership \/ RejectStaleMembershipReply
  \/ InstallMembershipAttach \/ InstallMembershipSelect
  \/ InstallMembershipEmpty
  \/ CompleteAttach \/ FailAttach \/ ExpireAttach
  \/ AdmitRefresh \/ ReadRefreshAvailable \/ ReadRefreshUnavailable
  \/ ReadRefreshDivergent
  \/ ApplyRefreshMerge \/ ApplyRefreshRepairLeafless
  \/ ApplyRefreshMembershipTabGone \/ ApplyRefreshUnavailable
  \/ ApplyRefreshDivergent \/ RejectStaleRefreshConsume
  \/ RequestSessionRetirement \/ Exit

Spec == Init /\ [][Next]_vars
  /\ WF_vars(ClockAdvance)
  /\ WF_vars(ReleaseMailboxLock)
  /\ WF_vars(ExpirePanelFlight)
  /\ WF_vars(PlaceResult)
  /\ WF_vars(AdmitRefresh)
  /\ WF_vars(ConsumeNotSubmitted) /\ WF_vars(ConsumePrecommit)
  /\ WF_vars(ConsumeUnchangedSame) /\ WF_vars(ConsumeUnchangedForeign)
  /\ WF_vars(ConsumeCloseAccepted)
  /\ WF_vars(ConsumeCommittedMerge) /\ WF_vars(ConsumeCommittedLeaflessRepair)
  /\ WF_vars(ConsumeCommittedTabGone) /\ WF_vars(ConsumeCommittedUnavailable)
  /\ WF_vars(ConsumeCommittedDivergent) /\ WF_vars(ConsumeInvalidGraftCommit)
  /\ WF_vars(ConsumeTabUnavailableMembership)
  /\ WF_vars(ConsumeTabUnavailableDivergent)
  /\ WF_vars(ConsumeTabUnavailableNotApplied)
  /\ WF_vars(ReleaseOldAttachment) /\ WF_vars(FailMembershipCleanup)
  /\ WF_vars(FailMembershipInstall) /\ WF_vars(ExpireMembership)
  /\ WF_vars(InstallMembershipAttach) /\ WF_vars(InstallMembershipSelect)
  /\ WF_vars(InstallMembershipEmpty)
  /\ WF_vars(CompleteAttach) /\ WF_vars(FailAttach) /\ WF_vars(ExpireAttach)
  /\ WF_vars(ApplyRefreshMerge) /\ WF_vars(ApplyRefreshRepairLeafless)
  /\ WF_vars(ApplyRefreshMembershipTabGone) /\ WF_vars(ApplyRefreshUnavailable)
  /\ WF_vars(ApplyRefreshDivergent)

TypeOK ==
  /\ s.now \in 0..MaxTime /\ s.retired \in BOOLEAN /\ s.closed \in BOOLEAN
  /\ s.retireReason \in Reasons
  /\ s.lockFree \in BOOLEAN /\ s.queued \in 0..MaxQueue
  /\ s.otherStructural \in BOOLEAN
  /\ s.sample \in {"none", "panel"} /\ s.sampleDeadline \in -1..MaxTime
  /\ s.opUsed \in BOOLEAN /\ s.cursor \in 0..MaxRequest
  /\ s.serial \in 0..MaxRequest
  /\ s.stage \in Stages /\ s.opReq \in 0..MaxRequest
  /\ s.opSerial \in 0..MaxRequest /\ s.opBaselineRev \in 0..MaxRevision
  /\ s.opBaselinePanels \in SUBSET Panels
  /\ s.opTarget \in Panels \cup {NoPanel}
  /\ s.opMoveTarget \in Panels \cup {NoPanel}
  /\ s.opDeadline \in -1..MaxTime
  /\ s.submitted \in BOOLEAN /\ s.submitCount \in 0..1
  /\ s.commitHeld \in BOOLEAN /\ s.commitRev \in 0..MaxRevision
  /\ s.commitExpected \in SUBSET Panels /\ s.commitValid \in BOOLEAN
  /\ s.wire \in WireKinds /\ s.result \in WireKinds
  /\ s.resultReq \in 0..MaxRequest /\ s.resultSerial \in 0..MaxRequest
  /\ s.candidate \in Candidates
  /\ s.candidatePanels \in SUBSET Panels /\ s.candidateRev \in 0..MaxRevision
  /\ s.tabUnavailRev \in 0..MaxRevision
  /\ s.membership \in MembershipStages /\ s.membershipReq \in 0..MaxRequest
  /\ s.membershipOld \in Panels \cup {NoPanel}
  /\ s.membershipOrigin \in MembershipOrigins
  /\ s.membershipDeadline \in -1..(MaxTime + Budget)
  /\ s.attached \in SUBSET Panels /\ s.selected \in Panels \cup {NoPanel}
  /\ s.desired \in Panels \cup {NoPanel}
  /\ s.cachePanels \in SUBSET Panels /\ s.cacheRev \in 1..MaxRevision
  /\ s.serverTab \in BOOLEAN /\ s.serverPanels \in SUBSET Panels
  /\ s.serverRev \in 1..MaxRevision
  /\ s.peerSteps \in 0..MaxPeer
  /\ s.refreshGen \in 0..MaxRefresh /\ s.refreshCount \in 0..MaxRefresh
  /\ s.refreshRead \in Reads
  /\ s.publicationDebt \in Debt /\ s.publicationLate \in BOOLEAN
  /\ s.applied \in BOOLEAN /\ s.leafless \in BOOLEAN
  /\ s.staleRejected \in BOOLEAN /\ s.closeConsumed \in BOOLEAN
  /\ s.publicationInstalled \in BOOLEAN

SingleAttachment == Cardinality(s.attached) <= 1
AttachedIsSelected == s.attached \subseteq {s.selected}
SelectedIsKnownLeaf == (s.selected = NoPanel) <=> (s.cachePanels = {})
DesiredIsSelection == s.desired = s.selected
StructuralExclusion ==
  s.stage \in {"queued", "in-flight", "await-ui"} =>
    ~s.otherStructural /\ s.refreshGen = 0
OtherStructuralExcludesPanel ==
  s.otherStructural => s.stage = "none" /\ s.refreshGen = 0
PanelFlightFreezesCache ==
  s.stage \in {"queued", "in-flight", "await-ui"} =>
    /\ s.cacheRev = s.opBaselineRev /\ s.cachePanels = s.opBaselinePanels
    /\ s.desired = s.opTarget
OnlyCurrentResultPlaced ==
  s.stage = "await-ui" => s.resultReq = s.opReq /\ s.resultSerial = s.opSerial
CommittedRevisionIsSuccessor ==
  s.commitHeld => s.commitRev = s.opBaselineRev + 1
RecoveryRetainsCommit ==
  s.stage = "recovery" => s.commitHeld /\ s.commitValid
InvalidGraftNeverApplies == ~s.commitValid => ~s.applied
LeaflessRepairNotDivergence ==
  (s.leafless /\ ~s.applied) => ~(s.retired /\ s.retireReason = "divergence")
CloseReceiptKeepsSelection ==
  (s.closeConsumed /\ ~s.publicationInstalled) =>
    /\ s.selected = s.opTarget /\ s.attached \subseteq {s.opTarget}
MembershipCorrelationNamesRequest ==
  s.membershipOrigin = "panel" => s.membershipReq = s.opReq
PendingMembershipOwnsOld ==
  s.membership = "staged" => s.membershipOld \in s.attached
InstalledMembershipReleasedOld ==
  (s.stage = "installing" /\ s.membership = "cleaned") => s.attached = {}
SubmissionIsAtMostOnce == s.submitCount <= 1

AdmissionRefusalPreservesCore == [][
  s.sample = "panel" /\ s'.sample = "none" /\ s'.stage = s.stage =>
    /\ s'.cachePanels = s.cachePanels /\ s'.selected = s.selected
    /\ s'.attached = s.attached /\ s'.cacheRev = s.cacheRev
]_vars
PanelAdmissionCapturesIdentity == [][
  s.sample = "panel" /\ s'.stage = "queued" =>
    /\ s'.opReq = s.cursor + 1 /\ s'.opBaselineRev = s.cacheRev
    /\ s'.opTarget = s.selected /\ s'.opDeadline = s.sampleDeadline
]_vars
DepositCorrelatesCurrent == [][
  s.stage = "in-flight" /\ s'.stage = "await-ui" =>
    s'.resultReq = s.opReq /\ s'.resultSerial = s.opSerial
]_vars
UnchangedForeignDiverges == [][
  s.stage = "await-ui" /\ s.result = "unchanged-foreign"
    /\ s'.stage # "await-ui" =>
    s'.retired /\ s'.retireReason = "divergence"
]_vars
CloseAcceptKeepsSelection == [][
  s.stage = "await-ui" /\ s.result = "close-accepted" =>
    s'.selected = s.selected /\ s'.attached = s.attached
]_vars
CommittedFactIsRetained == [][
  s.commitHeld /\ s.commitValid => s'.commitHeld
]_vars
LeaflessSupersedingRepairs == [][
  s.stage = "await-ui" /\ s.result = "committed" /\ s.commitValid
    /\ s.candidate = "superseding-leafless" /\ s'.stage # "await-ui" =>
    s'.stage \in {"cleanup", "installing"} /\ ~s'.retired
]_vars
AttachRequiresReleasedOld == [][
  (\E leaf \in Panels: leaf \in s'.attached /\ leaf \notin s.attached) =>
    s.attached = {}
]_vars
NoReplayAfterAmbiguous == [][
  s.retireReason = "unknown" => s'.submitCount = s.submitCount
]_vars
RequestCursorNeverRewinds == [][s'.cursor >= s.cursor]_vars
PendingEventuallySettlesOrRetires ==
  (s.stage \in PendingStages) ~> (s.stage \notin PendingStages \/ s.retired)

=============================================================================
