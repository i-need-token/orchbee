--------------------- MODULE TerminalNativeInputSettlement ---------------------
EXTENDS FiniteSets, Naturals, TLC

\* Models the short native-text settlement window owned by ProjectedTerminal.
\* Two requests expose cancellation followed by re-entry. Time is represented
\* by independently ordered timer, DOM-value, composition, repair, and disposal
\* actions, plus publication of a later Host-directed input. The concrete 30 ms
\* delay is intentionally outside this model.

MaxRequests == 2
Requests == 1..MaxRequests
NoRequest == 0
MaxEpoch == 2
MaxInputGeneration == 2
Admissions == {"accepting", "repairing", "disposed"}
CompositionPhases == {"idle", "eligible"}
Outcomes == {"none", "published-native", "published-fallback", "noop", "cancelled"}
Actions == {
  "init", "queue-without-fallback", "queue-with-fallback",
  "coalesce-without-fallback", "coalesce-fallback", "native-value-changes",
  "defer-independent-input", "fire-native-value", "fire-fallback",
  "fire-noop", "publish-following-input", "start-composition",
  "end-composition", "begin-repair", "complete-repair", "dispose"
}

VARIABLES
  admission,
  compositionPhase,
  repairEpoch,
  inputGeneration,
  requestOrdinal,
  pendingRequest,
  timerGeneration,
  activeTimer,
  capturedEpoch,
  baselineGeneration,
  fallbackPresent,
  fallbackEligible,
  deferredInputRequests,
  nativeChangeObserved,
  outcome,
  publicationCount,
  lastAction,
  lastTarget

vars == <<admission, compositionPhase, repairEpoch, inputGeneration,
  requestOrdinal, pendingRequest, timerGeneration, activeTimer, capturedEpoch,
  baselineGeneration, fallbackPresent, fallbackEligible,
  deferredInputRequests, nativeChangeObserved, outcome, publicationCount,
  lastAction, lastTarget>>

Init ==
  /\ admission = "accepting"
  /\ compositionPhase = "idle"
  /\ repairEpoch = 0
  /\ inputGeneration = 0
  /\ requestOrdinal = 0
  /\ pendingRequest = NoRequest
  /\ timerGeneration = 0
  /\ activeTimer = 0
  /\ capturedEpoch = 0
  /\ baselineGeneration = 0
  /\ fallbackPresent = FALSE
  /\ fallbackEligible = {}
  /\ deferredInputRequests = {}
  /\ nativeChangeObserved = {}
  /\ outcome = [request \in Requests |-> "none"]
  /\ publicationCount = [request \in Requests |-> 0]
  /\ lastAction = "init"
  /\ lastTarget = NoRequest

Record(action, request) ==
  /\ lastAction' = action
  /\ lastTarget' = request

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
       /\ fallbackPresent' = FALSE
       /\ UNCHANGED <<admission, compositionPhase, repairEpoch,
            inputGeneration, fallbackEligible, deferredInputRequests,
            nativeChangeObserved, outcome, publicationCount>>
       /\ Record("queue-without-fallback", request)

QueueWithFallback ==
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
       /\ fallbackPresent' = TRUE
       /\ fallbackEligible' = fallbackEligible \cup {request}
       /\ UNCHANGED <<admission, compositionPhase, repairEpoch,
            inputGeneration, deferredInputRequests, nativeChangeObserved,
            outcome, publicationCount>>
       /\ Record("queue-with-fallback", request)

CoalesceWithoutFallback ==
  /\ pendingRequest # NoRequest
  /\ UNCHANGED <<admission, compositionPhase, repairEpoch, inputGeneration,
       requestOrdinal, pendingRequest, timerGeneration, activeTimer,
       capturedEpoch, baselineGeneration, fallbackPresent, fallbackEligible,
       deferredInputRequests, nativeChangeObserved, outcome, publicationCount>>
  /\ Record("coalesce-without-fallback", pendingRequest)

CoalesceFallback ==
  /\ pendingRequest # NoRequest
  /\ ~fallbackPresent
  /\ fallbackPresent' = TRUE
  /\ fallbackEligible' = fallbackEligible \cup {pendingRequest}
  /\ UNCHANGED <<admission, compositionPhase, repairEpoch, inputGeneration,
       requestOrdinal, pendingRequest, timerGeneration, activeTimer,
       capturedEpoch, baselineGeneration, deferredInputRequests,
       nativeChangeObserved, outcome, publicationCount>>
  /\ Record("coalesce-fallback", pendingRequest)

NativeValueChanges ==
  /\ admission = "accepting"
  /\ inputGeneration < MaxInputGeneration
  /\ inputGeneration' = inputGeneration + 1
  /\ UNCHANGED <<admission, compositionPhase, repairEpoch, requestOrdinal,
       pendingRequest, timerGeneration, activeTimer, capturedEpoch,
       baselineGeneration, fallbackPresent, fallbackEligible,
       deferredInputRequests, nativeChangeObserved, outcome, publicationCount>>
  /\ Record("native-value-changes", pendingRequest)

DeferIndependentInput ==
  /\ pendingRequest # NoRequest
  /\ inputGeneration # baselineGeneration
  /\ pendingRequest \notin deferredInputRequests
  /\ deferredInputRequests' = deferredInputRequests \cup {pendingRequest}
  /\ UNCHANGED <<admission, compositionPhase, repairEpoch, inputGeneration,
       requestOrdinal, pendingRequest, timerGeneration, activeTimer,
       capturedEpoch, baselineGeneration, fallbackPresent, fallbackEligible,
       nativeChangeObserved, outcome, publicationCount>>
  /\ Record("defer-independent-input", pendingRequest)

FireNativeValue ==
  /\ admission = "accepting"
  /\ compositionPhase = "idle"
  /\ pendingRequest # NoRequest
  /\ activeTimer # 0
  /\ capturedEpoch = repairEpoch
  /\ inputGeneration # baselineGeneration
  /\ outcome' = [outcome EXCEPT ![pendingRequest] = "published-native"]
  /\ publicationCount' = [publicationCount EXCEPT ![pendingRequest] = @ + 1]
  /\ nativeChangeObserved' = nativeChangeObserved \cup {pendingRequest}
  /\ pendingRequest' = NoRequest
  /\ activeTimer' = 0
  /\ capturedEpoch' = 0
  /\ baselineGeneration' = 0
  /\ fallbackPresent' = FALSE
  /\ UNCHANGED <<admission, compositionPhase, repairEpoch, inputGeneration,
       requestOrdinal, timerGeneration, fallbackEligible,
       deferredInputRequests>>
  /\ Record("fire-native-value", pendingRequest)

FireFallback ==
  /\ admission = "accepting"
  /\ compositionPhase = "idle"
  /\ pendingRequest # NoRequest
  /\ activeTimer # 0
  /\ capturedEpoch = repairEpoch
  /\ inputGeneration = baselineGeneration
  /\ fallbackPresent
  /\ outcome' = [outcome EXCEPT ![pendingRequest] = "published-fallback"]
  /\ publicationCount' = [publicationCount EXCEPT ![pendingRequest] = @ + 1]
  /\ pendingRequest' = NoRequest
  /\ activeTimer' = 0
  /\ capturedEpoch' = 0
  /\ baselineGeneration' = 0
  /\ fallbackPresent' = FALSE
  /\ UNCHANGED <<admission, compositionPhase, repairEpoch, inputGeneration,
       requestOrdinal, timerGeneration, fallbackEligible,
       deferredInputRequests, nativeChangeObserved>>
  /\ Record("fire-fallback", pendingRequest)

FireNoop ==
  /\ admission = "accepting"
  /\ compositionPhase = "idle"
  /\ pendingRequest # NoRequest
  /\ activeTimer # 0
  /\ capturedEpoch = repairEpoch
  /\ inputGeneration = baselineGeneration
  /\ ~fallbackPresent
  /\ outcome' = [outcome EXCEPT ![pendingRequest] = "noop"]
  /\ pendingRequest' = NoRequest
  /\ activeTimer' = 0
  /\ capturedEpoch' = 0
  /\ baselineGeneration' = 0
  /\ UNCHANGED <<admission, compositionPhase, repairEpoch, inputGeneration,
       requestOrdinal, timerGeneration, fallbackPresent, fallbackEligible,
       deferredInputRequests, nativeChangeObserved, publicationCount>>
  /\ Record("fire-noop", pendingRequest)

PublishFollowingInput ==
  /\ admission = "accepting"
  /\ compositionPhase = "idle"
  /\ pendingRequest = NoRequest
  /\ requestOrdinal > 0
  /\ UNCHANGED <<admission, compositionPhase, repairEpoch, inputGeneration,
       requestOrdinal, pendingRequest, timerGeneration, activeTimer,
       capturedEpoch, baselineGeneration, fallbackPresent, fallbackEligible,
       deferredInputRequests, nativeChangeObserved, outcome, publicationCount>>
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
  /\ fallbackPresent' = FALSE
  /\ UNCHANGED <<admission, repairEpoch, inputGeneration, requestOrdinal,
       timerGeneration, fallbackEligible, deferredInputRequests,
       nativeChangeObserved, publicationCount>>
  /\ Record("start-composition", pendingRequest)

EndComposition ==
  /\ compositionPhase = "eligible"
  /\ compositionPhase' = "idle"
  /\ UNCHANGED <<admission, repairEpoch, inputGeneration, requestOrdinal,
       pendingRequest, timerGeneration, activeTimer, capturedEpoch,
       baselineGeneration, fallbackPresent, fallbackEligible,
       deferredInputRequests, nativeChangeObserved, outcome, publicationCount>>
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
  /\ fallbackPresent' = FALSE
  /\ UNCHANGED <<inputGeneration, requestOrdinal, timerGeneration,
       fallbackEligible, deferredInputRequests, nativeChangeObserved,
       publicationCount>>
  /\ Record("begin-repair", pendingRequest)

CompleteRepair ==
  /\ admission = "repairing"
  /\ admission' = "accepting"
  /\ UNCHANGED <<compositionPhase, repairEpoch, inputGeneration,
       requestOrdinal, pendingRequest, timerGeneration, activeTimer,
       capturedEpoch, baselineGeneration, fallbackPresent, fallbackEligible,
       deferredInputRequests, nativeChangeObserved, outcome, publicationCount>>
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
  /\ fallbackPresent' = FALSE
  /\ UNCHANGED <<repairEpoch, inputGeneration, requestOrdinal,
       timerGeneration, fallbackEligible, deferredInputRequests,
       nativeChangeObserved, publicationCount>>
  /\ Record("dispose", pendingRequest)

FireCurrentTimer == FireNativeValue \/ FireFallback \/ FireNoop

Next ==
  \/ QueueWithoutFallback
  \/ QueueWithFallback
  \/ CoalesceWithoutFallback
  \/ CoalesceFallback
  \/ NativeValueChanges
  \/ DeferIndependentInput
  \/ FireCurrentTimer
  \/ PublishFollowingInput
  \/ StartComposition
  \/ EndComposition
  \/ BeginRepair
  \/ CompleteRepair
  \/ Dispose

Spec == Init /\ [][Next]_vars /\ WF_vars(FireCurrentTimer)

TypeOK ==
  /\ admission \in Admissions
  /\ compositionPhase \in CompositionPhases
  /\ repairEpoch \in 0..MaxEpoch
  /\ inputGeneration \in 0..MaxInputGeneration
  /\ requestOrdinal \in 0..MaxRequests
  /\ pendingRequest \in Requests \cup {NoRequest}
  /\ timerGeneration \in 0..MaxRequests
  /\ activeTimer \in 0..MaxRequests
  /\ capturedEpoch \in 0..MaxEpoch
  /\ baselineGeneration \in 0..MaxInputGeneration
  /\ fallbackPresent \in BOOLEAN
  /\ fallbackEligible \subseteq Requests
  /\ deferredInputRequests \subseteq Requests
  /\ nativeChangeObserved \subseteq Requests
  /\ outcome \in [Requests -> Outcomes]
  /\ publicationCount \in [Requests -> 0..1]
  /\ lastAction \in Actions
  /\ lastTarget \in Requests \cup {NoRequest}

PendingIdentityIsCurrent ==
  /\ pendingRequest # NoRequest =>
       /\ pendingRequest = requestOrdinal
       /\ activeTimer = timerGeneration
       /\ activeTimer # 0
       /\ capturedEpoch = repairEpoch
       /\ outcome[pendingRequest] = "none"
  /\ pendingRequest = NoRequest =>
       /\ activeTimer = 0
       /\ capturedEpoch = 0
       /\ baselineGeneration = 0
       /\ ~fallbackPresent

EveryAdmittedRequestSettlesOnce ==
  \A request \in Requests:
    (outcome[request] = "none") =
      (request > requestOrdinal \/ request = pendingRequest)

PublicationIsExactlyOnce ==
  \A request \in Requests:
    (publicationCount[request] = 1) =
      (outcome[request] \in {"published-native", "published-fallback"})

NativePublicationRequiresChangedValue ==
  \A request \in Requests:
    outcome[request] = "published-native" => request \in nativeChangeObserved

FallbackPublicationRequiresUnchangedValue ==
  \A request \in Requests:
    outcome[request] = "published-fallback" =>
      /\ request \in fallbackEligible
      /\ request \notin nativeChangeObserved

DeferredInputDoesNotPublishEarly ==
  \A request \in deferredInputRequests:
    request = pendingRequest => publicationCount[request] = 0

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
