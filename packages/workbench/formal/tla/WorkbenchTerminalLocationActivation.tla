---------------- MODULE WorkbenchTerminalLocationActivation ------------------
\* Exact structured location and presentation resolution is a trusted,
\* deterministic input boundary. ObserveReady records its normalized result.
\* Models the initiating-client focus intent created only by a pointer click on
\* an exact Workbench terminal location entry. Action settlement and the exact
\* destination terminal projection may arrive in either order; only the latest
\* live pointer request may emit one focus command after both have been observed.
EXTENDS FiniteSets, Naturals, TLC

RequestIds == 1..2
None == "none"
Phases == {"disposed", "idle", "waiting"}
Origins == {None, "other", "pointer"}
Settlements == {None, "pending", "accepted", "rejected"}

VARIABLES
  phase,
  pendingRequest,
  latestRequest,
  requestOrigin,
  settlement,
  activationAccepted,
  terminalProjectionReady,
  readyObserved,
  focusCount,
  focusLatestRequest,
  lastAction

vars ==
  <<phase, pendingRequest, latestRequest, requestOrigin, settlement,
    activationAccepted, terminalProjectionReady, readyObserved, focusCount,
    focusLatestRequest, lastAction>>

Init ==
  /\ phase = "disposed"
  /\ pendingRequest = 0
  /\ latestRequest = 0
  /\ requestOrigin = [request \in RequestIds |-> None]
  /\ settlement = [request \in RequestIds |-> None]
  /\ activationAccepted = FALSE
  /\ terminalProjectionReady = FALSE
  /\ readyObserved = {}
  /\ focusCount = [request \in RequestIds |-> 0]
  /\ focusLatestRequest = [request \in RequestIds |-> 0]
  /\ lastAction = "init"

Mount ==
  /\ phase = "disposed"
  /\ phase' = "idle"
  /\ lastAction' = "mount"
  /\ UNCHANGED
       <<pendingRequest, latestRequest, requestOrigin, settlement,
         activationAccepted, terminalProjectionReady, readyObserved,
         focusCount, focusLatestRequest>>

PointerActivate ==
  /\ phase \in {"idle", "waiting"}
  /\ latestRequest < Cardinality(RequestIds)
  /\ phase' = "waiting"
  /\ pendingRequest' = latestRequest + 1
  /\ latestRequest' = latestRequest + 1
  /\ requestOrigin' =
       [requestOrigin EXCEPT ![latestRequest + 1] = "pointer"]
  /\ settlement' =
       [settlement EXCEPT ![latestRequest + 1] = "pending"]
  /\ activationAccepted' = FALSE
  /\ terminalProjectionReady' = FALSE
  /\ lastAction' = "pointer-activate"
  /\ UNCHANGED <<readyObserved, focusCount, focusLatestRequest>>

NonPointerActivate ==
  /\ phase \in {"idle", "waiting"}
  /\ latestRequest < Cardinality(RequestIds)
  /\ phase' = "idle"
  /\ pendingRequest' = 0
  /\ latestRequest' = latestRequest + 1
  /\ requestOrigin' =
       [requestOrigin EXCEPT ![latestRequest + 1] = "other"]
  /\ settlement' =
       [settlement EXCEPT ![latestRequest + 1] = "pending"]
  /\ activationAccepted' = FALSE
  /\ terminalProjectionReady' = FALSE
  /\ lastAction' = "non-pointer-activate"
  /\ UNCHANGED <<readyObserved, focusCount, focusLatestRequest>>

SettleAccepted ==
  /\ phase = "waiting"
  /\ settlement[pendingRequest] = "pending"
  /\ settlement' =
       [settlement EXCEPT ![pendingRequest] = "accepted"]
  /\ activationAccepted' = TRUE
  /\ lastAction' = "settle-accepted"
  /\ UNCHANGED
       <<phase, pendingRequest, latestRequest, requestOrigin,
         terminalProjectionReady, readyObserved, focusCount,
         focusLatestRequest>>

SettleRejected ==
  /\ phase = "waiting"
  /\ settlement[pendingRequest] = "pending"
  /\ phase' = "idle"
  /\ settlement' =
       [settlement EXCEPT ![pendingRequest] = "rejected"]
  /\ pendingRequest' = 0
  /\ activationAccepted' = FALSE
  /\ terminalProjectionReady' = FALSE
  /\ lastAction' = "settle-rejected"
  /\ UNCHANGED
       <<latestRequest, requestOrigin, readyObserved, focusCount,
         focusLatestRequest>>

RejectStaleSettlement ==
  \E candidate \in RequestIds:
    /\ candidate <= latestRequest
    /\ settlement[candidate] = "pending"
    /\ ~(phase = "waiting" /\ pendingRequest = candidate)
    /\ \E outcome \in {"accepted", "rejected"}:
         /\ settlement' = [settlement EXCEPT ![candidate] = outcome]
         /\ lastAction' = "reject-stale-settlement"
         /\ UNCHANGED
              <<phase, pendingRequest, latestRequest, requestOrigin,
                activationAccepted, terminalProjectionReady, readyObserved,
                focusCount, focusLatestRequest>>

ObserveOrigin ==
  /\ phase = "waiting"
  /\ ~(activationAccepted /\ terminalProjectionReady)
  /\ terminalProjectionReady' = FALSE
  /\ lastAction' = "observe-origin"
  /\ UNCHANGED
       <<phase, pendingRequest, latestRequest, requestOrigin, settlement,
         activationAccepted, readyObserved, focusCount, focusLatestRequest>>

ObserveUnavailable ==
  /\ phase = "waiting"
  /\ ~(activationAccepted /\ terminalProjectionReady)
  /\ terminalProjectionReady' = FALSE
  /\ lastAction' = "observe-unavailable"
  /\ UNCHANGED
       <<phase, pendingRequest, latestRequest, requestOrigin, settlement,
         activationAccepted, readyObserved, focusCount, focusLatestRequest>>

ObserveReady ==
  /\ phase = "waiting"
  /\ terminalProjectionReady = FALSE
  /\ terminalProjectionReady' = TRUE
  /\ readyObserved' = readyObserved \cup {pendingRequest}
  /\ lastAction' = "observe-ready"
  /\ UNCHANGED
       <<phase, pendingRequest, latestRequest, requestOrigin, settlement,
         activationAccepted, focusCount, focusLatestRequest>>

ObserveSuperseded ==
  /\ phase = "waiting"
  /\ phase' = "idle"
  /\ pendingRequest' = 0
  /\ activationAccepted' = FALSE
  /\ terminalProjectionReady' = FALSE
  /\ lastAction' = "observe-superseded"
  /\ UNCHANGED
       <<latestRequest, requestOrigin, settlement, readyObserved, focusCount,
         focusLatestRequest>>

Focus ==
  /\ phase = "waiting"
  /\ activationAccepted
  /\ terminalProjectionReady
  /\ phase' = "idle"
  /\ focusCount' = [focusCount EXCEPT ![pendingRequest] = @ + 1]
  /\ focusLatestRequest' =
       [focusLatestRequest EXCEPT ![pendingRequest] = latestRequest]
  /\ pendingRequest' = 0
  /\ activationAccepted' = FALSE
  /\ terminalProjectionReady' = FALSE
  /\ lastAction' = "focus"
  /\ UNCHANGED <<latestRequest, requestOrigin, settlement, readyObserved>>

Dispose ==
  /\ phase # "disposed"
  /\ phase' = "disposed"
  /\ pendingRequest' = 0
  /\ activationAccepted' = FALSE
  /\ terminalProjectionReady' = FALSE
  /\ lastAction' = "dispose"
  /\ UNCHANGED
       <<latestRequest, requestOrigin, settlement, readyObserved, focusCount,
         focusLatestRequest>>

Next ==
  \/ Mount
  \/ PointerActivate
  \/ NonPointerActivate
  \/ SettleAccepted
  \/ SettleRejected
  \/ RejectStaleSettlement
  \/ ObserveOrigin
  \/ ObserveUnavailable
  \/ ObserveReady
  \/ ObserveSuperseded
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
  /\ requestOrigin \in [RequestIds -> Origins]
  /\ settlement \in [RequestIds -> Settlements]
  /\ activationAccepted \in BOOLEAN
  /\ terminalProjectionReady \in BOOLEAN
  /\ readyObserved \subseteq RequestIds
  /\ focusCount \in [RequestIds -> 0..1]
  /\ focusLatestRequest \in [RequestIds -> 0..Cardinality(RequestIds)]
  /\ lastAction \in
       {"init", "mount", "pointer-activate", "non-pointer-activate",
        "settle-accepted", "settle-rejected", "reject-stale-settlement",
        "observe-origin", "observe-unavailable", "observe-ready",
        "observe-superseded", "focus", "dispose"}

WaitingOwnsLatestPointerRequest ==
  phase = "waiting" =>
    /\ pendingRequest = latestRequest
    /\ pendingRequest \in RequestIds
    /\ requestOrigin[pendingRequest] = "pointer"

InactiveHasNoPendingIntent ==
  phase \in {"disposed", "idle"} =>
    /\ pendingRequest = 0
    /\ ~activationAccepted
    /\ ~terminalProjectionReady

AcceptedStateMatchesCurrentSettlement ==
  activationAccepted =>
    /\ phase = "waiting"
    /\ settlement[pendingRequest] = "accepted"

ReadyStateHasNormalizedProjectionEvidence ==
  terminalProjectionReady =>
    /\ phase = "waiting"
    /\ pendingRequest \in readyObserved

FocusRequiresLatestAcceptedReadyPointer ==
  \A request \in RequestIds:
    focusCount[request] = 1 =>
      /\ requestOrigin[request] = "pointer"
      /\ settlement[request] = "accepted"
      /\ request \in readyObserved
      /\ focusLatestRequest[request] = request

EachRequestFocusesAtMostOnce ==
  \A request \in RequestIds: focusCount[request] <= 1

AcceptedReadyRequestEventuallyResolves ==
  \A request \in RequestIds:
    []((/\ phase = "waiting"
        /\ pendingRequest = request
        /\ activationAccepted
        /\ terminalProjectionReady)
       ~> (\/ focusCount[request] = 1
           \/ pendingRequest # request))

=============================================================================
