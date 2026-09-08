---------------------- MODULE WorkbenchAgentNavigation ----------------------
\* Models one initiating client navigating from a Workspace Agent row to the
\* exact terminal presentation. Action settlement and exact projection
\* readiness may arrive in either order. A newer click replaces the older
\* request, and only the latest still-live request may emit one focus command.
EXTENDS FiniteSets, Naturals, TLC

RequestIds == 1..2
None == "none"
Phases == {"disposed", "idle", "waiting"}
Settlements == {None, "pending", "accepted", "rejected"}
ProjectionStates == {"pending", "ready"}

VARIABLES
  phase,
  pendingRequest,
  latestRequest,
  settlement,
  actionAccepted,
  projectionState,
  originObserved,
  targetPendingObserved,
  readyObserved,
  unavailableObserved,
  unrelatedObserved,
  focusCount,
  focusLatestRequest,
  lastAction

vars ==
  <<phase, pendingRequest, latestRequest, settlement, actionAccepted,
    projectionState, originObserved, targetPendingObserved, readyObserved,
    unavailableObserved, unrelatedObserved, focusCount, focusLatestRequest,
    lastAction>>

Init ==
  /\ phase = "disposed"
  /\ pendingRequest = 0
  /\ latestRequest = 0
  /\ settlement = [request \in RequestIds |-> None]
  /\ actionAccepted = FALSE
  /\ projectionState = "pending"
  /\ originObserved = {}
  /\ targetPendingObserved = {}
  /\ readyObserved = {}
  /\ unavailableObserved = {}
  /\ unrelatedObserved = {}
  /\ focusCount = [request \in RequestIds |-> 0]
  /\ focusLatestRequest = [request \in RequestIds |-> 0]
  /\ lastAction = "init"

Mount ==
  /\ phase = "disposed"
  /\ phase' = "idle"
  /\ lastAction' = "mount"
  /\ UNCHANGED
       <<pendingRequest, latestRequest, settlement, actionAccepted,
         projectionState, originObserved, targetPendingObserved, readyObserved,
         unavailableObserved, unrelatedObserved, focusCount,
         focusLatestRequest>>

Navigate ==
  /\ phase \in {"idle", "waiting"}
  /\ latestRequest < Cardinality(RequestIds)
  /\ phase' = "waiting"
  /\ pendingRequest' = latestRequest + 1
  /\ latestRequest' = latestRequest + 1
  /\ settlement' = [settlement EXCEPT ![latestRequest + 1] = "pending"]
  /\ actionAccepted' = FALSE
  /\ projectionState' = "pending"
  /\ lastAction' = "navigate"
  /\ UNCHANGED
       <<originObserved, targetPendingObserved, readyObserved,
         unavailableObserved, unrelatedObserved, focusCount,
         focusLatestRequest>>

ActionAccepted ==
  /\ phase = "waiting"
  /\ settlement[pendingRequest] = "pending"
  /\ settlement' = [settlement EXCEPT ![pendingRequest] = "accepted"]
  /\ actionAccepted' = TRUE
  /\ lastAction' = "action-accepted"
  /\ UNCHANGED
       <<phase, pendingRequest, latestRequest, projectionState,
         originObserved, targetPendingObserved, readyObserved,
         unavailableObserved, unrelatedObserved, focusCount,
         focusLatestRequest>>

ActionRejected ==
  /\ phase = "waiting"
  /\ settlement[pendingRequest] = "pending"
  /\ phase' = "idle"
  /\ settlement' = [settlement EXCEPT ![pendingRequest] = "rejected"]
  /\ pendingRequest' = 0
  /\ actionAccepted' = FALSE
  /\ projectionState' = "pending"
  /\ lastAction' = "action-rejected"
  /\ UNCHANGED
       <<latestRequest, originObserved, targetPendingObserved, readyObserved,
         unavailableObserved, unrelatedObserved, focusCount,
         focusLatestRequest>>

SettleStale ==
  \E candidate \in RequestIds:
    /\ candidate <= latestRequest
    /\ settlement[candidate] = "pending"
    /\ ~(phase = "waiting" /\ pendingRequest = candidate)
    /\ \E outcome \in {"accepted", "rejected"}:
         /\ settlement' = [settlement EXCEPT ![candidate] = outcome]
         /\ lastAction' = "settle-stale"
         /\ UNCHANGED
              <<phase, pendingRequest, latestRequest, actionAccepted,
                projectionState, originObserved, targetPendingObserved,
                readyObserved, unavailableObserved, unrelatedObserved,
                focusCount, focusLatestRequest>>

ObserveOrigin ==
  /\ phase = "waiting"
  /\ \/ projectionState = "ready"
     \/ pendingRequest \notin originObserved
  /\ projectionState' = "pending"
  /\ originObserved' = originObserved \cup {pendingRequest}
  /\ lastAction' = "observe-origin"
  /\ UNCHANGED
       <<phase, pendingRequest, latestRequest, settlement, actionAccepted,
         targetPendingObserved, readyObserved,
         unavailableObserved, unrelatedObserved, focusCount,
         focusLatestRequest>>

ObserveTargetPending ==
  /\ phase = "waiting"
  /\ \/ projectionState = "ready"
     \/ pendingRequest \notin targetPendingObserved
  /\ projectionState' = "pending"
  /\ targetPendingObserved' = targetPendingObserved \cup {pendingRequest}
  /\ lastAction' = "observe-target-pending"
  /\ UNCHANGED
       <<phase, pendingRequest, latestRequest, settlement, actionAccepted,
         originObserved, readyObserved,
         unavailableObserved, unrelatedObserved, focusCount,
         focusLatestRequest>>

ObserveReady ==
  /\ phase = "waiting"
  /\ projectionState = "pending"
  /\ projectionState' = "ready"
  /\ readyObserved' = readyObserved \cup {pendingRequest}
  /\ lastAction' = "observe-ready"
  /\ UNCHANGED
       <<phase, pendingRequest, latestRequest, settlement, actionAccepted,
         originObserved, targetPendingObserved, unavailableObserved,
         unrelatedObserved, focusCount, focusLatestRequest>>

ObserveUnavailable ==
  /\ phase = "waiting"
  /\ phase' = "idle"
  /\ unavailableObserved' = unavailableObserved \cup {pendingRequest}
  /\ pendingRequest' = 0
  /\ actionAccepted' = FALSE
  /\ projectionState' = "pending"
  /\ lastAction' = "observe-unavailable"
  /\ UNCHANGED
       <<latestRequest, settlement, originObserved, targetPendingObserved,
         readyObserved, unrelatedObserved, focusCount, focusLatestRequest>>

ObserveUnrelated ==
  /\ phase = "waiting"
  /\ phase' = "idle"
  /\ unrelatedObserved' = unrelatedObserved \cup {pendingRequest}
  /\ pendingRequest' = 0
  /\ actionAccepted' = FALSE
  /\ projectionState' = "pending"
  /\ lastAction' = "observe-unrelated"
  /\ UNCHANGED
       <<latestRequest, settlement, originObserved, targetPendingObserved,
         readyObserved, unavailableObserved, focusCount, focusLatestRequest>>

Focus ==
  /\ phase = "waiting"
  /\ actionAccepted
  /\ projectionState = "ready"
  /\ phase' = "idle"
  /\ focusCount' = [focusCount EXCEPT ![pendingRequest] = @ + 1]
  /\ focusLatestRequest' =
       [focusLatestRequest EXCEPT ![pendingRequest] = latestRequest]
  /\ pendingRequest' = 0
  /\ actionAccepted' = FALSE
  /\ projectionState' = "pending"
  /\ lastAction' = "focus"
  /\ UNCHANGED
       <<latestRequest, settlement, originObserved, targetPendingObserved,
         readyObserved, unavailableObserved, unrelatedObserved>>

Dispose ==
  /\ phase # "disposed"
  /\ phase' = "disposed"
  /\ pendingRequest' = 0
  /\ actionAccepted' = FALSE
  /\ projectionState' = "pending"
  /\ lastAction' = "dispose"
  /\ UNCHANGED
       <<latestRequest, settlement, originObserved, targetPendingObserved,
         readyObserved, unavailableObserved, unrelatedObserved, focusCount,
         focusLatestRequest>>

Next ==
  \/ Mount
  \/ Navigate
  \/ ActionAccepted
  \/ ActionRejected
  \/ SettleStale
  \/ ObserveOrigin
  \/ ObserveTargetPending
  \/ ObserveReady
  \/ ObserveUnavailable
  \/ ObserveUnrelated
  \/ Focus
  \/ Dispose

Spec ==
  /\ Init
  /\ [][Next]_vars
  /\ WF_vars(Focus)

TypeOK ==
  /\ phase \in Phases
  /\ pendingRequest \in 0..Cardinality(RequestIds)
  /\ latestRequest \in 0..Cardinality(RequestIds)
  /\ settlement \in [RequestIds -> Settlements]
  /\ actionAccepted \in BOOLEAN
  /\ projectionState \in ProjectionStates
  /\ originObserved \subseteq RequestIds
  /\ targetPendingObserved \subseteq RequestIds
  /\ readyObserved \subseteq RequestIds
  /\ unavailableObserved \subseteq RequestIds
  /\ unrelatedObserved \subseteq RequestIds
  /\ focusCount \in [RequestIds -> 0..1]
  /\ focusLatestRequest \in [RequestIds -> 0..Cardinality(RequestIds)]
  /\ lastAction \in
       {"init", "mount", "navigate", "action-accepted", "action-rejected",
        "settle-stale", "observe-origin", "observe-target-pending",
        "observe-ready", "observe-unavailable", "observe-unrelated", "focus",
        "dispose"}

WaitingOwnsLatestRequest ==
  phase = "waiting" =>
    /\ pendingRequest = latestRequest
    /\ pendingRequest \in RequestIds

InactiveHasNoPendingIntent ==
  phase \in {"disposed", "idle"} =>
    /\ pendingRequest = 0
    /\ ~actionAccepted
    /\ projectionState = "pending"

AcceptedStateMatchesCurrentSettlement ==
  actionAccepted =>
    /\ phase = "waiting"
    /\ settlement[pendingRequest] = "accepted"

ReadyStateHasExactProjectionEvidence ==
  projectionState = "ready" =>
    /\ phase = "waiting"
    /\ pendingRequest \in readyObserved

CurrentProjectionMatchesLatestObservation ==
  /\ lastAction \in {"observe-origin", "observe-target-pending"} =>
       projectionState = "pending"
  /\ lastAction = "observe-ready" => projectionState = "ready"

FocusRequiresLatestAcceptedReadyRequest ==
  \A request \in RequestIds:
    focusCount[request] = 1 =>
      /\ settlement[request] = "accepted"
      /\ request \in readyObserved
      /\ focusLatestRequest[request] = request

EachRequestFocusesAtMostOnce ==
  \A request \in RequestIds: focusCount[request] <= 1

InvalidatedRequestNeverFocuses ==
  \A request \in unavailableObserved \cup unrelatedObserved:
    focusCount[request] = 0

AcceptedReadyIntervalEventuallyFocusesOrEnds ==
  \A request \in RequestIds:
    []((/\ phase = "waiting"
        /\ pendingRequest = request
        /\ actionAccepted
        /\ projectionState = "ready")
       ~> (\/ focusCount[request] = 1
           \/ pendingRequest # request
           \/ projectionState # "ready"))

=============================================================================
