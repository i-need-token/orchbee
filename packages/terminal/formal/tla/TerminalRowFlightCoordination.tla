-------------------- MODULE TerminalRowFlightCoordination --------------------
EXTENDS FiniteSets, Naturals, TLC

CONSTANT MaxAdmissions

\* Models the three independently correlated row-read lanes owned by
\* ProjectedTerminal. A viewport request, a search request, and a general
\* buffer request may coexist, while each lane remains single-flight. Replies
\* can settle in any order because the concrete protocol correlates them by
\* request sequence. Timeouts and transport delivery are outside this model.

Lanes == {"viewport", "search", "buffer"}
AuxiliaryLanes == {"search", "buffer"}
Phases == {"idle", "in-flight", "settled", "disposed"}
Outcomes == {"none", "rows", "unavailable", "cancelled", "disposed"}
NoLane == "none"
Actions == {
  "init", "admit-viewport", "admit-search", "admit-buffer",
  "resolve-viewport", "resolve-search", "resolve-buffer",
  "fail-viewport", "fail-search", "fail-buffer", "abort-search",
  "abort-buffer", "dispose"
}

VARIABLES active, phase, outcome, admissionCount, sequence, nextSequence, priorPhase,
  priorOutcome, priorAdmissionCount, priorSequence, lastAction, lastTarget

vars == <<active, phase, outcome, admissionCount, sequence, nextSequence, priorPhase,
  priorOutcome, priorAdmissionCount, priorSequence, lastAction, lastTarget>>

Init ==
  /\ active = TRUE
  /\ phase = [lane \in Lanes |-> "idle"]
  /\ outcome = [lane \in Lanes |-> "none"]
  /\ admissionCount = [lane \in Lanes |-> 0]
  /\ sequence = [lane \in Lanes |-> 0]
  /\ nextSequence = 1
  /\ priorPhase = phase
  /\ priorOutcome = outcome
  /\ priorAdmissionCount = admissionCount
  /\ priorSequence = sequence
  /\ lastAction = "init"
  /\ lastTarget = NoLane

Record(action, lane, oldPhase, oldOutcome, oldAdmissionCount, oldSequence) ==
  /\ priorPhase' = oldPhase
  /\ priorOutcome' = oldOutcome
  /\ priorAdmissionCount' = oldAdmissionCount
  /\ priorSequence' = oldSequence
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
  /\ nextSequence' = nextSequence + 1
  /\ UNCHANGED active
  /\ Record(action, lane, phase, outcome, admissionCount, sequence)

AdmitViewport == Admit("viewport", "admit-viewport")
AdmitSearch == Admit("search", "admit-search")
AdmitBuffer == Admit("buffer", "admit-buffer")

Settle(lane, nextOutcome, action) ==
  /\ active
  /\ lane \in Lanes
  /\ phase[lane] = "in-flight"
  /\ phase' = [phase EXCEPT ![lane] = "settled"]
  /\ outcome' = [outcome EXCEPT ![lane] = nextOutcome]
  /\ UNCHANGED <<active, admissionCount, sequence, nextSequence>>
  /\ Record(action, lane, phase, outcome, admissionCount, sequence)

ResolveViewport == Settle("viewport", "rows", "resolve-viewport")
ResolveSearch == Settle("search", "rows", "resolve-search")
ResolveBuffer == Settle("buffer", "rows", "resolve-buffer")
FailViewport == Settle("viewport", "unavailable", "fail-viewport")
FailSearch == Settle("search", "unavailable", "fail-search")
FailBuffer == Settle("buffer", "unavailable", "fail-buffer")
AbortSearch == Settle("search", "cancelled", "abort-search")
AbortBuffer == Settle("buffer", "cancelled", "abort-buffer")

Dispose ==
  /\ active
  /\ active' = FALSE
  /\ phase' = [lane \in Lanes |-> "disposed"]
  /\ outcome' = [lane \in Lanes |->
       IF phase[lane] = "in-flight" THEN "disposed" ELSE outcome[lane]]
  /\ UNCHANGED <<admissionCount, sequence, nextSequence>>
  /\ Record("dispose", NoLane, phase, outcome, admissionCount, sequence)

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

Next ==
  \/ AdmitViewport
  \/ AdmitSearch
  \/ AdmitBuffer
  \/ ResolveViewport
  \/ ResolveSearch
  \/ ResolveBuffer
  \/ FailViewport
  \/ FailSearch
  \/ FailBuffer
  \/ AbortSearch
  \/ AbortBuffer
  \/ Dispose
  \/ CoverageAdmitViewport
  \/ CoverageAdmitSearch
  \/ CoverageAdmitBuffer
  \/ CoverageResolveViewport
  \/ CoverageResolveSearch
  \/ CoverageResolveBuffer
  \/ CoverageFailViewport
  \/ CoverageFailSearch
  \/ CoverageFailBuffer
  \/ CoverageAbortSearch
  \/ CoverageAbortBuffer

Resolve(lane) ==
  CASE lane = "viewport" -> ResolveViewport
    [] lane = "search" -> ResolveSearch
    [] OTHER -> ResolveBuffer

Spec ==
  /\ Init
  /\ [][Next]_vars
  /\ \A lane \in Lanes: WF_vars(Resolve(lane))

TypeOK ==
  /\ active \in BOOLEAN
  /\ phase \in [Lanes -> Phases]
  /\ outcome \in [Lanes -> Outcomes]
  /\ admissionCount \in [Lanes -> Nat]
  /\ sequence \in [Lanes -> Nat]
  /\ nextSequence \in Nat
  /\ priorPhase \in [Lanes -> Phases]
  /\ priorOutcome \in [Lanes -> Outcomes]
  /\ priorAdmissionCount \in [Lanes -> Nat]
  /\ priorSequence \in [Lanes -> Nat]
  /\ lastAction \in Actions
  /\ lastTarget \in Lanes \cup {NoLane}

AtMostOneFlightPerLane ==
  \A lane \in Lanes:
    /\ admissionCount[lane] <= MaxAdmissions
    /\ phase[lane] = "in-flight" =>
         /\ admissionCount[lane] >= 1
         /\ outcome[lane] = "none"

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
    /\ phase[lane] = "settled" => outcome[lane] \in Outcomes \ {"none", "disposed"}
    /\ active /\ outcome[lane] \in Outcomes \ {"none", "disposed"} =>
         phase[lane] = "settled"

CrossLaneSettlementIsolation ==
  lastTarget \in Lanes =>
    \A lane \in Lanes \ {lastTarget}:
      /\ phase[lane] = priorPhase[lane]
      /\ outcome[lane] = priorOutcome[lane]
      /\ admissionCount[lane] = priorAdmissionCount[lane]
      /\ sequence[lane] = priorSequence[lane]

DisposalClosesEveryLane ==
  ~active => \A lane \in Lanes: phase[lane] = "disposed"

EveryAdmittedFlightEventuallySettles ==
  \A lane \in Lanes:
    [](phase[lane] = "in-flight" => <>(phase[lane] \in {"settled", "disposed"}))

=============================================================================
