-------------------------- MODULE TerminalImagePreview --------------------------
EXTENDS FiniteSets, Naturals, TLC

CONSTANTS MaxTargets, MaxRequests

Targets == 1..MaxTargets
Requests == 1..MaxRequests
NoTarget == 0
NoRequest == 0

Phases == {"idle", "waiting", "loading", "visible", "unavailable", "disposed"}
Actions == {
  "init",
  "observe-ineligible-hover",
  "start-hover",
  "replace-hover",
  "dwell-elapsed",
  "complete-current",
  "fail-current",
  "complete-stale",
  "leave-hover",
  "retire-hover",
  "revoke-capability",
  "dispose"
}

VARIABLES
  active,
  capability,
  phase,
  hoverTarget,
  hoverEpoch,
  requestCounter,
  currentRequest,
  inFlight,
  requestTarget,
  requestHoverEpoch,
  visibleTarget,
  visibleRequest,
  admissions,
  publicationCount,
  priorActive,
  priorCapability,
  priorPhase,
  priorHoverTarget,
  priorHoverEpoch,
  priorRequestCounter,
  priorCurrentRequest,
  priorInFlight,
  priorRequestTarget,
  priorRequestHoverEpoch,
  priorVisibleTarget,
  priorVisibleRequest,
  priorAdmissions,
  priorPublicationCount,
  lastAction

vars == <<
  active,
  capability,
  phase,
  hoverTarget,
  hoverEpoch,
  requestCounter,
  currentRequest,
  inFlight,
  requestTarget,
  requestHoverEpoch,
  visibleTarget,
  visibleRequest,
  admissions,
  publicationCount,
  priorActive,
  priorCapability,
  priorPhase,
  priorHoverTarget,
  priorHoverEpoch,
  priorRequestCounter,
  priorCurrentRequest,
  priorInFlight,
  priorRequestTarget,
  priorRequestHoverEpoch,
  priorVisibleTarget,
  priorVisibleRequest,
  priorAdmissions,
  priorPublicationCount,
  lastAction
>>

EmptyRequestTarget == [request \in Requests |-> NoTarget]
EmptyRequestEpoch == [request \in Requests |-> 0]

Init ==
  /\ active = TRUE
  /\ capability \in BOOLEAN
  /\ phase = "idle"
  /\ hoverTarget = NoTarget
  /\ hoverEpoch = 0
  /\ requestCounter = 0
  /\ currentRequest = NoRequest
  /\ inFlight = {}
  /\ requestTarget = EmptyRequestTarget
  /\ requestHoverEpoch = EmptyRequestEpoch
  /\ visibleTarget = NoTarget
  /\ visibleRequest = NoRequest
  /\ admissions = 0
  /\ publicationCount = 0
  /\ priorActive = TRUE
  /\ priorCapability = capability
  /\ priorPhase = "idle"
  /\ priorHoverTarget = NoTarget
  /\ priorHoverEpoch = 0
  /\ priorRequestCounter = 0
  /\ priorCurrentRequest = NoRequest
  /\ priorInFlight = {}
  /\ priorRequestTarget = EmptyRequestTarget
  /\ priorRequestHoverEpoch = EmptyRequestEpoch
  /\ priorVisibleTarget = NoTarget
  /\ priorVisibleRequest = NoRequest
  /\ priorAdmissions = 0
  /\ priorPublicationCount = 0
  /\ lastAction = "init"

Record(action) ==
  /\ priorActive' = active
  /\ priorCapability' = capability
  /\ priorPhase' = phase
  /\ priorHoverTarget' = hoverTarget
  /\ priorHoverEpoch' = hoverEpoch
  /\ priorRequestCounter' = requestCounter
  /\ priorCurrentRequest' = currentRequest
  /\ priorInFlight' = inFlight
  /\ priorRequestTarget' = requestTarget
  /\ priorRequestHoverEpoch' = requestHoverEpoch
  /\ priorVisibleTarget' = visibleTarget
  /\ priorVisibleRequest' = visibleRequest
  /\ priorAdmissions' = admissions
  /\ priorPublicationCount' = publicationCount
  /\ lastAction' = action

ObserveIneligibleHover ==
  /\ active
  /\ ~capability
  /\ phase = "idle"
  /\ UNCHANGED <<
       active, capability, phase, hoverTarget, hoverEpoch, requestCounter,
       currentRequest, inFlight, requestTarget, requestHoverEpoch,
       visibleTarget, visibleRequest, admissions, publicationCount
     >>
  /\ Record("observe-ineligible-hover")

StartHover ==
  /\ active
  /\ capability
  /\ phase \in {"idle", "unavailable"}
  /\ hoverEpoch < MaxRequests + 2
  /\ \E target \in Targets:
       /\ phase' = "waiting"
       /\ hoverTarget' = target
       /\ hoverEpoch' = hoverEpoch + 1
  /\ currentRequest' = NoRequest
  /\ visibleTarget' = NoTarget
  /\ visibleRequest' = NoRequest
  /\ UNCHANGED <<
       active, capability, requestCounter, inFlight, requestTarget,
       requestHoverEpoch, admissions, publicationCount
     >>
  /\ Record("start-hover")

ReplaceHover ==
  /\ active
  /\ capability
  /\ phase \in {"waiting", "loading", "visible"}
  /\ hoverEpoch < MaxRequests + 2
  /\ \E target \in Targets:
       /\ target # hoverTarget
       /\ phase' = "waiting"
       /\ hoverTarget' = target
       /\ hoverEpoch' = hoverEpoch + 1
  /\ currentRequest' = NoRequest
  /\ visibleTarget' = NoTarget
  /\ visibleRequest' = NoRequest
  /\ UNCHANGED <<
       active, capability, requestCounter, inFlight, requestTarget,
       requestHoverEpoch, admissions, publicationCount
     >>
  /\ Record("replace-hover")

DwellElapsed ==
  /\ active
  /\ capability
  /\ phase = "waiting"
  /\ hoverTarget \in Targets
  /\ requestCounter < MaxRequests
  /\ LET request == requestCounter + 1 IN
       /\ phase' = "loading"
       /\ requestCounter' = request
       /\ currentRequest' = request
       /\ inFlight' = inFlight \cup {request}
       /\ requestTarget' = [requestTarget EXCEPT ![request] = hoverTarget]
       /\ requestHoverEpoch' = [requestHoverEpoch EXCEPT ![request] = hoverEpoch]
  /\ admissions' = admissions + 1
  /\ UNCHANGED <<
       active, capability, hoverTarget, hoverEpoch, visibleTarget,
       visibleRequest, publicationCount
     >>
  /\ Record("dwell-elapsed")

CurrentRequestHasAuthority ==
  /\ active
  /\ capability
  /\ phase = "loading"
  /\ currentRequest \in inFlight
  /\ requestTarget[currentRequest] = hoverTarget
  /\ requestHoverEpoch[currentRequest] = hoverEpoch

CompleteCurrent ==
  /\ CurrentRequestHasAuthority
  /\ phase' = "visible"
  /\ inFlight' = inFlight \ {currentRequest}
  /\ visibleTarget' = hoverTarget
  /\ visibleRequest' = currentRequest
  /\ currentRequest' = NoRequest
  /\ publicationCount' = publicationCount + 1
  /\ UNCHANGED <<
       active, capability, hoverTarget, hoverEpoch, requestCounter,
       requestTarget, requestHoverEpoch, admissions
     >>
  /\ Record("complete-current")

FailCurrent ==
  /\ CurrentRequestHasAuthority
  /\ phase' = "unavailable"
  /\ inFlight' = inFlight \ {currentRequest}
  /\ currentRequest' = NoRequest
  /\ visibleTarget' = NoTarget
  /\ visibleRequest' = NoRequest
  /\ UNCHANGED <<
       active, capability, hoverTarget, hoverEpoch, requestCounter,
       requestTarget, requestHoverEpoch, admissions, publicationCount
     >>
  /\ Record("fail-current")

CompleteStale ==
  /\ \E request \in inFlight:
       /\ \/ request # currentRequest
          \/ ~active
          \/ ~capability
          \/ phase # "loading"
          \/ requestTarget[request] # hoverTarget
          \/ requestHoverEpoch[request] # hoverEpoch
       /\ inFlight' = inFlight \ {request}
  /\ UNCHANGED <<
       active, capability, phase, hoverTarget, hoverEpoch, requestCounter,
       currentRequest, requestTarget, requestHoverEpoch, visibleTarget,
       visibleRequest, admissions, publicationCount
     >>
  /\ Record("complete-stale")

ClearHover(action) ==
  /\ active
  /\ phase # "idle"
  /\ hoverEpoch < MaxRequests + 2
  /\ phase' = "idle"
  /\ hoverTarget' = NoTarget
  /\ hoverEpoch' = hoverEpoch + 1
  /\ currentRequest' = NoRequest
  /\ visibleTarget' = NoTarget
  /\ visibleRequest' = NoRequest
  /\ UNCHANGED <<
       active, capability, requestCounter, inFlight, requestTarget,
       requestHoverEpoch, admissions, publicationCount
     >>
  /\ Record(action)

LeaveHover == ClearHover("leave-hover")
RetireHover == ClearHover("retire-hover")

CoverageLeaveHover ==
  /\ lastAction = "leave-hover"
  /\ UNCHANGED vars

CoverageRetireHover ==
  /\ lastAction = "retire-hover"
  /\ UNCHANGED vars

RevokeCapability ==
  /\ active
  /\ capability
  /\ capability' = FALSE
  /\ phase' = "idle"
  /\ hoverTarget' = NoTarget
  /\ hoverEpoch' = hoverEpoch
  /\ currentRequest' = NoRequest
  /\ visibleTarget' = NoTarget
  /\ visibleRequest' = NoRequest
  /\ UNCHANGED <<
       active, requestCounter, inFlight, requestTarget, requestHoverEpoch,
       admissions, publicationCount
     >>
  /\ Record("revoke-capability")

Dispose ==
  /\ active
  /\ active' = FALSE
  /\ phase' = "disposed"
  /\ hoverTarget' = NoTarget
  /\ hoverEpoch' = hoverEpoch
  /\ currentRequest' = NoRequest
  /\ visibleTarget' = NoTarget
  /\ visibleRequest' = NoRequest
  /\ UNCHANGED <<
       capability, requestCounter, inFlight, requestTarget,
       requestHoverEpoch, admissions, publicationCount
     >>
  /\ Record("dispose")

Next ==
  \/ ObserveIneligibleHover
  \/ StartHover
  \/ ReplaceHover
  \/ DwellElapsed
  \/ CompleteCurrent
  \/ FailCurrent
  \/ CompleteStale
  \/ LeaveHover
  \/ RetireHover
  \/ CoverageLeaveHover
  \/ CoverageRetireHover
  \/ RevokeCapability
  \/ Dispose

Spec == Init /\ [][Next]_vars

TypeOK ==
  /\ active \in BOOLEAN
  /\ capability \in BOOLEAN
  /\ phase \in Phases
  /\ hoverTarget \in {NoTarget} \cup Targets
  /\ hoverEpoch \in 0..(MaxRequests + 2)
  /\ requestCounter \in 0..MaxRequests
  /\ currentRequest \in {NoRequest} \cup Requests
  /\ inFlight \subseteq Requests
  /\ requestTarget \in [Requests -> {NoTarget} \cup Targets]
  /\ requestHoverEpoch \in [Requests -> 0..(MaxRequests + 2)]
  /\ visibleTarget \in {NoTarget} \cup Targets
  /\ visibleRequest \in {NoRequest} \cup Requests
  /\ admissions \in 0..MaxRequests
  /\ publicationCount \in 0..MaxRequests
  /\ priorActive \in BOOLEAN
  /\ priorCapability \in BOOLEAN
  /\ priorPhase \in Phases
  /\ priorHoverTarget \in {NoTarget} \cup Targets
  /\ priorHoverEpoch \in 0..(MaxRequests + 2)
  /\ priorRequestCounter \in 0..MaxRequests
  /\ priorCurrentRequest \in {NoRequest} \cup Requests
  /\ priorInFlight \subseteq Requests
  /\ priorRequestTarget \in [Requests -> {NoTarget} \cup Targets]
  /\ priorRequestHoverEpoch \in [Requests -> 0..(MaxRequests + 2)]
  /\ priorVisibleTarget \in {NoTarget} \cup Targets
  /\ priorVisibleRequest \in {NoRequest} \cup Requests
  /\ priorAdmissions \in 0..MaxRequests
  /\ priorPublicationCount \in 0..MaxRequests
  /\ lastAction \in Actions

CurrentRequestIsLatest ==
  currentRequest # NoRequest =>
    /\ active
    /\ capability
    /\ phase = "loading"
    /\ currentRequest \in inFlight
    /\ requestTarget[currentRequest] = hoverTarget
    /\ requestHoverEpoch[currentRequest] = hoverEpoch

VisibleRequiresCurrentHover ==
  phase = "visible" =>
    /\ active
    /\ capability
    /\ hoverTarget \in Targets
    /\ visibleTarget = hoverTarget
    /\ visibleRequest \in Requests
    /\ currentRequest = NoRequest

NoPresentationWithoutCapability ==
  ~capability =>
    /\ phase \in {"idle", "disposed"}
    /\ currentRequest = NoRequest
    /\ visibleTarget = NoTarget
    /\ visibleRequest = NoRequest

PublicationChangesOnlyOnCurrentCompletion ==
  publicationCount # priorPublicationCount =>
    /\ lastAction = "complete-current"
    /\ publicationCount = priorPublicationCount + 1

PublicationRequiresCurrentAuthority ==
  lastAction = "complete-current" =>
    /\ priorActive
    /\ priorCapability
    /\ priorPhase = "loading"
    /\ priorCurrentRequest \in priorInFlight
    /\ priorRequestTarget[priorCurrentRequest] = priorHoverTarget
    /\ priorRequestHoverEpoch[priorCurrentRequest] = priorHoverEpoch
    /\ visibleTarget = priorHoverTarget
    /\ visibleRequest = priorCurrentRequest

StaleCompletionCannotPublish ==
  lastAction = "complete-stale" =>
    /\ phase = priorPhase
    /\ hoverTarget = priorHoverTarget
    /\ hoverEpoch = priorHoverEpoch
    /\ currentRequest = priorCurrentRequest
    /\ visibleTarget = priorVisibleTarget
    /\ visibleRequest = priorVisibleRequest
    /\ publicationCount = priorPublicationCount

RetirementClearsPresentation ==
  lastAction \in {"leave-hover", "retire-hover", "revoke-capability", "dispose"} =>
    /\ currentRequest = NoRequest
    /\ visibleTarget = NoTarget
    /\ visibleRequest = NoRequest

AtMostOnePublicationPerAdmission == publicationCount <= admissions

=============================================================================
