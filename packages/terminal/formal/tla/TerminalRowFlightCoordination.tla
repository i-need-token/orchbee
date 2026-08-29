-------------------- MODULE TerminalRowFlightCoordination --------------------
EXTENDS FiniteSets, Naturals, TLC

CONSTANTS MaxAdmissions, MaxRevision, MaxViewportAdmissions

\* Models the three independently correlated row-read lanes owned by
\* ProjectedTerminal. The interactive viewport lane uses Host-linearized
\* current-view semantics; search and general-buffer lanes retain exact
\* revision snapshots. Revision churn can therefore never stale or starve an
\* admitted viewport flight, while exact lanes settle stale rather than accept
\* bytes from a different revision. A valid-anchor maintenance refresh may be
\* queued alongside newer user viewport motion; once the current flight
\* settles, user motion is admitted first and subsumes that refresh. A
\* current-view response also establishes a newer pending Host pin, so it
\* supersedes anchor recovery or maintenance work queued before that response.
\* Generation invalidation is owned by the Host history model and is outside
\* this client-lane abstraction.

Lanes == {"viewport", "search", "buffer"}
AuxiliaryLanes == {"search", "buffer"}
Phases == {"idle", "in-flight", "settled", "disposed"}
Outcomes == {"none", "rows", "stale", "unavailable", "cancelled", "disposed"}
NoLane == "none"
Actions == {
  "init", "queue-user-intent", "queue-maintenance-refresh",
  "queue-anchor-recovery", "recover-anchor",
  "admit-viewport-user", "admit-viewport-refresh", "admit-search", "admit-buffer",
  "resolve-viewport", "resolve-search", "resolve-buffer",
  "stale-search", "stale-buffer", "fail-viewport", "fail-search",
  "fail-buffer", "abort-search", "abort-buffer", "advance-revision",
  "dispose"
}

VARIABLES active, revision, phase, outcome, admissionCount, sequence,
  flightRevision, nextSequence, priorRevision, priorPhase, priorOutcome,
  priorAdmissionCount, priorSequence, priorFlightRevision,
  pendingUserIntent, pendingMaintenanceRefresh, pendingAnchorRecovery,
  lastViewportIntentKind,
  priorPendingUserIntent, lastAction, lastTarget

vars == <<active, revision, phase, outcome, admissionCount, sequence,
  flightRevision, nextSequence, priorRevision, priorPhase, priorOutcome,
  priorAdmissionCount, priorSequence, priorFlightRevision,
  pendingUserIntent, pendingMaintenanceRefresh, pendingAnchorRecovery,
  lastViewportIntentKind,
  priorPendingUserIntent, lastAction, lastTarget>>

Init ==
  /\ active = TRUE
  /\ revision = 1
  /\ phase = [lane \in Lanes |-> "idle"]
  /\ outcome = [lane \in Lanes |-> "none"]
  /\ admissionCount = [lane \in Lanes |-> 0]
  /\ sequence = [lane \in Lanes |-> 0]
  /\ flightRevision = [lane \in Lanes |-> 0]
  /\ nextSequence = 1
  /\ priorRevision = revision
  /\ priorPhase = phase
  /\ priorOutcome = outcome
  /\ priorAdmissionCount = admissionCount
  /\ priorSequence = sequence
  /\ priorFlightRevision = flightRevision
  /\ pendingUserIntent = FALSE
  /\ pendingMaintenanceRefresh = FALSE
  /\ pendingAnchorRecovery = FALSE
  /\ lastViewportIntentKind = "none"
  /\ priorPendingUserIntent = pendingUserIntent
  /\ lastAction = "init"
  /\ lastTarget = NoLane

Record(action, lane, oldRevision, oldPhase, oldOutcome, oldAdmissionCount,
       oldSequence, oldFlightRevision, oldPendingUserIntent) ==
  /\ priorRevision' = oldRevision
  /\ priorPhase' = oldPhase
  /\ priorOutcome' = oldOutcome
  /\ priorAdmissionCount' = oldAdmissionCount
  /\ priorSequence' = oldSequence
  /\ priorFlightRevision' = oldFlightRevision
  /\ priorPendingUserIntent' = oldPendingUserIntent
  /\ lastAction' = action
  /\ lastTarget' = lane

AdmissionLimit(lane) ==
  IF lane = "viewport" THEN MaxViewportAdmissions ELSE MaxAdmissions

Admit(lane, action, nextPendingUserIntent, nextPendingMaintenanceRefresh,
      nextViewportIntentKind) ==
  /\ active
  /\ lane \in Lanes
  /\ phase[lane] \in {"idle", "settled"}
  /\ admissionCount[lane] < AdmissionLimit(lane)
  /\ phase' = [phase EXCEPT ![lane] = "in-flight"]
  /\ outcome' = [outcome EXCEPT ![lane] = "none"]
  /\ admissionCount' = [admissionCount EXCEPT ![lane] = @ + 1]
  /\ sequence' = [sequence EXCEPT ![lane] = nextSequence]
  /\ flightRevision' = [flightRevision EXCEPT ![lane] = revision]
  /\ nextSequence' = nextSequence + 1
  /\ pendingUserIntent' = nextPendingUserIntent
  /\ pendingMaintenanceRefresh' = nextPendingMaintenanceRefresh
  /\ lastViewportIntentKind' = nextViewportIntentKind
  /\ UNCHANGED <<active, revision, pendingAnchorRecovery>>
  /\ Record(action, lane, revision, phase, outcome, admissionCount, sequence,
       flightRevision, pendingUserIntent)

QueueUserIntent ==
  /\ active
  /\ ~pendingUserIntent
  /\ admissionCount["viewport"] < MaxViewportAdmissions
  /\ pendingUserIntent' = TRUE
  /\ UNCHANGED <<active, revision, phase, outcome, admissionCount, sequence,
       flightRevision, nextSequence, pendingMaintenanceRefresh,
       pendingAnchorRecovery, lastViewportIntentKind>>
  /\ Record("queue-user-intent", NoLane, revision, phase, outcome,
       admissionCount, sequence, flightRevision, pendingUserIntent)

QueueMaintenanceRefresh ==
  /\ active
  /\ ~pendingMaintenanceRefresh
  /\ pendingMaintenanceRefresh' = TRUE
  /\ UNCHANGED <<active, revision, phase, outcome, admissionCount, sequence,
       flightRevision, nextSequence, pendingUserIntent,
       pendingAnchorRecovery, lastViewportIntentKind>>
  /\ Record("queue-maintenance-refresh", NoLane, revision, phase, outcome,
       admissionCount, sequence, flightRevision, pendingUserIntent)

QueueAnchorRecovery ==
  /\ active
  /\ ~pendingAnchorRecovery
  /\ pendingAnchorRecovery' = TRUE
  /\ UNCHANGED <<active, revision, phase, outcome, admissionCount, sequence,
       flightRevision, nextSequence, pendingUserIntent,
       pendingMaintenanceRefresh, lastViewportIntentKind>>
  /\ Record("queue-anchor-recovery", NoLane, revision, phase, outcome,
       admissionCount, sequence, flightRevision, pendingUserIntent)

AdmitViewportUser ==
  /\ ~pendingAnchorRecovery
  /\ pendingUserIntent
  /\ Admit("viewport", "admit-viewport-user", FALSE, FALSE, "user")

AdmitViewportRefresh ==
  /\ ~pendingAnchorRecovery
  /\ ~pendingUserIntent
  /\ pendingMaintenanceRefresh
  /\ Admit("viewport", "admit-viewport-refresh", FALSE, FALSE, "refresh")

AdmitSearch ==
  Admit("search", "admit-search", pendingUserIntent,
    pendingMaintenanceRefresh, lastViewportIntentKind)
AdmitBuffer ==
  Admit("buffer", "admit-buffer", pendingUserIntent,
    pendingMaintenanceRefresh, lastViewportIntentKind)

Settle(lane, nextOutcome, action, nextPendingMaintenanceRefresh,
       nextPendingAnchorRecovery) ==
  /\ active
  /\ lane \in Lanes
  /\ phase[lane] = "in-flight"
  /\ phase' = [phase EXCEPT ![lane] = "settled"]
  /\ outcome' = [outcome EXCEPT ![lane] = nextOutcome]
  /\ pendingMaintenanceRefresh' = nextPendingMaintenanceRefresh
  /\ pendingAnchorRecovery' = nextPendingAnchorRecovery
  /\ UNCHANGED <<active, revision, admissionCount, sequence,
       flightRevision, nextSequence, pendingUserIntent, lastViewportIntentKind>>
  /\ Record(action, lane, revision, phase, outcome, admissionCount, sequence,
       flightRevision, pendingUserIntent)

ResolveViewport == Settle("viewport", "rows", "resolve-viewport", FALSE, FALSE)

ResolveExact(lane, action) ==
  /\ lane \in AuxiliaryLanes
  /\ flightRevision[lane] = revision
  /\ Settle(lane, "rows", action, pendingMaintenanceRefresh, pendingAnchorRecovery)

ResolveSearch == ResolveExact("search", "resolve-search")
ResolveBuffer == ResolveExact("buffer", "resolve-buffer")

StaleExact(lane, action) ==
  /\ lane \in AuxiliaryLanes
  /\ flightRevision[lane] # revision
  /\ Settle(lane, "stale", action, pendingMaintenanceRefresh, pendingAnchorRecovery)

StaleSearch == StaleExact("search", "stale-search")
StaleBuffer == StaleExact("buffer", "stale-buffer")

FailViewport == Settle("viewport", "unavailable", "fail-viewport",
  pendingMaintenanceRefresh, pendingAnchorRecovery)
FailSearch == Settle("search", "unavailable", "fail-search",
  pendingMaintenanceRefresh, pendingAnchorRecovery)
FailBuffer == Settle("buffer", "unavailable", "fail-buffer",
  pendingMaintenanceRefresh, pendingAnchorRecovery)
AbortSearch == Settle("search", "cancelled", "abort-search",
  pendingMaintenanceRefresh, pendingAnchorRecovery)
AbortBuffer == Settle("buffer", "cancelled", "abort-buffer",
  pendingMaintenanceRefresh, pendingAnchorRecovery)

RecoverInvalidAnchor ==
  /\ active
  /\ pendingAnchorRecovery
  /\ phase["viewport"] \in {"idle", "settled"}
  /\ pendingUserIntent' = FALSE
  /\ pendingMaintenanceRefresh' = FALSE
  /\ pendingAnchorRecovery' = FALSE
  /\ UNCHANGED <<active, revision, phase, outcome, admissionCount, sequence,
       flightRevision, nextSequence, lastViewportIntentKind>>
  /\ Record("recover-anchor", NoLane, revision, phase, outcome,
       admissionCount, sequence, flightRevision, pendingUserIntent)

AdvanceRevision ==
  /\ active
  /\ revision < MaxRevision
  /\ revision' = revision + 1
  /\ pendingMaintenanceRefresh' = TRUE
  /\ UNCHANGED <<active, phase, outcome, admissionCount, sequence,
       flightRevision, nextSequence, pendingUserIntent,
       pendingAnchorRecovery, lastViewportIntentKind>>
  /\ Record("advance-revision", NoLane, revision, phase, outcome,
       admissionCount, sequence, flightRevision, pendingUserIntent)

Dispose ==
  /\ active
  /\ active' = FALSE
  /\ phase' = [lane \in Lanes |-> "disposed"]
  /\ outcome' = [lane \in Lanes |->
       IF phase[lane] = "in-flight" THEN "disposed" ELSE outcome[lane]]
  /\ UNCHANGED <<revision, admissionCount, sequence, flightRevision,
       nextSequence, pendingUserIntent, pendingMaintenanceRefresh,
       pendingAnchorRecovery, lastViewportIntentKind>>
  /\ Record("dispose", NoLane, revision, phase, outcome, admissionCount,
       sequence, flightRevision, pendingUserIntent)

CoverageAdmitViewport ==
  /\ lastAction = "admit-viewport-user"
  /\ UNCHANGED vars

CoverageAdmitViewportRefresh ==
  /\ lastAction = "admit-viewport-refresh"
  /\ UNCHANGED vars

CoverageQueueUserIntent ==
  /\ lastAction = "queue-user-intent"
  /\ UNCHANGED vars

CoverageQueueMaintenanceRefresh ==
  /\ lastAction = "queue-maintenance-refresh"
  /\ UNCHANGED vars

CoverageQueueAnchorRecovery ==
  /\ lastAction = "queue-anchor-recovery"
  /\ UNCHANGED vars

CoverageRecoverInvalidAnchor ==
  /\ lastAction = "recover-anchor"
  /\ UNCHANGED vars

CoverageAdmitSearch ==
  /\ lastAction = "admit-search"
  /\ UNCHANGED vars

CoverageAdmitBuffer ==
  /\ lastAction = "admit-buffer"
  /\ UNCHANGED vars

CoverageResolveViewport ==
  /\ lastAction = "resolve-viewport"
  /\ UNCHANGED vars

CoverageResolveSearch ==
  /\ lastAction = "resolve-search"
  /\ UNCHANGED vars

CoverageResolveBuffer ==
  /\ lastAction = "resolve-buffer"
  /\ UNCHANGED vars

CoverageStaleSearch ==
  /\ lastAction = "stale-search"
  /\ UNCHANGED vars

CoverageStaleBuffer ==
  /\ lastAction = "stale-buffer"
  /\ UNCHANGED vars

CoverageFailViewport ==
  /\ lastAction = "fail-viewport"
  /\ UNCHANGED vars

CoverageFailSearch ==
  /\ lastAction = "fail-search"
  /\ UNCHANGED vars

CoverageFailBuffer ==
  /\ lastAction = "fail-buffer"
  /\ UNCHANGED vars

CoverageAbortSearch ==
  /\ lastAction = "abort-search"
  /\ UNCHANGED vars

CoverageAbortBuffer ==
  /\ lastAction = "abort-buffer"
  /\ UNCHANGED vars

SettleByConsistency(lane) ==
  CASE lane = "viewport" -> ResolveViewport
    [] lane = "search" -> ResolveSearch \/ StaleSearch
    [] OTHER -> ResolveBuffer \/ StaleBuffer

Next ==
  \/ QueueUserIntent
  \/ QueueMaintenanceRefresh
  \/ QueueAnchorRecovery
  \/ RecoverInvalidAnchor
  \/ AdmitViewportUser
  \/ AdmitViewportRefresh
  \/ AdmitSearch
  \/ AdmitBuffer
  \/ ResolveViewport
  \/ ResolveSearch
  \/ ResolveBuffer
  \/ StaleSearch
  \/ StaleBuffer
  \/ FailViewport
  \/ FailSearch
  \/ FailBuffer
  \/ AbortSearch
  \/ AbortBuffer
  \/ AdvanceRevision
  \/ Dispose
  \/ CoverageAdmitViewport
  \/ CoverageAdmitViewportRefresh
  \/ CoverageQueueUserIntent
  \/ CoverageQueueMaintenanceRefresh
  \/ CoverageQueueAnchorRecovery
  \/ CoverageRecoverInvalidAnchor
  \/ CoverageAdmitSearch
  \/ CoverageAdmitBuffer
  \/ CoverageResolveViewport
  \/ CoverageResolveSearch
  \/ CoverageResolveBuffer
  \/ CoverageStaleSearch
  \/ CoverageStaleBuffer
  \/ CoverageFailViewport
  \/ CoverageFailSearch
  \/ CoverageFailBuffer
  \/ CoverageAbortSearch
  \/ CoverageAbortBuffer

Spec ==
  /\ Init
  /\ [][Next]_vars
  /\ \A lane \in Lanes: WF_vars(SettleByConsistency(lane))
  /\ WF_vars(AdmitViewportUser)
  /\ WF_vars(AdmitViewportRefresh)
  /\ WF_vars(RecoverInvalidAnchor)

TypeOK ==
  /\ active \in BOOLEAN
  /\ revision \in 1..MaxRevision
  /\ phase \in [Lanes -> Phases]
  /\ outcome \in [Lanes -> Outcomes]
  /\ admissionCount \in [Lanes -> Nat]
  /\ sequence \in [Lanes -> Nat]
  /\ flightRevision \in [Lanes -> 0..MaxRevision]
  /\ nextSequence \in Nat
  /\ priorRevision \in 1..MaxRevision
  /\ priorPhase \in [Lanes -> Phases]
  /\ priorOutcome \in [Lanes -> Outcomes]
  /\ priorAdmissionCount \in [Lanes -> Nat]
  /\ priorSequence \in [Lanes -> Nat]
  /\ priorFlightRevision \in [Lanes -> 0..MaxRevision]
  /\ pendingUserIntent \in BOOLEAN
  /\ pendingMaintenanceRefresh \in BOOLEAN
  /\ pendingAnchorRecovery \in BOOLEAN
  /\ lastViewportIntentKind \in {"none", "user", "refresh"}
  /\ priorPendingUserIntent \in BOOLEAN
  /\ lastAction \in Actions
  /\ lastTarget \in Lanes \cup {NoLane}

AtMostOneFlightPerLane ==
  \A lane \in Lanes:
    /\ admissionCount[lane] <= AdmissionLimit(lane)
    /\ phase[lane] = "in-flight" =>
         /\ admissionCount[lane] >= 1
         /\ outcome[lane] = "none"
         /\ flightRevision[lane] > 0

AtMostThreeFlights ==
  Cardinality({lane \in Lanes: phase[lane] = "in-flight"}) <= 3

InFlightSequencesAreDistinct ==
  \A left, right \in Lanes:
    /\ left # right
    /\ phase[left] = "in-flight"
    /\ phase[right] = "in-flight"
    => sequence[left] # sequence[right]

SettledFlightHasExactlyOneOutcome ==
  \A lane \in Lanes:
    /\ phase[lane] = "settled" =>
         outcome[lane] \in Outcomes \ {"none", "disposed"}
    /\ active /\ outcome[lane] \in Outcomes \ {"none", "disposed"} =>
         phase[lane] = "settled"

CrossLaneSettlementIsolation ==
  lastTarget \in Lanes =>
    \A lane \in Lanes \ {lastTarget}:
      /\ phase[lane] = priorPhase[lane]
      /\ outcome[lane] = priorOutcome[lane]
      /\ admissionCount[lane] = priorAdmissionCount[lane]
      /\ sequence[lane] = priorSequence[lane]
      /\ flightRevision[lane] = priorFlightRevision[lane]

ExactRowsResolveOnlyAtCapturedRevision ==
  /\ lastAction = "resolve-search" =>
       priorFlightRevision["search"] = priorRevision
  /\ lastAction = "resolve-buffer" =>
       priorFlightRevision["buffer"] = priorRevision

RevisionAdvancePreservesFlights ==
  lastAction = "advance-revision" =>
    /\ phase = priorPhase
    /\ outcome = priorOutcome
    /\ admissionCount = priorAdmissionCount
    /\ sequence = priorSequence
    /\ flightRevision = priorFlightRevision

RefreshAdmissionNeverPreemptsQueuedUserIntent ==
  lastAction = "admit-viewport-refresh" => ~priorPendingUserIntent

UserAdmissionAbsorbsMaintenanceRefresh ==
  lastAction = "admit-viewport-user" =>
    /\ ~pendingUserIntent
    /\ ~pendingMaintenanceRefresh
    /\ lastViewportIntentKind = "user"

ViewportResolutionSupersedesPriorAnchorWork ==
  lastAction = "resolve-viewport" =>
    /\ ~pendingMaintenanceRefresh
    /\ ~pendingAnchorRecovery

ViewportAdmissionRequiresNoRecovery ==
  lastAction \in {"admit-viewport-user", "admit-viewport-refresh"} =>
    ~pendingAnchorRecovery

DisposalClosesEveryLane ==
  ~active => \A lane \in Lanes: phase[lane] = "disposed"

EveryAdmittedFlightEventuallySettles ==
  \A lane \in Lanes:
    [](phase[lane] = "in-flight" => <>(phase[lane] \in {"settled", "disposed"}))

EveryQueuedUserIntentEventuallyDispatches ==
  [](pendingUserIntent => <>(~pendingUserIntent \/ ~active))

=============================================================================
