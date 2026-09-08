------------------- MODULE WorkspaceCreationActivation -------------------
\* Refines the initiating-client Workspace activation workflow in
\* docs/project-workspace.md. A successful result may arrive before or after
\* the current canonical Workspace projection; only the latest exact target
\* in that current projection may produce one client-local activation command.
\* Each Target atom is an injective normalized (projectId, workspaceId) pair.
EXTENDS FiniteSets, Naturals, Sequences, TLC

Targets == {"project-1/workspace-2", "project-1/workspace-3"}
RequestIds == 1..2
None == "none"
Phases == {"disposed", "idle", "waiting"}

VARIABLES
  phase,
  target,
  pendingRequest,
  currentTargets,
  latestRequest,
  requestTarget,
  activationCount,
  activationProjection,
  lastAction

vars ==
  <<phase, target, pendingRequest, currentTargets, latestRequest, requestTarget,
    activationCount, activationProjection, lastAction>>

Init ==
  /\ phase = "disposed"
  /\ target = None
  /\ pendingRequest = 0
  /\ currentTargets = {}
  /\ latestRequest = 0
  /\ requestTarget = [request \in RequestIds |-> None]
  /\ activationCount = [request \in RequestIds |-> 0]
  /\ activationProjection = [request \in RequestIds |-> {}]
  /\ lastAction = "init"

Mount ==
  /\ phase = "disposed"
  /\ phase' = "idle"
  /\ lastAction' = "mount"
  /\ UNCHANGED
       <<target, pendingRequest, currentTargets, latestRequest, requestTarget,
         activationCount, activationProjection>>

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
  /\ UNCHANGED <<currentTargets, activationCount, activationProjection>>

RepeatRequest(candidate) ==
  /\ phase = "waiting"
  /\ candidate = target
  /\ lastAction' = "repeat-request"
  /\ UNCHANGED
       <<phase, target, pendingRequest, currentTargets, latestRequest,
         requestTarget, activationCount, activationProjection>>

Publish(nextTargets) ==
  /\ nextTargets \subseteq Targets
  /\ nextTargets # currentTargets
  /\ currentTargets' = nextTargets
  /\ lastAction' = "publish"
  /\ UNCHANGED
       <<phase, target, pendingRequest, latestRequest, requestTarget,
         activationCount, activationProjection>>

IgnoreAbsentResult ==
  /\ phase # "disposed"
  /\ lastAction' = "ignore-absent-result"
  /\ UNCHANGED
       <<phase, target, pendingRequest, currentTargets, latestRequest,
         requestTarget, activationCount, activationProjection>>

ObserveWaiting ==
  /\ phase = "waiting"
  /\ target \notin currentTargets
  /\ lastAction' = "observe-waiting"
  /\ UNCHANGED
       <<phase, target, pendingRequest, currentTargets, latestRequest,
         requestTarget, activationCount, activationProjection>>

ObserveActivate ==
  /\ phase = "waiting"
  /\ target \in currentTargets
  /\ phase' = "idle"
  /\ target' = None
  /\ activationCount' =
       [activationCount EXCEPT ![pendingRequest] = @ + 1]
  /\ activationProjection' =
       [activationProjection EXCEPT ![pendingRequest] = currentTargets]
  /\ pendingRequest' = 0
  /\ lastAction' = "observe-activate"
  /\ UNCHANGED <<currentTargets, latestRequest, requestTarget>>

Dispose ==
  /\ phase # "disposed"
  /\ phase' = "disposed"
  /\ target' = None
  /\ pendingRequest' = 0
  /\ lastAction' = "dispose"
  /\ UNCHANGED
       <<currentTargets, latestRequest, requestTarget, activationCount,
         activationProjection>>

Next ==
  \/ Mount
  \/ \E candidate \in Targets: Request(candidate)
  \/ \E candidate \in Targets: RepeatRequest(candidate)
  \/ \E nextTargets \in SUBSET Targets: Publish(nextTargets)
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
  /\ currentTargets \subseteq Targets
  /\ latestRequest \in 0..Cardinality(RequestIds)
  /\ requestTarget \in [RequestIds -> Targets \cup {None}]
  /\ activationCount \in [RequestIds -> 0..1]
  /\ activationProjection \in [RequestIds -> SUBSET Targets]
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
    activationCount[request] = 1 =>
      requestTarget[request] \in activationProjection[request]

EachRequestActivatesAtMostOnce ==
  \A request \in RequestIds: activationCount[request] <= 1

PublishedPendingRequestEventuallyResolves ==
  \A request \in RequestIds:
    []((/\ phase = "waiting"
        /\ pendingRequest = request
        /\ target \in currentTargets)
       ~> (\/ activationCount[request] = 1
           \/ latestRequest > request
           \/ phase = "disposed"
           \/ target \notin currentTargets))

=============================================================================
