------------------- MODULE WorkspaceCreationActivation -------------------
\* Refines the initiating-client Workspace activation workflow in
\* docs/project-workspace.md. A successful result may arrive before or after
\* the canonical Workspace publication; only the latest exact published target
\* may produce one client-local activation command.
EXTENDS FiniteSets, Naturals, Sequences, TLC

Targets == {"project-1/workspace-2", "project-1/workspace-3"}
RequestIds == 1..2
None == "none"
Phases == {"disposed", "idle", "waiting"}

VARIABLES
  phase,
  target,
  pendingRequest,
  published,
  latestRequest,
  requestTarget,
  activationCount,
  lastAction

vars ==
  <<phase, target, pendingRequest, published, latestRequest, requestTarget,
    activationCount, lastAction>>

Init ==
  /\ phase = "disposed"
  /\ target = None
  /\ pendingRequest = 0
  /\ published = {}
  /\ latestRequest = 0
  /\ requestTarget = [request \in RequestIds |-> None]
  /\ activationCount = [request \in RequestIds |-> 0]
  /\ lastAction = "init"

Mount ==
  /\ phase = "disposed"
  /\ phase' = "idle"
  /\ lastAction' = "mount"
  /\ UNCHANGED
       <<target, pendingRequest, published, latestRequest, requestTarget,
         activationCount>>

Request(candidate) ==
  /\ phase \in {"idle", "waiting"}
  /\ candidate \in Targets
  /\ target # candidate
  /\ latestRequest < Cardinality(RequestIds)
  /\ phase' = "waiting"
  /\ target' = candidate
  /\ pendingRequest' = latestRequest + 1
  /\ latestRequest' = latestRequest + 1
  /\ requestTarget' = [requestTarget EXCEPT ![latestRequest + 1] = candidate]
  /\ lastAction' = "request"
  /\ UNCHANGED <<published, activationCount>>

RepeatRequest(candidate) ==
  /\ phase = "waiting"
  /\ candidate = target
  /\ lastAction' = "repeat-request"
  /\ UNCHANGED
       <<phase, target, pendingRequest, published, latestRequest,
         requestTarget, activationCount>>

Publish(candidate) ==
  /\ candidate \in Targets
  /\ candidate \notin published
  /\ published' = published \cup {candidate}
  /\ lastAction' = "publish"
  /\ UNCHANGED
       <<phase, target, pendingRequest, latestRequest, requestTarget,
         activationCount>>

IgnoreAbsentResult ==
  /\ phase # "disposed"
  /\ lastAction' = "ignore-absent-result"
  /\ UNCHANGED
       <<phase, target, pendingRequest, published, latestRequest,
         requestTarget, activationCount>>

ObserveWaiting ==
  /\ phase = "waiting"
  /\ target \notin published
  /\ lastAction' = "observe-waiting"
  /\ UNCHANGED
       <<phase, target, pendingRequest, published, latestRequest,
         requestTarget, activationCount>>

ObserveActivate ==
  /\ phase = "waiting"
  /\ target \in published
  /\ phase' = "idle"
  /\ target' = None
  /\ activationCount' =
       [activationCount EXCEPT ![pendingRequest] = @ + 1]
  /\ pendingRequest' = 0
  /\ lastAction' = "observe-activate"
  /\ UNCHANGED <<published, latestRequest, requestTarget>>

Dispose ==
  /\ phase # "disposed"
  /\ phase' = "disposed"
  /\ target' = None
  /\ pendingRequest' = 0
  /\ lastAction' = "dispose"
  /\ UNCHANGED
       <<published, latestRequest, requestTarget, activationCount>>

Next ==
  \/ Mount
  \/ \E candidate \in Targets: Request(candidate)
  \/ \E candidate \in Targets: RepeatRequest(candidate)
  \/ \E candidate \in Targets: Publish(candidate)
  \/ IgnoreAbsentResult
  \/ ObserveWaiting
  \/ ObserveActivate
  \/ Dispose

Spec ==
  /\ Init
  /\ [][Next]_vars
  /\ WF_vars(ObserveActivate)

TypeOK ==
  /\ phase \in Phases
  /\ target \in Targets \cup {None}
  /\ pendingRequest \in 0..Cardinality(RequestIds)
  /\ published \subseteq Targets
  /\ latestRequest \in 0..Cardinality(RequestIds)
  /\ requestTarget \in [RequestIds -> Targets \cup {None}]
  /\ activationCount \in [RequestIds -> 0..1]
  /\ lastAction \in
       {"init", "mount", "request", "repeat-request", "publish",
        "ignore-absent-result", "observe-waiting", "observe-activate",
        "dispose"}

WaitingOwnsLatestRequest ==
  phase = "waiting" =>
    /\ pendingRequest = latestRequest
    /\ pendingRequest \in RequestIds
    /\ target = requestTarget[pendingRequest]

InactiveHasNoPendingIntent ==
  phase \in {"disposed", "idle"} =>
    /\ target = None
    /\ pendingRequest = 0

ActivationRequiresExactPublication ==
  \A request \in RequestIds:
    activationCount[request] = 1 => requestTarget[request] \in published

EachRequestActivatesAtMostOnce ==
  \A request \in RequestIds: activationCount[request] <= 1

PublishedPendingRequestEventuallyResolves ==
  \A request \in RequestIds:
    []((/\ phase = "waiting"
        /\ pendingRequest = request
        /\ target \in published)
       ~> (\/ activationCount[request] = 1
           \/ latestRequest > request
           \/ phase = "disposed"))

=============================================================================
