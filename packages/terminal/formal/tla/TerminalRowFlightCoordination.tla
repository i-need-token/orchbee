-------------------- MODULE TerminalRowFlightCoordination --------------------
EXTENDS FiniteSets, Naturals, TLC

CONSTANTS MaxAdmissions, MaxRevision

\* Models the three independently correlated row-read lanes owned by
\* ProjectedTerminal. The interactive viewport lane uses Host-linearized
\* current-view semantics; search and general-buffer lanes retain exact
\* revision snapshots. Revision churn can therefore never stale or starve an
\* admitted viewport flight, while exact lanes settle stale rather than accept
\* bytes from a different revision. Generation invalidation is owned by the
\* Host history model and is outside this client-lane abstraction.

Lanes == {"viewport", "search", "buffer"}
AuxiliaryLanes == {"search", "buffer"}
Phases == {"idle", "in-flight", "settled", "disposed"}
Outcomes == {"none", "rows", "stale", "unavailable", "cancelled", "disposed"}
NoLane == "none"
Actions == {
  "init", "admit-viewport", "admit-search", "admit-buffer",
  "resolve-viewport", "resolve-search", "resolve-buffer",
  "stale-search", "stale-buffer", "fail-viewport", "fail-search",
  "fail-buffer", "abort-search", "abort-buffer", "advance-revision",
  "dispose"
}

VARIABLES active, revision, phase, outcome, admissionCount, sequence,
  flightRevision, nextSequence, priorRevision, priorPhase, priorOutcome,
  priorAdmissionCount, priorSequence, priorFlightRevision, lastAction,
  lastTarget

vars == <<active, revision, phase, outcome, admissionCount, sequence,
  flightRevision, nextSequence, priorRevision, priorPhase, priorOutcome,
  priorAdmissionCount, priorSequence, priorFlightRevision, lastAction,
  lastTarget>>

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
  /\ lastAction = "init"
  /\ lastTarget = NoLane

Record(action, lane, oldRevision, oldPhase, oldOutcome, oldAdmissionCount,
       oldSequence, oldFlightRevision) ==
  /\ priorRevision' = oldRevision
  /\ priorPhase' = oldPhase
  /\ priorOutcome' = oldOutcome
  /\ priorAdmissionCount' = oldAdmissionCount
  /\ priorSequence' = oldSequence
  /\ priorFlightRevision' = oldFlightRevision
  /\ lastAction' = action
  /\ lastTarget' = lane

Admit(lane, action) ==
  /\ active
  /\ lane \in Lanes
  /\ phase[lane] \in {"idle", "settled"}
  /\ admissionCount[lane] < MaxAdmissions
  /\ phase' = [phase EXCEPT ![lane] = "in-flight"]
  /\ outcome' = [outcome EXCEPT ![lane] = "none"]
  /\ admissionCount' = [admissionCount EXCEPT ![lane] = @ + 1]
  /\ sequence' = [sequence EXCEPT ![lane] = nextSequence]
  /\ flightRevision' = [flightRevision EXCEPT ![lane] = revision]
  /\ nextSequence' = nextSequence + 1
  /\ UNCHANGED <<active, revision>>
  /\ Record(action, lane, revision, phase, outcome, admissionCount, sequence,
       flightRevision)

AdmitViewport == Admit("viewport", "admit-viewport")
AdmitSearch == Admit("search", "admit-search")
AdmitBuffer == Admit("buffer", "admit-buffer")

Settle(lane, nextOutcome, action) ==
  /\ active
  /\ lane \in Lanes
  /\ phase[lane] = "in-flight"
  /\ phase' = [phase EXCEPT ![lane] = "settled"]
  /\ outcome' = [outcome EXCEPT ![lane] = nextOutcome]
  /\ UNCHANGED <<active, revision, admissionCount, sequence,
       flightRevision, nextSequence>>
  /\ Record(action, lane, revision, phase, outcome, admissionCount, sequence,
       flightRevision)

ResolveViewport == Settle("viewport", "rows", "resolve-viewport")

ResolveExact(lane, action) ==
  /\ lane \in AuxiliaryLanes
  /\ flightRevision[lane] = revision
  /\ Settle(lane, "rows", action)

ResolveSearch == ResolveExact("search", "resolve-search")
ResolveBuffer == ResolveExact("buffer", "resolve-buffer")

StaleExact(lane, action) ==
  /\ lane \in AuxiliaryLanes
  /\ flightRevision[lane] # revision
  /\ Settle(lane, "stale", action)

StaleSearch == StaleExact("search", "stale-search")
StaleBuffer == StaleExact("buffer", "stale-buffer")

FailViewport == Settle("viewport", "unavailable", "fail-viewport")
FailSearch == Settle("search", "unavailable", "fail-search")
FailBuffer == Settle("buffer", "unavailable", "fail-buffer")
AbortSearch == Settle("search", "cancelled", "abort-search")
AbortBuffer == Settle("buffer", "cancelled", "abort-buffer")

AdvanceRevision ==
  /\ active
  /\ revision < MaxRevision
  /\ revision' = revision + 1
  /\ UNCHANGED <<active, phase, outcome, admissionCount, sequence,
       flightRevision, nextSequence>>
  /\ Record("advance-revision", NoLane, revision, phase, outcome,
       admissionCount, sequence, flightRevision)

Dispose ==
  /\ active
  /\ active' = FALSE
  /\ phase' = [lane \in Lanes |-> "disposed"]
  /\ outcome' = [lane \in Lanes |->
       IF phase[lane] = "in-flight" THEN "disposed" ELSE outcome[lane]]
  /\ UNCHANGED <<revision, admissionCount, sequence, flightRevision,
       nextSequence>>
  /\ Record("dispose", NoLane, revision, phase, outcome, admissionCount,
       sequence, flightRevision)

CoverageAdmitViewport ==
  /\ lastAction = "admit-viewport"
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
  \/ AdmitViewport
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
  /\ lastAction \in Actions
  /\ lastTarget \in Lanes \cup {NoLane}

AtMostOneFlightPerLane ==
  \A lane \in Lanes:
    /\ admissionCount[lane] <= MaxAdmissions
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

DisposalClosesEveryLane ==
  ~active => \A lane \in Lanes: phase[lane] = "disposed"

EveryAdmittedFlightEventuallySettles ==
  \A lane \in Lanes:
    [](phase[lane] = "in-flight" => <>(phase[lane] \in {"settled", "disposed"}))

=============================================================================
