--------------------- MODULE TerminalNativeInputSettlement ---------------------
EXTENDS FiniteSets, Naturals, TLC

\* Models the short native-text settlement window owned by ProjectedTerminal.
\* Two requests expose cancellation followed by re-entry. Printable fallback
\* requests distinguish unmodified input from Shift-produced punctuation and
\* digits, while independently ordered timer, DOM-value, composition, repair,
\* and disposal actions expose settlement races. The concrete 30 ms delay is
\* intentionally outside this model.

MaxRequests == 2
Requests == 1..MaxRequests
NoRequest == 0
MaxEpoch == 2
MaxInputGeneration == 2
InputValues == {"value-a", "value-b"}
NoInputValue == "no-input-value"
Admissions == {"accepting", "repairing", "disposed"}
CompositionPhases == {"idle", "eligible"}
Outcomes == {"none", "published-native", "published-fallback", "noop", "cancelled"}
Actions == {
  "init", "queue-without-fallback", "queue-unmodified-fallback",
  "queue-shifted-fallback",
  "coalesce-without-fallback", "coalesce-fallback", "native-value-changes",
  "defer-independent-input", "fire-native-value", "fire-fallback",
  "fire-noop", "fire-stale-timer", "publish-following-input", "start-composition",
  "end-composition", "begin-repair", "complete-repair", "dispose"
}

VARIABLES
  admission,
  compositionPhase,
  repairEpoch,
  inputGeneration,
  actualValue,
  requestOrdinal,
  pendingRequest,
  timerGeneration,
  activeTimer,
  capturedEpoch,
  baselineGeneration,
  baselineValue,
  fallbackPresent,
  fallbackEligible,
  shiftedFallbackRequests,
  deferredInputRequests,
  outcome,
  publicationCount,
  lastAction,
  lastTarget,
  lastTimer,
  lastTimerEpoch,
  priorPendingRequest,
  priorActiveTimer,
  priorCapturedEpoch,
  priorActualValue,
  priorBaselineValue,
  priorTargetOutcome,
  priorTargetPublicationCount

runtimeVars == <<admission, compositionPhase, repairEpoch, inputGeneration, actualValue,
  requestOrdinal, pendingRequest, timerGeneration, activeTimer, capturedEpoch,
  baselineGeneration, baselineValue, fallbackPresent, fallbackEligible,
  shiftedFallbackRequests, deferredInputRequests, outcome, publicationCount>>

historyVars == <<lastAction, lastTarget, lastTimer, lastTimerEpoch,
  priorPendingRequest, priorActiveTimer, priorCapturedEpoch, priorActualValue,
  priorBaselineValue, priorTargetOutcome, priorTargetPublicationCount>>

vars == <<runtimeVars, historyVars>>

Init ==
  /\ admission = "accepting"
  /\ compositionPhase = "idle"
  /\ repairEpoch = 0
  /\ inputGeneration = 0
  /\ actualValue \in InputValues
  /\ requestOrdinal = 0
  /\ pendingRequest = NoRequest
  /\ timerGeneration = 0
  /\ activeTimer = 0
  /\ capturedEpoch = 0
  /\ baselineGeneration = 0
  /\ baselineValue = NoInputValue
  /\ fallbackPresent = FALSE
  /\ fallbackEligible = {}
  /\ shiftedFallbackRequests = {}
  /\ deferredInputRequests = {}
  /\ outcome = [request \in Requests |-> "none"]
  /\ publicationCount = [request \in Requests |-> 0]
  /\ lastAction = "init"
  /\ lastTarget = NoRequest
  /\ lastTimer = 0
  /\ lastTimerEpoch = 0
  /\ priorPendingRequest = pendingRequest
  /\ priorActiveTimer = activeTimer
  /\ priorCapturedEpoch = capturedEpoch
  /\ priorActualValue = actualValue
  /\ priorBaselineValue = baselineValue
  /\ priorTargetOutcome = "none"
  /\ priorTargetPublicationCount = 0

RecordFields(action, request) ==
  /\ lastAction' = action
  /\ lastTarget' = request
  /\ priorPendingRequest' = pendingRequest
  /\ priorActiveTimer' = activeTimer
  /\ priorCapturedEpoch' = capturedEpoch
  /\ priorActualValue' = actualValue
  /\ priorBaselineValue' = baselineValue
  /\ priorTargetOutcome' = IF request = NoRequest THEN "none" ELSE outcome[request]
  /\ priorTargetPublicationCount' =
       IF request = NoRequest THEN 0 ELSE publicationCount[request]

Record(action, request) ==
  /\ RecordFields(action, request)
  /\ UNCHANGED <<lastTimer, lastTimerEpoch>>

RecordStaleTimer(timer, epoch) ==
  /\ RecordFields("fire-stale-timer", pendingRequest)
  /\ lastTimer' = timer
  /\ lastTimerEpoch' = epoch

QueueWithoutFallback ==
  /\ admission = "accepting"
  /\ compositionPhase = "idle"
  /\ pendingRequest = NoRequest
  /\ requestOrdinal < MaxRequests
  /\ LET request == requestOrdinal + 1 IN
       /\ requestOrdinal' = request
       /\ pendingRequest' = request
       /\ timerGeneration' = timerGeneration + 1
       /\ activeTimer' = timerGeneration + 1
       /\ capturedEpoch' = repairEpoch
       /\ baselineGeneration' = inputGeneration
       /\ baselineValue' = actualValue
       /\ fallbackPresent' = FALSE
       /\ UNCHANGED <<admission, compositionPhase, repairEpoch,
            inputGeneration, actualValue, fallbackEligible, shiftedFallbackRequests,
            deferredInputRequests, outcome,
            publicationCount>>
       /\ Record("queue-without-fallback", request)

QueueFallbackRequest ==
  /\ admission = "accepting"
  /\ compositionPhase = "idle"
  /\ pendingRequest = NoRequest
  /\ requestOrdinal < MaxRequests
  /\ LET request == requestOrdinal + 1 IN
       /\ requestOrdinal' = request
       /\ pendingRequest' = request
       /\ timerGeneration' = timerGeneration + 1
       /\ activeTimer' = timerGeneration + 1
       /\ capturedEpoch' = repairEpoch
       /\ baselineGeneration' = inputGeneration
       /\ baselineValue' = actualValue
       /\ fallbackPresent' = TRUE
       /\ fallbackEligible' = fallbackEligible \cup {request}
       /\ UNCHANGED <<admission, compositionPhase, repairEpoch,
            inputGeneration, actualValue, deferredInputRequests,
            outcome, publicationCount>>

QueueUnmodifiedFallback ==
  /\ QueueFallbackRequest
  /\ UNCHANGED shiftedFallbackRequests
  /\ Record("queue-unmodified-fallback", requestOrdinal + 1)

QueueShiftedFallback ==
  /\ QueueFallbackRequest
  /\ shiftedFallbackRequests' = shiftedFallbackRequests \cup {requestOrdinal + 1}
  /\ Record("queue-shifted-fallback", requestOrdinal + 1)

CoalesceWithoutFallback ==
  /\ pendingRequest # NoRequest
  /\ UNCHANGED <<admission, compositionPhase, repairEpoch, inputGeneration, actualValue,
       requestOrdinal, pendingRequest, timerGeneration, activeTimer,
       capturedEpoch, baselineGeneration, baselineValue, fallbackPresent, fallbackEligible,
       shiftedFallbackRequests, deferredInputRequests,
       outcome, publicationCount>>
  /\ Record("coalesce-without-fallback", pendingRequest)

CoalesceFallback ==
  /\ pendingRequest # NoRequest
  /\ ~fallbackPresent
  /\ fallbackPresent' = TRUE
  /\ fallbackEligible' = fallbackEligible \cup {pendingRequest}
  /\ UNCHANGED <<admission, compositionPhase, repairEpoch, inputGeneration, actualValue,
       requestOrdinal, pendingRequest, timerGeneration, activeTimer,
       capturedEpoch, baselineGeneration, baselineValue, deferredInputRequests,
       shiftedFallbackRequests, outcome,
       publicationCount>>
  /\ Record("coalesce-fallback", pendingRequest)

NativeValueChanges ==
  /\ admission = "accepting"
  /\ inputGeneration < MaxInputGeneration
  /\ inputGeneration' = inputGeneration + 1
  /\ actualValue' \in InputValues \ {actualValue}
  /\ UNCHANGED <<admission, compositionPhase, repairEpoch, requestOrdinal,
       pendingRequest, timerGeneration, activeTimer, capturedEpoch,
       baselineGeneration, baselineValue, fallbackPresent, fallbackEligible,
       shiftedFallbackRequests, deferredInputRequests,
       outcome, publicationCount>>
  /\ Record("native-value-changes", pendingRequest)

DeferIndependentInput ==
  /\ pendingRequest # NoRequest
  /\ pendingRequest \notin deferredInputRequests
  /\ deferredInputRequests' = deferredInputRequests \cup {pendingRequest}
  /\ UNCHANGED <<admission, compositionPhase, repairEpoch, inputGeneration, actualValue,
       requestOrdinal, pendingRequest, timerGeneration, activeTimer,
       capturedEpoch, baselineGeneration, baselineValue, fallbackPresent, fallbackEligible,
       shiftedFallbackRequests, outcome,
       publicationCount>>
  /\ Record("defer-independent-input", pendingRequest)

FireNativeValue ==
  /\ admission = "accepting"
  /\ compositionPhase = "idle"
  /\ pendingRequest # NoRequest
  /\ activeTimer # 0
  /\ capturedEpoch = repairEpoch
  /\ actualValue # baselineValue
  /\ outcome' = [outcome EXCEPT ![pendingRequest] = "published-native"]
  /\ publicationCount' = [publicationCount EXCEPT ![pendingRequest] = @ + 1]
  /\ pendingRequest' = NoRequest
  /\ activeTimer' = 0
  /\ capturedEpoch' = 0
  /\ baselineGeneration' = 0
  /\ baselineValue' = NoInputValue
  /\ fallbackPresent' = FALSE
  /\ UNCHANGED <<admission, compositionPhase, repairEpoch, inputGeneration, actualValue,
       requestOrdinal, timerGeneration, fallbackEligible,
       shiftedFallbackRequests, deferredInputRequests>>
  /\ Record("fire-native-value", pendingRequest)

FireFallback ==
  /\ admission = "accepting"
  /\ compositionPhase = "idle"
  /\ pendingRequest # NoRequest
  /\ activeTimer # 0
  /\ capturedEpoch = repairEpoch
  /\ actualValue = baselineValue
  /\ fallbackPresent
  /\ outcome' = [outcome EXCEPT ![pendingRequest] = "published-fallback"]
  /\ publicationCount' = [publicationCount EXCEPT ![pendingRequest] = @ + 1]
  /\ pendingRequest' = NoRequest
  /\ activeTimer' = 0
  /\ capturedEpoch' = 0
  /\ baselineGeneration' = 0
  /\ baselineValue' = NoInputValue
  /\ fallbackPresent' = FALSE
  /\ UNCHANGED <<admission, compositionPhase, repairEpoch, inputGeneration, actualValue,
       requestOrdinal, timerGeneration, fallbackEligible,
       shiftedFallbackRequests, deferredInputRequests>>
  /\ Record("fire-fallback", pendingRequest)

FireNoop ==
  /\ admission = "accepting"
  /\ compositionPhase = "idle"
  /\ pendingRequest # NoRequest
  /\ activeTimer # 0
  /\ capturedEpoch = repairEpoch
  /\ actualValue = baselineValue
  /\ ~fallbackPresent
  /\ outcome' = [outcome EXCEPT ![pendingRequest] = "noop"]
  /\ pendingRequest' = NoRequest
  /\ activeTimer' = 0
  /\ capturedEpoch' = 0
  /\ baselineGeneration' = 0
  /\ baselineValue' = NoInputValue
  /\ UNCHANGED <<admission, compositionPhase, repairEpoch, inputGeneration, actualValue,
       requestOrdinal, timerGeneration, fallbackPresent, fallbackEligible,
       shiftedFallbackRequests, deferredInputRequests,
       publicationCount>>
  /\ Record("fire-noop", pendingRequest)

PublishFollowingInput ==
  /\ admission = "accepting"
  /\ compositionPhase = "idle"
  /\ pendingRequest = NoRequest
  /\ requestOrdinal > 0
  /\ UNCHANGED <<admission, compositionPhase, repairEpoch, inputGeneration, actualValue,
       requestOrdinal, pendingRequest, timerGeneration, activeTimer,
       capturedEpoch, baselineGeneration, baselineValue, fallbackPresent, fallbackEligible,
       shiftedFallbackRequests, deferredInputRequests,
       outcome, publicationCount>>
  /\ Record("publish-following-input", NoRequest)

StartComposition ==
  /\ admission = "accepting"
  /\ compositionPhase = "idle"
  /\ compositionPhase' = "eligible"
  /\ outcome' = IF pendingRequest = NoRequest
       THEN outcome
       ELSE [outcome EXCEPT ![pendingRequest] = "cancelled"]
  /\ pendingRequest' = NoRequest
  /\ activeTimer' = 0
  /\ capturedEpoch' = 0
  /\ baselineGeneration' = 0
  /\ baselineValue' = NoInputValue
  /\ fallbackPresent' = FALSE
  /\ UNCHANGED <<admission, repairEpoch, inputGeneration, actualValue, requestOrdinal,
       timerGeneration, fallbackEligible, deferredInputRequests,
       shiftedFallbackRequests, publicationCount>>
  /\ Record("start-composition", pendingRequest)

EndComposition ==
  /\ compositionPhase = "eligible"
  /\ compositionPhase' = "idle"
  /\ UNCHANGED <<admission, repairEpoch, inputGeneration, actualValue, requestOrdinal,
       pendingRequest, timerGeneration, activeTimer, capturedEpoch,
       baselineGeneration, baselineValue, fallbackPresent, fallbackEligible,
       shiftedFallbackRequests, deferredInputRequests,
       outcome, publicationCount>>
  /\ Record("end-composition", NoRequest)

BeginRepair ==
  /\ admission = "accepting"
  /\ repairEpoch < MaxEpoch
  /\ admission' = "repairing"
  /\ compositionPhase' = "idle"
  /\ repairEpoch' = repairEpoch + 1
  /\ outcome' = IF pendingRequest = NoRequest
       THEN outcome
       ELSE [outcome EXCEPT ![pendingRequest] = "cancelled"]
  /\ pendingRequest' = NoRequest
  /\ activeTimer' = 0
  /\ capturedEpoch' = 0
  /\ baselineGeneration' = 0
  /\ baselineValue' = NoInputValue
  /\ fallbackPresent' = FALSE
  /\ UNCHANGED <<inputGeneration, actualValue, requestOrdinal, timerGeneration,
       fallbackEligible, deferredInputRequests,
       shiftedFallbackRequests, publicationCount>>
  /\ Record("begin-repair", pendingRequest)

CompleteRepair ==
  /\ admission = "repairing"
  /\ admission' = "accepting"
  /\ UNCHANGED <<compositionPhase, repairEpoch, inputGeneration, actualValue,
       requestOrdinal, pendingRequest, timerGeneration, activeTimer,
       capturedEpoch, baselineGeneration, baselineValue, fallbackPresent, fallbackEligible,
       shiftedFallbackRequests, deferredInputRequests,
       outcome, publicationCount>>
  /\ Record("complete-repair", NoRequest)

Dispose ==
  /\ admission # "disposed"
  /\ admission' = "disposed"
  /\ compositionPhase' = "idle"
  /\ outcome' = IF pendingRequest = NoRequest
       THEN outcome
       ELSE [outcome EXCEPT ![pendingRequest] = "cancelled"]
  /\ pendingRequest' = NoRequest
  /\ activeTimer' = 0
  /\ capturedEpoch' = 0
  /\ baselineGeneration' = 0
  /\ baselineValue' = NoInputValue
  /\ fallbackPresent' = FALSE
  /\ UNCHANGED <<repairEpoch, inputGeneration, actualValue, requestOrdinal,
       timerGeneration, fallbackEligible, deferredInputRequests,
       shiftedFallbackRequests, publicationCount>>
  /\ Record("dispose", pendingRequest)

FireStaleTimer(timer, epoch) ==
  /\ timer \in 1..timerGeneration
  /\ epoch \in 0..MaxEpoch
  /\ (timer # activeTimer \/ epoch # capturedEpoch)
  /\ UNCHANGED runtimeVars
  /\ RecordStaleTimer(timer, epoch)

CoverageDeferredInputBeforeValueChange ==
  /\ lastAction = "defer-independent-input"
  /\ priorActualValue = priorBaselineValue
  /\ pendingRequest = priorPendingRequest
  /\ UNCHANGED vars

CoverageNativeValueReturnsToBaseline ==
  /\ pendingRequest # NoRequest
  /\ lastAction = "native-value-changes"
  /\ priorActualValue # priorBaselineValue
  /\ inputGeneration = baselineGeneration + 2
  /\ actualValue = baselineValue
  /\ UNCHANGED vars

CoverageStaleTimerAfterReadmission ==
  /\ lastAction = "fire-stale-timer"
  /\ priorPendingRequest = 2
  /\ priorActiveTimer = 2
  /\ outcome[1] = "cancelled"
  /\ lastTimer = 1
  /\ lastTimerEpoch < priorCapturedEpoch
  /\ UNCHANGED vars

FireCurrentTimer == FireNativeValue \/ FireFallback \/ FireNoop

Next ==
  \/ QueueWithoutFallback
  \/ QueueUnmodifiedFallback
  \/ QueueShiftedFallback
  \/ CoalesceWithoutFallback
  \/ CoalesceFallback
  \/ NativeValueChanges
  \/ DeferIndependentInput
  \/ FireCurrentTimer
  \/ \E timer \in 1..MaxRequests, epoch \in 0..MaxEpoch:
       FireStaleTimer(timer, epoch)
  \/ PublishFollowingInput
  \/ StartComposition
  \/ EndComposition
  \/ BeginRepair
  \/ CompleteRepair
  \/ Dispose
  \/ CoverageDeferredInputBeforeValueChange
  \/ CoverageNativeValueReturnsToBaseline
  \/ CoverageStaleTimerAfterReadmission

Spec == Init /\ [][Next]_vars /\ WF_vars(FireCurrentTimer)

TypeOK ==
  /\ admission \in Admissions
  /\ compositionPhase \in CompositionPhases
  /\ repairEpoch \in 0..MaxEpoch
  /\ inputGeneration \in 0..MaxInputGeneration
  /\ actualValue \in InputValues
  /\ requestOrdinal \in 0..MaxRequests
  /\ pendingRequest \in Requests \cup {NoRequest}
  /\ timerGeneration \in 0..MaxRequests
  /\ activeTimer \in 0..MaxRequests
  /\ capturedEpoch \in 0..MaxEpoch
  /\ baselineGeneration \in 0..MaxInputGeneration
  /\ baselineValue \in InputValues \cup {NoInputValue}
  /\ fallbackPresent \in BOOLEAN
  /\ fallbackEligible \subseteq Requests
  /\ shiftedFallbackRequests \subseteq Requests
  /\ deferredInputRequests \subseteq Requests
  /\ outcome \in [Requests -> Outcomes]
  /\ publicationCount \in [Requests -> 0..1]
  /\ lastAction \in Actions
  /\ lastTarget \in Requests \cup {NoRequest}
  /\ lastTimer \in 0..MaxRequests
  /\ lastTimerEpoch \in 0..MaxEpoch
  /\ priorPendingRequest \in Requests \cup {NoRequest}
  /\ priorActiveTimer \in 0..MaxRequests
  /\ priorCapturedEpoch \in 0..MaxEpoch
  /\ priorActualValue \in InputValues
  /\ priorBaselineValue \in InputValues \cup {NoInputValue}
  /\ priorTargetOutcome \in Outcomes
  /\ priorTargetPublicationCount \in 0..1

PendingIdentityIsCurrent ==
  /\ pendingRequest # NoRequest =>
       /\ pendingRequest = requestOrdinal
       /\ activeTimer = timerGeneration
       /\ activeTimer # 0
       /\ capturedEpoch = repairEpoch
       /\ baselineGeneration <= inputGeneration
       /\ baselineValue \in InputValues
       /\ outcome[pendingRequest] = "none"
  /\ pendingRequest = NoRequest =>
       /\ activeTimer = 0
       /\ capturedEpoch = 0
       /\ baselineGeneration = 0
       /\ baselineValue = NoInputValue
       /\ ~fallbackPresent

EveryAdmittedRequestSettlesOnce ==
  \A request \in Requests:
    (outcome[request] = "none") =
      (request > requestOrdinal \/ request = pendingRequest)

PublicationIsExactlyOnce ==
  \A request \in Requests:
    (publicationCount[request] = 1) =
      (outcome[request] \in {"published-native", "published-fallback"})

ActualValueChangesOnlyOnNativeValueChange ==
  actualValue # priorActualValue => lastAction = "native-value-changes"

NativePublicationRequiresChangedValue ==
  lastAction = "fire-native-value" =>
    /\ priorPendingRequest = lastTarget
    /\ priorActualValue # priorBaselineValue
    /\ outcome[lastTarget] = "published-native"
    /\ publicationCount[lastTarget] = priorTargetPublicationCount + 1

FallbackPublicationRequiresUnchangedValue ==
  lastAction = "fire-fallback" =>
    /\ priorPendingRequest = lastTarget
    /\ priorActualValue = priorBaselineValue
    /\ lastTarget \in fallbackEligible
    /\ outcome[lastTarget] = "published-fallback"
    /\ publicationCount[lastTarget] = priorTargetPublicationCount + 1

NoopRequiresUnchangedValue ==
  lastAction = "fire-noop" =>
    /\ priorPendingRequest = lastTarget
    /\ priorActualValue = priorBaselineValue
    /\ lastTarget \notin fallbackEligible
    /\ outcome[lastTarget] = "noop"
    /\ publicationCount[lastTarget] = priorTargetPublicationCount

ShiftedRequestsAreAdmittedFallbacks ==
  /\ shiftedFallbackRequests \subseteq fallbackEligible
  /\ \A request \in shiftedFallbackRequests: request <= requestOrdinal
  /\ lastAction = "queue-shifted-fallback" =>
       /\ lastTarget \in shiftedFallbackRequests
       /\ lastTarget \in fallbackEligible
       /\ pendingRequest = lastTarget

DeferredInputDoesNotPublishEarly ==
  lastAction = "defer-independent-input" =>
    /\ priorPendingRequest = lastTarget
    /\ pendingRequest = priorPendingRequest
    /\ outcome[lastTarget] = priorTargetOutcome
    /\ outcome[lastTarget] = "none"
    /\ publicationCount[lastTarget] = priorTargetPublicationCount
    /\ publicationCount[lastTarget] = 0

StaleTimerIsInert ==
  lastAction = "fire-stale-timer" =>
    /\ (lastTimer # priorActiveTimer \/ lastTimerEpoch # priorCapturedEpoch)
    /\ pendingRequest = priorPendingRequest
    /\ activeTimer = priorActiveTimer
    /\ capturedEpoch = priorCapturedEpoch
    /\ actualValue = priorActualValue
    /\ baselineValue = priorBaselineValue
    /\ (lastTarget = NoRequest \/
          (outcome[lastTarget] = priorTargetOutcome /\
           publicationCount[lastTarget] = priorTargetPublicationCount))

FollowingInputCannotOvertakeSettlement ==
  lastAction = "publish-following-input" =>
    \A request \in 1..requestOrdinal: outcome[request] # "none"

ClosedAdmissionHasNoTimer ==
  admission # "accepting" => pendingRequest = NoRequest

CompositionOwnsNoSettlementTimer ==
  compositionPhase = "eligible" => pendingRequest = NoRequest

InvalidationCancelsPendingSettlement ==
  lastAction \in {"start-composition", "begin-repair", "dispose"} =>
    pendingRequest = NoRequest

PendingTimerEventuallySettles ==
  [](pendingRequest # NoRequest => <> (pendingRequest = NoRequest))

=============================================================================
