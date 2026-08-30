---------------- MODULE TerminalRequestPublicationRecovery ----------------
EXTENDS FiniteSets, Naturals, TLC

CONSTANT MaxAdmissions

\* Models the local publication boundary after ProjectedTerminal has installed
\* correlation state for a viewport row read, an exact buffer row read, or an
\* asynchronous link effect. A synchronous refusal and an asynchronous send
\* rejection settle only the matching request. A rejection that arrives after
\* completion, disposal, or readmission is stale and cannot
\* retire newer work. The viewport lane may admit a separately queued user
\* intent after settlement, but no lane automatically retries the failed
\* request itself.

Lanes == {"viewport", "buffer", "link"}
Phases == {"idle", "publishing", "in-flight", "settled", "disposed"}
Outcomes == {
  "none", "rows", "completed", "unavailable", "link-send-failed",
  "history-recovery", "cancelled"
}
NoLane == "none"
Actions == {
  "init", "admit-viewport", "admit-buffer", "admit-link",
  "queue-viewport-intent", "admit-pending-viewport",
  "retry-viewport", "retry-buffer", "retry-link",
  "readmit-viewport", "readmit-buffer", "readmit-link",
  "accept-viewport", "accept-buffer", "accept-link",
  "refuse-viewport", "refuse-buffer", "refuse-link",
  "complete-viewport", "complete-buffer", "complete-link",
  "reject-async", "dispose"
}

FailureOutcome(lane) ==
  CASE lane = "buffer" -> "unavailable"
    [] lane = "link" -> "link-send-failed"
    [] OTHER -> "history-recovery"

SuccessOutcome(lane) == IF lane = "link" THEN "completed" ELSE "rows"

VARIABLES active, phase, outcome, admissionCount, sequence, nextSequence,
  rejectable, pendingViewportIntent,
  priorActive, priorPhase, priorOutcome, priorAdmissionCount, priorSequence,
  priorPendingViewportIntent,
  lastAction, lastTarget, lastFailureSequence, lastFailureWasCurrent

vars == <<active, phase, outcome, admissionCount, sequence, nextSequence,
  rejectable, pendingViewportIntent,
  priorActive, priorPhase, priorOutcome, priorAdmissionCount, priorSequence,
  priorPendingViewportIntent,
  lastAction, lastTarget, lastFailureSequence, lastFailureWasCurrent>>

Init ==
  /\ active = TRUE
  /\ phase = [lane \in Lanes |-> "idle"]
  /\ outcome = [lane \in Lanes |-> "none"]
  /\ admissionCount = [lane \in Lanes |-> 0]
  /\ sequence = [lane \in Lanes |-> 0]
  /\ nextSequence = 1
  /\ rejectable = {}
  /\ pendingViewportIntent = FALSE
  /\ priorActive = active
  /\ priorPhase = phase
  /\ priorOutcome = outcome
  /\ priorAdmissionCount = admissionCount
  /\ priorSequence = sequence
  /\ priorPendingViewportIntent = pendingViewportIntent
  /\ lastAction = "init"
  /\ lastTarget = NoLane
  /\ lastFailureSequence = 0
  /\ lastFailureWasCurrent = FALSE

Record(action, lane, failureSequence, failureWasCurrent,
       oldActive, oldPhase, oldOutcome, oldAdmissionCount, oldSequence,
       oldPendingViewportIntent) ==
  /\ priorActive' = oldActive
  /\ priorPhase' = oldPhase
  /\ priorOutcome' = oldOutcome
  /\ priorAdmissionCount' = oldAdmissionCount
  /\ priorSequence' = oldSequence
  /\ priorPendingViewportIntent' = oldPendingViewportIntent
  /\ lastAction' = action
  /\ lastTarget' = lane
  /\ lastFailureSequence' = failureSequence
  /\ lastFailureWasCurrent' = failureWasCurrent

Admit(lane, action, nextPendingViewportIntent) ==
  /\ active
  /\ lane \in Lanes
  /\ phase[lane] \in {"idle", "settled"}
  /\ admissionCount[lane] < MaxAdmissions
  /\ phase' = [phase EXCEPT ![lane] = "publishing"]
  /\ outcome' = [outcome EXCEPT ![lane] = "none"]
  /\ admissionCount' = [admissionCount EXCEPT ![lane] = @ + 1]
  /\ sequence' = [sequence EXCEPT ![lane] = nextSequence]
  /\ nextSequence' = nextSequence + 1
  /\ pendingViewportIntent' = nextPendingViewportIntent
  /\ UNCHANGED <<active, rejectable>>
  /\ Record(action, lane, 0, FALSE, active, phase, outcome,
       admissionCount, sequence, pendingViewportIntent)

AdmitViewport ==
  /\ phase["viewport"] = "idle"
  /\ Admit("viewport", "admit-viewport", pendingViewportIntent)

AdmitBuffer ==
  /\ phase["buffer"] = "idle"
  /\ Admit("buffer", "admit-buffer", pendingViewportIntent)

AdmitLink ==
  /\ phase["link"] = "idle"
  /\ Admit("link", "admit-link", pendingViewportIntent)

QueueViewportIntent ==
  /\ active
  /\ phase["viewport"] \in {"publishing", "in-flight"}
  /\ admissionCount["viewport"] < MaxAdmissions
  /\ ~pendingViewportIntent
  /\ pendingViewportIntent' = TRUE
  /\ UNCHANGED <<active, phase, outcome, admissionCount, sequence,
       nextSequence, rejectable>>
  /\ Record("queue-viewport-intent", NoLane, 0, FALSE, active, phase,
       outcome, admissionCount, sequence, pendingViewportIntent)

AdmitPendingViewport ==
  /\ pendingViewportIntent
  /\ phase["viewport"] = "settled"
  /\ Admit("viewport", "admit-pending-viewport", FALSE)

RetryViewport ==
  /\ ~pendingViewportIntent
  /\ phase["viewport"] = "settled"
  /\ outcome["viewport"] = FailureOutcome("viewport")
  /\ Admit("viewport", "retry-viewport", pendingViewportIntent)

RetryBuffer ==
  /\ phase["buffer"] = "settled"
  /\ outcome["buffer"] = FailureOutcome("buffer")
  /\ Admit("buffer", "retry-buffer", pendingViewportIntent)

RetryLink ==
  /\ phase["link"] = "settled"
  /\ outcome["link"] = FailureOutcome("link")
  /\ Admit("link", "retry-link", pendingViewportIntent)

ReadmitViewport ==
  /\ ~pendingViewportIntent
  /\ phase["viewport"] = "settled"
  /\ outcome["viewport"] = SuccessOutcome("viewport")
  /\ Admit("viewport", "readmit-viewport", pendingViewportIntent)

ReadmitBuffer ==
  /\ phase["buffer"] = "settled"
  /\ outcome["buffer"] = SuccessOutcome("buffer")
  /\ Admit("buffer", "readmit-buffer", pendingViewportIntent)

ReadmitLink ==
  /\ phase["link"] = "settled"
  /\ outcome["link"] = SuccessOutcome("link")
  /\ Admit("link", "readmit-link", pendingViewportIntent)

Accept(lane, action) ==
  LET request == [lane |-> lane, sequence |-> sequence[lane]]
  IN
    /\ active
    /\ lane \in Lanes
    /\ phase[lane] = "publishing"
    /\ phase' = [phase EXCEPT ![lane] = "in-flight"]
    /\ rejectable' = rejectable \cup {request}
    /\ UNCHANGED <<active, outcome, admissionCount, sequence, nextSequence,
         pendingViewportIntent>>
    /\ Record(action, lane, 0, FALSE, active, phase, outcome,
         admissionCount, sequence, pendingViewportIntent)

AcceptViewport == Accept("viewport", "accept-viewport")
AcceptBuffer == Accept("buffer", "accept-buffer")
AcceptLink == Accept("link", "accept-link")

Refuse(lane, action) ==
  /\ active
  /\ lane \in Lanes
  /\ phase[lane] = "publishing"
  /\ phase' = [phase EXCEPT ![lane] = "settled"]
  /\ outcome' = [outcome EXCEPT ![lane] = FailureOutcome(lane)]
  /\ UNCHANGED <<active, admissionCount, sequence, nextSequence,
       rejectable, pendingViewportIntent>>
  /\ Record(action, lane, sequence[lane], TRUE, active, phase, outcome,
       admissionCount, sequence, pendingViewportIntent)

RefuseViewport == Refuse("viewport", "refuse-viewport")
RefuseBuffer == Refuse("buffer", "refuse-buffer")
RefuseLink == Refuse("link", "refuse-link")

Complete(lane, action) ==
  /\ active
  /\ lane \in Lanes
  /\ phase[lane] = "in-flight"
  /\ phase' = [phase EXCEPT ![lane] = "settled"]
  /\ outcome' = [outcome EXCEPT ![lane] = SuccessOutcome(lane)]
  /\ UNCHANGED <<active, admissionCount, sequence, nextSequence,
       rejectable, pendingViewportIntent>>
  /\ Record(action, lane, 0, FALSE, active, phase, outcome,
       admissionCount, sequence, pendingViewportIntent)

CompleteViewport == Complete("viewport", "complete-viewport")
CompleteBuffer == Complete("buffer", "complete-buffer")
CompleteLink == Complete("link", "complete-link")

RejectAsync ==
  \E request \in rejectable:
    LET lane == request.lane
        failureIsCurrent ==
          active /\ phase[lane] = "in-flight" /\ sequence[lane] = request.sequence
    IN
      /\ rejectable' = rejectable \ {request}
      /\ IF failureIsCurrent
           THEN /\ phase' = [phase EXCEPT ![lane] = "settled"]
                /\ outcome' = [outcome EXCEPT ![lane] = FailureOutcome(lane)]
           ELSE /\ phase' = phase
                /\ outcome' = outcome
      /\ UNCHANGED <<active, admissionCount, sequence, nextSequence,
           pendingViewportIntent>>
      /\ Record("reject-async", lane, request.sequence, failureIsCurrent,
           active, phase, outcome, admissionCount, sequence,
           pendingViewportIntent)

Dispose ==
  /\ active
  /\ active' = FALSE
  /\ phase' = [lane \in Lanes |-> "disposed"]
  /\ outcome' = [lane \in Lanes |->
       IF phase[lane] \in {"publishing", "in-flight"}
       THEN "cancelled"
       ELSE outcome[lane]]
  /\ pendingViewportIntent' = FALSE
  /\ UNCHANGED <<admissionCount, sequence, nextSequence, rejectable>>
  /\ Record("dispose", NoLane, 0, FALSE, active, phase, outcome,
       admissionCount, sequence, pendingViewportIntent)

CoverageRefuseViewport ==
  /\ lastAction = "refuse-viewport"
  /\ UNCHANGED vars

CoverageRefuseBuffer ==
  /\ lastAction = "refuse-buffer"
  /\ UNCHANGED vars

CoverageRefuseLink ==
  /\ lastAction = "refuse-link"
  /\ UNCHANGED vars

CoverageAcceptViewport ==
  /\ lastAction = "accept-viewport"
  /\ UNCHANGED vars

CoverageAcceptBuffer ==
  /\ lastAction = "accept-buffer"
  /\ UNCHANGED vars

CoverageAcceptLink ==
  /\ lastAction = "accept-link"
  /\ UNCHANGED vars

CoverageCompleteViewport ==
  /\ lastAction = "complete-viewport"
  /\ UNCHANGED vars

CoverageCompleteBuffer ==
  /\ lastAction = "complete-buffer"
  /\ UNCHANGED vars

CoverageCompleteLink ==
  /\ lastAction = "complete-link"
  /\ UNCHANGED vars

CoverageRejectCurrentViewport ==
  /\ lastAction = "reject-async"
  /\ lastTarget = "viewport"
  /\ lastFailureWasCurrent
  /\ UNCHANGED vars

CoverageRejectCurrentBuffer ==
  /\ lastAction = "reject-async"
  /\ lastTarget = "buffer"
  /\ lastFailureWasCurrent
  /\ UNCHANGED vars

CoverageRejectCurrentLink ==
  /\ lastAction = "reject-async"
  /\ lastTarget = "link"
  /\ lastFailureWasCurrent
  /\ UNCHANGED vars

CoverageRejectStale ==
  /\ lastAction = "reject-async"
  /\ ~lastFailureWasCurrent
  /\ UNCHANGED vars

Next ==
  \/ AdmitViewport
  \/ AdmitBuffer
  \/ AdmitLink
  \/ QueueViewportIntent
  \/ AdmitPendingViewport
  \/ RetryViewport
  \/ RetryBuffer
  \/ RetryLink
  \/ ReadmitViewport
  \/ ReadmitBuffer
  \/ ReadmitLink
  \/ AcceptViewport
  \/ AcceptBuffer
  \/ AcceptLink
  \/ RefuseViewport
  \/ RefuseBuffer
  \/ RefuseLink
  \/ CompleteViewport
  \/ CompleteBuffer
  \/ CompleteLink
  \/ RejectAsync
  \/ Dispose
  \/ CoverageRefuseViewport
  \/ CoverageRefuseBuffer
  \/ CoverageRefuseLink
  \/ CoverageAcceptViewport
  \/ CoverageAcceptBuffer
  \/ CoverageAcceptLink
  \/ CoverageCompleteViewport
  \/ CoverageCompleteBuffer
  \/ CoverageCompleteLink
  \/ CoverageRejectCurrentViewport
  \/ CoverageRejectCurrentBuffer
  \/ CoverageRejectCurrentLink
  \/ CoverageRejectStale

Spec ==
  /\ Init
  /\ [][Next]_vars
  /\ WF_vars(AdmitPendingViewport)

TypeOK ==
  /\ active \in BOOLEAN
  /\ phase \in [Lanes -> Phases]
  /\ outcome \in [Lanes -> Outcomes]
  /\ admissionCount \in [Lanes -> 0..MaxAdmissions]
  /\ sequence \in [Lanes -> Nat]
  /\ nextSequence \in Nat
  /\ rejectable \subseteq [lane: Lanes, sequence: Nat]
  /\ pendingViewportIntent \in BOOLEAN
  /\ priorActive \in BOOLEAN
  /\ priorPhase \in [Lanes -> Phases]
  /\ priorOutcome \in [Lanes -> Outcomes]
  /\ priorAdmissionCount \in [Lanes -> 0..MaxAdmissions]
  /\ priorSequence \in [Lanes -> Nat]
  /\ priorPendingViewportIntent \in BOOLEAN
  /\ lastAction \in Actions
  /\ lastTarget \in Lanes \cup {NoLane}
  /\ lastFailureSequence \in Nat
  /\ lastFailureWasCurrent \in BOOLEAN

AtMostOneCurrentRequestPerLane ==
  \A lane \in Lanes:
    /\ admissionCount[lane] <= MaxAdmissions
    /\ phase[lane] \in {"publishing", "in-flight"} => sequence[lane] > 0

CurrentSequencesAreDistinct ==
  \A left, right \in Lanes:
    /\ left # right
    /\ phase[left] \in {"publishing", "in-flight"}
    /\ phase[right] \in {"publishing", "in-flight"}
    => sequence[left] # sequence[right]

SettledRequestHasTypedOutcome ==
  \A lane \in Lanes:
    phase[lane] = "settled" => outcome[lane] \in Outcomes \ {"none", "cancelled"}

CrossLaneSettlementIsolation ==
  lastTarget \in Lanes =>
    \A lane \in Lanes \ {lastTarget}:
      /\ phase[lane] = priorPhase[lane]
      /\ outcome[lane] = priorOutcome[lane]
      /\ admissionCount[lane] = priorAdmissionCount[lane]
      /\ sequence[lane] = priorSequence[lane]

CurrentPublicationFailureSettlesExactRequest ==
  lastFailureWasCurrent =>
    /\ lastTarget \in Lanes
    /\ phase[lastTarget] = "settled"
    /\ outcome[lastTarget] = FailureOutcome(lastTarget)
    /\ sequence[lastTarget] = lastFailureSequence

PublicationFailureDoesNotRetry ==
  lastFailureWasCurrent =>
    /\ admissionCount = priorAdmissionCount
    /\ sequence = priorSequence

StalePublicationFailureIsInert ==
  /\ lastAction = "reject-async"
  /\ ~lastFailureWasCurrent
  => /\ active = priorActive
     /\ phase = priorPhase
     /\ outcome = priorOutcome
     /\ admissionCount = priorAdmissionCount
     /\ sequence = priorSequence
     /\ pendingViewportIntent = priorPendingViewportIntent

ReadmissionUsesFreshSequence ==
  lastAction \in {
    "admit-pending-viewport", "retry-viewport", "retry-buffer", "retry-link",
    "readmit-viewport", "readmit-buffer", "readmit-link"
  } => sequence[lastTarget] > priorSequence[lastTarget]

PendingViewportAdmissionConsumesOnlyQueuedIntent ==
  lastAction = "admit-pending-viewport" =>
    /\ priorPendingViewportIntent
    /\ ~pendingViewportIntent

DisposalClosesEveryLane ==
  ~active =>
    /\ \A lane \in Lanes: phase[lane] = "disposed"
    /\ ~pendingViewportIntent

EverySettledViewportIntentEventuallyDispatches ==
  []((pendingViewportIntent /\ phase["viewport"] = "settled")
     => <>(~pendingViewportIntent \/ ~active))

=============================================================================
