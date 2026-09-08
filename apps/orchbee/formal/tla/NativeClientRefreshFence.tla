-------------------------- MODULE NativeClientRefreshFence --------------------------
EXTENDS Naturals, Sequences, FiniteSets

CONSTANTS MaxQueue, MaxRefreshGeneration

ASSUME
  /\ MaxQueue \in Nat \ {0}
  /\ MaxRefreshGeneration \in Nat \ {0}

TabA == "tab-a"
TabB == "tab-b"
TabC == "tab-c"
NoTab == "no-tab"
Tabs == {TabA, TabB, TabC}

AttachmentA == [generation |-> 0, tab |-> TabA]
AttachmentB == [generation |-> 1, tab |-> TabB]
Attachments == {AttachmentA, AttachmentB}

ValidCaches == {
  <<TabA>>, <<TabA, TabB>>, <<TabB, TabA>>,
  <<TabA, TabB, TabC>>, <<TabA, TabC, TabB>>,
  <<TabB, TabA, TabC>>, <<TabB, TabC, TabA>>,
  <<TabC, TabA, TabB>>, <<TabC, TabB, TabA>>
}
CandidateCaches == ValidCaches \cup {<<TabB, TabA>>, <<TabA, TabB, TabB>>}

Phases == {
  "ready", "retryable-unavailable", "projection-unavailable",
  "recovered-started", "selected-no-attach", "retired", "closed"
}
NavigatorStatuses == {"idle", "refreshing", "success", "failure", "busy"}
CommandKinds == {"poll", "refresh", "control"}
Flights == {"idle", "polling", "waiting-draw", "feedback-ready", "feedback-in-flight"}
RefreshOutcomes == {"available", "failed"}
RecoveryOutcomes == {"none", "started", "non-started"}

Command(kind, generation) == [kind |-> kind, generation |-> generation]
CommandRecords == [kind : CommandKinds, generation : 0..MaxRefreshGeneration]

RefreshEvent(generation, outcome, candidate, revision) == [
  generation |-> generation,
  outcome |-> outcome,
  candidate |-> candidate,
  revision |-> revision
]
RefreshEventRecords == [
  generation : 1..MaxRefreshGeneration,
  outcome : RefreshOutcomes,
  candidate : CandidateCaches,
  revision : 1..4
]

ContainsTab(sequence, tab) ==
  \E index \in 1..Len(sequence): sequence[index] = tab

TabIndex(sequence, tab) ==
  IF tab = NoTab THEN 0
  ELSE CHOOSE index \in 1..Len(sequence): sequence[index] = tab

ValidReplacement(current, currentRevision, candidate, candidateRevision) ==
  /\ candidate \in ValidCaches
  /\ candidateRevision >= currentRevision
  /\ \A index \in 1..Len(current): ContainsTab(candidate, current[index])
  /\ (candidateRevision = currentRevision => candidate = current)

ValidRecoveryEvent(event) ==
  /\ event.revision >= 2
  /\ ContainsTab(event.candidate, TabB)
  /\ (event.revision = 2 => event.candidate = <<TabA, TabB>>)

RemapProvenance(current, provenance, candidate) ==
  [index \in 1..Len(candidate) |->
    IF ContainsTab(current, candidate[index])
    THEN provenance[TabIndex(current, candidate[index])]
    ELSE "binding-only"]

InitialProvenance(order, active, viewers) ==
  [index \in 1..Len(order) |->
    IF order[index] = active /\ viewers # {} THEN "started"
    ELSE IF order[index] = TabA THEN "started"
    ELSE IF order[index] = TabB THEN "unavailable"
    ELSE "binding-only"]

RecoveryProvenance(state, event) ==
  LET remapped == RemapProvenance(state.cache, state.provenance, event.candidate)
  IN IF state.phase = "projection-unavailable"
     THEN [remapped EXCEPT ![TabIndex(event.candidate, TabB)] =
       IF state.recoveryOutcome = "started" THEN "started" ELSE "unavailable"]
     ELSE remapped

ContainsKind(queue, kind) ==
  \E index \in 1..Len(queue): queue[index].kind = kind

MemberOf(sequence, value) ==
  \E index \in 1..Len(sequence): sequence[index] = value

CurrentQueueCount(state) ==
  Cardinality({index \in 1..Len(state.mailbox):
    state.mailbox[index].kind = "refresh"
      /\ state.mailbox[index].generation = state.pendingRefresh})

CurrentEventCount(state) ==
  Cardinality({index \in 1..Len(state.refreshEvents):
    state.refreshEvents[index].generation = state.pendingRefresh})

CurrentPendingCount(state) ==
  CurrentQueueCount(state)
    + (IF state.refreshInFlight = state.pendingRefresh
            /\ state.pendingRefresh # 0 THEN 1 ELSE 0)
    + CurrentEventCount(state)

CoreSnapshot(state) == [
  phase |-> state.phase,
  cache |-> state.cache,
  cacheRevision |-> state.cacheRevision,
  provenance |-> state.provenance,
  activeIndex |-> state.activeIndex,
  active |-> state.active,
  committedTarget |-> state.committedTarget,
  projection |-> state.projection,
  viewers |-> state.viewers,
  controllers |-> state.controllers,
  latestAdmitted |-> state.latestAdmitted,
  latestSettled |-> state.latestSettled,
  pendingRefresh |-> state.pendingRefresh
]

VisibleSnapshot(state) == [
  core |-> CoreSnapshot(state),
  navigatorStatus |-> state.navigatorStatus,
  recoveryOutcome |-> state.recoveryOutcome
]

PendingSnapshot(state) == [
  mailbox |-> state.mailbox,
  refreshInFlight |-> state.refreshInFlight,
  refreshEvents |-> state.refreshEvents,
  latestAdmitted |-> state.latestAdmitted,
  latestSettled |-> state.latestSettled,
  pendingRefresh |-> state.pendingRefresh
]

Retired(state) ==
  [state EXCEPT
    !.phase = "retired",
    !.carrierLive = FALSE,
    !.mailbox = <<>>,
    !.refreshInFlight = 0,
    !.refreshEvents = <<>>,
    !.pendingRefresh = 0,
    !.workerAwaitingUi = FALSE,
    !.controlDiscardedOnRetireSeen =
      @ \/ ContainsKind(state.mailbox, "control"),
    !.flight = "idle",
    !.viewers = {},
    !.controllers = {},
    !.projection = NoTab]

InitialStable(phaseValue, cacheValue, revisionValue, activeValue,
              viewerSet, controllerSet, projectionValue) == [
  phase |-> phaseValue,
  cache |-> cacheValue,
  cacheRevision |-> revisionValue,
  provenance |-> InitialProvenance(cacheValue, activeValue, viewerSet),
  activeIndex |-> TabIndex(cacheValue, activeValue),
  active |-> activeValue,
  committedTarget |-> NoTab,
  projection |-> projectionValue,
  viewers |-> viewerSet,
  controllers |-> controllerSet,
  anchorViewers |-> viewerSet,
  anchorControllers |-> controllerSet,
  mailbox |-> <<>>,
  flight |-> "idle",
  carrierLive |-> TRUE,
  refreshCursor |-> 0,
  latestAdmitted |-> 0,
  latestSettled |-> 0,
  pendingRefresh |-> 0,
  refreshInFlight |-> 0,
  refreshEvents |-> <<>>,
  workerAwaitingUi |-> FALSE,
  navigatorOpen |-> FALSE,
  navigatorStatus |-> "idle",
  recoveryOutcome |-> "none",
  busyCoreChanged |-> FALSE,
  staleVisibleChanged |-> FALSE,
  dismissalCanceled |-> FALSE,
  reopenDuplicated |-> FALSE,
  activeChanged |-> FALSE,
  controlQueuedWhileAwaitingUiSeen |-> FALSE,
  controlDeliveredAfterUiSeen |-> FALSE,
  controlDiscardedOnRetireSeen |-> FALSE
]

InitialRecovery(outcome) == [
  phase |-> "projection-unavailable",
  cache |-> <<TabA>>,
  cacheRevision |-> 1,
  provenance |-> <<"started">>,
  activeIndex |-> 0,
  active |-> NoTab,
  committedTarget |-> TabB,
  projection |-> NoTab,
  viewers |-> {},
  controllers |-> {},
  anchorViewers |-> {},
  anchorControllers |-> {},
  mailbox |-> <<>>,
  flight |-> "idle",
  carrierLive |-> TRUE,
  refreshCursor |-> 0,
  latestAdmitted |-> 0,
  latestSettled |-> 0,
  pendingRefresh |-> 0,
  refreshInFlight |-> 0,
  refreshEvents |-> <<>>,
  workerAwaitingUi |-> FALSE,
  navigatorOpen |-> FALSE,
  navigatorStatus |-> "idle",
  recoveryOutcome |-> outcome,
  busyCoreChanged |-> FALSE,
  staleVisibleChanged |-> FALSE,
  dismissalCanceled |-> FALSE,
  reopenDuplicated |-> FALSE,
  activeChanged |-> FALSE,
  controlQueuedWhileAwaitingUiSeen |-> FALSE,
  controlDeliveredAfterUiSeen |-> FALSE,
  controlDiscardedOnRetireSeen |-> FALSE
]

VARIABLE s
vars == <<s>>

Init ==
  \/ \E controllerSet \in {{}, {AttachmentA}}:
       s = InitialStable("ready", <<TabA>>, 1, TabA,
                         {AttachmentA}, controllerSet, TabA)
  \/ \E controllerSet \in {{}, {AttachmentB}}:
       s = InitialStable("ready", <<TabA, TabB>>, 2, TabB,
                         {AttachmentB}, controllerSet, TabB)
  \/ s = InitialStable("retryable-unavailable", <<TabA, TabB>>, 2, TabB,
                       {}, {}, NoTab)
  \/ s = InitialStable("ready", <<TabA, TabB, TabC>>, 3, TabA,
                       {AttachmentA}, {AttachmentA}, TabA)
  \/ \E outcome \in {"started", "non-started"}: s = InitialRecovery(outcome)

RefreshAdmissionPhase ==
  s.phase \in {"ready", "retryable-unavailable",
                "projection-unavailable", "selected-no-attach"}

OpenAndAdmitRefresh ==
  /\ s.carrierLive
  /\ ~s.navigatorOpen
  /\ s.pendingRefresh = 0
  /\ RefreshAdmissionPhase
  /\ Len(s.mailbox) < MaxQueue
  /\ s.refreshCursor < MaxRefreshGeneration
  /\ LET nextGeneration == s.refreshCursor + 1 IN
       s' = [s EXCEPT
         !.navigatorOpen = TRUE,
         !.navigatorStatus = "refreshing",
         !.refreshCursor = nextGeneration,
         !.latestAdmitted = nextGeneration,
         !.pendingRefresh = nextGeneration,
         !.mailbox = Append(@, Command("refresh", nextGeneration))]

OpenWhileRefreshPending ==
  /\ ~s.navigatorOpen
  /\ s.pendingRefresh # 0
  /\ LET next == [s EXCEPT
       !.navigatorOpen = TRUE,
       !.navigatorStatus = "refreshing"] IN
       s' = [next EXCEPT
         !.reopenDuplicated = @ \/ (PendingSnapshot(next) # PendingSnapshot(s))]

OpenNavigatorBusy ==
  /\ ~s.navigatorOpen
  /\ s.pendingRefresh = 0
  /\ ~(s.carrierLive
       /\ RefreshAdmissionPhase
       /\ Len(s.mailbox) < MaxQueue
       /\ s.refreshCursor < MaxRefreshGeneration)
  /\ LET next == [s EXCEPT
       !.navigatorOpen = TRUE,
       !.navigatorStatus = "busy"] IN
       s' = [next EXCEPT
         !.busyCoreChanged = @ \/ (CoreSnapshot(next) # CoreSnapshot(s))]

CloseNavigator ==
  /\ s.navigatorOpen
  /\ LET next == [s EXCEPT !.navigatorOpen = FALSE] IN
       s' = [next EXCEPT
         !.dismissalCanceled = @ \/ (PendingSnapshot(next) # PendingSnapshot(s))]

QueuePoll ==
  /\ s.carrierLive
  /\ s.phase = "ready"
  /\ s.viewers # {}
  /\ s.flight = "idle"
  /\ ~ContainsKind(s.mailbox, "poll")
  /\ Len(s.mailbox) < MaxQueue
  /\ s' = [s EXCEPT !.mailbox = Append(@, Command("poll", 0))]

BeginPoll ==
  /\ s.mailbox # <<>>
  /\ Head(s.mailbox).kind = "poll"
  /\ s.flight = "idle"
  /\ s.refreshInFlight = 0
  /\ s.pendingRefresh = 0
  /\ ~s.workerAwaitingUi
  /\ s' = [s EXCEPT
       !.mailbox = Tail(@),
       !.flight = "polling"]

PollProducesFeedback ==
  /\ s.flight = "polling"
  /\ s' = [s EXCEPT !.flight = "waiting-draw"]

DrawPollResult ==
  /\ s.flight = "waiting-draw"
  /\ s' = [s EXCEPT !.flight = "feedback-ready"]

SendCausalFeedback ==
  /\ s.flight = "feedback-ready"
  /\ s' = [s EXCEPT !.flight = "feedback-in-flight"]

AcceptCausalFeedback ==
  /\ s.flight = "feedback-in-flight"
  /\ s' = [s EXCEPT !.flight = "idle"]

\* One abstract post-marker control represents either an already accepted Input
\* or an authorized Resize. The bound intentionally keeps this model focused on
\* the Refresh/UI settlement fence rather than the full terminal command product.
AdmitPostRefreshControl ==
  /\ s.carrierLive
  /\ s.phase = "ready"
  /\ s.controllers # {}
  /\ s.pendingRefresh # 0
  /\ ~ContainsKind(s.mailbox, "control")
  /\ Len(s.mailbox) < MaxQueue
  /\ LET next == [s EXCEPT
       !.mailbox = Append(@, Command("control", s.pendingRefresh))] IN
       s' = [next EXCEPT
         !.controlQueuedWhileAwaitingUiSeen =
           @ \/ s.workerAwaitingUi]

QueuePostRefreshControl ==
  /\ ~s.workerAwaitingUi
  /\ AdmitPostRefreshControl

QueuePostRefreshControlWhileAwaitingUi ==
  /\ s.workerAwaitingUi
  /\ AdmitPostRefreshControl

DeliverPostRefreshControl ==
  /\ s.mailbox # <<>>
  /\ Head(s.mailbox).kind = "control"
  /\ ~s.workerAwaitingUi
  /\ s.refreshInFlight = 0
  /\ s.flight = "idle"
  /\ Head(s.mailbox).generation <= s.latestSettled
  /\ s' = [s EXCEPT
       !.mailbox = Tail(@),
       !.controlDeliveredAfterUiSeen = TRUE]

IssueRefresh ==
  /\ s.mailbox # <<>>
  /\ Head(s.mailbox).kind = "refresh"
  /\ s.flight = "idle"
  /\ s.refreshInFlight = 0
  /\ Head(s.mailbox).generation = s.pendingRefresh
  /\ s' = [s EXCEPT
       !.mailbox = Tail(@),
       !.refreshInFlight = Head(s.mailbox).generation]

CompleteRefresh(outcome, candidate, revision) ==
  /\ outcome \in RefreshOutcomes
  /\ candidate \in CandidateCaches
  /\ revision \in 1..4
  /\ s.refreshInFlight = s.pendingRefresh
  /\ s.pendingRefresh # 0
  /\ Len(s.refreshEvents) < MaxQueue
  /\ s' = [s EXCEPT
       !.refreshEvents = Append(@,
         RefreshEvent(s.refreshInFlight, outcome, candidate, revision)),
       !.refreshInFlight = 0,
       !.workerAwaitingUi = TRUE]

InjectDefensiveStaleCompletion ==
  /\ s.pendingRefresh \in 2..MaxRefreshGeneration
  /\ Len(s.refreshEvents) < MaxQueue
  /\ s' = [s EXCEPT
       !.refreshEvents = Append(@,
         RefreshEvent(s.pendingRefresh - 1, "available", <<TabB, TabA>>, 1))]

RefreshTransportAmbiguous ==
  /\ s.refreshInFlight = s.pendingRefresh
  /\ s.pendingRefresh # 0
  /\ s' = Retired(s)

DropStaleRefresh ==
  /\ s.refreshEvents # <<>>
  /\ Head(s.refreshEvents).generation # s.pendingRefresh
  /\ LET next == [s EXCEPT !.refreshEvents = Tail(@)] IN
       s' = [next EXCEPT
         !.staleVisibleChanged = @ \/ (VisibleSnapshot(next) # VisibleSnapshot(s))]

ApplyCurrentRefreshFailure ==
  /\ s.refreshEvents # <<>>
  /\ Head(s.refreshEvents).generation = s.pendingRefresh
  /\ Head(s.refreshEvents).outcome = "failed"
  /\ s' = [s EXCEPT
       !.refreshEvents = Tail(@),
       !.latestSettled = s.pendingRefresh,
       !.pendingRefresh = 0,
       !.workerAwaitingUi = FALSE,
       !.navigatorStatus = "failure"]

ApplyCurrentRefreshAvailable ==
  /\ s.refreshEvents # <<>>
  /\ LET event == Head(s.refreshEvents) IN
       /\ event.generation = s.pendingRefresh
       /\ event.outcome = "available"
       /\ ValidReplacement(s.cache, s.cacheRevision,
                           event.candidate, event.revision)
       /\ (s.phase = "projection-unavailable" => ValidRecoveryEvent(event))
       /\ LET next == [s EXCEPT
            !.refreshEvents = Tail(@),
            !.cache = event.candidate,
            !.cacheRevision = event.revision,
            !.provenance = RecoveryProvenance(s, event),
            !.activeIndex = TabIndex(event.candidate,
              IF s.phase = "projection-unavailable" THEN TabB ELSE s.active),
            !.latestSettled = s.pendingRefresh,
            !.pendingRefresh = 0,
            !.workerAwaitingUi = FALSE,
            !.navigatorStatus = "success",
            !.active = IF s.phase = "projection-unavailable" THEN TabB ELSE @,
            !.committedTarget = IF s.phase = "projection-unavailable" THEN NoTab ELSE @,
            !.phase = IF s.phase = "projection-unavailable"
                      THEN IF s.recoveryOutcome = "started"
                           THEN "recovered-started"
                           ELSE "selected-no-attach"
                      ELSE @] IN
            s' = [next EXCEPT
              !.activeChanged = @ \/
                (s.phase # "projection-unavailable" /\ next.active # s.active)]

ApplyMovedRefresh ==
  /\ s.refreshEvents # <<>>
  /\ s.phase # "projection-unavailable"
  /\ LET event == Head(s.refreshEvents) IN
       /\ event.candidate \in ValidCaches
       /\ ContainsTab(event.candidate, s.active)
       /\ TabIndex(event.candidate, s.active) # s.activeIndex
       /\ ApplyCurrentRefreshAvailable

ApplyMovedCreateRecovery ==
  /\ s.refreshEvents # <<>>
  /\ s.phase = "projection-unavailable"
  /\ LET event == Head(s.refreshEvents) IN
       /\ event.revision > 2
       /\ event.candidate \in ValidCaches
       /\ ContainsTab(event.candidate, TabB)
       /\ TabIndex(event.candidate, TabB) # 2
       /\ ApplyCurrentRefreshAvailable

RejectCurrentRefreshDivergence ==
  /\ s.refreshEvents # <<>>
  /\ LET event == Head(s.refreshEvents) IN
       /\ event.generation = s.pendingRefresh
       /\ event.outcome = "available"
       /\ ~(ValidReplacement(s.cache, s.cacheRevision,
                             event.candidate, event.revision)
            /\ (s.phase = "projection-unavailable" => ValidRecoveryEvent(event)))
       /\ s' = Retired(s)

ExpireRefreshTransportDeadline ==
  /\ s.pendingRefresh # 0
  /\ (CurrentQueueCount(s) = 1 \/ s.refreshInFlight = s.pendingRefresh)
  /\ s' = Retired(s)

ConsumeRefreshEvent ==
  \/ DropStaleRefresh
  \/ ApplyCurrentRefreshFailure
  \/ ApplyCurrentRefreshAvailable
  \/ RejectCurrentRefreshDivergence

Exit ==
  /\ s.phase # "closed"
  /\ s' = [Retired(s) EXCEPT !.phase = "closed"]

Next ==
  \/ OpenAndAdmitRefresh
  \/ OpenWhileRefreshPending
  \/ OpenNavigatorBusy
  \/ CloseNavigator
  \/ QueuePoll
  \/ BeginPoll
  \/ PollProducesFeedback
  \/ DrawPollResult
  \/ SendCausalFeedback
  \/ AcceptCausalFeedback
  \/ QueuePostRefreshControl
  \/ QueuePostRefreshControlWhileAwaitingUi
  \/ DeliverPostRefreshControl
  \/ IssueRefresh
  \/ \E outcome \in RefreshOutcomes:
       \E candidate \in CandidateCaches:
         \E revision \in 1..4:
           CompleteRefresh(outcome, candidate, revision)
  \/ InjectDefensiveStaleCompletion
  \/ RefreshTransportAmbiguous
  \/ ConsumeRefreshEvent
  \/ ApplyMovedRefresh
  \/ ApplyMovedCreateRecovery
  \/ ExpireRefreshTransportDeadline
  \/ Exit

Spec ==
  /\ Init
  /\ [][Next]_vars
  /\ WF_vars(ExpireRefreshTransportDeadline)
  /\ WF_vars(ConsumeRefreshEvent)

TypeOK ==
  /\ s.phase \in Phases
  /\ s.cache \in ValidCaches
  /\ s.cacheRevision \in 1..4
  /\ s.provenance \in Seq({"started", "unavailable", "binding-only"})
  /\ Len(s.provenance) = Len(s.cache)
  /\ s.activeIndex \in 0..3
  /\ s.active \in Tabs \cup {NoTab}
  /\ s.committedTarget \in Tabs \cup {NoTab}
  /\ s.projection \in Tabs \cup {NoTab}
  /\ s.viewers \subseteq Attachments
  /\ s.controllers \subseteq Attachments
  /\ s.anchorViewers \subseteq Attachments
  /\ s.anchorControllers \subseteq Attachments
  /\ s.mailbox \in Seq(CommandRecords)
  /\ Len(s.mailbox) <= MaxQueue
  /\ s.flight \in Flights
  /\ s.carrierLive \in BOOLEAN
  /\ s.refreshCursor \in 0..MaxRefreshGeneration
  /\ s.latestAdmitted \in 0..MaxRefreshGeneration
  /\ s.latestSettled \in 0..MaxRefreshGeneration
  /\ s.pendingRefresh \in 0..MaxRefreshGeneration
  /\ s.refreshInFlight \in 0..MaxRefreshGeneration
  /\ s.refreshEvents \in Seq(RefreshEventRecords)
  /\ Len(s.refreshEvents) <= MaxQueue
  /\ s.workerAwaitingUi \in BOOLEAN
  /\ s.navigatorOpen \in BOOLEAN
  /\ s.navigatorStatus \in NavigatorStatuses
  /\ s.recoveryOutcome \in RecoveryOutcomes
  /\ s.busyCoreChanged \in BOOLEAN
  /\ s.staleVisibleChanged \in BOOLEAN
  /\ s.dismissalCanceled \in BOOLEAN
  /\ s.reopenDuplicated \in BOOLEAN
  /\ s.activeChanged \in BOOLEAN
  /\ s.controlQueuedWhileAwaitingUiSeen \in BOOLEAN
  /\ s.controlDeliveredAfterUiSeen \in BOOLEAN
  /\ s.controlDiscardedOnRetireSeen \in BOOLEAN

AtMostOneViewerAndController ==
  /\ Cardinality(s.viewers) <= 1
  /\ Cardinality(s.controllers) <= 1
  /\ s.controllers \subseteq s.viewers

AtMostOnePendingRefresh ==
  /\ CurrentPendingCount(s) <= 1
  /\ (s.pendingRefresh = 0 => CurrentPendingCount(s) = 0)
  /\ (s.pendingRefresh # 0 => CurrentPendingCount(s) = 1)

RefreshGenerationsNeverRegress ==
  /\ s.latestSettled <= s.latestAdmitted
  /\ s.latestAdmitted <= s.refreshCursor

CacheBackedActiveOrExplicitCommittedTarget ==
  \/ /\ MemberOf(s.cache, s.active)
     /\ s.committedTarget = NoTab
  \/ /\ s.active = NoTab
     /\ s.committedTarget = TabB
     /\ s.recoveryOutcome # "none"
     /\ s.phase \in {"projection-unavailable", "retired", "closed"}

RefreshNeverChangesAttachment ==
  s.carrierLive =>
    /\ s.viewers = s.anchorViewers
    /\ s.controllers = s.anchorControllers

BusyOpeningPreservesCore == ~s.busyCoreChanged
StaleCompletionIsInvisible == ~s.staleVisibleChanged
DismissalDoesNotCancelRefresh == ~s.dismissalCanceled
ReopenDoesNotDuplicateRefresh == ~s.reopenDuplicated
NonRecoveryRefreshPreservesExactActive == ~s.activeChanged

RecoveredCommittedTargetIsExact ==
  s.phase \in {"recovered-started", "selected-no-attach"} /\ s.active = TabB =>
    /\ Len(s.cache) >= 2
    /\ ContainsTab(s.cache, TabB)
    /\ s.cacheRevision >= 2
    /\ s.committedTarget = NoTab

NonStartedRecoveryDoesNotAttach ==
  s.recoveryOutcome = "non-started" => AttachmentB \notin s.viewers

AwaitingUiNamesCurrentResult ==
  s.workerAwaitingUi =>
    /\ s.pendingRefresh # 0
    /\ CurrentEventCount(s) = 1
    /\ s.refreshInFlight = 0

PendingUiSettlementBlocksExecutor ==
  s.workerAwaitingUi =>
    /\ ~ENABLED BeginPoll
    /\ ~ENABLED DeliverPostRefreshControl

AcceptedPostRefreshControlsWaitForSettlement ==
  \A index \in 1..Len(s.mailbox):
    s.mailbox[index].kind = "control" =>
      s.mailbox[index].generation > s.latestSettled \/ ~s.workerAwaitingUi

DeliveredControlsFollowUiSettlement ==
  s.controlDeliveredAfterUiSeen => s.latestSettled > 0

RetirementDiscardsQueuedControl ==
  ~s.carrierLive => ~ContainsKind(s.mailbox, "control")

PendingRefreshEventuallySettlesOrRetires ==
  (s.pendingRefresh # 0) ~> (s.pendingRefresh = 0)


ActiveIndexIsDerived ==
  s.activeIndex = TabIndex(s.cache, s.active)

OldProvenanceFollowsIdentity == [][
  s.cache' # s.cache =>
    \A index \in 1..Len(s.cache):
      s.provenance'[TabIndex(s.cache', s.cache[index])] = s.provenance[index]
]_vars

=============================================================================
